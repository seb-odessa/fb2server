use sqlx::sqlite::SqlitePool;

use crate::database;
use crate::opds::Feed;
use crate::utils;

pub enum Sort {
    BySerie(u32),
    NoSerie,
    ByGenre(u32),
    Alphabet,
    Added,
}

pub async fn add_authors(
    catalog: &SqlitePool,
    name: &String,
    feed: &mut Feed,
) -> anyhow::Result<()> {
    let mut authors = database::find_authors(&catalog, &name).await?;
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
    Ok(())
}

pub async fn root_opds_author_series(
    catalog: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Feed> {
    let (fid, mid, lid) = ids;
    let author = database::author(&catalog, (fid, mid, lid)).await?;
    let mut feed = Feed::new(&author);
    let mut series = database::author_series(&catalog, (fid, mid, lid)).await?;
    series.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

    for serie in series {
        let sid = serie.id;
        let name = serie.name;
        let count = serie.count;
        feed.add(
            format!("{name} [{count} книг]"),
            format!("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}"),
        );
    }
    feed.add(author, format!("/opds/author/id/{fid}/{mid}/{lid}"));
    return Ok(feed);
}

pub async fn root_opds_author_books(
    catalog: &SqlitePool,
    ids: (u32, u32, u32),
    sort: Sort,
) -> anyhow::Result<Feed> {
    let (fid, mid, lid) = ids;
    let mut feed = Feed::new("Книги");
    let mut books = match sort {
        Sort::BySerie(sid) => database::author_serie_books(&catalog, (fid, mid, lid, sid)).await?,
        Sort::NoSerie => database::author_nonserie_books(&catalog, (fid, mid, lid)).await?,
        Sort::ByGenre(_) => Vec::new(), // Not Impl
        Sort::Alphabet => database::author_books(&catalog, (fid, mid, lid)).await?,
        Sort::Added => database::author_books(&catalog, (fid, mid, lid)).await?,
    };

    match sort {
        Sort::NoSerie => {
            books.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));
        }
        Sort::Alphabet => {
            books.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));
        }
        Sort::Added => {
            books.sort_by(|a, b| b.date.cmp(&a.date));
        }
        _ => {}
    }

    for book in books {
        let id = book.id;
        let num = book.num;
        let name = book.name;
        let date = book.date;
        let title = if let Sort::BySerie(_) = sort {
            format!("{num} - {name} ({date})")
        } else {
            format!("{name} ({date})")
        };
        let link = format!("/opds/book/{id}");
        feed.book(title, link);
    }
    return Ok(feed);
}
