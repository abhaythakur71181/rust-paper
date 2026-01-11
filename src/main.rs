mod helper;
mod lock;

use anyhow::Error;
use clap::{Parser, Subcommand};
use rust_paper::RustPaper;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Sync,
    Add {
        #[arg(required = true)]
        paths: Vec<String>,
    },
    Remove {
        #[arg(required = true)]
        ids: Vec<String>,
    },
    List,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::parse();
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
    }

    Ok(())
}
