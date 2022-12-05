use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use log::info;
use regex::Regex;
use sqlx::sqlite::SqlitePool;

use crate::database;
use crate::database::QueryType;
use crate::opds::Feed;
use crate::utils;

lazy_static! {
    static ref WRONG: HashSet<char> = HashSet::from(['À', '#', '.', '-', '%']);
}

pub async fn root_opds_by_mask(
    pool: &SqlitePool,
    query: QueryType,
    title: String,
    root: String,
) -> anyhow::Result<Feed> {
    let all = &String::from("");
    let mut feed = Feed::new(title);
    let patterns = database::make_patterns(&pool, query, &all).await?;
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
    pool: &SqlitePool,
    query: QueryType,
    title: String,
    root: String,
    mut pattern: String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new(title);

    loop {
        let mut found = false;
        let patterns = database::make_patterns(&pool, query, &pattern).await?;
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
                QueryType::Author => add_authors(&pool, &pattern, &mut feed).await?,
                QueryType::Serie => add_series(&pool, &pattern, &mut feed).await?,
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

async fn add_authors(pool: &SqlitePool, name: &String, feed: &mut Feed) -> anyhow::Result<()> {
    let mut authors = database::find_authors(&pool, &name).await?;
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

async fn add_series(pool: &SqlitePool, name: &String, feed: &mut Feed) -> anyhow::Result<()> {
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

pub async fn root_opds_authors_mask(
    pool: &SqlitePool,
    mut pattern: String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new("Поиск книг по авторам");

    loop {
        let mut found = false;
        let patterns = database::make_patterns(&pool, QueryType::Author, &pattern).await?;
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
            // Prepare authors list with exact surename (lastname)
            let mut authors = database::find_authors(&pool, &pattern).await?;
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
        }

        if tail.is_empty() {
            break;
        } else if 1 == tail.len() {
            std::mem::swap(&mut pattern, &mut tail[0]);
        } else {
            for prefix in utils::sorted(tail) {
                feed.add(
                    format!("{prefix}..."),
                    format!("/opds/authors/mask/{prefix}"),
                );
            }
            break;
        }
    }

    Ok(feed)
}

pub async fn root_opds_author_series(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Feed> {
    let (fid, mid, lid) = ids;
    let author = database::author(&pool, (fid, mid, lid)).await?;
    let mut feed = Feed::new(&author);
    let mut series = database::author_series(&pool, (fid, mid, lid)).await?;
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

pub async fn root_opds_author_serie_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32, u32),
) -> anyhow::Result<Feed> {
    let (fid, mid, lid, sid) = ids;
    let mut feed = Feed::new("Книги");
    let books = database::author_serie_books(&pool, (fid, mid, lid, sid)).await?;
    for book in books {
        let id = book.id;
        let num = book.num;
        let name = book.name;
        let date = book.date;

        feed.book(
            format!("{num} - {name} ({date})"),
            format!("/opds/book/{id}"),
        );
    }
    return Ok(feed);
}

pub async fn root_opds_author_nonserie_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Feed> {
    let (fid, mid, lid) = ids;
    let mut feed = Feed::new("Книги");
    let books = database::author_nonserie_books(&pool, (fid, mid, lid)).await?;
    for book in books {
        let id = book.id;
        let name = book.name;
        let date = book.date;

        feed.book(format!("{name} ({date})"), format!("/opds/book/{id}"));
    }
    return Ok(feed);
}

pub async fn root_opds_author_alphabet_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Feed> {
    let (fid, mid, lid) = ids;
    let mut feed = Feed::new("Книги");
    let mut books = database::author_alphabet_books(&pool, (fid, mid, lid)).await?;
    books.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

    for book in books {
        let id = book.id;
        let name = book.name;
        let date = book.date;

        feed.book(format!("{name} ({date})"), format!("/opds/book/{id}"));
    }
    return Ok(feed);
}

pub async fn root_opds_author_added_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Feed> {
    let (fid, mid, lid) = ids;
    let mut feed = Feed::new("Книги");
    let books = database::root_opds_author_added_books(&pool, (fid, mid, lid)).await?;
    for book in books {
        let id = book.id;
        let name = book.name;
        let date = book.date;

        feed.book(format!("{name} ({date})"), format!("/opds/book/{id}"));
    }
    return Ok(feed);
}

pub fn extract_book(root: PathBuf, id: u32) -> std::io::Result<PathBuf> {
    let rx = Regex::new("fb2-([0-9]+)-([0-9]+)")
        .map_err(|e| Error::new(ErrorKind::Other, format!("{e}")))?;
    let book_name = format!("{id}.fb2");
    info!("book_name: {book_name}");

    if root.is_dir() {
        for entry in fs::read_dir(&root)? {
            let path = entry?.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    let name = name.to_string_lossy();
                    if let Some(caps) = rx.captures(&name) {
                        let min = caps
                            .get(1)
                            .map_or("", |m| m.as_str())
                            .parse::<u32>()
                            .map_err(|err| Error::new(ErrorKind::Other, format!("{err}")))?;

                        let max = caps
                            .get(2)
                            .map_or("", |m| m.as_str())
                            .parse::<u32>()
                            .map_err(|err| Error::new(ErrorKind::Other, format!("{err}")))?;

                        if min <= id && id <= max {
                            let file = fs::File::open(&path)?;
                            let mut archive = zip::ZipArchive::new(file)?;
                            if let Ok(mut file) = archive.by_name(&book_name) {
                                let crc32 = file.crc32();
                                let outname = PathBuf::from(std::env::temp_dir())
                                    .join(format!("{crc32}"))
                                    .with_extension("fb2");
                                info!(
                                    "Found {} -> crc32: {crc32}, path: {}",
                                    file.name(),
                                    outname.display()
                                );
                                let mut outfile = fs::File::create(&outname)?;
                                io::copy(&mut file, &mut outfile)?;
                                return Ok(outname);
                            };
                        }
                    }
                }
            }
        }
    }
    Err(Error::new(
        ErrorKind::Other,
        format!("The book {id} was not found in {}", root.display()),
    ))
}
