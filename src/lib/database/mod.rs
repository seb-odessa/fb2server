use crate::utils;
use async_trait::async_trait;
use log::debug;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

pub mod lib_rus_ec;
pub use lib_rus_ec::LibRusEcOffline;

#[async_trait]
pub trait LibraryProvider: Send {
    async fn get_authors_names_starts_with(
        &self,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)>;

    async fn get_authors_names_by_genre_starts_with(
        &self,
        genre_name: &String,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)>;

    async fn get_series_names_starts_with(
        &self,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)>;

    async fn get_series_names_by_genre_starts_with(
        &self,
        genre_name: &String,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)>;

    async fn get_authors_by_last_name(&self, qp: AuthorsByLastName) -> anyhow::Result<Vec<Author>>;

    async fn get_series_by_name(&self, qp: SeriesByName) -> anyhow::Result<Vec<Value>>;

    async fn get_meta_genres(&self) -> anyhow::Result<Vec<String>>;

    async fn get_genres_by_meta(&self, qp: GenresByMeta) -> anyhow::Result<Vec<String>>;

    async fn books_by_serie_id(&self, sid: BooksBySerieId) -> anyhow::Result<Vec<BookSerie>>;

    async fn author_full_name(&self, ids: AuthorIds) -> anyhow::Result<String>;

    async fn series_by_author(&self, ids: AuthorIds) -> anyhow::Result<Vec<Serie>>;

    async fn books_by_author(&self, arg: BooksByAuthor) -> anyhow::Result<Vec<BookDesc>>;
}

pub trait QueryProvider {
    type Item;
    fn query(&self) -> String;
}

pub struct AuthorsNameStartsWith;
impl AuthorsNameStartsWith {
    pub fn new() -> Self {
        Self
    }
}

pub struct SeriesNameStartsWith;
impl SeriesNameStartsWith {
    pub fn new() -> Self {
        Self
    }
}

pub struct AuthorsByLastName {
    name: String,
}
impl AuthorsByLastName {
    pub fn new(name: &String) -> Self {
        Self { name: name.clone() }
    }
}

pub struct SeriesByName {
    name: String,
}
impl SeriesByName {
    pub fn new(name: &String) -> Self {
        Self { name: name.clone() }
    }
}

pub struct GenresByMeta {
    name: String,
}
impl GenresByMeta {
    pub fn new(name: &String) -> Self {
        Self { name: name.clone() }
    }
}

pub struct AuthorsNameByGenreStartsWith {
    pub genre_name: String,
}
impl AuthorsNameByGenreStartsWith {
    pub fn new(genre_name: &String) -> Self {
        Self {
            genre_name: genre_name.clone(),
        }
    }
}

pub struct SeriesNameByGenreStartsWith {
    pub genre_name: String,
}
impl SeriesNameByGenreStartsWith {
    pub fn new(genre_name: &String) -> Self {
        Self {
            genre_name: genre_name.clone(),
        }
    }
}

pub struct BooksBySerieId {
    id: u32,
}
impl BooksBySerieId {
    pub fn new(id: u32) -> Self {
        Self { id: id }
    }
}

pub struct AuthorFullName {
    author: AuthorIds,
}
impl AuthorFullName {
    pub fn new(author: AuthorIds) -> Self {
        Self { author: author }
    }
}

pub struct SeriesByAuthor {
    author: AuthorIds,
}
impl SeriesByAuthor {
    pub fn new(author: AuthorIds) -> Self {
        Self { author: author }
    }
}

pub enum BooksFilter {
    ByTheSerieOnly(u32),
    WithoutSerieOnly,
    // ByGenre(u32), NOT IMPL YET
    All,
}

