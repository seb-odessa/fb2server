use sqlx::sqlite::SqlitePool;
use std::collections::HashSet;


use crate::database;
use crate::database::QueryType;
use crate::opds::Feed;
use crate::utils;

lazy_static! {
    static ref WRONG: HashSet<char> = HashSet::from(['À', '#', '.', '-', '%']);
}

pub mod authors;
use authors::add_authors;
pub use authors::root_opds_author_books;
pub use authors::root_opds_author_series;

pub mod series;
use series::add_series;
pub use series::root_opds_serie_books;

pub mod books;
pub use books::extract_book;

pub mod genres;
pub use genres::root_opds_genres_authors;
pub use genres::root_opds_genres_meta;
pub use genres::root_opds_genres_series;
pub use genres::root_opds_meta;

pub async fn root_opds_by_mask(
    catalog: &SqlitePool,
    query: QueryType,
    title: String,
    root: String,
) -> anyhow::Result<Feed> {
    let all = &String::from("");
    let mut feed = Feed::new(title);
    let patterns = database::make_patterns(&catalog, query, &all).await?;
    for pattern in utils::sorted(patterns) {
        if !pattern.is_empty() {
            let ch = pattern.chars().next().unwrap();
            if WRONG.contains(&ch) {
                continue;
            }
            feed.add(format!("{pattern}..."), format!("{root}/{pattern}"));
        }
    }
    Ok(feed)
}

pub async fn root_opds_search_by_mask(
    catalog: &SqlitePool,
    query: QueryType,
    title: String,
    root: String,
    mut pattern: String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(title);

    loop {
        let mut found = false;
        let patterns = database::make_patterns(&catalog, query, &pattern).await?;
        let mut tail = patterns
            .into_iter()
            .filter(|name| {
                if !found {
                    found = pattern.eq(name);
                }
                *name != pattern
            })
            .collect::<Vec<String>>();

        if found {
            match query {
                QueryType::Author => add_authors(&catalog, &pattern, &mut feed).await?,
                QueryType::Serie => add_series(&catalog, &pattern, &mut feed).await?,
            }
        }

        if tail.is_empty() {
            break;
        } else if 1 == tail.len() {
            std::mem::swap(&mut pattern, &mut tail[0]);
        } else {
            for prefix in utils::sorted(tail) {
                feed.add(format!("{prefix}..."), format!("{root}/{prefix}"));
            }
            break;
        }
    }

    Ok(feed)
}

pub async fn root_opds_favorite_authors(books: &SqlitePool, statistic: &SqlitePool) -> anyhow::Result<Feed> {

    let ids = database::get_favorites(statistic).await?;
    let mut feed = Feed::new("Favorites");
    let authors = database::root_opds_favorite_authors(books, ids).await?;

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