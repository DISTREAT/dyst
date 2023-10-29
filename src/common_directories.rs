use anyhow::Result;
use microxdg::{Xdg, XdgApp};
use std::env;
use std::path::PathBuf;

pub fn get_package_store() -> Result<PathBuf> {
    match env::var("DYST_PACKAGE_STORE") {
        Ok(package_store) => return Ok(PathBuf::from(package_store)),
        Err(_) => {
            let xdg = XdgApp::new("dyst")?;
            let config_dir = xdg.app_data()?;

            return Ok(config_dir);
        }
    }
}

pub fn get_executables_path() -> Result<PathBuf> {
    match env::var("DYST_BINARIES_PATH") {
        Ok(package_store) => return Ok(PathBuf::from(package_store)),
        Err(_) => return Ok(Xdg::new()?.exec()),
    }
}

pub fn open_database() -> Result<sqlite3::Connection> {
    let mut package_store = get_package_store()?;
    package_store.push("index.db3");

    let connection = sqlite3::open(package_store)?;

    connection.execute(
        "
        CREATE TABLE IF NOT EXISTS packages (
            repository TEXT PRIMARY KEY UNIQUE,
            tag TEXT NOT NULL,
            lock INTEGER NOT NULL,
            assetFilter TEXT,
            execRename TEXT,
            preReleases INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS dyst (
            key TEXT PRIMARY KEY UNIQUE,
            value TEXT
        );
        INSERT OR IGNORE INTO dyst (key, value) VALUES('version', '1');
        ",
    )?;

    Ok(connection)
}
