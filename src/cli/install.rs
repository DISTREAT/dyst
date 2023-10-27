use crate::common_directories;
use anyhow::{anyhow, Context, Result};

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

    println!("{}", latest_release.name.unwrap());

    Ok(())
}
