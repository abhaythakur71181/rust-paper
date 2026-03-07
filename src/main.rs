use anyhow::Error;
use clap::Parser;
use rust_paper::{Cli, Command, RustPaper, WallhavenClient};

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
            let mut client = WallhavenClient::new(cli.command)
                .await
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
