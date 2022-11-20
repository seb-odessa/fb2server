use actix_web::{get, web, App, HttpServer, Responder};
use chrono;
use log::{info, warn};
use quick_xml::events::{BytesText, Event};
use quick_xml::writer::Writer;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use std::cmp::*;
use std::convert::Into;
use std::io::Cursor;
use std::sync::Mutex;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE: &'static str = "books.db";
const XML_HEAD: &'static str = r#"xml version="1.0" encoding="utf-8""#;

// #[derive(Clone)]
struct AppState {
    // counter: Mutex<i32>,
    pool: Mutex<SqlitePool>,
}
impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            // counter: Mutex::new(0),
            pool: Mutex::new(pool),
        }
    }
}

struct Entry {
    id: String,
    title: String,
    link: String,
}
impl Entry {
    pub fn new<T: Into<String>>(title: T, link: T) -> Self {
        let link = link.into();
        let id = String::from("root") + &link.clone().as_str().replace("/", ":");
        Self {
            id: id.into(),
            title: title.into(),
            link: link.into(),
        }
    }
}

struct Feed {
    pub title: String,
    pub entries: Vec<Entry>,
}
impl Feed {
    pub fn new<T: Into<String>>(title: T) -> Self {
        Self {
            title: title.into(),
            entries: Vec::new(),
        }
    }

    pub fn add<T: Into<String>>(&mut self, title: T, link: T) {
        let entry = Entry::new(title, link);
        self.entries.push(entry);
    }
}

fn format_feed(feed: Feed) -> String {
    match make_feed(feed) {
        Ok(xml) => xml,
        Err(err) => format!("{err}"),
    }
}

#[get("/opds")]
async fn opds() -> impl Responder {
    info!("/opds");
    let mut feed = Feed::new("Catalog Root");
    feed.add("Поиск книг по авторам", "/opds/authors");
    feed.add("Поиск книг по сериям", "/opds/series");
    feed.add("Поиск книг по жанрам", "/opds/genres");
    format_feed(feed)
}

fn sorted(mut patterns: Vec<String>) -> Vec<String> {
    patterns.sort_by(|a, b| {
        let length = a.chars().count().cmp(&b.chars().count());
        if length == Ordering::Equal {
            let ac = a.chars().collect::<Vec<char>>();
            let bc = b.chars().collect::<Vec<char>>();
            for i in 0..ac.len() {
                if ac[i].is_ascii() && bc[i].is_ascii() {
                    let r = ac[i].cmp(&bc[i]);
                    if r != Ordering::Equal {
                        return r;
                    }
                } else if ac[i].is_ascii() && !bc[i].is_ascii() {
                    return Ordering::Greater;
                } else if !ac[i].is_ascii() && bc[i].is_ascii() {
                    return Ordering::Less;
                } else {
                    let r = ac[i].cmp(&bc[i]);
                    if r != Ordering::Equal {
                        return r;
                    }
                }
            }
            return a.cmp(&b);
        }
        return length;
    });
    return patterns;
}

#[get("/opds/authors")]
async fn authors_root(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/authors");
    match make_patterns(ctx, String::from("")).await {
        Ok(patterns) => {
            let mut feed = Feed::new("Поиск книг по авторам");
            for pattern in sorted(patterns) {
                if pattern.is_empty() {
                    continue;
                }
                feed.add(
                    format!("Авторы на '{pattern}'"),
                    format!("/opds/authors/{pattern}"),
                );
            }
            format_feed(feed)
        }
        Err(err) => format!("{err}"),
    }
}

#[get("/opds/authors/{pattern}")]
async fn authors_by_mask(ctx: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let pattern = path.into_inner();
    info!("/opds/authors/{pattern}");

    match make_patterns(ctx.clone(), pattern.clone()).await {
        Ok(patterns) => {
            let mut feed = Feed::new("Поиск книг по авторам");
            if let Ok(id) = get_last_name_id(ctx, &pattern).await {
                feed.add(
                    format!("Авторы '{pattern}' - {id}"),
                    format!("/opds/author/{id}"),
                );
            }

            for prefix in sorted(patterns) {
                if pattern.ne(&prefix) {
                    feed.add(
                        format!("Авторы на '{prefix}'"),
                        format!("/opds/authors/{prefix}"),
                    );
                }
            }
            format_feed(feed)
        }
        Err(err) => format!("{err}"),
    }
}

#[get("/opds/author/{id}")]
async fn authors_by_id(ctx: web::Data<AppState>, path: web::Path<u32>) -> impl Responder {
    let id = path.into_inner();
    info!("/opds/authors/{id}");

    match find_authors(ctx.clone(), id).await {
        Ok(authors) => {
            let mut feed = Feed::new("Поиск книг по авторам");
            for author in authors {
                feed.add(
                    format!("{} {} {}", author.first_name, author.middle_name, author.last_name),
                    format!("/opds/author/{}/{}/{}", author.first_id, author.middle_id, author.last_id),
                );
            }
            format_feed(feed)
        }
        Err(err) => format!("{err}"),
    }
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let (addr, port, database) = read_params();
    info!("Try http://{addr}:{port}/opds");

    let pool = SqlitePool::connect(&database).await?;

    let ctx = web::Data::new(AppState::new(pool));

    HttpServer::new(move || {
        App::new()
            .app_data(ctx.clone())
            .service(opds)
            .service(authors_root)
            .service(authors_by_mask)
            .service(authors_by_id)
    })
    .bind((addr.as_str(), port))?
    .run()
    .await
    .map_err(anyhow::Error::from)
}

