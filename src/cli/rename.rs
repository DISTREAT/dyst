use crate::common_directories;
use anyhow::Result;
use file_format::{FileFormat, Kind};
use std::fs::{read_dir, read_link, remove_file};
use symlink::symlink_file;
use walkdir::WalkDir;

pub async fn rename_executable(
    index_db: &sqlite3::Connection,
    repository_author: &str,
    repository_name: &str,
    old_executable_name: &str,
    new_executable_name: &str,
) -> Result<()> {
    let package_store = common_directories::get_package_store()?;
    let executables_path = common_directories::get_executables_path()?;

    let mut package_src_path = package_store.clone();
    package_src_path.push(repository_author);
    package_src_path.push(repository_name);

    for entry in read_dir(&executables_path)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_symlink() {
            let linked_path = read_link(&path)?;
            if linked_path.starts_with(&package_src_path) {
                remove_file(&path)?;
            }
        }
    }

    for entry in WalkDir::new(&package_src_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let format = FileFormat::from_file(path)?;

        if format.kind() == Kind::Executable {
            let mut binary_path = executables_path.clone();
            let file_name = entry.file_name().to_str().unwrap();

            if old_executable_name == file_name {
                binary_path.push(new_executable_name);
            } else {
                binary_path.push(file_name);
            }

            symlink_file(path, binary_path)?;
        }
    }

    let mut statement =
        index_db.prepare("UPDATE packages SET execRename = ? WHERE repository = ?")?;
    statement.bind(
        1,
        format!("{}/{}", old_executable_name, new_executable_name).as_str(),
    )?;
    statement.bind(
        2,
        format!("{}/{}", repository_author, repository_name).as_str(),
    )?;

    loop {
        if statement.next().unwrap() == sqlite3::State::Done {
            break;
        }
    }

    Ok(())
}
