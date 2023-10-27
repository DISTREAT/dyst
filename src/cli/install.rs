use crate::common_directories;
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use octocrab::models::repos::Asset;
use std::env::consts;

fn auto_select_asset(assets: &Vec<Asset>) -> Option<&Asset> {
    match assets
        .into_iter()
        .map(|asset| {
            let os_match_count = asset.name.to_lowercase().matches(consts::OS).count();
            let architecture_match_count = asset.name.to_lowercase().matches(consts::ARCH).count();

            (os_match_count + architecture_match_count, asset)
        })
        .collect::<Vec<(usize, &Asset)>>()
        .into_iter()
        .sorted_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .last()
    {
        Some(lookup) => Some(lookup.1),
        None => None,
    }
}

pub async fn install_package(
    repository_author: &str,
    repository_name: &str,
    including_prerelease: bool,
) -> Result<()> {
    let package_store = common_directories::get_package_store()?;

    let releases = match octocrab::instance()
        .repos(repository_author, repository_name)
        .releases()
        .list()
        .send()
        .await
    {
        Ok(releases) => releases,
        Err(octocrab::Error::GitHub { ref source, .. }) => {
            return Err(anyhow!(
                "The requested repository could not be fetched ({})",
                source.message
            ))
        }
        Err(error) => return Err(error.into()),
    };

    let latest_release = releases
        .into_iter()
        .filter(|release| !release.prerelease || including_prerelease)
        .next()
        .context("There is no release available")?;

    let auto_selected_asset = auto_select_asset(&latest_release.assets);

    println!("{}", auto_selected_asset.unwrap().name);

    Ok(())
}
