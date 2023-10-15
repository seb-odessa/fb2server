use sqlx::sqlite::SqlitePool;
use sqlx::Row;

pub struct Value {
    pub id: u32,
    pub value: String,
}
impl Value {
    pub fn new(id: u32, value: String) -> Self {
        Self { id, value }
    }
}

pub struct Author {
    pub first_name: Value,
    pub middle_name: Value,
    pub last_name: Value,
}

pub struct Serie {
    pub id: u32,
    pub name: String,
    pub count: u32,
}

pub struct BookDesc {
    pub id: u32,
    pub num: u32,
    pub name: String,
    pub size: u32,
    pub date: String,
}

pub struct BookSerie {
    pub id: u32,
    pub num: u32,
    pub name: String,
    pub size: u32,
    pub date: String,
    pub author: String,
}

#[derive(Clone, Copy, Debug)]
pub enum QueryType {
    Author,
    Serie,
}

pub async fn make_patterns(
    catalog: &SqlitePool,
    query: QueryType,
    pattern: &String,
) -> anyhow::Result<Vec<String>> {
    let len = (pattern.chars().count() + 1) as u32;
    let sql = match query {
        QueryType::Author => {
            r#"
            SELECT DISTINCT substr(value, 1, $1) AS name
            FROM last_names WHERE value LIKE $2 || '%'
            "#
        }
        QueryType::Serie => {
            r#"
            SELECT DISTINCT substr(value, 1, $1) AS name
            FROM series WHERE value LIKE $2 || '%'
            "#
        }
    };

    let rows = sqlx::query(&sql)
        .bind(len)
        .bind(format!("{pattern}"))
        .fetch_all(&*catalog)
        .await?;

    let mut out = Vec::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        out.push(name);
    }

    Ok(out)
}

pub async fn find_authors(catalog: &SqlitePool, name: &String) -> anyhow::Result<Vec<Author>> {
    let sql = r#"
        SELECT DISTINCT
	        first_names.id AS first_id,
            middle_names.id AS middle_id,
            last_names.id AS last_id,
            first_names.value AS first_name,
            middle_names.value AS middle_name,
            last_names.value AS last_name
        FROM authors_map, first_names, middle_names, last_names
        WHERE
            last_names.id = (SELECT id FROM last_names WHERE value = $1)
	    AND first_names.id = first_name_id
	    AND middle_names.id = middle_name_id
	    AND last_names.id = last_name_id;
    "#;

    let rows = sqlx::query(&sql).bind(name).fetch_all(&*catalog).await?;
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

pub async fn find_series(catalog: &SqlitePool, name: &String) -> anyhow::Result<Vec<Value>> {
    let sql = "SELECT DISTINCT id, value FROM series WHERE value = $1";
    let rows = sqlx::query(&sql).bind(name).fetch_all(&*catalog).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(Value::new(row.try_get("id")?, row.try_get("value")?));
    }
    Ok(out)
}

pub async fn author(catalog: &SqlitePool, ids: (u32, u32, u32)) -> anyhow::Result<String> {
    let (fid, mid, lid) = ids;
    let sql = r#"
        SELECT
	        first_names.value || ' ' ||
	        middle_names.value || ' ' ||
	        last_names.value AS author
        FROM first_names, middle_names, last_names
        WHERE first_names.id = $1 AND middle_names.id = $2 AND last_names.id = $3;
    "#;

    let row = sqlx::query(&sql)
        .bind(fid)
        .bind(mid)
        .bind(lid)
        .fetch_one(&*catalog)
        .await?;
    Ok(row.try_get("author")?)
}

pub async fn author_series(
    catalog: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<Vec<Serie>> {
    let (fid, mid, lid) = ids;
    let sql = r#"
        SELECT
            series.id AS id,
            series.value AS name,
            count(series.value) as count
        FROM authors_map
        LEFT JOIN books ON authors_map.book_id = books.book_id
        LEFT JOIN titles ON books.title_id = titles.id
        LEFT JOIN series_map ON  books.book_id = series_map.book_id
        LEFT JOIN series ON series_map.serie_id = series.id
        LEFT JOIN dates ON  books.date_id = dates.id
        WHERE
        first_name_id = $1 AND middle_name_id = $2 AND last_name_id = $3
        AND name IS NOT NULL
        GROUP by 1, 2;
        "#;

    let rows = sqlx::query(&sql)
        .bind(fid)
        .bind(mid)
        .bind(lid)
        .fetch_all(&*catalog)
        .await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(Serie {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            count: row.try_get("count")?,
        });
    }

    Ok(out)
}

