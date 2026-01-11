use anyhow::{Context, Result};
use serde_json::Value;
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
        use tokio::task::JoinHandle;
        
        let handles: Vec<JoinHandle<Result<()>>> = self
            .wallpapers
            .iter()
            .map(|wallpaper| {
                let config = self.config.clone();
                let lock_file = Arc::clone(&self.lock_file);
                let wallpaper = wallpaper.clone();

                tokio::spawn(async move {
                    process_wallpaper(&config, &lock_file, &wallpaper).await
                })
            })
            .collect();

        for handle in handles {
            if let Err(e) = handle.await.expect("Task panicked") {
                eprintln!("  Error processing wallpaper: {}", e);
            }
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
                eprintln!("  Warning: Invalid wallpaper ID format '{}', skipping", wallpaper);
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
            let status = check_download_status(&self.config.save_location, wallpaper_id, &self.lock_file).await?;
            
            match status {
                WallpaperStatus::Downloaded { path } => {
                    println!("  ✓ {} - Downloaded ({})", wallpaper_id, path.display());
                    downloaded_count += 1;
                }
                WallpaperStatus::DownloadedWithIntegrity { path } => {
                    println!("  ✓ {} - Downloaded (verified) ({})", wallpaper_id, path.display());
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
            downloaded_count,
            not_downloaded_count
        );

        Ok(())
    }
}

/// Status of a wallpaper
enum WallpaperStatus {
    Downloaded { path: PathBuf },
    DownloadedWithIntegrity { path: PathBuf },
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
            if let Ok(existing_image_sha256) = helper::calculate_sha256(&existing_path).await {
                if lock_file.contains(wallpaper_id, &existing_image_sha256) {
                    return Ok(WallpaperStatus::DownloadedWithIntegrity { path: existing_path });
                }
            }
            // File exists but integrity check failed or not in lock file
            return Ok(WallpaperStatus::Downloaded { path: existing_path });
        }
        // File exists but integrity is not enabled
        Ok(WallpaperStatus::Downloaded { path: existing_path })
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

async fn process_wallpaper(
    config: &config::Config,
    lock_file: &Arc<Mutex<Option<LockFile>>>,
    wallpaper: &str,
) -> Result<()> {
    if let Some(existing_path) = find_existing_image(&config.save_location, wallpaper).await? {
        if config.integrity {
            if check_integrity(&existing_path, &wallpaper, &lock_file).await? {
                println!(
                    "   Skipping {}: already exists and integrity check passed",
                    wallpaper
                );
                return Ok(());
            }
            println!(
                "   Integrity check failed for {}: re-downloading",
                wallpaper
            );
        } else {
            println!("   Skipping {}: already exists", wallpaper);
            return Ok(());
        }
    }

    let wallhaven_img_link = format!("{}/{}", WALLHEAVEN_API, wallpaper.trim());
    let curl_data = retry_get_curl_content(&wallhaven_img_link).await?;
    let res: Value = serde_json::from_str(&curl_data)?;

    if let Some(error) = res.get("error") {
        eprintln!("Error : {}", error);
        return Err(anyhow::anyhow!("   API error: {}", error));
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

/// Check if an existing image matches the integrity hash
async fn check_integrity(
    existing_path_given: impl AsRef<Path>,
    wallpaper: &str,
    lock_file: &Arc<Mutex<Option<LockFile>>>,
) -> Result<bool> {
    let existing_path = existing_path_given.as_ref();
    let lock_file_guard = lock_file.lock().await;
    if let Some(ref lock_file) = *lock_file_guard {
        let existing_image_sha256 = helper::calculate_sha256(existing_path).await?;
        Ok(lock_file.contains(wallpaper, &existing_image_sha256))
    } else {
        Ok(false)
    }
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
