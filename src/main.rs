use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

mod cli;
mod common_directories;

#[derive(Parser)]
#[command(author, version, about)]
struct ArgumentParser {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install an asset from a GitHub repository
    Install {
        /// The repository to install from (ex. DISTREAT/projavu)
        repository: String,

        /// Select a specific asset by applying a custom filter on the asset name
        #[arg(short, long)]
        filter: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = ArgumentParser::parse();

    match &arguments.command {
        Commands::Install { repository, filter } => {
            if repository.matches('/').count() != 1 {
                return Err(anyhow!(
                    "The provided repository seems invalid (expected `AUTHOR/NAME`)"
                ));
            }

            let mut split_iterator = repository.split('/');
            let author = split_iterator.next().unwrap();
            let name = split_iterator.next().unwrap();

            cli::install::install_package(author, name, true, filter).await?;
        }
    }

    Ok(())
}