/*********************************************************************************/

fn read_params() -> (String, u16, String) {
    let addr = match std::env::var("FB2S_ADDRESS") {
        Ok(addr) => addr,
        Err(e) => {
            warn!("FB2S_ADDRESS: {e}");
            String::from(DEFAULT_ADDRESS)
        }
    };

    let port = match std::env::var("FB2S_PORT") {
        Ok(string) => string.as_str().parse::<u16>().unwrap_or(DEFAULT_PORT),
        Err(e) => {
            warn!("FB2S_PORT: {e}");
            DEFAULT_PORT
        }
    };

    let database = match std::env::var("FB2S_DATABASE") {
        Ok(addr) => addr,
        Err(e) => {
            warn!("FB2S_DATABASE: {e}");
            String::from(DEFAULT_DATABASE)
        }
    };

    return (addr, port, database);
}

fn make_feed(feed: Feed) -> anyhow::Result<String> {
    let mut w = Writer::new(Cursor::new(Vec::new()));

    w.write_event(Event::PI(BytesText::from_escaped(XML_HEAD)))?;
    w.create_element("feed")
        .with_attribute(("xmlns", "http://www.w3.org/2005/Atom"))
        .with_attribute(("xmlns:dc", "http://purl.org/dc/terms/"))
        .with_attribute(("xmlns:os", "http://a9.com/-/spec/opensearch/1.1/"))
        .with_attribute(("xmlns:opds", "http://opds-spec.org/2010/catalog"))
        .write_inner_content(|w| {
            w.create_element("title")
                .write_text_content(BytesText::new(&feed.title))?;

            let updated = format!("{:?}", chrono::Utc::now());
            w.create_element("updated")
                .write_text_content(BytesText::new(&updated))?;

            w.create_element("link")
                .with_attribute(("href", "/opds"))
                .with_attribute(("rel", "/start"))
                .with_attribute(("type", "application/atom+xml;profile=opds-catalog"))
                .write_empty()?;

            for entry in &feed.entries {
                w.create_element("entry").write_inner_content(|w| {
                    w.create_element("id")
                        .write_text_content(BytesText::new(&entry.id))?;

                    w.create_element("title")
                        .write_text_content(BytesText::new(&entry.title))?;

                    w.create_element("link")
                        .with_attribute(("href", entry.link.as_str()))
                        .with_attribute(("type", "application/atom+xml;profile=opds-catalog"))
                        .write_empty()?;

                    Ok(())
                })?;
            }

            Ok(())
        })?;

    Ok(String::from_utf8_lossy(&w.into_inner().into_inner()).into_owned())
}

async fn get_last_name_id(ctx: web::Data<AppState>, name: &String) -> anyhow::Result<u32> {
    let sql = format!(
        r#"
            SELECT id 
            FROM last_names 
            WHERE name = "{name}"
        "#
    );
    let pool = ctx.pool.try_lock().unwrap();
    let row = sqlx::query(&sql).fetch_one(&*pool).await?;
    let id: u32 = row.try_get("id")?;

    Ok(id)
}

async fn make_patterns(ctx: web::Data<AppState>, pattern: String) -> anyhow::Result<Vec<String>> {
    let len = pattern.chars().count() + 1;
    let sql = format!(
        r#"
            SELECT DISTINCT substr(name, 1, {len}) AS name 
            FROM last_names 
            WHERE name LIKE "{pattern}%"
            ORDER BY 1
        "#
    );

    let pool = ctx.pool.lock().unwrap();
    let rows = sqlx::query(&sql).fetch_all(&*pool).await?;
    let mut out = Vec::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        out.push(format!("{}", name));
    }

    Ok(out)
}

struct Author {
    pub first_id: u32,
    pub middle_id: u32,
    pub last_id: u32,
    pub first_name: String,
    pub middle_name: String,
    pub last_name: String,
}

async fn find_authors(ctx: web::Data<AppState>, id: u32) -> anyhow::Result<Vec<Author>> {
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
            WHERE last_name_id = {id}
        "#
    );

    let pool = ctx.pool.lock().unwrap();
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_1() {
        let vec = vec![String::from("b"), String::from("a")];
        let exp = vec![String::from("a"), String::from("b")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_2() {
        let vec = vec![String::from("ab"), String::from("aa")];
        let exp = vec![String::from("aa"), String::from("ab")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_and_ascii_1() {
        let vec = vec![String::from("a"), String::from("я")];
        let exp = vec![String::from("я"), String::from("a")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_and_ascii_2() {
        let vec = vec![String::from("aя"), String::from("яя")];
        let exp = vec![String::from("яя"), String::from("aя")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_lc() {
        let vec = vec![String::from("яя"), String::from("ая")];
        let exp = vec![String::from("ая"), String::from("яя")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_uc() {
        let vec = vec![String::from("ЫЫ"), String::from("АЫ")];
        let exp = vec![String::from("АЫ"), String::from("ЫЫ")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_mc() {
        let vec = vec![String::from("Ыа"), String::from("АЫ")];
        let exp = vec![String::from("АЫ"), String::from("Ыа")];
        assert_eq!(sorted(vec), exp);
    }
}