pub struct BooksByAuthor {
    author: AuthorIds,
    filter: BooksFilter,
}
impl BooksByAuthor {
    pub fn new(author: AuthorIds, filter: BooksFilter) -> Self {
        Self {
            author: author,
            filter: filter,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////

pub async fn get_sequels<T: QueryProvider>(
    pool: &SqlitePool,
    provider: &T,
    pattern: &String,
) -> anyhow::Result<Vec<String>> {
    let len = (pattern.chars().count() + 1) as u32;
    let sql = provider.query();

    let rows = sqlx::query(&sql)
        .bind(len)
        .bind(pattern)
        .fetch_all(pool)
        .await?;

    let mut out = Vec::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        out.push(name);
    }

    Ok(out)
}

pub async fn get_substings<T: QueryProvider>(
    pool: &SqlitePool,
    provider: &T,
    pattern: &String,
) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let mut mask = pattern.clone();
    let mut exact = Vec::new();
    let mut rest = Vec::new();

    loop {
        debug!("mask: '{mask}'");

        let patterns = get_sequels(&pool, provider, &mask).await?;
        debug!("patterns: {:?}", patterns);
        let len = patterns.len();
        let mut tail = patterns
            .into_iter()
            .filter(|name| *name != mask)
            .collect::<Vec<String>>();

        if len != tail.len() {
            exact.push(mask.clone());
        }

        debug!("tail: {:?}", tail);
        debug!("exact: {:?}", exact);

        if tail.is_empty() {
            break;
        } else if 1 == tail.len() {
            std::mem::swap(&mut mask, &mut tail[0]);
        } else {
            for prefix in utils::sorted(tail) {
                rest.push(prefix);
            }
            debug!("rest: {:?}", rest);
            break;
        }
    }

    debug!("-> {:?} {:?}", exact, rest);

    Ok((exact, rest))
}

////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug, PartialEq)]
pub struct Value {
    pub id: u32,
    pub value: String,
}
impl Value {
    pub fn new(id: u32, value: String) -> Self {
        Self { id, value }
    }
}

#[derive(Debug, PartialEq)]
pub struct Author {
    pub first_name: Value,
    pub middle_name: Value,
    pub last_name: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorIds {
    first_name_id: u32,
    middle_name_id: u32,
    last_name_id: u32,
}
impl From<(u32, u32, u32)> for AuthorIds {
    fn from(args: (u32, u32, u32)) -> Self {
        Self {
            first_name_id: args.0,
            middle_name_id: args.1,
            last_name_id: args.2,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Serie {
    pub id: u32,
    pub name: String,
    pub count: u32,
}

#[derive(Debug, PartialEq)]
pub struct BookDesc {
    pub id: u32,
    pub num: u32,
    pub name: String,
    pub size: u32,
    pub date: String,
}

#[derive(Debug, PartialEq)]
pub struct BookSerie {
    pub id: u32,
    pub num: u32,
    pub name: String,
    pub size: u32,
    pub date: String,
    pub author: String,
}

////////////////////////////////////////////////////////////////////////////////////////////////

pub async fn author_serie_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32, u32),
) -> anyhow::Result<Vec<BookDesc>> {
    let (fid, mid, lid, sid) = ids;
    let sql = r#"
        SELECT
            books.book_id AS id,
            series_map.serie_num AS num,
            titles.value AS name,
            books.book_size AS size,
            dates.value AS added
        FROM authors_map
        LEFT JOIN books ON authors_map.book_id = books.book_id
        LEFT JOIN titles ON books.title_id = titles.id
        LEFT JOIN series_map ON  books.book_id = series_map.book_id
        LEFT JOIN series ON series_map.serie_id = series.id
        LEFT JOIN dates ON  books.date_id = dates.id
        WHERE
            first_name_id = $1 AND middle_name_id = $2 AND last_name_id = $3 AND series.id = $4
        ORDER BY 2, 3, 5;
    "#;
    let rows = sqlx::query(&sql)
        .bind(fid)
        .bind(mid)
        .bind(lid)
        .bind(sid)
        .fetch_all(&*pool)
        .await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(BookDesc {
            id: row.try_get("id")?,
            num: row.try_get("num")?,
            name: row.try_get("name")?,
            size: row.try_get("size")?,
            date: row.try_get("added")?,
        });
    }

    Ok(out)
}

pub async fn author_nonserie_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Vec<BookDesc>> {
    let (fid, mid, lid) = ids;
    let sql = r#"
        SELECT
            books.book_id AS id,
            series_map.serie_num AS num,
            titles.value AS name,
            books.book_size AS size,
            dates.value AS added
        FROM authors_map
        LEFT JOIN books ON authors_map.book_id = books.book_id
        LEFT JOIN titles ON books.title_id = titles.id
        LEFT JOIN series_map ON  books.book_id = series_map.book_id
        LEFT JOIN dates ON  books.date_id = dates.id
        WHERE
            first_name_id = $1 AND middle_name_id = $2 AND last_name_id = $3
        AND series_map.serie_num IS NULL
        ORDER BY 2, 3, 5;
    "#;
    let rows = sqlx::query(&sql)
        .bind(fid)
        .bind(mid)
        .bind(lid)
        .fetch_all(&*pool)
        .await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(BookDesc {
            id: row.try_get("id")?,
            num: row.try_get("num")?,
            name: row.try_get("name")?,
            size: row.try_get("size")?,
            date: row.try_get("added")?,
        });
    }

