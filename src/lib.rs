use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{create_dir_all, File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;
use tokio::time::sleep;

mod config;
mod helper;
mod lock;

use lock::LockFile;

const WALLHEAVEN_API: &str = "https://wallhaven.cc/api/v1/w";
const MAX_RETRY: u32 = 3;

/// Main RustPaper struct for managing wallpapers
#[derive(Clone)]
pub struct RustPaper {
    config: config::Config,
    config_folder: PathBuf,
    wallpapers: Vec<String>,
    wallpapers_list_file_location: PathBuf,
    lock_file: Arc<Mutex<Option<LockFile>>>,
}

/// INFO: Build a map of wallpaper IDs to file paths (cached directory listing)
async fn build_file_map(save_location: &str) -> Result<HashMap<String, PathBuf>> {
    let save_path = Path::new(save_location);
    let mut file_map = HashMap::new();
    if !save_path.exists() {
        return Ok(file_map);
    }
    let mut entries = tokio::fs::read_dir(save_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                file_map.insert(file_stem.to_string(), path);
            }
        }
    }
    Ok(file_map)
}

async fn process_wallpaper_optimized(
    config: &config::Config,
    lock_file: &Arc<Mutex<Option<LockFile>>>,
    wallpaper: &str,
) -> Result<()> {
    let wallhaven_img_link = format!("{}/{}", WALLHEAVEN_API, wallpaper.trim());
    let curl_data = retry_get_curl_content(&wallhaven_img_link).await?;
    let res: Value = serde_json::from_str(&curl_data)?;
    if let Some(error) = res.get("error") {
        eprintln!("Error : {}", error);
        return Err(anyhow::anyhow!("❌ API error: {}", error));
    }
    let image_location = download_and_save(&res, wallpaper, &config.save_location).await?;
    if config.integrity {
        let mut lock_file_guard = lock_file.lock().await;
        if let Some(ref mut lock_file) = *lock_file_guard {
            let image_sha256 = helper::calculate_sha256(&image_location).await?;
            lock_file
                .add(wallpaper.to_string(), image_location, image_sha256)
                .await?;
        }
    }
    println!("   Downloaded {}", wallpaper);
    Ok(())
}

impl RustPaper {
    /// Create a new RustPaper instance with loaded configuration
    pub async fn new() -> Result<Self> {
        let config: config::Config =
            confy::load("rust-paper", "config").context("   Failed to load configuration")?;

        let config_folder = helper::get_folder_path().context("   Failed to get folder path")?;

        tokio::try_join!(
            create_dir_all(&config_folder),
            create_dir_all(&config.save_location)
        )?;

        let wallpapers_list_file_location = config_folder.join("wallpapers.lst");
        let wallpapers = load_wallpapers(&wallpapers_list_file_location).await?;

        let lock_file = if config.integrity {
            Some(LockFile::load_or_new().await)
        } else {
            None
        };

        Ok(Self {
            config,
            config_folder,
            wallpapers,
            wallpapers_list_file_location,
            lock_file: Arc::new(Mutex::new(lock_file)),
        })
    }

