use crate::common_directories;
use anyhow::{anyhow, Result};
use file_format::{FileFormat, Kind};
use walkdir::WalkDir;

pub async fn list_repositories() -> Result<()> {
    let index_db = common_directories::open_database()?;

    let mut statement = index_db
        .prepare("SELECT repository, tag FROM packages")
        .unwrap();

    while let sqlite3::State::Row = statement.next().unwrap() {
        println!(
            "{} {}",
            statement.read::<String>(0).unwrap(),
            statement.read::<String>(1).unwrap(),
        );
    }

    Ok(())
}

pub fn list_executables(repository_author: &str, repository_name: &str) -> Result<()> {
    let package_store = common_directories::get_package_store()?;

    let mut asset_path = package_store.clone();
    asset_path.push(repository_author);
    asset_path.push(repository_name);

    if !asset_path.is_dir() {
        return Err(anyhow!("The requested repository is not installed"));
    }

    for entry in WalkDir::new(&asset_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let file_name = entry.file_name().to_str().unwrap();
        let format = FileFormat::from_file(path)?;

        if format.kind() == Kind::Executable {
            println!("{}", file_name);
        }
    }

    Ok(())
}