    Ok(out)
}

pub async fn author_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Vec<BookDesc>> {
    let (fid, mid, lid) = ids;
    let sql = r#"
        SELECT
            books.book_id AS id,
            series_map.serie_num AS num,
            titles.value AS name,
            books.book_size AS size,
            dates.value AS added
        FROM authors_map
        LEFT JOIN books ON authors_map.book_id = books.book_id
        LEFT JOIN titles ON books.title_id = titles.id
        LEFT JOIN series_map ON  books.book_id = series_map.book_id
        LEFT JOIN dates ON  books.date_id = dates.id
        WHERE
            first_name_id = $1 AND middle_name_id = $2 AND last_name_id = $3 ;
    "#;
    let rows = sqlx::query(&sql)
        .bind(fid)
        .bind(mid)
        .bind(lid)
        .fetch_all(&*pool)
        .await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(BookDesc {
            id: row.try_get("id")?,
            num: row.try_get("num")?,
            name: row.try_get("name")?,
            size: row.try_get("size")?,
            date: row.try_get("added")?,
        });
    }

    Ok(out)
}

pub async fn root_opds_author_added_books(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Vec<BookDesc>> {
    let (fid, mid, lid) = ids;
    let sql = r#"
        SELECT
            books.book_id AS id,
            series_map.serie_num AS num,
            titles.value AS name,
            books.book_size AS size,
            dates.value AS added
        FROM authors_map
        LEFT JOIN books ON authors_map.book_id = books.book_id
        LEFT JOIN titles ON books.title_id = titles.id
        LEFT JOIN series_map ON  books.book_id = series_map.book_id
        LEFT JOIN dates ON  books.date_id = dates.id
        WHERE
            first_name_id = $1 AND middle_name_id = $2 AND last_name_id = $3
        ORDER BY added DESC;
    "#;
    let rows = sqlx::query(&sql)
        .bind(fid)
        .bind(mid)
        .bind(lid)
        .fetch_all(&*pool)
        .await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(BookDesc {
            id: row.try_get("id")?,
            num: row.try_get("num")?,
            name: row.try_get("name")?,
            size: row.try_get("size")?,
            date: row.try_get("added")?,
        });
    }

    Ok(out)
}

pub async fn genres_by_meta(pool: &SqlitePool, meta: &String) -> anyhow::Result<Vec<String>> {
    let sql = "SELECT DISTINCT genre FROM genres_def WHERE meta = $1 ORDER BY 1;";
    let rows = sqlx::query(&sql).bind(meta).fetch_all(&*pool).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.try_get("genre")?);
    }
    Ok(out)
}

pub async fn init_statistic_db(pool: &SqlitePool) -> anyhow::Result<()> {
    let create_downloads_query = r"
    CREATE TABLE IF NOT EXISTS downloads(
        book_id     INTEGER NOT NULL,
        downloaded  DATETIME DEFAULT CURRENT_TIMESTAMP
    );";

    sqlx::query(create_downloads_query).execute(pool).await?;
    Ok(())
}

pub async fn insert_book(pool: &SqlitePool, id: u32) -> anyhow::Result<()> {
    let insert_downloads_query = r"
    INSERT INTO downloads(book_id)
    VALUES (?);";
    sqlx::query(&insert_downloads_query)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_favorites(pool: &SqlitePool) -> anyhow::Result<Vec<u32>> {
    let sql = "
    SELECT DISTINCT book_id AS id FROM downloads
    WHERE downloaded > DATE('now', '-1 month')
    GROUP BY book_id;";
    let rows = sqlx::query(&sql).fetch_all(&*pool).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.try_get("id")?);
    }
    Ok(out)
}

pub async fn root_opds_favorite_authors(
    pool: &SqlitePool,
    ids: Vec<u32>,
) -> anyhow::Result<Vec<Author>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let params = format!("?{}", ", ?".repeat(ids.len() - 1));
    let sql = format!(
        "
        SELECT DISTINCT
            first_names.id AS first_id,
            middle_names.id AS middle_id,
            last_names.id AS last_id,
            first_names.value AS first_name,
            middle_names.value AS middle_name,
            last_names.value AS last_name
        FROM authors_map
        LEFT JOIN first_names ON first_names.id = authors_map.first_name_id
        LEFT JOIN middle_names ON middle_names.id = authors_map.middle_name_id
        LEFT JOIN last_names ON last_names.id = authors_map.last_name_id
        WHERE book_id IN ({params})
        ORDER BY 6,4,5
    "
    );
    let mut query = sqlx::query(&sql);
    for id in &ids {
        query = query.bind(id);
    }

    let rows = query.fetch_all(&*pool).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(Author {
            first_name: Value::new(row.try_get("first_id")?, row.try_get("first_name")?),
            middle_name: Value::new(row.try_get("middle_id")?, row.try_get("middle_name")?),
            last_name: Value::new(row.try_get("last_id")?, row.try_get("last_name")?),
        });
    }

    Ok(out)
}

/////////////////////////// Generic Functions
