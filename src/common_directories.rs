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
