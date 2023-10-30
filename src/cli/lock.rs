use anyhow::Result;

pub async fn lock_package(
    index_db: &sqlite3::Connection,
    repository_author: &str,
    repository_name: &str,
) -> Result<()> {
    let mut statement = index_db.prepare("UPDATE packages SET lock = ? WHERE repository = ?")?;
    statement.bind(1, 1)?;
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

pub async fn unlock_package(
    index_db: &sqlite3::Connection,
    repository_author: &str,
    repository_name: &str,
) -> Result<()> {
    let mut statement = index_db.prepare("UPDATE packages SET lock = ? WHERE repository = ?")?;
    statement.bind(1, 0)?;
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
