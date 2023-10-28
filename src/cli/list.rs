use crate::common_directories;
use anyhow::Result;

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