    /// Sync all wallpapers in the list
    pub async fn sync(&self) -> Result<()> {
        let file_map = build_file_map(&self.config.save_location).await?;
        let lock_file_map: Option<HashMap<String, (String, String)>> = if self.config.integrity {
            let lock_file_guard = self.lock_file.lock().await;
            if let Some(ref lock_file) = *lock_file_guard {
                Some(
                    lock_file
                        .entries()
                        .iter()
                        .map(|e| {
                            (
                                e.image_id().to_string(),
                                (e.image_location().to_string(), e.image_sha256().to_string()),
                            )
                        })
                        .collect(),
                )
            } else {
                None
            }
        } else {
            None
        };

        let mut needs_download = Vec::new();
        let mut integrity_checks = Vec::new();
        for wallpaper in &self.wallpapers {
            if let Some(existing_path) = file_map.get(wallpaper) {
                if self.config.integrity {
                    if let Some(ref lock_map) = lock_file_map {
                        if let Some((lock_location, expected_sha256)) = lock_map.get(wallpaper) {
                            let path_str = existing_path.to_string_lossy().to_string();
                            if lock_location == &path_str {
                                integrity_checks.push((
                                    wallpaper.clone(),
                                    existing_path.clone(),
                                    expected_sha256.clone(),
                                ));
                                continue;
                            }
                        }
                    }
                    needs_download.push(wallpaper.clone());
                } else {
                    println!("   Skipping {}: already exists", wallpaper);
                }
            } else {
                needs_download.push(wallpaper.clone());
            }
        }

        if !integrity_checks.is_empty() {
            let check_tasks: FuturesUnordered<_> = integrity_checks
                .into_iter()
                .map(|(wallpaper_id, path, expected_hash)| {
                    tokio::spawn(async move {
                        match helper::calculate_sha256(&path).await {
                            Ok(actual_sha256) => {
                                if actual_sha256 == expected_hash {
                                    println!(
                                        "   Skipping {}: already exists and integrity check passed",
                                        wallpaper_id
                                    );
                                    Ok::<(String, bool), anyhow::Error>((wallpaper_id, false))
                                } else {
                                    println!(
                                        "   Integrity check failed for {}: re-downloading",
                                        wallpaper_id
                                    );
                                    Ok::<(String, bool), anyhow::Error>((wallpaper_id, true))
                                }
                            }
                            Err(_) => {
                                println!("   Skipping {}: already exists", wallpaper_id);
                                Ok::<(String, bool), anyhow::Error>((wallpaper_id, true))
                            },
                        }
                    })
                })
                .collect();

            let mut check_tasks = check_tasks;
            while let Some(result) = check_tasks.next().await {
                match result {
                    Ok(Ok((wallpaper_id, should_download))) => {
                        if should_download {
                            needs_download.push(wallpaper_id);
                        }
                    }
                    _ => {
                        unreachable!()
                    }
                }
            }
        }

        if needs_download.is_empty() {
            println!("   All wallpapers are up to date.");
            return Ok(());
        }

        let mut tasks = FuturesUnordered::new();
        for wallpaper in needs_download {
            let config = self.config.clone();
            let lock_file = Arc::clone(&self.lock_file);
            tasks.push(tokio::spawn(async move {
                process_wallpaper_optimized(&config, &lock_file, &wallpaper).await
            }));
        }

        let mut errors = 0;
        let mut completed = 0;
        let total = tasks.len();
        while let Some(result) = tasks.next().await {
            completed += 1;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    eprintln!("❌ Error processing wallpaper: {}", e);
                    errors += 1;
                }
                Err(e) => {
                    eprintln!("❌ Task panicked: {}", e);
                    errors += 1;
                }
            }
        }

        if errors > 0 {
            eprintln!(
                "   Completed {} of {} with {} error(s)",
                completed, total, errors
            );
        }

        Ok(())
    }

    /// Add new wallpapers to the list
    pub async fn add(&mut self, new_wallpapers: &mut Vec<String>) -> Result<()> {
        *new_wallpapers = new_wallpapers
            .iter()
            .map(|wall| {
                if helper::is_url(wall) {
                    wall.split('/')
                        .last()
                        .unwrap_or_default()
                        .split('?')
                        .next()
                        .unwrap_or_default()
                        .to_string()
                } else {
                    wall.to_string()
                }
            })
            .collect();

        // Validate wallpaper IDs
        let mut valid_wallpapers = Vec::new();
        for wallpaper in new_wallpapers.iter().flat_map(|s| helper::to_array(s)) {
            if helper::validate_wallpaper_id(&wallpaper) {
                valid_wallpapers.push(wallpaper);
            } else {
                eprintln!(
                    "  Warning: Invalid wallpaper ID format '{}', skipping",
                    wallpaper
                );
            }
        }

        self.wallpapers.extend(valid_wallpapers);
        self.wallpapers.sort_unstable();
        self.wallpapers.dedup();
        update_wallpaper_list(&self.wallpapers, &self.wallpapers_list_file_location).await
    }

    /// Remove wallpapers from the list
    pub async fn remove(&mut self, ids_to_remove: &[String]) -> Result<()> {
        // Extract and validate wallpaper IDs (support URLs and comma-separated)
        let ids: Vec<String> = ids_to_remove
            .iter()
            .flat_map(|id| {
                let processed = if helper::is_url(id) {
                    id.split('/')
                        .last()
                        .unwrap_or_default()
                        .split('?')
                        .next()
                        .unwrap_or_default()
                        .to_string()
                } else {
                    id.clone()
                };
                helper::to_array(&processed)
            })
            .filter(|id| helper::validate_wallpaper_id(id))
            .collect();

        if ids.is_empty() {
            return Err(anyhow::anyhow!("No valid wallpaper IDs provided"));
        }

        // Track what was removed
        let original_len = self.wallpapers.len();

        // Remove IDs from the list
        self.wallpapers.retain(|id| !ids.contains(id));

        let removed_count = original_len - self.wallpapers.len();

        if removed_count == 0 {
            println!("  No matching wallpaper IDs found in the list");
            return Ok(());
        }

        // Update the wallpapers list file
        update_wallpaper_list(&self.wallpapers, &self.wallpapers_list_file_location).await?;

        // Optionally remove from lock file if integrity is enabled
        if self.config.integrity {
            let mut lock_file_guard = self.lock_file.lock().await;
            if let Some(ref mut lock_file) = *lock_file_guard {
                for id in &ids {
                    lock_file.remove(id).await?;
                }
            }
        }

        if removed_count == ids.len() {
            println!("  Removed {} wallpaper ID(s) from the list", removed_count);
        } else {
            println!(
                "  Removed {} of {} requested wallpaper ID(s) from the list",
                removed_count,
                ids.len()
            );
        }

        Ok(())
    }

    /// List all tracked wallpapers with their download status
    pub async fn list(&self) -> Result<()> {
        if self.wallpapers.is_empty() {
            println!("  No wallpapers tracked.");
            return Ok(());
        }

        println!("  Tracked wallpapers ({} total):", self.wallpapers.len());
        println!();

        let mut downloaded_count = 0;
        let mut not_downloaded_count = 0;

        for wallpaper_id in &self.wallpapers {
            let status =
                check_download_status(&self.config.save_location, wallpaper_id, &self.lock_file)
                    .await?;

            match status {
                WallpaperStatus::Downloaded { path } => {
                    println!("  ✓ {} - Downloaded ({})", wallpaper_id, path.display());
                    downloaded_count += 1;
                }
                WallpaperStatus::NotDownloaded => {
                    println!("  ○ {} - Not downloaded", wallpaper_id);
                    not_downloaded_count += 1;
                }
            }
        }

        println!();
        println!(
            "  Summary: {} downloaded, {} not downloaded",
            downloaded_count, not_downloaded_count
        );

        Ok(())
    }

    /// Clean up downloaded wallpapers that are no longer in the list
    pub async fn clean(&mut self) -> Result<()> {
        let save_location = Path::new(&self.config.save_location);
        if !save_location.exists() {
            println!(
                "  Save location does not exist: {}",
                save_location.display()
            );
            return Ok(());
        }
        let mut entries = tokio::fs::read_dir(save_location).await?;
        let mut removed_count = 0;
        let mut total_size = 0u64;
        let mut files_to_check = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    files_to_check.push((path.clone(), file_stem.to_string()));
                }
            }
        }
        println!(
            "  Checking {} file(s) in save location...",
            files_to_check.len()
        );
        for (file_path, file_stem) in files_to_check {
            if !self.wallpapers.contains(&file_stem) {
                if let Ok(metadata) = tokio::fs::metadata(&file_path).await {
                    total_size += metadata.len();
                }
                if self.config.integrity {
                    let mut lock_file_guard = self.lock_file.lock().await;
                    if let Some(ref mut lock_file) = *lock_file_guard {
                        lock_file.remove(&file_stem).await?;
                    }
                }
                match tokio::fs::remove_file(&file_path).await {
                    Ok(_) => {
                        println!("  Removed: {} ({})", file_stem, file_path.display());
                        removed_count += 1;
                    }
                    Err(e) => {
                        eprintln!("  Error removing {}: {}", file_path.display(), e);
                    }
                }
            }
        }

        if removed_count == 0 {
            println!("  No orphaned files found. Everything is clean!");
        } else {
            println!();
            println!(
                "  Cleaned up {} file(s), freed approximately {:.2} MB",
                removed_count,
                total_size as f64 / 1_048_576.0
            );
        }

        Ok(())
    }

    pub async fn info(&self, id: &str) -> Result<()> {
        let wallpaper_id = if helper::is_url(id) {
            id.split('/')
                .last()
                .unwrap_or_default()
                .split('?')
                .next()
                .unwrap_or_default()
                .to_string()
        } else {
            id.to_string()
        };

        if !helper::validate_wallpaper_id(&wallpaper_id) {
            return Err(anyhow::anyhow!(
                "Invalid wallpaper ID format: '{}'",
                wallpaper_id
            ));
        }

        let api_url = format!("{}/{}", WALLHEAVEN_API, wallpaper_id);
        let response_data = retry_get_curl_content(&api_url).await?;
        let json: Value = serde_json::from_str(&response_data)?;
        if let Some(error) = json.get("error") {
            return Err(anyhow::anyhow!("API error: {}", error));
        }
        if let Some(data) = json.get("data") {
            println!("  Wallpaper Information:");
            println!("  ─────────────────────");
            if let Some(id_val) = data.get("id").and_then(Value::as_str) {
                println!("  ID: {}", id_val);
            }
            if let Some(url) = data.get("url").and_then(Value::as_str) {
                println!("  URL: {}", url);
            }
            if let Some(width) = data.get("resolution").and_then(Value::as_str) {
                println!("  Resolution: {}", width);
            }
            if let Some(size) = data.get("file_size").and_then(Value::as_u64) {
                println!("  File Size: {:.2} MB", size as f64 / 1_048_576.0);
            }
            if let Some(category) = data.get("category").and_then(Value::as_str) {
                println!("  Category: {}", category);
            }
            if let Some(purity) = data.get("purity").and_then(Value::as_str) {
                println!("  Purity: {}", purity);
            }
            if let Some(views) = data.get("views").and_then(Value::as_u64) {
                println!("  Views: {}", views);
            }
            if let Some(favorites) = data.get("favorites").and_then(Value::as_u64) {
                println!("  Favorites: {}", favorites);
            }
            if let Some(date) = data.get("created_at").and_then(Value::as_str) {
                println!("  Uploaded: {}", date);
            }
            if let Some(uploader) = data.get("uploader") {
                if let Some(username) = uploader.get("username").and_then(Value::as_str) {
                    println!("  Uploader: {}", username);
                }
            }
            if let Some(tags) = data.get("tags").and_then(Value::as_array) {
                if !tags.is_empty() {
                    let tag_names: Vec<String> = tags
                        .iter()
                        .filter_map(|tag| tag.get("name").and_then(Value::as_str))
                        .map(String::from)
                        .collect();
                    if !tag_names.is_empty() {
                        println!("  Tags: {}", tag_names.join(", "));
                    }
                }
            }
            if let Some(path) = data.get("path").and_then(Value::as_str) {
                println!("  Image URL: {}", path);
            }
            if self.wallpapers.contains(&wallpaper_id) {
                println!("  Status: Tracked");
                if let Some(local_path) =
                    find_existing_image(&self.config.save_location, &wallpaper_id).await?
                {
                    println!("  Local: {}", local_path.display());
                } else {
                    println!("  Local: Not downloaded");
                }
            } else {
                println!("  Status: Not tracked");
            }
        } else {
            return Err(anyhow::anyhow!("Invalid API response: no data field"));
        }

        Ok(())
    }
}

