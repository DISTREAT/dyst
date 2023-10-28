use crate::common_directories;
use anyhow::{anyhow, Context, Result};
use archive_reader::Archive;
use file_format::{FileFormat, Kind};
use itertools::Itertools;
use octocrab::models::repos::Asset;
use regex::Regex;
use std::env::consts;
use std::fs::{create_dir_all, metadata, remove_dir_all, set_permissions, File};
use std::io::{copy, Cursor};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use symlink::symlink_file;
use walkdir::WalkDir;

const UNARCHIVABLE_EXTENSIONS: &'static [&str] = &["tar", "zip", "gz", "bz2", "xz", "zst", "rar"];

fn auto_select_asset<'a>(
    assets: &'a Vec<Asset>,
    custom_filter: &'a Option<Regex>,
) -> Option<&'a Asset> {
    match assets
        .into_iter()
        .map(|asset| match custom_filter {
            Some(filter) => (filter.find_iter(&asset.name.to_lowercase()).count(), asset),
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

async fn download_and_extract_asset(
    source_url: &str,
    file_base_name: &str,
    output_directory: &PathBuf,
) -> Result<()> {
    // I considered implementing a stream decompressor/unarchiver for different (combinations of) formats myself
    // but it would be unnecessary yak shaving. Thus, I am required to temporarily store the file
    // on disk to use one of the existing libraries to unarchive it.
    let file_path = PathBuf::from(file_base_name);
    let file_extension = file_path.extension().unwrap_or(std::ffi::OsStr::new(""));
    let response = reqwest::get(source_url).await?;
    let mut content = Cursor::new(response.bytes().await?);

    if UNARCHIVABLE_EXTENSIONS.contains(&file_extension.to_str().unwrap()) {
        let mut temporary_file = tempfile::NamedTempFile::new()?;
        copy(&mut content, &mut temporary_file)?;

        let temporary_path = temporary_file.into_temp_path();
        let mut archive = Archive::open(&temporary_path);
        let file_names = archive
            .block_size(1024 * 1024)
            .list_file_names()?
            .collect::<archive_reader::error::Result<Vec<_>>>()?;

        for file_name in file_names {
            let mut output_path = output_directory.clone();
            output_path.push(&file_name);
            create_dir_all(output_path.parent().unwrap())?;

            if output_path.to_str().unwrap().chars().last().unwrap() != '/' {
                let mut output_file = File::create(output_path)?;
                let _ = archive.read_file(&file_name, &mut output_file)?;
            }
        }

        temporary_path.close()?;
    } else {
        let mut output_file = output_directory.clone();
        output_file.push(file_base_name);

        let mut destination_file = File::create(output_file)?;
        copy(&mut content, &mut destination_file)?;
    }

    Ok(())
}

struct InstallErrorCleanup {
    repository: String,
    directory: PathBuf,
    persist: bool,
}

impl InstallErrorCleanup {
    pub fn new(repository: String, directory: PathBuf) -> InstallErrorCleanup {
        InstallErrorCleanup {
            repository: repository,
            directory: directory,
            persist: false,
        }
    }

    pub fn persist(&mut self) {
        self.persist = true;
    }
}

impl Drop for InstallErrorCleanup {
    fn drop(&mut self) {
        if !self.persist {
            let index_db = common_directories::open_database();

            if index_db.is_ok() {
                let connection = index_db.unwrap();
                let mut statement = connection
                    .prepare("DELETE FROM packages WHERE repository = ?")
                    .unwrap();
                statement.bind(1, self.repository.as_str()).unwrap();
                while let state = statement.next().unwrap() {
                    if state == sqlite3::State::Done {
                        break;
                    }
                }
            }

            let _ = remove_dir_all(&self.directory); // ignore error
        }
    }
}

pub async fn install_package(
    repository_author: &str,
    repository_name: &str,
    including_prerelease: bool,
    custom_filter: &Option<Regex>,
    rename_executable: &Option<(&str, &str)>,
) -> Result<()> {
    let package_store = common_directories::get_package_store()?;
    let executables_path = common_directories::get_executables_path()?;
    let index_db = common_directories::open_database()?;

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
    let mut errdefer = InstallErrorCleanup::new(
        format!("{}/{}", repository_author, repository_name),
        asset_path.clone(),
    );

    download_and_extract_asset(
        auto_selected_asset.browser_download_url.as_str(),
        &auto_selected_asset.name,
        &asset_path,
    )
    .await
    .context("Failed to download the asset")?;

    let mut statement =
        index_db.prepare("INSERT INTO packages (repository, tag, lock) VALUES(?, ?, ?)")?;
    statement
        .bind(
            1,
            format!("{}/{}", repository_author, repository_name).as_str(),
        )
        .unwrap();
    statement.bind(2, latest_release.tag_name.as_str()).unwrap();
    statement.bind(3, 0).unwrap();
    while let state = statement.next()? {
        if state == sqlite3::State::Done {
            break;
        }
    }

    for entry in WalkDir::new(&asset_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let format = FileFormat::from_file(path)?;

        if format.kind() == Kind::Executable {
            let mut permissions = metadata(path)?.permissions();
            permissions.set_mode(0o755);
            set_permissions(path, permissions)?;

            let mut binary_path = executables_path.clone();
            let file_name = entry.file_name();

            if rename_executable.unwrap_or(("", "")).0 == file_name {
                binary_path.push(rename_executable.unwrap().1);
            } else {
                binary_path.push(file_name);
            }

            symlink_file(path, binary_path)?;
        }
    }

    errdefer.persist();

    Ok(())
}
