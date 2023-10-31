use crate::common_directories;
use anyhow::{anyhow, Context, Result};
use archive_reader::Archive;
use file_format::{FileFormat, Kind};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use octocrab::models::repos::{Asset, Release};
use regex::Regex;
use std::cmp::min;
use std::env::consts;
use std::fs::{create_dir_all, metadata, remove_dir_all, set_permissions, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use symlink::symlink_file;
use walkdir::WalkDir;

struct InstallErrorCleanup<'a> {
    index_db: &'a sqlite3::Connection,
    repository: String,
    directory: PathBuf,
    persist: bool,
}

impl InstallErrorCleanup<'_> {
    pub fn new<'a>(
        index_db: &'a sqlite3::Connection,
        repository: String,
        directory: PathBuf,
    ) -> InstallErrorCleanup {
        InstallErrorCleanup {
            index_db: index_db,
            repository: repository,
            directory: directory,
            persist: false,
        }
    }

    pub fn persist(&mut self) {
        self.persist = true;
    }
}

impl Drop for InstallErrorCleanup<'_> {
    fn drop(&mut self) {
        if !self.persist {
            let mut statement = self
                .index_db
                .prepare("DELETE FROM packages WHERE repository = ?")
                .unwrap();
            statement.bind(1, self.repository.as_str()).unwrap();

            loop {
                if statement.next().unwrap() == sqlite3::State::Done {
                    break;
                }
            }

            let _ = remove_dir_all(&self.directory); // ignore error
        }
    }
}

pub struct PackageInstallation<'a> {
    index_db: &'a sqlite3::Connection,
    repository_author: &'a str,
    repository_name: &'a str,
    pub selected_release: Option<Release>,
    including_prerelease: bool,
    lock_updates: bool,
    override_latest_tag: Option<String>,
    asset_regex_filter: Option<Regex>,
    rename_executable: Option<(String, String)>,
}

