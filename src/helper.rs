use anyhow::{anyhow, Context, Error, Result};
use image::{self, guess_format, ImageFormat};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
};

use crate::RustPaper;

const ENV_API_KEY: &str = "WALLHAVEN_API_KEY";

pub struct DownloadResult {
    pub file_path: String,
    pub sha256: Option<String>,
}

pub fn get_key_from_config_or_env(config_key: Option<&str>) -> Option<String> {
    // Prioritize config API key, fallback to environment variable
    if let Some(key) = config_key {
        return Some(key.to_string());
    }
    std::env::var(ENV_API_KEY).ok()
}

/// Get the file extension for an image format
pub fn get_img_extension(format: &ImageFormat) -> &'static str {
    let extensions: HashMap<ImageFormat, &'static str> = [
        (ImageFormat::Png, "png"),
        (ImageFormat::Jpeg, "jpeg"),
        (ImageFormat::Gif, "gif"),
        (ImageFormat::WebP, "webp"),
        (ImageFormat::Pnm, "pnm"),
        (ImageFormat::Tiff, "tiff"),
        (ImageFormat::Tga, "tga"),
        (ImageFormat::Dds, "dds"),
        (ImageFormat::Bmp, "bmp"),
        (ImageFormat::Ico, "ico"),
        (ImageFormat::Hdr, "hdr"),
    ]
    .iter()
    .cloned()
    .collect();

    extensions.get(format).unwrap_or(&"jpg")
}

/// Create an HTTP client with the given timeout
pub fn create_http_client(timeout_secs: u64, api_key: Option<&String>) -> Result<Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(k) = api_key {
        let header_api_value =
            reqwest::header::HeaderValue::from_str(&k).context("Invalid API key format")?;
        headers.insert("X-API-KEY", header_api_value);
    }
    reqwest::ClientBuilder::new()
        .default_headers(headers)
        .user_agent("rust-paper/0.1.2")
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .context("Failed to create HTTP client")
}

/// Fetch content from a URL with proper error handling
pub async fn get_curl_content(
    link: &str,
    client: &Client,
    api_key: Option<&str>,
) -> Result<String> {
    let mut request = client.get(link);
    if let Some(key) = api_key {
        request = request.query(&[("apikey", key)]);
    }
    let response = request
        .send()
        .await
        .context("Failed to send HTTP request")?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "HTTP request failed with status {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown error")
        ));
    }

    let body = response
        .text()
        .await
        .context("Failed to read response body")?;

    Ok(body)
}

/// Calculate SHA256 hash of a file
pub async fn calculate_sha256(file_path: impl AsRef<Path>) -> Result<String> {
    let file_path = file_path.as_ref();

    if !file_path.exists() {
        return Err(anyhow!(" 󱀷  File does not exist: {}", file_path.display()));
    }

    let mut file = File::open(file_path)
        .await
        .with_context(|| format!(" 󱀷  Failed to open file: {}", file_path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let n = file
            .read(&mut buffer)
            .await
            .with_context(|| format!(" 󱀷  Failed to read file: {}", file_path.display()))?;

        if n == 0 {
            break;
        }

        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Download an image from a URL and save it to disk
pub async fn download_image(
    url: &str,
    id: &str,
    save_location: &str,
    client: &Client,
) -> Result<String> {
    let url = reqwest::Url::parse(url).context("Invalid image URL")?;
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to download image")?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download image: HTTP {}",
            status.as_u16()
        ));
    }

    let img_bytes = response
        .bytes()
        .await
        .context("Failed to read image bytes")?;

    // Detect format to get the correct extension
    let img_format = guess_format(&img_bytes).context("Failed to detect image format")?;

    let image_name = format!(
        "{}/{}.{}",
        save_location,
        id,
        get_img_extension(&img_format)
    );

    // Save raw bytes directly to preserve integrity
    tokio::fs::write(&image_name, &img_bytes)
        .await
        .context("Failed to save image")?;

    Ok(image_name)
}

