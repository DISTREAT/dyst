use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use itertools::Itertools;
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

        /// Lock the package, preventing updates
        #[arg(short, long)]
        lock: bool,

        // This is an option of install and not a command for it is easier accessible when needed
        /// List all assets for the selected release
        #[arg(short, long)]
        assets: bool,
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
    /// Update all downloaded repositories
    Update,
    /// Lock a repository, preventing updates
    Lock {
        /// The repository to lock
        repository: String,
    },
    /// Unlock a repository, allowing updates
    Unlock {
        /// The repository to unlock
        repository: String,
    },
    /// Allow downloads of prereleases for a repository
    AllowPrereleases {
        /// The repository in question
        repository: String,
    },
    /// List all executables of an installed repository
    ListExecs {
        /// The repository in question
        repository: String,
    },
    /// Rename an executable
    Rename {
        /// The repository in question
        repository: String,
        /// Replace the executable's name (ex. `binary-xyz/binary` to replace `binary-xyz` with `binary`)
        rename: String,
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
            lock,
            assets,
        } => {
            let index_db = common_directories::open_database()?;
            let (author, name) = split_repository_argument(repository)?;

            if is_repository_installed(author, name)? && !*assets {
                return Err(anyhow!("The requested repository is already installed"));
            }

            let mut installer = cli::install::PackageInstallation::new(&index_db, author, name);
            installer.prereleases(*prerelease);
            installer.lock(*lock);

            if tag.is_some() {
                installer.latest_tag(tag.clone().unwrap());
            }

            if filter.is_some() {
                let regular_expression = Regex::new(&filter.as_ref().unwrap())
                    .context("The filter contains illegal regex syntax")?;

                installer.asset_regex_filter(regular_expression);
            }

            if rename.is_some() {
                let search_replace = rename.as_ref().unwrap();

                if search_replace.matches('/').count() != 1 {
                    return Err(anyhow!(
                        "The provided rename option seems invalid (expected `match/replace`)"
                    ));
                }

                let mut search_replace_split_iterator = search_replace.split('/');
                let search = search_replace_split_iterator.next().unwrap();
                let replace = search_replace_split_iterator.next().unwrap();

                installer.rename_executable(search.to_string(), replace.to_string());
            }

            installer.fetch_release().await?;

            if *assets {
                println!(
                    "{}",
                    installer
                        .selected_release
                        .clone()
                        .unwrap()
                        .assets
                        .into_iter()
                        .map(|asset| asset.name)
                        .join("\n")
                );
            } else {
                installer.install().await?;
            }
        }
        Commands::Remove { repository } => {
            let index_db = common_directories::open_database()?;
            let (author, name) = split_repository_argument(repository)?;

            if !is_repository_installed(author, name)? {
                return Err(anyhow!("The requested repository is not installed"));
            }

            cli::remove::uninstall_package(&index_db, author, name).await?;
        }
        Commands::List => {
            cli::list::list_repositories().await?;
        }
        Commands::Search { query } => {
            cli::search::search_repositories(query).await?;
        }
        Commands::Update => {
            let index_db = common_directories::open_database()?;

            cli::update::update_repositories(&index_db).await?;
        }
        Commands::Lock { repository } => {
            let index_db = common_directories::open_database()?;
            let (author, name) = split_repository_argument(repository)?;

            if !is_repository_installed(author, name)? {
                return Err(anyhow!("The requested repository is not installed"));
            }

            cli::lock::lock_package(&index_db, author, name).await?;
        }
        Commands::Unlock { repository } => {
            let index_db = common_directories::open_database()?;
            let (author, name) = split_repository_argument(repository)?;

            if !is_repository_installed(author, name)? {
                return Err(anyhow!("The requested repository is not installed"));
            }

            cli::lock::unlock_package(&index_db, author, name).await?;
        }
        Commands::AllowPrereleases { repository } => {
            let index_db = common_directories::open_database()?;
            let (author, name) = split_repository_argument(repository)?;

            if !is_repository_installed(author, name)? {
                return Err(anyhow!("The requested repository is not installed"));
            }

            cli::prereleases::allow_prereleases(&index_db, author, name).await?;
        }
        Commands::ListExecs { repository } => {
            let (author, name) = split_repository_argument(repository)?;

            if !is_repository_installed(author, name)? {
                return Err(anyhow!("The requested repository is not installed"));
            }

            cli::list::list_executables(author, name)?;
        }
        Commands::Rename { repository, rename } => {
            let index_db = common_directories::open_database()?;
            let (author, name) = split_repository_argument(repository)?;

            if !is_repository_installed(author, name)? {
                return Err(anyhow!("The requested repository is not installed"));
            }

            if rename.matches('/').count() != 1 {
                return Err(anyhow!(
                    "The provided rename option seems invalid (expected `match/replace`)"
                ));
            }

            let mut search_replace_split_iterator = rename.split('/');
            let search = search_replace_split_iterator.next().unwrap();
            let replace = search_replace_split_iterator.next().unwrap();

            cli::rename::rename_executable(&index_db, author, name, search, replace).await?;
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

fn is_repository_installed(author: &str, name: &str) -> Result<bool> {
    let package_store = common_directories::get_package_store()?;

    let mut asset_path = package_store.clone();
    asset_path.push(author);
    asset_path.push(name);

    Ok(asset_path.is_dir())
}
