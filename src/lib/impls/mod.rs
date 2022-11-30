use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use log::info;
use regex::Regex;
use sqlx::sqlite::SqlitePool;

use crate::database;
use crate::opds::Feed;
use crate::utils;

lazy_static! {
    static ref WRONG: HashSet<char> = HashSet::from(['À', '#', '.', '-', '%']);
}

pub async fn root_opds_authors(pool: &SqlitePool) -> anyhow::Result<Feed> {
    let all = &String::from("");
    let mut feed = Feed::new("Поиск книг по авторам");
    let patterns = database::make_patterns(&pool, &all).await?;
    for pattern in utils::sorted(patterns) {
        if !pattern.is_empty() {
            let ch = pattern.chars().next().unwrap();
            if WRONG.contains(&ch) {
                continue;
            }
            feed.add(
                format!("{pattern}..."),
                format!("/opds/authors/mask/{pattern}"),
            );
        }
    }
    Ok(feed)
}

pub async fn root_opds_authors_mask(
    pool: &SqlitePool,
    mut pattern: String,
) -> anyhow::Result<Feed> {
    let mut feed = Feed::new("Поиск книг по авторам");

    loop {
        // Prepare authors list with exact surename (lastname)
        let mut authors = database::find_authors(&pool, &pattern).await?;
        authors.sort_by(|a, b| utils::fb2sort(&a.first_name, &b.first_name));
        for author in authors {
            let title = format!(
                "{} {} {}",
                author.last_name, author.first_name, author.middle_name,
            );
            let link = format!(
                "/opds/author/id/{}/{}/{}",
                author.first_id, author.middle_id, author.last_id
            );
            feed.add(title, link);
        }

        let patterns = database::make_patterns(&pool, &pattern).await?;
        let mut tail = patterns
            .into_iter()
            .filter(|name| *name != pattern)
            .collect::<Vec<String>>();

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
