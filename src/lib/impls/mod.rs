use sqlx::sqlite::SqlitePool;
use std::collections::HashSet;

use crate::database;
use crate::opds::Feed;
use crate::utils;
use log::info;

lazy_static! {
    static ref WRONG: HashSet<char> = HashSet::from(['À', '#', '.', '-', '%']);
}

pub mod books;
pub use books::extract_book;

#[derive(Debug, PartialEq)]
pub struct FeedSrc {
    pub label: String,
    pub root: String,
    pub path: String,
}
impl FeedSrc {
    pub fn new<T: Into<String>>(label: T, root: T, path: T) -> Self {
        Self {
            label: label.into(),
            root: root.into(),
            path: path.into(),
        }
    }
}

fn href_label(content: &String) -> String {
    format!("{content}...")
}

fn href_link(root: &String, content: &String) -> String {
    format!("{root}/{}", utils::encode(content))
}

pub fn add_to_feed(src: FeedSrc, feed: &mut Feed) {
    let label = href_label(&src.label);
    let link = href_link(&src.root, &src.path);
    info!("add_to_feed: {label} -> {link}");
    feed.add(label, link);
}

pub fn fill_feed_from(lines: Vec<String>, root: String, feed: &mut Feed) {
    for line in utils::sorted(lines) {
        if !line.is_empty() {
            // let ch = line.chars().next().unwrap();
            // if WRONG.contains(&ch) {
            //     continue;
            // }
            let label = href_label(&line);
            let link = href_link(&root, &line);
            info!("make_feed_from: {label} -> {link}");
            feed.add(label, link);
        }
    }
}

pub fn make_feed_from(
    lines: anyhow::Result<Vec<String>>,
    title: String,
    root: String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(title);

    fill_feed_from(lines?, root, &mut feed);

    Ok(feed)
}

pub async fn root_opds_favorite_authors(
    books: &SqlitePool,
    statistic: &SqlitePool,
) -> anyhow::Result<Feed> {
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
