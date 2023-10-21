use crate::database::get_substings;
use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use super::AuthorFullName;
use super::AuthorsByLastName;
use super::AuthorsNameByGenreStartsWith;
use super::AuthorsNameStartsWith;
use super::BooksByAuthor;
use super::BooksBySerieId;
use super::BooksFilter;
use super::GenresByMeta;
use super::SeriesByAuthor;
use super::SeriesByName;
use super::SeriesNameByGenreStartsWith;
use super::SeriesNameStartsWith;

use crate::database::LibraryProvider;
use crate::database::QueryProvider;

use crate::database::Author;
use crate::database::AuthorIds;
use crate::database::BookDesc;
use crate::database::BookSerie;
use crate::database::Serie;
use crate::database::Value;

pub struct LibRusEcOffline {
    pool: SqlitePool,
}
impl LibRusEcOffline {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool: pool }
    }
}
#[async_trait]
impl LibraryProvider for LibRusEcOffline {
    async fn get_authors_names_starts_with(
        &self,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)> {
        let provider = AuthorsNameStartsWith::new();
        get_substings(&self.pool, &provider, mask).await
    }

    async fn get_authors_names_by_genre_starts_with(
        &self,
        genre_name: &String,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)> {
        let provider = AuthorsNameByGenreStartsWith::new(genre_name);
        get_substings(&self.pool, &provider, mask).await
    }

    async fn get_series_names_starts_with(
        &self,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)> {
        let provider = SeriesNameStartsWith::new();
        get_substings(&self.pool, &provider, mask).await
    }

    async fn get_series_names_by_genre_starts_with(
        &self,
        genre_name: &String,
        mask: &String,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)> {
        let provider = SeriesNameByGenreStartsWith::new(genre_name);
        get_substings(&self.pool, &provider, mask).await
    }

    async fn get_authors_by_last_name(&self, qp: AuthorsByLastName) -> anyhow::Result<Vec<Author>> {
        let sql = qp.query();
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        let mut authors = Vec::new();
        for row in rows {
            authors.push(Author {
                first_name: Value::new(row.try_get("first_id")?, row.try_get("first_name")?),
                middle_name: Value::new(row.try_get("middle_id")?, row.try_get("middle_name")?),
                last_name: Value::new(row.try_get("last_id")?, row.try_get("last_name")?),
            });
        }
        Ok(authors)
    }

    async fn get_series_by_name(&self, qp: SeriesByName) -> anyhow::Result<Vec<Value>> {
        let sql = qp.query();
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        let mut series = Vec::new();
        for row in rows {
            series.push(Value::new(row.try_get("id")?, row.try_get("value")?));
        }
        Ok(series)
    }

    async fn get_meta_genres(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SELECT DISTINCT meta FROM genres_def;";
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        let mut meta = Vec::new();
        for row in rows {
            meta.push(row.try_get("meta")?);
        }
        Ok(meta)
    }

    async fn get_genres_by_meta(&self, qp: GenresByMeta) -> anyhow::Result<Vec<String>> {
        let sql = qp.query();
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        let mut genres = Vec::new();
        for row in rows {
            genres.push(row.try_get("genre")?);
        }
        Ok(genres)
    }