impl PackageInstallation<'_> {
    pub fn new<'a>(
        index_db: &'a sqlite3::Connection,
        repository_author: &'a str,
        repository_name: &'a str,
    ) -> PackageInstallation<'a> {
        PackageInstallation {
            index_db: index_db,
            repository_author: repository_author,
            repository_name: repository_name,
            selected_release: None,
            including_prerelease: false,
            lock_updates: false,
            override_latest_tag: None,
            asset_regex_filter: None,
            rename_executable: None,
        }
    }

    pub fn prereleases(&mut self, include: bool) {
        self.including_prerelease = include;
    }

    pub fn lock(&mut self, lock: bool) {
        self.lock_updates = lock;
    }

    pub fn latest_tag(&mut self, tag: String) {
        self.override_latest_tag = Some(tag);
    }

    pub fn asset_regex_filter(&mut self, filter: Regex) {
        self.asset_regex_filter = Some(filter);
    }

    pub fn rename_executable(&mut self, old_name: String, new_name: String) {
        self.rename_executable = Some((old_name, new_name));
    }

    pub async fn fetch_release(&mut self) -> Result<()> {
        println!(
            "Fetching releases for '{}/{}'...",
            self.repository_author, self.repository_name
        );
        let releases = match octocrab::instance()
            .repos(self.repository_author, self.repository_name)
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

        self.selected_release = Some(match &self.override_latest_tag {
            Some(tag_name) => releases
                .into_iter()
                .filter(|release| release.tag_name == *tag_name)
                .next()
                .context("A release with the specified tag could not be found")?,
            None => releases
                .into_iter()
                .filter(|release| !release.prerelease || self.including_prerelease)
                .next()
                .context("There is no release available (consider passing `--prerelease`)")?,
        });

        Ok(())
    }

    pub async fn install(&self) -> Result<()> {
        if self.selected_release.is_none() {
            return Err(anyhow!("No release was selected prior to installation"));
        }

        let package_store = common_directories::get_package_store()?;
        let executables_path = common_directories::get_executables_path()?;
        let selected_release = self.selected_release.clone().unwrap();

        println!(
            "Starting installation for release: {}",
            selected_release.tag_name
        );

        if selected_release.assets.clone().into_iter().count() == 0 {
            return Err(anyhow!("There are no assets available"));
        }

        let auto_selected_asset = self.auto_select_asset(&selected_release.assets)
            .context(format!(
                "An asset could not be automatically selected, try applying a custom filter to select one: {}",
                selected_release.assets
                    .clone()
                    .into_iter()
                    .map(|asset| asset.name)
                    .collect::<Vec<String>>()
                    .join(", ")
            ))?;
        println!(
            "  Preparing for asset download: {}",
            auto_selected_asset.name
        );

        let mut asset_path = package_store.clone();
        asset_path.push(self.repository_author);
        asset_path.push(self.repository_name);

        create_dir_all(&asset_path)?;
        let mut errdefer = InstallErrorCleanup::new(
            self.index_db,
            format!("{}/{}", self.repository_author, self.repository_name),
            asset_path.clone(),
        );

        println!("  Downloading asset...");
        Self::download_and_extract_asset(
            auto_selected_asset.browser_download_url.as_str(),
            &auto_selected_asset.name,
            &asset_path,
        )
        .await
        .context("Failed to download the asset")?;

        println!("\n  Adding an entry to the database");
        self.add_index_db_entry()?;

        println!("  Creating symlinks to the executables...");
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
                let file_name = entry.file_name().to_str().unwrap();

                if self.rename_executable.is_some() {
                    if self.rename_executable.as_ref().unwrap().0 == file_name {
                        binary_path.push(&self.rename_executable.as_ref().unwrap().1);
                    } else {
                        binary_path.push(file_name);
                    }
                } else {
                    binary_path.push(file_name);
                }

                symlink_file(path, binary_path)?;
            }
        }

        errdefer.persist();

        println!("  Done.");
        Ok(())
    }

    fn auto_select_asset<'a>(&'a self, assets: &'a Vec<Asset>) -> Option<&Asset> {
        match assets
            .into_iter()
            .map(|asset| match &self.asset_regex_filter {
                Some(filter) => (filter.find_iter(&asset.name.to_lowercase()).count(), asset),
                None => {
                    let mut os_match_count = asset.name.to_lowercase().matches(consts::OS).count();
                    let architecture_match_count =
                        asset.name.to_lowercase().matches(consts::ARCH).count();

                    if consts::OS == "x86_64" {
                        os_match_count += asset.name.to_lowercase().matches("amd64").count();
                    }

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
        let total_size = response.content_length().unwrap(); // GitHub as a source returns the content length, so unlikely to fail
        let stream = response.bytes_stream().boxed();

        let progressbar = ProgressBar::new(total_size);
        progressbar.set_style(ProgressStyle::default_bar()
            .template("  [{elapsed_precise}] [{wide_bar}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
            .progress_chars("=>-"));

        const UNARCHIVABLE_EXTENSIONS: &'static [&str] =
            &["tar", "zip", "gz", "bz2", "xz", "zst", "rar"];

        if UNARCHIVABLE_EXTENSIONS.contains(&file_extension.to_str().unwrap()) {
            let mut temporary_file = tempfile::NamedTempFile::new()?;
            Self::copy_stream_to_file(&mut temporary_file, stream, &progressbar, &total_size)
                .await?;

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
            Self::copy_stream_to_file(&mut destination_file, stream, &progressbar, &total_size)
                .await?;
        }

        progressbar.finish();

        Ok(())
    }

    // abstraction for download_and_extract_asset
    async fn copy_stream_to_file(
        file: &mut dyn Write,
        mut stream: impl futures_core::Stream<Item = reqwest::Result<bytes::Bytes>> + std::marker::Unpin,
        progressbar: &ProgressBar,
        total_size: &u64,
    ) -> Result<()> {
        let mut progress: u64 = 0;

        while let Some(item) = stream.next().await {
            let chunk = item.or(Err(anyhow!("Could not download the asset")))?;
            file.write_all(&chunk)
                .or(Err(anyhow!("Could not save the asset to disk")))?;
            progress = min(progress + (chunk.len() as u64), *total_size);
            progressbar.set_position(progress);
        }

        Ok(())
    }

    fn add_index_db_entry(&self) -> Result<()> {
        // stupid cargo formatter makes the code look horrendous (well, at least consistently horrendous)
        let mut statement =
            self.index_db.prepare("INSERT INTO packages (repository, tag, lock, assetFilter, execRename, preReleases) VALUES(?, ?, ?, ?, ?, ?)")?;
        statement
            .bind(
                1,
                format!("{}/{}", self.repository_author, self.repository_name).as_str(),
            )
            .unwrap();
        statement
            .bind(2, self.selected_release.as_ref().unwrap().tag_name.as_str())
            .unwrap();
        statement.bind(3, self.lock_updates as i64).unwrap();
        match &self.asset_regex_filter {
            Some(filter) => statement.bind(4, filter.as_str()).unwrap(),
            None => statement.bind(4, &sqlite3::Value::Null).unwrap(),
        };
        match &self.rename_executable {
            Some(rename) => statement
                .bind(5, format!("{}/{}", rename.0, rename.1).as_str())
                .unwrap(),
            None => statement.bind(5, &sqlite3::Value::Null).unwrap(),
        };
        statement.bind(6, self.including_prerelease as i64).unwrap();

        loop {
            if statement.next().unwrap() == sqlite3::State::Done {
                break;
            }
        }

        Ok(())
    }
}
