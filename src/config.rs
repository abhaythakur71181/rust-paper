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
}

impl Default for Config {
    fn default() -> Self {
        let username = helper::get_home_location();

        let save_location = format!("{}/Pictures/wall", username);

        Config {
            save_location,
            integrity: true,
        }
    }
}
