use serde::{Deserialize, Serialize};
use std::default::Default;

use crate::helper;

/// Configuration for Rust Paper
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// Directory where wallpapers will be saved
    pub save_location: String,
    /// Whether to enable integrity checks using SHA256
    pub integrity: bool,
    /// Wallhaven API key for higher rate limits (optional)
    pub api_key: Option<String>,
    /// Maximum number of concurrent downloads (default: 10)
    pub max_concurrent_downloads: usize,
    /// Request timeout in seconds (default: 30)
    pub timeout: u64,
    /// Number of retry attempts (default: 3)
    pub retry_count: u32,
}

impl Default for Config {
    fn default() -> Self {
        let username = helper::get_home_location();

        let save_location = format!("{}/Pictures/wall", username);

        Config {
            save_location,
            integrity: true,
            api_key: None,
            max_concurrent_downloads: 10,
            timeout: 30,
            retry_count: 3,
        }
    }
}
