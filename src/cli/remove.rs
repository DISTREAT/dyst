use crate::common_directories;
use anyhow::Result;
use std::fs::{read_dir, read_link, remove_dir_all, remove_file};

pub async fn uninstall_package(repository_author: &str, repository_name: &str) -> Result<()> {
    let package_store = common_directories::get_package_store()?;
    let executables_path = common_directories::get_executables_path()?;
    let index_db = common_directories::open_database()?;

    let mut package_src_path = package_store.clone();
    package_src_path.push(repository_author);
    package_src_path.push(repository_name);

    remove_dir_all(&package_src_path)?;

    let mut statement = index_db.prepare("DELETE FROM packages WHERE repository = ?")?;
    statement.bind(
        1,
        format!("{}/{}", repository_author, repository_name).as_str(),
    )?;

    loop {
        if statement.next().unwrap() == sqlite3::State::Done {
            break;
        }
    }

    for entry in read_dir(executables_path)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_symlink() {
            let linked_path = read_link(&path)?;
            if linked_path.starts_with(&package_src_path) {
                remove_file(&path)?;
            }
        }
    }

    Ok(())
}
