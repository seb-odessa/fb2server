use crate::opds::Feed;
use crate::database;
use crate::utils;

use sqlx::sqlite::SqlitePool;

pub async fn add_series(pool: &SqlitePool, name: &String, feed: &mut Feed) -> anyhow::Result<()> {
    let mut values = database::find_series(&pool, &name).await?;
    values.sort_by(|a, b| utils::fb2sort(&a.value, &b.value));

    for value in values {
        let title = format!("{}", value.value);
        let link = format!("/opds/serie/id/{}", value.id);
        feed.add(title, link);
    }
    Ok(())
}

pub async fn root_opds_serie_books(
    pool: &SqlitePool,
    id: u32,
    sort: String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new("Книги в серии");
    let mut values = database::root_opds_serie_books(&pool, id).await?;
    match sort.as_str() {
        "numbered" => values.sort_by(|a, b| a.num.cmp(&b.num)),
        "alphabet" => values.sort_by(|a, b| utils::fb2sort(&a.name, &b.name)),
        "added" => values.sort_by(|a, b| utils::fb2sort(&a.date, &b.date)),
        _ => {}
    }
    for book in values {
        let id = book.id;
        let num = book.num;
        let name = book.name;
        let date = book.date;
        let author = book.author;
        feed.book(
            format!("{num} {name} - {author} ({date})"),
            format!("/opds/book/{id}"),
        );
    }
    return Ok(feed);
}
