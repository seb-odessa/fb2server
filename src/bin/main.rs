use actix_web::{get, web, App, HttpServer, Responder};
use log::{info, warn};
use sqlx::sqlite::SqlitePool;

use std::sync::Mutex;

use lib::impls;
use lib::opds;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE: &'static str = "books.db";


struct AppState {
    pool: Mutex<SqlitePool>,
}
impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
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
    match impls::root_opds_authors(&pool).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
}

#[get("/opds/authors/mask/{pattern}")]
async fn root_opds_authors_mask(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let pattern = path.into_inner();
    info!("/opds/authors/mask/{pattern}");

    let pool = ctx.pool.lock().unwrap();
    match impls::root_opds_authors_mask(&pool, pattern).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
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
    match impls::root_opds_author_series(&pool, (fid, mid, lid)).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
}

#[get("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}")]
async fn root_opds_author_serie_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid, sid) = path.into_inner();
    info!("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}");

    let pool = ctx.pool.lock().unwrap();
    match impls::root_opds_author_serie_books(&pool, (fid, mid, lid, sid)).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
}

#[actix_web::main] 
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
            .service(root_opds_author_serie_books)
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
