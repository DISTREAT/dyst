use crate::cli;
use crate::split_repository_argument;
use anyhow::{Context, Result};
use regex::Regex;

pub async fn update_repositories(index_db: &sqlite3::Connection) -> Result<()> {
    let mut statement = index_db
        .prepare("SELECT repository, tag, lock, assetFilter, execRename, preReleases FROM packages")
        .unwrap();

    while let sqlite3::State::Row = statement.next().unwrap() {
        let repository = statement.read::<String>(0).unwrap();
        let tag = statement.read::<String>(1).unwrap();
        let lock = statement.read::<i64>(2).unwrap() != 0;
        let asset_filter = statement.read::<String>(3);
        let exec_rename = statement.read::<String>(4);
        let prereleases = statement.read::<i64>(5).unwrap() != 0;

        if lock {
            println!(
                "Warning: '{}' is locked and will not be updated.",
                repository
            );
            continue;
        }

        let (repository_author, repository_name) = split_repository_argument(&repository)?;

        let mut installer =
            cli::install::PackageInstallation::new(&index_db, repository_author, repository_name);
        installer.prereleases(prereleases);
        installer.fetch_release().await?;

        if asset_filter.is_ok() {
            let regular_expression = Regex::new(&asset_filter.as_ref().unwrap())
                .context("The asset filter contains illegal regex syntax")?;

            installer.asset_regex_filter(regular_expression);
        }

        if exec_rename.is_ok() {
            let search_replace = exec_rename.as_ref().unwrap();
            let mut search_replace_split_iterator = search_replace.split('/');
            let search = search_replace_split_iterator.next().unwrap();
            let replace = search_replace_split_iterator.next().unwrap();

            installer.rename_executable(search.to_string(), replace.to_string());
        }

        let release = installer.selected_release.as_ref().unwrap();

        if release.tag_name != tag {
            println!(
                "Updating '{}' from '{}' to '{}'...",
                repository, tag, release.tag_name
            );
            cli::remove::uninstall_package(&index_db, repository_author, repository_name).await?;
            installer.install().await?;
        } else {
            println!("'{}' is up to date.", repository);
        }
    }

    Ok(())
}
