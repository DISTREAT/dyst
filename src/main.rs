use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use regex::Regex;

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

        /// Allow the download of prereleases
        #[arg(short, long)]
        prerelease: bool,

        /// Select a specific asset by applying a custom regex filter on the asset name
        #[arg(short, long)]
        filter: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = ArgumentParser::parse();

    match &arguments.command {
        Commands::Install {
            repository,
            prerelease,
            filter,
        } => {
            if repository.matches('/').count() != 1 {
                return Err(anyhow!(
                    "The provided repository seems invalid (expected `AUTHOR/NAME`)"
                ));
            }

            let mut split_iterator = repository.split('/');
            let author = split_iterator.next().unwrap();
            let name = split_iterator.next().unwrap();

            let regular_expression: Option<Regex> = match filter {
                Some(custom_expression) => Some(
                    Regex::new(custom_expression)
                        .context("The filter contains illegal regex syntax")?,
                ),
                None => None,
            };

            cli::install::install_package(author, name, *prerelease, &regular_expression).await?;
        }
    }

    Ok(())
}
