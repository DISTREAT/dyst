use crate::common_directories;
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use octocrab::models::repos::Asset;
use std::env::consts;
use std::fs::{create_dir_all, File};
use std::io::{copy, Cursor};
use std::path::PathBuf;

fn auto_select_asset<'a>(
    assets: &'a Vec<Asset>,
    custom_filter: &'a Option<String>,
) -> Option<&'a Asset> {
    match assets
        .into_iter()
        .map(|asset| match custom_filter {
            Some(filter) => (
                asset
                    .name
                    .to_lowercase()
                    .matches(&filter.to_lowercase())
                    .count(),
                asset,
            ),
            None => {
                let os_match_count = asset.name.to_lowercase().matches(consts::OS).count();
                let architecture_match_count =
                    asset.name.to_lowercase().matches(consts::ARCH).count();

                (os_match_count + architecture_match_count, asset)
            }
        })
        .filter(|lookup| lookup.0 > 0)
        .collect::<Vec<(usize, &Asset)>>()
        .into_iter()
        .sorted_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .last()
    {
        Some(lookup) => Some(lookup.1),
        None => None,
    }
}

async fn downloadFile(source_url: &str, output_file: &PathBuf) -> Result<()> {
    let response = reqwest::get(source_url).await?;
    let mut destination_file = File::create(output_file)?;
    let mut content = Cursor::new(response.bytes().await?);
    copy(&mut content, &mut destination_file)?;
    Ok(())
}

pub async fn install_package(
    repository_author: &str,
    repository_name: &str,
    including_prerelease: bool,
    custom_filter: &Option<String>,
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
        .context("There is no release available (consider passing `--prerelease`)")?;

    let auto_selected_asset = auto_select_asset(&latest_release.assets, custom_filter)
        .context(format!(
            "An asset could not be automatically selected, try applying a custom filter to select one: {}",
            latest_release.assets
                .clone()
                .into_iter()
                .map(|asset| asset.name)
                .collect::<Vec<String>>()
                .join(", ")
        ))?;

    let mut asset_path = package_store.clone();
    asset_path.push(repository_author);
    asset_path.push(repository_name);
    create_dir_all(&asset_path)?;
    asset_path.push(&auto_selected_asset.name);

    downloadFile(
        auto_selected_asset.browser_download_url.as_str(),
        &asset_path,
    )
    .await
    .context("Failed to download the asset")?;

    Ok(())
}
