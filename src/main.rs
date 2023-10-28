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

        /// Specify a tag to install
        #[arg(short, long)]
        tag: Option<String>,

        /// Allow the download of prereleases
        #[arg(short, long)]
        prerelease: bool,

        /// Select a specific asset by applying a custom regex filter on the asset name
        #[arg(short, long)]
        filter: Option<String>,

        /// Replace the executable's name (ex. `binary-xyz/binary` to replace `binary-xyz` with `binary`)
        #[arg(short, long)]
        rename: Option<String>,
    },
    /// Remove an installed asset
    Remove {
        /// The repository to uninstall (ex. DISTREAT/projavu)
        repository: String,
    },
    /// List all installed repositories
    List,
    /// Search GitHub for repositories
    Search {
        /// The keyword to search for
        query: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = ArgumentParser::parse();

    match &arguments.command {
        Commands::Install {
            repository,
            tag,
            prerelease,
            filter,
            rename,
        } => {
            let (author, name) = split_repository_argument(repository)?;

            let regular_expression: Option<Regex> = match filter {
                Some(custom_expression) => Some(
                    Regex::new(custom_expression)
                        .context("The filter contains illegal regex syntax")?,
                ),
                None => None,
            };

            let executable_rename: Option<(&str, &str)> = match rename {
                Some(search_replace) => {
                    if search_replace.matches('/').count() != 1 {
                        return Err(anyhow!(
                            "The provided rename option seems invalid (expected `match/replace`)"
                        ));
                    }

                    let mut search_replace_split_iterator = search_replace.split('/');
                    let search = search_replace_split_iterator.next().unwrap();
                    let replace = search_replace_split_iterator.next().unwrap();

                    Some((search, replace))
                }
                None => None,
            };

            cli::install::install_package(
                author,
                name,
                *prerelease,
                &tag,
                &regular_expression,
                &executable_rename,
            )
            .await?;
        }
        Commands::Remove { repository } => {
            let (author, name) = split_repository_argument(repository)?;

            cli::remove::uninstall_package(author, name).await?;
        }
        Commands::List => {
            cli::list::list_repositories().await?;
        }
        Commands::Search { query } => {
            cli::search::search_repositories(query).await?;
        }
    }

    Ok(())
}

fn split_repository_argument(repository: &str) -> Result<(&str, &str)> {
    if repository.matches('/').count() != 1 {
        return Err(anyhow!(
            "The provided repository seems invalid (expected `author/name`)"
        ));
    }

    let mut split_iterator = repository.split('/');
    let author = split_iterator.next().unwrap();
    let name = split_iterator.next().unwrap();

    Ok((author, name))
}
