use crate::helper;
use crate::RustPaper;
use anyhow::{Context, Error};

pub const BASE_URL: &str = "https://wallhaven.cc/api/v1";

// ------------------------------------------------------------
// Api response types
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    #[serde(rename = "data")]
    pub data: Vec<Wallpaper>,
    #[serde(rename = "meta")]
    pub meta: WallpaperMeta,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct Wallpaper {
    pub id: String,
    pub url: String,
    pub short_url: String,
    pub views: i32,
    pub favorites: i32,
    pub source: String,
    pub purity: String,
    pub category: String,
    pub dimension_x: i32,
    pub dimension_y: i32,
    pub resolution: String,
    pub ratio: String,
    pub file_size: i32,
    pub file_type: String,
    pub created_at: String,
    pub colors: Vec<String>,
    pub path: String,
    pub thumbs: Thumbs,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Thumbs {
    large: String,
    original: String,
    small: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct WallpaperMeta {
    current_page: i32,
    last_page: i32,
    #[serde(deserialize_with = "serde_aux::field_attributes::deserialize_number_from_string")]
    per_page: i32, //Should be int,idk why its string even when api guide defines as int
    total: i32,
    query: MetaQuery,
    seed: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum MetaQuery {
    Query(Option<String>),
    Querytag { id: i32, tag: Option<String> },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct WallpaperInfoResponse {
    #[serde(rename = "data")]
    pub data: WallpaperInfo,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct WallpaperInfo {
    pub id: String,
    pub url: String,
    pub short_url: String,
    pub uploader: Uploader,
    pub views: i32,
    pub favorites: i32,
    pub source: String,
    pub purity: String,
    pub category: String,
    pub dimension_x: i32,
    pub dimension_y: i32,
    pub resolution: String,
    pub ratio: String,
    pub file_size: i32,
    pub file_type: String,
    pub created_at: String,
    pub colors: Vec<String>,
    pub path: String,
    pub thumbs: Thumbs,
    pub tags: Vec<Tag>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Uploader {
    pub username: String,
    pub group: String,
    pub avatar: Avatar,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Avatar {
    #[serde(rename = "200px")]
    pub _200px: String,
    #[serde(rename = "128px")]
    pub _128px: String,
    #[serde(rename = "32px")]
    pub _32px: String,
    #[serde(rename = "20px")]
    pub _20px: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct TagResponse {
    #[serde(rename = "data")]
    pub data: Tag,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct Tag {
    pub id: i32,
    pub name: String,
    pub alias: String,
    pub category_id: i32,
    pub category: String,
    pub purity: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct UserSettingsResponse {
    #[serde(rename = "data")]
    pub data: UserSettings,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct UserSettings {
    pub thumb_size: String,
    pub per_page: String,
    pub purity: Vec<String>,
    pub categories: Vec<String>,
    pub resolutions: Vec<String>,
    pub aspect_ratios: Vec<String>,
    pub toplist_range: String,
    pub tag_blacklist: Vec<String>,
    pub user_blacklist: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct UserCollectionsResponse {
    #[serde(rename = "data")]
    pub data: Vec<UserCollections>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct UserCollections {
    pub id: i32,
    pub label: String,
    pub views: i32,
    pub public: i32,
    pub count: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "")]
pub struct ErrorResponse {
    pub error: String,
}

pub(crate) trait Url {
    fn to_url(&self, base_url: &str) -> String;
}

use futures::stream::StreamExt;
use futures::TryFutureExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use crate::args::Command;
use crate::helper::get_key_from_config_or_env;

#[derive(Debug)]
pub enum WallhavenClientError {
    RequestError(String),
    DecodeError(String),
    WriteError(String),
    Error(String),
}

impl std::fmt::Display for WallhavenClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecodeError(e) => {
                write!(f, "Decode Error - {}", e)
            }
            Self::WriteError(e) => {
                write!(f, "Write Error - {}", e)
            }
            Self::RequestError(e) => {
                write!(f, "Request Error - {}", e)
            }
            Self::Error(e) => {
                write!(f, "Error - {}", e)
            }
        }
    }
}

impl std::error::Error for WallhavenClientError {}

pub struct WallhavenClient {
    http_client: reqwest::Client,
    commands: Command,
    rust_paper: RustPaper,
}

impl WallhavenClient {
    pub async fn new(commands: Command) -> Result<Self, Error> {
        let rust_paper = RustPaper::new().await?;
        let api_key = get_key_from_config_or_env(rust_paper.config().api_key.as_deref());
        if api_key.is_none() {
            eprintln!("❌ Error: API key is required for this command.");
            eprintln!(
                "   Please set WALLHAVEN_API_KEY environment variable or add api_key to config."
            );
            eprintln!("   Example: export WALLHAVEN_API_KEY=\"your_api_key_here\"");
            std::process::exit(1);
        }
        /* Create http client */
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        if let Some(k) = api_key {
            let header_api_value =
                reqwest::header::HeaderValue::from_str(&k).context("Invalid API key format")?;
            headers.insert("X-API-KEY", header_api_value);
        }

        let client = reqwest::ClientBuilder::new()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(rust_paper.config.timeout))
            .build()
            .context("Unable to create http client")?;

        Ok(Self {
            http_client: client,
            commands,
            rust_paper,
        })
    }

    pub async fn execute(&mut self) -> Result<String, WallhavenClientError> {
        let resp = match &self.commands {
            Command::Search(s) => {
                let res = self.request(s.to_url(BASE_URL)).await?;

                // Check if we got bad status response and return it
                if let Ok(r) = serde_json::from_str::<ErrorResponse>(&res) {
                    return Err(WallhavenClientError::RequestError(r.error));
                }

                // Check if response has the structure as described in api guide
                let searchresp: SearchResponse = serde_json::from_str(&res)
                    .map_err(|e| WallhavenClientError::DecodeError(e.to_string()))?;

                //download wallpapers
                if s.download {
                    use crate::helper::download_with_progress;

                    println!(
                        "  Found {} wallpaper(s), downloading to: {}\n",
                        searchresp.data.len(),
                        self.rust_paper.config.save_location
                    );

                    let mut lock_updates = Vec::new();

                    for w in &searchresp.data {
                        match download_with_progress(
                            &w.path,
                            &w.id,
                            &self.rust_paper.config.save_location,
                            &self.http_client,
                            self.rust_paper.config.integrity,
                            true,
                        )
                        .await
                        {
                            Ok(result) => {
                                println!("  ✓ Downloaded {} - {}", &w.id, &result.file_path);
                                lock_updates.push((
                                    w.id.clone(),
                                    result.file_path,
                                    result.sha256.clone(),
                                ));
                                if let Some(hash) = &result.sha256 {
                                    println!("    SHA256: {}", &hash);
                                }
                            }
                            Err(e) => {
                                eprintln!("  ✗ Failed to download {}: {}", w.id, e);
                            }
                        }
                    }

                    // Update wallpapers.lst and lock file
                    if !lock_updates.is_empty() {
                        if let Err(e) = helper::update_wallpapers_list_and_lock(
                            lock_updates,
                            &mut self.rust_paper,
                        )
                        .await
                        {
                            eprintln!("  ⚠ Failed to update wallpapers list and lock file: {}", e);
                        }
                    }

                    String::from("\n  ✅ Download complete!")
                } else {
                    format_search_results(&searchresp)
                }
            }
            Command::TagInfo(t) => {
                let res = self.request(t.to_url(BASE_URL)).await?;

                if let Ok(r) = serde_json::from_str::<ErrorResponse>(&res) {
                    return Err(WallhavenClientError::RequestError(r.error));
                }

                let taginfo: TagResponse = serde_json::from_str(&res)
                    .map_err(|e| WallhavenClientError::DecodeError(e.to_string()))?;

                format_tag_info(&taginfo.data)
            }
            Command::UserSettings(us) => {
                let res = self.request(us.to_url(BASE_URL)).await?;

                if let Ok(r) = serde_json::from_str::<ErrorResponse>(&res) {
                    return Err(WallhavenClientError::RequestError(r.error));
                }

                let usersettings: UserSettingsResponse = serde_json::from_str(&res)
                    .map_err(|e| WallhavenClientError::DecodeError(e.to_string()))?;

                format_user_settings(&usersettings.data)
            }
            Command::UserCollections(uc) => {
                let res = self.request(uc.to_url(BASE_URL)).await?;

                if let Ok(r) = serde_json::from_str::<ErrorResponse>(&res) {
                    return Err(WallhavenClientError::RequestError(r.error));
                }

                let usercollections: UserCollectionsResponse = serde_json::from_str(&res)
                    .map_err(|e| WallhavenClientError::DecodeError(e.to_string()))?;

                format_user_collections(&usercollections.data)
            }
            _ => String::new(),
        };

        Ok(resp)
    }

    pub async fn request(&self, url: String) -> Result<String, WallhavenClientError> {
        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| WallhavenClientError::RequestError(e.to_string()))?;

        match response.text().await {
            Ok(r) => Ok(r),
            Err(e) => Err(WallhavenClientError::DecodeError(e.to_string())),
        }
    }

    pub async fn download_image(
        &self,
        url: &str,
        path: &std::path::PathBuf,
    ) -> Result<(), WallhavenClientError> {
        // Reqwest setup
        let res = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| WallhavenClientError::RequestError(e.to_string()))?;

        // Get information for bar
        let total_size = res
            .content_length()
            .ok_or(format!("Failed to get content length from '{}'", &url))
            .map_err(|e| WallhavenClientError::RequestError(e))?;

        // Indicatif setup
        let pb = ProgressBar::new(total_size);
        let style = ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(style);
        pb.set_message(format!("Downloading {}", url));

        // Create file path
        let file_path = std::path::Path::new(path);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
            .await
            .map_err(|e| {
                WallhavenClientError::WriteError(format!(
                    "Failed to create file - {}",
                    e.to_string()
                ))
            })?;

        // Write file
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item.or(Err(WallhavenClientError::RequestError(format!(
                "Error while downloading file"
            ))))?;

            file.write_all(&chunk)
                .map_err(|e| {
                    WallhavenClientError::WriteError(format!(
                        "Error while writing to file - {}",
                        e.to_string()
                    ))
                })
                .await?;

            let new = u64::min(downloaded + (chunk.len() as u64), total_size);
            downloaded = new;
            pb.set_position(new);
        }

        pb.finish_with_message(format!("Downloaded {}", url));

        Ok(())
    }

    /// Download image with SHA256 hashing support
    pub async fn download_image_with_hash(
        &self,
        url: &str,
        path: &std::path::PathBuf,
    ) -> Result<String, WallhavenClientError> {
        use sha2::{Digest, Sha256};
        let res = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| WallhavenClientError::RequestError(e.to_string()))?;
        // Get information for bar
        let total_size = res
            .content_length()
            .ok_or(format!("Failed to get content length from '{}'", &url))
            .map_err(|e| WallhavenClientError::RequestError(e))?;
        // Indicatif setup
        let pb = ProgressBar::new(total_size);
        let style = ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(style);
        pb.set_message(format!("Downloading {}", url));
        let file_path = std::path::Path::new(path);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
            .await
            .map_err(|e| {
                WallhavenClientError::WriteError(format!(
                    "Failed to create file - {}",
                    e.to_string()
                ))
            })?;
        let mut hasher = Sha256::new();
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk = item.or(Err(WallhavenClientError::RequestError(format!(
                "Error while downloading file"
            ))))?;
            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|e| {
                    WallhavenClientError::WriteError(format!(
                        "Error while writing to file - {}",
                        e.to_string()
                    ))
                })
                .await?;
            let new = u64::min(downloaded + (chunk.len() as u64), total_size);
            downloaded = new;
            pb.set_position(new);
        }
        pb.finish_with_message(format!("Downloaded {}", url));
        let hash = format!("{:x}", hasher.finalize());
        Ok(hash)
    }
}

/// Format tag information for display
fn format_tag_info(tag: &Tag) -> String {
    let mut output = String::new();
    output.push_str("  Tag Information:\n");
    output.push_str("  ────────────────\n");
    output.push_str(&format!("  ID: {}\n", tag.id));
    output.push_str(&format!("  Name: {}\n", tag.name));
    if !tag.alias.is_empty() {
        output.push_str(&format!("  Alias: {}\n", tag.alias));
    }
    output.push_str(&format!(
        "  Category: {} (ID: {})\n",
        tag.category, tag.category_id
    ));
    output.push_str(&format!("  Purity: {}\n", tag.purity));
    output.push_str(&format!("  Created: {}\n", tag.created_at));
    output
}

/// Format user settings for display
fn format_user_settings(settings: &UserSettings) -> String {
    let mut output = String::new();
    output.push_str("  Your Wallhaven Settings:\n");
    output.push_str("  ────────────────────────\n");
    output.push_str(&format!("  Thumbnail Size: {}\n", settings.thumb_size));
    output.push_str(&format!("  Per Page: {}\n", settings.per_page));
    output.push_str(&format!("  Purity: {}\n", settings.purity.join(", ")));
    output.push_str(&format!(
        "  Categories: {}\n",
        settings.categories.join(", ")
    ));
    if !settings.resolutions.is_empty() && settings.resolutions[0] != "" {
        output.push_str(&format!(
            "  Resolutions: {}\n",
            settings.resolutions.join(", ")
        ));
    }
    if !settings.aspect_ratios.is_empty() && settings.aspect_ratios[0] != "" {
        output.push_str(&format!(
            "  Aspect Ratios: {}\n",
            settings.aspect_ratios.join(", ")
        ));
    }
    output.push_str(&format!("  Toplist Range: {}\n", settings.toplist_range));
    if !settings.tag_blacklist.is_empty() && settings.tag_blacklist[0] != "" {
        output.push_str(&format!(
            "  Tag Blacklist: {}\n",
            settings.tag_blacklist.join(", ")
        ));
    }
    if !settings.user_blacklist.is_empty() && settings.user_blacklist[0] != "" {
        output.push_str(&format!(
            "  User Blacklist: {}\n",
            settings.user_blacklist.join(", ")
        ));
    }
    output
}

/// Format user collections for display
fn format_user_collections(collections: &[UserCollections]) -> String {
    let mut output = String::new();
    if collections.is_empty() {
        output.push_str("  No collections found.\n");
        return output;
    }
    output.push_str(&format!("  Collections ({} total):\n", collections.len()));
    output.push_str("  ────────────────────────\n\n");
    for collection in collections {
        output.push_str(&format!("  📁 {}\n", collection.label));
        output.push_str(&format!("     ID: {}\n", collection.id));
        output.push_str(&format!("     Wallpapers: {}\n", collection.count));
        output.push_str(&format!("     Views: {}\n", collection.views));
        output.push_str(&format!(
            "     Visibility: {}\n",
            if collection.public == 1 {
                "Public"
            } else {
                "Private"
            }
        ));
        output.push_str("\n");
    }
    output
}

/// Format search results for display
fn format_search_results(search_resp: &SearchResponse) -> String {
    let mut output = String::new();
    if search_resp.data.is_empty() {
        output.push_str("  No wallpapers found matching your search criteria.\n");
        return output;
    }
    output.push_str(&format!("  Search Results:\n"));
    output.push_str("  ───────────────\n");
    output.push_str(&format!(
        "  Found: {} wallpaper(s)\n",
        search_resp.meta.total
    ));
    output.push_str(&format!(
        "  Page: {} of {}\n",
        search_resp.meta.current_page, search_resp.meta.last_page
    ));
    output.push_str(&format!("  Per Page: {}\n", search_resp.meta.per_page));
    if let Some(ref seed) = search_resp.meta.seed {
        output.push_str(&format!("  Seed: {}\n", seed));
    }
    output.push_str("\n");
    // Display each wallpaper
    for (idx, wallpaper) in search_resp.data.iter().enumerate() {
        output.push_str(&format!(
            "  {}. 🖼️  {} ({})\n",
            idx + 1,
            wallpaper.id,
            wallpaper.resolution
        ));
        output.push_str(&format!("     URL: {}\n", wallpaper.url));
        output.push_str(&format!(
            "     Category: {} | Purity: {}\n",
            wallpaper.category, wallpaper.purity
        ));
        output.push_str(&format!(
            "     Size: {:.2} MB | Type: {}\n",
            wallpaper.file_size as f64 / 1_048_576.0,
            wallpaper.file_type.replace("image/", "")
        ));
        output.push_str(&format!(
            "     Views: {} | Favorites: {}\n",
            wallpaper.views, wallpaper.favorites
        ));
        if !wallpaper.colors.is_empty() {
            output.push_str(&format!("     Colors: {}\n", wallpaper.colors.join(", ")));
        }
        output.push_str(&format!("     Download: {}\n", wallpaper.path));
        output.push_str("\n");
    }

    // Add pagination hint if there are more pages
    if search_resp.meta.current_page < search_resp.meta.last_page {
        output.push_str(&format!(
            "  💡 Tip: Use --page {} to see more results\n",
            search_resp.meta.current_page + 1
        ));
    }

    output
}
