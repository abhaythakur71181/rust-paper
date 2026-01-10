use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

use crate::helper;

#[derive(Debug, Serialize, Deserialize)]
struct LockEntry {
    image_id: String,
    image_location: String,
    sha256: String,
}

/// Lock file for tracking wallpaper integrity checksums
#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    entries: Vec<LockEntry>,
}

impl LockFile {
    /// Create a new empty lock file
    pub fn new() -> Self {
        LockFile {
            entries: Vec::new(),
        }
    }

    /// Load lock file from disk asynchronously
    pub async fn load() -> Result<Self> {
        let lock_file_location = helper::get_folder_path()
            .context("  Failed to get folder path")?
            .join("wallpaper.lock");

        if tokio::fs::metadata(&lock_file_location).await.is_ok() {
            let file = File::open(&lock_file_location).await?;
            let mut reader = BufReader::new(file);
            let mut contents = String::new();
            reader.read_to_string(&mut contents).await?;
            let lock_file: LockFile = serde_json::from_str(&contents)
                .context("  Failed to parse lock file")?;
            Ok(lock_file)
        } else {
            Err(anyhow!("  Lock file does not exist"))
        }
    }

    /// Create lock file, loading from disk if it exists, otherwise creating a new one
    pub async fn load_or_new() -> Self {
        Self::load().await.unwrap_or_else(|_| Self::new())
    }

    /// Add or update an entry in the lock file
    pub async fn add(
        &mut self,
        image_id: String,
        image_location: String,
        sha256: String,
    ) -> Result<()> {
        let lock_file_location = helper::get_folder_path()
            .context("  Failed to get folder path")?
            .join("wallpaper.lock");

        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.image_id == image_id)
        {
            entry.image_location = image_location;
            entry.sha256 = sha256;
        } else {
            self.entries.push(LockEntry {
                image_id,
                image_location,
                sha256,
            });
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&lock_file_location)
            .await
            .context("  Failed to open lock file for writing")?;

        let mut writer = BufWriter::new(file);
        let json = serde_json::to_string_pretty(&self)
            .context("  Failed to serialize lock file")?;
        writer.write_all(json.as_bytes()).await
            .context("  Failed to write lock file")?;
        writer.flush().await
            .context("  Failed to flush lock file")?;

        Ok(())
    }

    /// Check if the lock file contains an entry with the given image_id and hash
    pub fn contains(&self, image_id: &str, hash: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.image_id == image_id && entry.sha256 == hash)
    }
}

impl Default for LockFile {
    fn default() -> Self {
        Self::new()
    }
}
