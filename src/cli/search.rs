use anyhow::Result;

pub async fn search_repositories(query: &str) -> Result<()> {
    let page = octocrab::instance()
        .search()
        .repositories(query)
        .send()
        .await?;

    for repository in page.into_iter() {
        if repository.releases_url.is_some() {
            println!(
                "https://github.com/{} - {}",
                repository.full_name.unwrap(),
                repository.description.unwrap_or(String::from("n/a"))
            );
        }
    }

    Ok(())
}