/// Download an image with SHA256 hashing support (for API downloads)
pub async fn download_image_with_hash(
    url: &str,
    id: &str,
    save_location: &str,
    client: &Client,
) -> Result<(String, String)> {
    use sha2::{Digest, Sha256};

    let url = reqwest::Url::parse(url).context("Invalid image URL")?;
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to download image")?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download image: HTTP {}",
            status.as_u16()
        ));
    }

    let img_bytes = response
        .bytes()
        .await
        .context("Failed to read image bytes")?;

    // Calculate SHA256 hash on raw bytes
    let mut hasher = Sha256::new();
    hasher.update(&img_bytes);
    let hash = format!("{:x}", hasher.finalize());

    // Detect format to get the correct extension
    let img_format = guess_format(&img_bytes).context("Failed to detect image format")?;

    let image_name = format!(
        "{}/{}.{}",
        save_location,
        id,
        get_img_extension(&img_format)
    );

    // Save raw bytes directly to preserve integrity
    tokio::fs::write(&image_name, &img_bytes)
        .await
        .context("Failed to save image")?;

    Ok((image_name, hash))
}

/// Unified download function with progress bar, hash calculation, and file saving
/// Returns the saved file path and optional SHA256 hash
pub async fn download_with_progress(
    url: &str,
    id: &str,
    save_location: &str,
    client: &Client,
    calculate_hash: bool,
) -> Result<DownloadResult> {
    let url = reqwest::Url::parse(url).context("Invalid image URL")?;
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to download image")?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download image: HTTP {}",
            status.as_u16()
        ));
    }

    let total_size = response
        .content_length()
        .ok_or_else(|| anyhow!("Failed to get content length"))?;

    let pb = ProgressBar::new(total_size);
    let style = ProgressStyle::with_template(
        "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
    )
    .unwrap()
    .progress_chars("#>-");
    pb.set_style(style);
    pb.set_message(format!("Downloading {}", id));

    let file_path = PathBuf::from(save_location);
    tokio::fs::create_dir_all(&file_path)
        .await
        .context("Failed to create save directory")?;

    // Download all bytes first to detect format
    let img_bytes = response
        .bytes()
        .await
        .context("Failed to read image bytes")?;

    // Detect format to get the correct extension
    let img_format = guess_format(&img_bytes).context("Failed to detect image format")?;
    let extension = get_img_extension(&img_format);

    let file_name = format!("{}/{}.{}", save_location, id, extension);
    let file_path_ref = Path::new(&file_name);

    let mut file = tokio::fs::File::create(file_path_ref)
        .await
        .context("Failed to create file")?;

    let mut hasher = if calculate_hash {
        Some(Sha256::new())
    } else {
        None
    };

    // Calculate hash on the bytes we already have
    if let Some(ref mut h) = hasher {
        h.update(&img_bytes);
    }

    // Write to file
    file.write_all(&img_bytes)
        .await
        .context("Error writing to file")?;

    pb.set_position(total_size);
    pb.finish_with_message(format!("Downloaded {}", id));

    let sha256 = hasher.map(|h| format!("{:x}", h.finalize()));

    Ok(DownloadResult {
        file_path: file_name,
        sha256,
    })
}

pub fn scrape_img_link(curl_data: String) -> Result<String> {
    let regex_pattern = r#"<img[^>]*id="wallpaper"[^>]*src="([^">]+)""#;
    let regex = regex::Regex::new(regex_pattern).unwrap();
    let mut links: Vec<String> = Vec::new();

    for cap in regex.captures_iter(curl_data.as_str()) {
        links.push(cap[1].to_string());
    }

    match links.len() {
        0 => Err(anyhow!("   Unable to scrape img link")),
        _ => Ok(links.into_iter().next().unwrap()),
    }
}