    async fn books_by_serie_id(&self, sid: BooksBySerieId) -> anyhow::Result<Vec<BookSerie>> {
        let sql = sid.query();
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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

    async fn author_full_name(&self, ids: AuthorIds) -> anyhow::Result<String> {
        let sql = AuthorFullName::new(ids).query();
        let row = sqlx::query(&sql).fetch_one(&self.pool).await?;
        Ok(row.try_get("author")?)
    }

    async fn series_by_author(&self, ids: AuthorIds) -> anyhow::Result<Vec<Serie>> {
        let sql = SeriesByAuthor::new(ids).query();
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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

    async fn books_by_author(&self, arg: BooksByAuthor) -> anyhow::Result<Vec<BookDesc>> {
        let sql = arg.query();
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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
}

impl QueryProvider for AuthorsNameStartsWith {
    type Item = LibRusEcOffline;

    fn query(&self) -> String {
        format!("SELECT DISTINCT substr(value, 1, $1) AS name FROM last_names WHERE value LIKE $2 || '%'")
    }
}

impl QueryProvider for SeriesNameStartsWith {
    type Item = LibRusEcOffline;

    fn query(&self) -> String {
        format!(
            "SELECT DISTINCT substr(value, 1, $1) AS name FROM series WHERE value LIKE $2 || '%'"
        )
    }
}

impl QueryProvider for GenresByMeta {
    type Item = LibRusEcOffline;

    fn query(&self) -> String {
        format!(
            "SELECT DISTINCT genre FROM genres_def WHERE meta = '{}' ORDER BY 1;",
            &self.name
        )
    }
}

impl QueryProvider for AuthorsByLastName {
    type Item = LibRusEcOffline;

    fn query(&self) -> String {
        format!(
            r###"
            SELECT DISTINCT
    	        first_names.id AS first_id,
                middle_names.id AS middle_id,
                last_names.id AS last_id,
                first_names.value AS first_name,
                middle_names.value AS middle_name,
                last_names.value AS last_name
            FROM authors_map, first_names, middle_names, last_names
            WHERE
                last_names.id = (SELECT id FROM last_names WHERE value = '{}')
	        AND first_names.id = first_name_id
	        AND middle_names.id = middle_name_id
	        AND last_names.id = last_name_id
        "###,
            &self.name
        )
    }
}

impl QueryProvider for SeriesByName {
    type Item = LibRusEcOffline;

    fn query(&self) -> String {
        format!(
            "SELECT DISTINCT id, value FROM series WHERE value = '{}'",
            &self.name
        )
    }
}

impl QueryProvider for AuthorsNameByGenreStartsWith {
    type Item = LibRusEcOffline;

    fn query(&self) -> String {
        format!(
            r#"
            SELECT DISTINCT substr(last_names.value, 1, $1) AS name
            FROM genres_def
            JOIN genres ON genres.value = genres_def.code
            JOIN genres_map ON genres_map.genre_id = genres.id
            JOIN authors_map ON authors_map.book_id = genres_map.book_id
            JOIN last_names ON last_names.id = authors_map.last_name_id
            WHERE genres_def.genre = '{}' AND last_names.value LIKE $2 || '%'
            "#,
            self.genre_name
        )
    }
}

impl QueryProvider for SeriesNameByGenreStartsWith {
    type Item = LibRusEcOffline;
    fn query(&self) -> String {
        format!(
            r#"
            SELECT DISTINCT substr(series.value, 1, $1) AS name
            FROM genres_def
            JOIN genres ON genres.value = genres_def.code
            JOIN genres_map ON genres_map.genre_id = genres.id
            JOIN series_map ON genres_map.book_id = series_map.book_id
            JOIN series ON series.id = series_map.serie_id
            WHERE genres_def.genre = '{}' AND series.value LIKE $2 || '%'
            "#,
            self.genre_name
        )
    }
}

impl QueryProvider for BooksBySerieId {
    type Item = LibRusEcOffline;
    fn query(&self) -> String {
        format!(
            r#"
            SELECT
                books.book_id AS id,
                series_map.serie_num AS num,
                titles.value AS name,
                books.book_size AS size,
                dates.value AS added,
                first_names.value || ' ' || middle_names.value || ' ' || last_names.value AS author
            FROM series
            JOIN series_map ON series.id = series_map.serie_id
            JOIN books ON books.book_id = series_map.book_id
            JOIN titles ON titles.id = books.title_id
            JOIN dates ON books.date_id = dates.id
            JOIN authors_map ON authors_map.book_id = books.book_id
            JOIN first_names ON first_names.id = first_name_id
            JOIN middle_names ON middle_names.id = middle_name_id
            JOIN last_names ON last_names.id = last_name_id
            WHERE series.id = {}
            "#,
            &self.id
        )
    }
}

impl QueryProvider for AuthorFullName {
    type Item = LibRusEcOffline;
    fn query(&self) -> String {
        format!(
            r#"
            SELECT first_names.value || ' ' || middle_names.value || ' ' || last_names.value AS author
            FROM first_names, middle_names, last_names
            WHERE first_names.id = {} AND middle_names.id = {} AND last_names.id = {}
            LIMIT 1;
            "#,
            self.author.first_name_id, self.author.middle_name_id, self.author.last_name_id
        )
    }
}

impl QueryProvider for SeriesByAuthor {
    type Item = LibRusEcOffline;
    fn query(&self) -> String {
        format!(
            r#"
            SELECT
                series.id AS id,
                series.value AS name,
                count(series.value) as count
            FROM authors_map
            JOIN books ON authors_map.book_id = books.book_id
            JOIN titles ON books.title_id = titles.id
            JOIN series_map ON  books.book_id = series_map.book_id
            JOIN series ON series_map.serie_id = series.id
            JOIN dates ON  books.date_id = dates.id
            WHERE first_name_id = {} AND middle_name_id = {} AND last_name_id = {}
                  AND name IS NOT NULL
            GROUP by 1, 2;
            "#,
            self.author.first_name_id, self.author.middle_name_id, self.author.last_name_id
        )
    }
}

impl QueryProvider for BooksByAuthor {
    type Item = LibRusEcOffline;
    fn query(&self) -> String {
        let fid = self.author.first_name_id;
        let mid = self.author.middle_name_id;
        let lid = self.author.last_name_id;

        let where_clause_content = match self.filter {
            BooksFilter::ByTheSerieOnly(sid) =>
                format!("first_name_id = {fid} AND middle_name_id = {mid} AND last_name_id = {lid} AND series.id = {sid}"),
            BooksFilter::WithoutSerieOnly =>
                format!("first_name_id = {fid} AND middle_name_id = {mid} AND last_name_id = {lid} AND series_map.serie_num IS NULL"),
            BooksFilter::All =>
                format!("first_name_id = {fid} AND middle_name_id = {mid} AND last_name_id = {lid}"),
        };

        format!(
            r#"
            SELECT
                books.book_id AS id,
                series_map.serie_num AS num,
                titles.value AS name,
                books.book_size AS size,
                dates.value AS added
            FROM authors_map
            JOIN books ON authors_map.book_id = books.book_id
            JOIN titles ON books.title_id = titles.id
            LEFT JOIN series_map ON  books.book_id = series_map.book_id
            LEFT JOIN series ON series_map.serie_id = series.id
            JOIN dates ON  books.date_id = dates.id
            WHERE {where_clause_content}
            "#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils;
    use sqlx::sqlite::SqlitePool;
    use sqlx::Pool;
    use sqlx::Sqlite;

    lazy_static! {
        static ref TEST_DATABASE: &'static str = "sqlite:./test/fb2-749200-750279.db?mode=ro";
        static ref POOL: Result<Pool<Sqlite>, sqlx::Error> =
            SqlitePool::connect_lazy(&TEST_DATABASE);
        static ref POOL2: Pool<Sqlite> =
            SqlitePool::connect_lazy(&TEST_DATABASE).expect("Connection failed");
        static ref LIBRARY: LibRusEcOffline = LibRusEcOffline::new(POOL2.clone());
    }

    #[actix_rt::test]
    async fn get_authors_names_starts_with() {
        {
            let result = LIBRARY
                .get_authors_names_starts_with(&String::from("А"))
                .await;
            assert!(result.is_ok());

            let (exact, rest) = result.expect("The result is not Ok");
            assert!(exact.is_empty());
            assert_eq!(
                utils::sorted(rest),
                utils::sorted(vec![
                    "Ал", "Ан", "Аф", "Аз", "Ар", "Аб", "Ай", "Ап", "Ад", "Ав", "Ах", "Ам",
                ])
            );
        }
        {
            let result = LIBRARY
                .get_authors_names_starts_with(&String::from("Бау"))
                .await;
            assert!(result.is_ok());

            let (exact, rest) = result.expect("The result is not Ok");
            assert_eq!(utils::sorted(exact), utils::sorted(vec!["Бау"]));
            assert_eq!(utils::sorted(rest), utils::sorted(vec!["Баун", "Бауэ"]));
        }
    }

    #[actix_rt::test]
    async fn get_series_names_starts_with() {
        {
            let result = LIBRARY
                .get_series_names_starts_with(&String::from("А"))
                .await;
            assert!(result.is_ok());
            let (exact, rest) = result.expect("The result is not Ok");

            assert!(exact.is_empty());
            assert_eq!(
                utils::sorted(rest),
                utils::sorted(vec!["Ан", "Ас", "Ак", "А ", "Ав", "Аг"])
            );
        }
        {
            let result = LIBRARY
                .get_series_names_starts_with(&String::from("Бау"))
                .await;
            assert!(result.is_ok());

            let (exact, rest) = result.expect("The result is not Ok");
            assert_eq!(
                utils::sorted(exact),
                utils::sorted(vec!["Баукелэ. Александра"])
            );
            assert!(rest.is_empty());
        }
    }

    #[actix_rt::test]
    async fn get_authors_by_last_name() {
        let result = LIBRARY
            .get_authors_by_last_name(AuthorsByLastName::new(&String::from("Иванов")))
            .await;
        assert!(result.is_ok());
        let authors = result.expect("The result is not Ok");

        let ivanov_peter = Author {
            first_name: Value::new(99, "Петр".to_string()),
            middle_name: Value::new(54, "Константинович".to_string()),
            last_name: Value::new(18, "Иванов".to_string()),
        };
        assert_eq!(
            authors.iter().find(|a| **a == ivanov_peter),
            Some(&ivanov_peter)
        );

        let ivanov_sergey = Author {
            first_name: Value::new(10, "Сергей".to_string()),
            middle_name: Value::new(4, "Григорьевич".to_string()),
            last_name: Value::new(18, "Иванов".to_string()),
        };
        assert_eq!(
            authors.iter().find(|a| **a == ivanov_sergey),
            Some(&ivanov_sergey)
        );
    }

    #[actix_rt::test]
    async fn get_series_by_name() {
        let result = LIBRARY
            .get_series_by_name(SeriesByName::new(&String::from("Куница")))
            .await;
        assert!(result.is_ok());
        let series = result.expect("The result is not Ok");
        assert_eq!(series, vec![Value::new(109, "Куница".to_string())]);
    }

    #[actix_rt::test]
    async fn get_meta_genres() {
        let result = LIBRARY.get_meta_genres().await;
        assert!(result.is_ok());
        let meta_genres = result.expect("The result is not Ok");
        assert!(meta_genres.iter().any(|genre| genre == "Фантастика"));
        assert!(meta_genres.iter().any(|genre| genre == "Дом и семья"));
        assert!(meta_genres.iter().any(|genre| genre == "Эзотерика"));
    }

    #[actix_rt::test]
    async fn get_genres_by_meta() {
        let result = LIBRARY
            .get_genres_by_meta(GenresByMeta::new(&String::from("Фантастика")))
            .await;
        assert!(result.is_ok());
        let genres = result.expect("The result is not Ok");
        assert!(genres.iter().any(|genre| genre == "Альтернативная история"));
        assert!(genres.iter().any(|genre| genre == "Детективная фантастика"));
        assert!(genres.iter().any(|genre| genre == "Киберпанк"));
    }

    #[actix_rt::test]
    async fn get_series_names_by_genre_starts_with() {
        let genre = String::from("Альтернативная история");
        {
            let mask = String::from("Ма");
            let result = LIBRARY
                .get_series_names_by_genre_starts_with(&genre, &mask)
                .await;
            assert!(result.is_ok());

            let (exact, rest) = result.expect("The result is not Ok");
            assert!(exact.is_empty());
            assert_eq!(utils::sorted(rest), utils::sorted(vec!["Маз", "Мас"]));
        }
        {
            let mask = String::from("Маз");
            let result = LIBRARY
                .get_series_names_by_genre_starts_with(&genre, &mask)
                .await;
            assert!(result.is_ok());

            let (exact, rest) = result.expect("The result is not Ok");
            assert_eq!(
                utils::sorted(exact),
                utils::sorted(vec!["Мазурка Домбровского"])
            );
            assert!(rest.is_empty());
        }
    }

    #[actix_rt::test]
    async fn get_authors_names_by_genre_starts_with() {
        let genre = String::from("Альтернативная история");
        {
            let mask = String::from("Ба");
            let result = LIBRARY
                .get_authors_names_by_genre_starts_with(&genre, &mask)
                .await;
            assert!(result.is_ok());
            let (exact, rest) = result.expect("The result is not Ok");

            assert!(exact.is_empty());
            assert_eq!(utils::sorted(rest), utils::sorted(vec!["Бар", "Бат"]));
        }
        {
            let mask = String::from("Бар");
            let result = LIBRARY
                .get_authors_names_by_genre_starts_with(&genre, &mask)
                .await;
            assert!(result.is_ok());

            let (exact, rest) = result.expect("The result is not Ok");
            assert_eq!(utils::sorted(exact), utils::sorted(vec!["Барчук"]));
            assert!(rest.is_empty());
        }
    }

    #[actix_rt::test]
    async fn books_by_serie_id() {
        let result = LIBRARY.books_by_serie_id(BooksBySerieId::new(131)).await;
        assert!(result.is_ok());

        let books = result.expect("The result is not Ok");
        assert_eq!(
            books,
            vec![BookSerie {
                id: 749883,
                num: 6,
                name: String::from("Город драконов. Книга шестая"),
                size: 813219,
                date: String::from("2023-02-24"),
                author: String::from("Елена  Звездная"),
            }]
        );
    }

    #[actix_rt::test]
    async fn author_full_name() {
        let result = LIBRARY.author_full_name(AuthorIds::from((29, 1, 38))).await;
        assert!(result.is_ok());

        let author = result.expect("The result is not Ok");
        assert_eq!(author, String::from("Илья  Соломенный"));
    }

    #[actix_rt::test]
    async fn series_by_author() {
        let result = LIBRARY.series_by_author(AuthorIds::from((29, 1, 38))).await;
        assert!(result.is_ok());

        let series = result.expect("The result is not Ok");
        assert_eq!(
            series,
            vec![Serie {
                id: 6,
                name: String::from("Не время для героев"),
                count: 2,
            }]
        );
    }

    #[actix_rt::test]
    async fn books_by_author() {
        let author = AuthorIds::from((4, 3, 541));
        {
            let arg = BooksByAuthor::new(author.clone(), BooksFilter::ByTheSerieOnly(74));
            let result = LIBRARY.books_by_author(arg).await;
            assert!(result.is_ok());

            let books = result.expect("The result is not Ok");

            assert_eq!(
                books,
                vec![BookDesc {
                    id: 749846,
                    num: 0,
                    name: String::from("Византийский букварь. Введение в историю Византии"),
                    size: 4520617,
                    date: String::from("2023-02-23"),
                }]
            );
        }
        {
            let arg = BooksByAuthor::new(author.clone(), BooksFilter::WithoutSerieOnly);
            let result = LIBRARY.books_by_author(arg).await;
            assert!(result.is_ok());

            let books = result.expect("The result is not Ok");

            assert_eq!(
                books,
                vec![BookDesc {
                    id: 749845,
                    num: 0,
                    name: String::from("Святая Земля и Русское Зарубежье"),
                    size: 1697167,
                    date: String::from("2023-02-23"),
                }]
            );
        }

        {
            let arg = BooksByAuthor::new(author.clone(), BooksFilter::All);
            let result = LIBRARY.books_by_author(arg).await;
            assert!(result.is_ok());

            let mut expected = vec![
                BookDesc {
                    id: 749846,
                    num: 0,
                    name: String::from("Византийский букварь. Введение в историю Византии"),
                    size: 4520617,
                    date: String::from("2023-02-23"),
                },
                BookDesc {
                    id: 749845,
                    num: 0,
                    name: String::from("Святая Земля и Русское Зарубежье"),
                    size: 1697167,
                    date: String::from("2023-02-23"),
                },
            ];
            expected.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

            let mut books = result.expect("The result is not Ok");
            books.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

            assert_eq!(books, expected);
        }
    }
}
