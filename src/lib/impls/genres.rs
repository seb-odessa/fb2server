use sqlx::sqlite::SqlitePool;

use crate::database;
use crate::opds::Feed;

pub async fn root_opds_meta(
    pool: &SqlitePool,
    title: &String,
    root: &String
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(title);
    for genre in database::genres_meta(&pool).await? {
        feed.add(format!("{genre}"), format!("{root}/{genre}"));
    }
    return Ok(feed);
}

pub async fn root_opds_genres_meta(
    pool: &SqlitePool,
    title: &String,
    root: &String,
    meta: &String
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(title);
    for genre in database::genres_by_meta(&pool, &meta).await? {
        feed.add(format!("{genre}"), format!("{root}/{genre}"));
    }
    return Ok(feed);
}