pub async fn update_wallpapers_list_and_lock(
    updates: Vec<(String, String, Option<String>)>,
    rust_paper: &mut RustPaper,
) -> Result<()> {
    // ===== Update wallpapers.lst =====
    let id_list: Vec<String> = updates.iter().map(|(id, ..)| id.clone()).collect();
    rust_paper.wallpapers.extend(id_list);
    rust_paper.wallpapers.sort_unstable();
    rust_paper.wallpapers.dedup();
    update_wallpaper_list(
        &rust_paper.wallpapers,
        rust_paper.wallpapers_list_file_location.clone(),
    )
    .await?;

    // ===== Update lock file =====
    if rust_paper.config.integrity {
        let mut lock_file_guard = rust_paper.lock_file.lock().await;
        if let Some(lock_file) = lock_file_guard.as_mut() {
            let mut has_updates = false;
            for (id, location, hash) in updates {
                if let Some(hash) = hash {
                    lock_file.add_entry(id, location, hash);
                    has_updates = true;
                }
            }
            if has_updates {
                lock_file.save().await?;
            }
        }
    }
    Ok(())
}

/// Update the wallpaper list file with the given list of wallpapers
pub async fn update_wallpaper_list(list: &[String], file_given: impl AsRef<Path>) -> Result<()> {
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

/// Get the home directory path as a string
pub fn get_home_location() -> String {
    dirs::home_dir()
        .map(|path| path.to_str().unwrap_or_default().to_string())
        .unwrap_or_else(|| "~".to_string())
}

/// Get the configuration folder path
pub fn get_folder_path() -> Result<PathBuf> {
    let path = confy::get_configuration_file_path("rust-paper", "config").map_err(Error::new)?;
    if let Some(parent) = path.parent() {
        Ok(parent.to_path_buf())
    } else {
        Ok(PathBuf::new())
    }
}

/// Split comma-separated values into a vector of strings
pub fn to_array(comma_separated_values: &str) -> Vec<String> {
    comma_separated_values
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Check if a string is a valid URL
pub fn is_url(input: &str) -> bool {
    url::Url::parse(input).is_ok()
}

/// Validate wallpaper ID format (6 alphanumeric characters)
pub fn validate_wallpaper_id(id: &str) -> bool {
    id.len() == 6 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_array() {
        assert_eq!(to_array("a,b,c"), vec!["a", "b", "c"]);
        assert_eq!(to_array("a, b, c"), vec!["a", "b", "c"]);
        assert_eq!(to_array("a"), vec!["a"]);
        assert_eq!(to_array(""), Vec::<String>::new());
        assert_eq!(to_array("a,,b"), vec!["a", "b"]);
    }

    #[test]
    fn test_is_url() {
        assert!(is_url("https://wallhaven.cc/w/7pmgv9"));
        assert!(is_url("http://example.com"));
        assert!(!is_url("not a url"));
        assert!(!is_url(""));
    }

    #[test]
    fn test_validate_wallpaper_id() {
        assert!(validate_wallpaper_id("7pmgv9"));
        assert!(validate_wallpaper_id("abcdef"));
        assert!(validate_wallpaper_id("123456"));
        assert!(validate_wallpaper_id("ABC123"));
        assert!(!validate_wallpaper_id("7pmgv")); // too short
        assert!(!validate_wallpaper_id("7pmgv90")); // too long
        assert!(!validate_wallpaper_id("7pmgv-9")); // invalid character
        assert!(!validate_wallpaper_id(""));
    }

    #[test]
    fn test_get_img_extension() {
        assert_eq!(get_img_extension(&ImageFormat::Png), "png");
        assert_eq!(get_img_extension(&ImageFormat::Jpeg), "jpeg");
        assert_eq!(get_img_extension(&ImageFormat::Gif), "gif");
        assert_eq!(get_img_extension(&ImageFormat::WebP), "webp");
    }

    #[test]
    fn test_remove_url_extraction() {
        // Test that URLs are correctly parsed to extract wallpaper IDs
        let url = "https://wallhaven.cc/w/7pmgv9";
        let processed = if is_url(url) {
            url.split('/')
                .last()
                .unwrap_or_default()
                .split('?')
                .next()
                .unwrap_or_default()
                .to_string()
        } else {
            url.to_string()
        };
        assert_eq!(processed, "7pmgv9");
        assert!(validate_wallpaper_id(&processed));
    }
}
