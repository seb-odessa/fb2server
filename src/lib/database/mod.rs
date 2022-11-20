use sqlx::sqlite::SqlitePool;
use sqlx::Row;

pub struct Author {
    pub first_id: u32,
    pub middle_id: u32,
    pub last_id: u32,
    pub first_name: String,
    pub middle_name: String,
    pub last_name: String,
}

pub async fn make_patterns(pool: &SqlitePool, pattern: &String) -> anyhow::Result<Vec<String>> {
    let len = pattern.chars().count() + 1;
    let sql = format!(
        r#"
            SELECT DISTINCT substr(name, 1, {len}) AS name
            FROM last_names
            WHERE name LIKE "{pattern}%"
            ORDER BY 1
        "#
    );

    let rows = sqlx::query(&sql).fetch_all(&*pool).await?;
    let mut out = Vec::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        out.push(format!("{}", name));
    }

    Ok(out)
}

pub async fn find_authors(pool: &SqlitePool, name: &String) -> anyhow::Result<Vec<Author>> {
    let sql = format!(
        r#"
            SELECT DISTINCT
                first_name_id AS first_id,
                middle_name_id AS middle_id,
                last_name_id AS last_id,
                first_names.name AS first_name,
                middle_names.name AS middle_name,
                last_names.name AS last_name
            FROM authors_map
            LEFT JOIN first_names ON first_names.id = first_name_id
            LEFT JOIN middle_names ON middle_names.id = middle_name_id
            LEFT JOIN last_names ON last_names.id = last_name_id
            WHERE last_names.name = "{name}"
            ORDER BY 4, 5, 6;
        "#
    );

    let rows = sqlx::query(&sql).fetch_all(&*pool).await?;
    let mut out = Vec::new();
    for row in rows {
        out.push(Author {
            first_id: row.try_get("first_id")?,
            middle_id: row.try_get("middle_id")?,
            last_id: row.try_get("last_id")?,
            first_name: row.try_get("first_name")?,
            middle_name: row.try_get("middle_name")?,
            last_name: row.try_get("last_name")?,
        });
    }

    Ok(out)
}

/*
--     EXPLAIN QUERY PLAN
	SELECT DISTINCT
	  authors_map.book_id,
	  titles.title,
	  dates.date,
	  series.name,
	  series_map.serie_num

    FROM authors_map
	LEFT JOIN books ON authors_map.book_id = books.id
	LEFT JOIN titles ON books.title_id = titles.id
    LEFT JOIN series_map ON  books.id = series_map.book_id
	LEFT JOIN series ON series_map.serie_id = series.id
	LEFT JOIN dates ON  books.date_id = dates.id

    WHERE
	  first_name_id = 105
      AND middle_name_id = 22
      AND last_name_id = 23918
    ORDER BY 4,5,2
      ;


*/

