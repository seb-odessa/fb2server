use actix_web::{get, web, App, HttpServer, Responder};
use log::{info, warn};
use sqlx::sqlite::SqlitePool;

use std::sync::Mutex;

use lib::database;
use lib::opds;
use lib::utils;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE: &'static str = "books.db";

// #[derive(Clone)]
struct AppState {
    // counter: Mutex<i32>,
    pool: Mutex<SqlitePool>,
    // names: Query<'a, DB, A>,
}
impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            // counter: Mutex::new(0),
            pool: Mutex::new(pool),
        }
    }
}

#[get("/opds")]
async fn root_opds() -> impl Responder {
    info!("/opds");
    let mut feed = opds::Feed::new("Catalog Root");
    feed.add("Поиск книг по авторам", "/opds/authors");
    feed.add("Поиск книг по сериям", "/opds/series");
    feed.add("Поиск книг по жанрам", "/opds/genres");
    opds::format_feed(feed)
}

#[get("/opds/authors")]
async fn root_opds_authors(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/authors");

    let pool = ctx.pool.lock().unwrap();
    let mut feed = opds::Feed::new("Поиск книг по авторам");
    match database::make_patterns(&pool, &String::from("")).await {
        Ok(patterns) => {
            for pattern in utils::sorted(patterns) {
                if !pattern.is_empty() {
                    feed.add(
                        format!("{pattern}..."),
                        format!("/opds/authors/mask/{pattern}"),
                    );
                }
            }
        }
        Err(err) => return format!("{err}"),
    }
    opds::format_feed(feed)
}

#[get("/opds/authors/mask/{pattern}")]
async fn root_opds_authors_mask(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let pattern = path.into_inner();
    info!("/opds/authors/mask/{pattern}");

    let pool = ctx.pool.lock().unwrap();
    match root_opds_authors_mask_impl(&pool, pattern).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
}

async fn root_opds_authors_mask_impl(
    pool: &SqlitePool,
    mut pattern: String,
) -> anyhow::Result<opds::Feed> {
    let mut feed = opds::Feed::new("Поиск книг по авторам");

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

#[get("/opds/author/id/{fid}/{mid}/{lid}")]
async fn root_opds_author_id(path: web::Path<(u32, u32, u32)>) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/id/{fid}/{mid}/{lid}");

    let mut feed = opds::Feed::new("Книги автора");
    feed.add(
        "Книги по сериям",
        &format!("/opds/author/series/{fid}/{mid}/{lid}"),
    );
    feed.add(
        "Книги без серий",
        &format!("/opds/author/wo_series/{fid}/{mid}/{lid}"),
    );
    feed.add(
        "Книги по жанрам",
        &format!("/opds/author/genres/{fid}/{mid}/{lid}"),
    );
    feed.add(
        "Книги по алфавиту",
        &format!("/opds/author/alphabet/{fid}/{mid}/{lid}"),
    );

    opds::format_feed(feed)
}

#[get("/opds/author/series/{fid}/{mid}/{lid}")]
async fn root_opds_author_series(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/series/{fid}/{mid}/{lid}");

    let pool = ctx.pool.lock().unwrap();
    match root_opds_author_series_impl(&pool, (fid, mid, lid)).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
}

async fn root_opds_author_series_impl(
    pool: &SqlitePool,
    ids: (u32, u32, u32),
) -> anyhow::Result<opds::Feed> {
    let (fid, mid, lid) = ids;
    let author = database::author(&pool, (fid, mid, lid)).await?;
    let mut feed = opds::Feed::new(author);
    let mut series = database::author_series(&pool, (fid, mid, lid)).await?;
    series.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

    for serie in series {
        let sid = serie.id;
        let name = serie.name;
        let count = serie.count;
        feed.add(
            format!("{name} [{count} книг]"),
            format!("/opds/author/serie/{fid}/{mid}/{lid}/{sid}"),
        );
    }
    return Ok(feed);
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
            .service(root_opds)
            .service(root_opds_authors)
            .service(root_opds_authors_mask)
            .service(root_opds_author_id)
            .service(root_opds_author_series)
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
