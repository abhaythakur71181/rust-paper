mod api;
mod args;
mod helper;
mod lock;

use anyhow::Error;
use clap::Parser;
use rust_paper::RustPaper;

use crate::api::{get_key_from_config_or_env, WallhavenClient};
use crate::args::{Cli, Command};

#[tokio::main(flavor = "multi_thread", worker_threads = 100)]
async fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        // Original commands - don't require API key
        Command::Sync
        | Command::Add { .. }
        | Command::Remove { .. }
        | Command::List
        | Command::Clean
        | Command::Info { .. } => {
            let mut rust_paper = RustPaper::new().await?;
            match cli.command {
                Command::Sync => {
                    rust_paper.sync().await?;
                }
                Command::Add { mut paths } => {
                    rust_paper.add(&mut paths).await?;
                }
                Command::Remove { ids } => {
                    rust_paper.remove(&ids).await?;
                }
                Command::List => {
                    rust_paper.list().await?;
                }
                Command::Clean => {
                    rust_paper.clean().await?;
                }
                Command::Info { id } => {
                    rust_paper.info(&id).await?;
                }
                _ => unreachable!(),
            }
        }
        // New API commands - require API key
        Command::Search(_)
        | Command::TagInfo(_)
        | Command::UserSettings(_)
        | Command::UserCollections(_) => {
            let rust_paper = RustPaper::new().await?;
            let api_key = get_key_from_config_or_env(rust_paper.config().api_key.as_deref());
            if api_key.is_none() {
                eprintln!("❌ Error: API key is required for this command.");
                eprintln!("   Please set WALLHAVEN_API_KEY environment variable or add api_key to config.");
                eprintln!("   Example: export WALLHAVEN_API_KEY=\"your_api_key_here\"");
                std::process::exit(1);
            }
            let client = WallhavenClient::new(cli.command, api_key)
                .map_err(|e| anyhow::anyhow!("Failed to create API client: {}", e))?;
            let result = client
                .execute()
                .await
                .map_err(|e| anyhow::anyhow!("API request failed: {}", e))?;
            if !result.is_empty() {
                println!("{}", result);
            }
        }
    }

    Ok(())
}
