use anyhow::{anyhow, Context, Error, Result};
use image::{self, guess_format, load_from_memory, ImageFormat};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::{fs::File, io::AsyncReadExt};

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

static HTTP_CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();

/// Get the global HTTP client instance (reused for all requests)
fn get_http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("rust-paper/0.1.2")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Fetch content from a URL with proper error handling
pub async fn get_curl_content(link: &str) -> Result<String> {
    let client = get_http_client();

    let response = client
        .get(link)
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
pub async fn download_image(url: &str, id: &str, save_location: &str) -> Result<String> {
    let url = reqwest::Url::parse(url).context("Invalid image URL")?;
    let client = get_http_client();
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

    let img = load_from_memory(&img_bytes).context("Failed to decode image")?;
    let img_format = guess_format(&img_bytes).context("Failed to detect image format")?;

    let image_name = format!(
        "{}/{}.{}",
        save_location,
        id,
        get_img_extension(&img_format)
    );

    img.save_with_format(&image_name, img_format)
        .context("Failed to save image")?;

    Ok(image_name)
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