/// Status of a wallpaper
enum WallpaperStatus {
    Downloaded { path: PathBuf },
    NotDownloaded,
}

/// Check the download status of a wallpaper
async fn check_download_status(
    save_location: &str,
    wallpaper_id: &str,
    lock_file: &Arc<Mutex<Option<LockFile>>>,
) -> Result<WallpaperStatus> {
    if let Some(existing_path) = find_existing_image(save_location, wallpaper_id).await? {
        // Check if integrity is enabled and verified
        let lock_file_guard = lock_file.lock().await;
        if let Some(ref lock_file) = *lock_file_guard {
            return Ok(WallpaperStatus::Downloaded {
                path: existing_path,
            });
        }
        // File exists but integrity is not enabled
        Ok(WallpaperStatus::Downloaded {
            path: existing_path,
        })
    } else {
        Ok(WallpaperStatus::NotDownloaded)
    }
}

/// Update the wallpaper list file with the given list of wallpapers
async fn update_wallpaper_list(list: &[String], file_given: impl AsRef<Path>) -> Result<()> {
    let file_path = file_given.as_ref();
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)
        .await?;

    let mut writer = BufWriter::new(file);

    for wallpaper in list {
        writer.write_all(wallpaper.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    writer.flush().await?;
    Ok(())
}

/// Load wallpaper IDs from a file
async fn load_wallpapers(given_file: impl AsRef<Path>) -> Result<Vec<String>> {
    let file_path = given_file.as_ref();
    if !file_path.exists() {
        File::create(file_path).await?;
        return Ok(vec![]);
    }

    let file = File::open(file_path).await?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut lines_stream = reader.lines();

    while let Some(line) = lines_stream.next_line().await? {
        lines.extend(helper::to_array(&line));
    }

    Ok(lines)
}

/// Find an existing image file for a wallpaper ID
async fn find_existing_image(
    save_location_given: impl AsRef<Path>,
    wallpaper: &str,
) -> Result<Option<PathBuf>> {
    let save_location = save_location_given.as_ref();
    let mut entries = tokio::fs::read_dir(save_location).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.file_stem().and_then(|s| s.to_str()) == Some(wallpaper) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Download and save an image from API data
async fn download_and_save(api_data: &Value, id: &str, save_location: &str) -> Result<String> {
    let img_link = api_data
        .get("data")
        .and_then(|data| data.get("path"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("   Failed to get image link from API response"))?;
    helper::download_image(&img_link, id, save_location).await
}

/// Retry fetching content from a URL with exponential backoff
async fn retry_get_curl_content(url: &str) -> Result<String> {
    for retry_count in 0..MAX_RETRY {
        match helper::get_curl_content(url).await {
            Ok(content) => return Ok(content),
            Err(e) if retry_count + 1 < MAX_RETRY => {
                let delay = 2_u64.pow(retry_count); // Exponential backoff
                eprintln!(
                    "Error fetching content (attempt {} of {}): {}. Retrying in {}s...",
                    retry_count + 1,
                    MAX_RETRY,
                    e,
                    delay
                );
                sleep(Duration::from_secs(delay)).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