pub async fn author_serie_books(
    catalog: &SqlitePool,
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
        .fetch_all(&*catalog)
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
    catalog: &SqlitePool,
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
        .fetch_all(&*catalog)
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
    catalog: &SqlitePool,
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
        .fetch_all(&*catalog)
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
    catalog: &SqlitePool,
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
        .fetch_all(&*catalog)
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

pub async fn root_opds_serie_books(
    catalog: &SqlitePool,
    id: u32,
) -> anyhow::Result<Vec<BookSerie>> {
    let sql = r#"
    SELECT
        books.book_id AS id,
        series_map.serie_num AS num,
        titles.value AS name,
        books.book_size AS size,
        dates.value AS added,
        first_names.value || ' ' || middle_names.value || ' ' || last_names.value AS author
    FROM authors_map
    LEFT JOIN books ON authors_map.book_id = books.book_id
    LEFT JOIN titles ON books.title_id = titles.id
    LEFT JOIN series_map ON  books.book_id = series_map.book_id
    LEFT JOIN series ON series_map.serie_id = series.id
    LEFT JOIN dates ON  books.date_id = dates.id
    LEFT JOIN first_names ON first_names.id = first_name_id
    LEFT JOIN middle_names ON middle_names.id = middle_name_id
    LEFT JOIN last_names ON last_names.id = last_name_id
    WHERE series.id = $1
    ORDER BY 2, 3, 5, 6;
    "#;
    let rows = sqlx::query(&sql).bind(id).fetch_all(&*catalog).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(BookSerie {
            id: row.try_get("id")?,
            num: row.try_get("num")?,
            name: row.try_get("name")?,
            size: row.try_get("size")?,
            date: row.try_get("added")?,
            author: row.try_get("author")?,
        });
    }

    Ok(out)
}

pub async fn genres_meta(catalog: &SqlitePool) -> anyhow::Result<Vec<String>> {
    let sql = "SELECT DISTINCT meta FROM genres_def ORDER BY 1;";
    let rows = sqlx::query(&sql).fetch_all(&*catalog).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.try_get("meta")?);
    }
    Ok(out)
}

pub async fn genres_by_meta(catalog: &SqlitePool, meta: &String) -> anyhow::Result<Vec<String>> {
    let sql = "SELECT DISTINCT genre FROM genres_def WHERE meta = $1 ORDER BY 1;";
    let rows = sqlx::query(&sql).bind(meta).fetch_all(&*catalog).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.try_get("genre")?);
    }
    Ok(out)
}

pub async fn root_opds_genres_series(
    catalog: &SqlitePool,
    genre: &String,
) -> anyhow::Result<Vec<Serie>> {
    let sql = r#"
        WITH genre_codes(id, value) AS (
            SELECT id, code FROM genres_def LEFT JOIN genres
            WHERE genre = $1 AND value = code
        )
        SELECT series.id AS id, series.value AS name, count(series.value) AS count
        FROM genre_codes
        LEFT JOIN genres_map ON genres_map.genre_id = genre_codes.id
        LEFT JOIN series_map ON series_map.book_id = genres_map.book_id
        LEFT JOIN series ON series.id = series_map.serie_id
        WHERE series.value IS NOT NULL
        GROUP BY 1, 2
    "#;

    let rows = sqlx::query(&sql).bind(genre).fetch_all(&*catalog).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(Serie {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            count: row.try_get("count")?,
        });
    }

    Ok(out)
}

pub async fn root_opds_genres_authors(
    catalog: &SqlitePool,
    genre: &String,
) -> anyhow::Result<Vec<Author>> {
    let sql = r#"
        WITH genre_codes(id, value) AS (
            SELECT id, code FROM genres_def LEFT JOIN genres
        WHERE genre = $1 AND value = code
        )
        SELECT DISTINCT
            first_names.id AS first_id,
            middle_names.id AS middle_id,
            last_names.id AS last_id,
            first_names.value AS first_name,
            middle_names.value AS middle_name,
            last_names.value AS last_name
        FROM genre_codes
        LEFT JOIN genres_map ON genres_map.genre_id = genre_codes.id
        LEFT JOIN authors_map ON authors_map.book_id = genres_map.book_id
        LEFT JOIN first_names ON first_names.id = authors_map.first_name_id
        LEFT JOIN middle_names ON middle_names.id = authors_map.middle_name_id
        LEFT JOIN last_names ON last_names.id = authors_map.last_name_id
        ORDER BY 6,4,5
    "#;

    let rows = sqlx::query(&sql).bind(genre).fetch_all(&*catalog).await?;
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

pub async fn init_statistic_db(catalog: &SqlitePool) -> anyhow::Result<()> {
    let create_downloads_query = r"
    CREATE TABLE IF NOT EXISTS downloads(
        book_id     INTEGER NOT NULL,
        downloaded  DATETIME DEFAULT CURRENT_TIMESTAMP
    );";

    sqlx::query(create_downloads_query).execute(catalog).await?;
    Ok(())
}

pub async fn insert_book(catalog: &SqlitePool, id: u32) -> anyhow::Result<()> {
    let insert_downloads_query = r"
    INSERT INTO downloads(book_id)
    VALUES (?);";
    sqlx::query(&insert_downloads_query)
        .bind(id)
        .execute(catalog)
        .await?;
    Ok(())
}

pub async fn get_favorites(catalog: &SqlitePool) -> anyhow::Result<Vec<u32>> {
    let sql = "
    SELECT DISTINCT book_id AS id FROM downloads
    WHERE downloaded > DATE('now', '-1 month')
    GROUP BY book_id;";
    let rows = sqlx::query(&sql).fetch_all(&*catalog).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.try_get("id")?);
    }
    Ok(out)
}

pub async fn root_opds_favorite_authors(
    catalog: &SqlitePool,
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

    let rows = query.fetch_all(&*catalog).await?;
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
