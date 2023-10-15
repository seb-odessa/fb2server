use sqlx::sqlite::SqlitePool;

use crate::database;
use crate::opds::Feed;
use crate::utils;

pub async fn root_opds_meta(
    catalog: &SqlitePool,
    title: &String,
    root: &String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(title);
    for genre in database::genres_meta(&catalog).await? {
        feed.add(format!("{genre}"), format!("{root}/{genre}"));
    }
    return Ok(feed);
}

pub async fn root_opds_genres_meta(
    catalog: &SqlitePool,
    title: &String,
    root: &String,
    meta: &String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(title);
    for genre in database::genres_by_meta(&catalog, &meta).await? {
        feed.add(format!("{genre}"), format!("{root}/{genre}"));
    }
    return Ok(feed);
}

pub async fn root_opds_genres_series(catalog: &SqlitePool, genre: &String) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(genre);
    let mut series = database::root_opds_genres_series(&catalog, genre).await?;
    series.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

    for serie in series {
        let id = serie.id;
        let name = serie.name;
        let count = serie.count;
        feed.add(
            format!("{name} [{count} книг]"),
            format!("/opds/serie/id/{id}"),
        );
    }
    return Ok(feed);
}

pub async fn root_opds_genres_authors(catalog: &SqlitePool, genre: &String) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(genre);
    let mut authors = database::root_opds_genres_authors(&catalog, genre).await?;
    authors.sort_by(|a, b| utils::fb2sort(&a.first_name.value, &b.first_name.value));

    for author in authors {
        let title = format!(
            "{} {} {}",
            author.last_name.value, author.first_name.value, author.middle_name.value,
        );
        let link = format!(
            "/opds/author/id/{}/{}/{}",
            author.first_name.id, author.middle_name.id, author.last_name.id
        );
        feed.add(title, link);
    }
    return Ok(feed);
}
