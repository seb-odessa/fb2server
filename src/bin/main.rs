use actix_files::NamedFile;
use actix_web::{get, web, App, HttpServer, Responder};

use log::{info, warn};
use sqlx::sqlite::SqlitePool;

use std::env::VarError;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Mutex;

use lib::impls;
use lib::opds;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE: &'static str = "./books.db";
const DEFAULT_LIBRARY: &'static str = "/lib.rus.ec";

struct AppState {
    pool: Mutex<SqlitePool>,
    path: PathBuf,
}
impl AppState {
    pub fn new(pool: SqlitePool, library: PathBuf) -> Self {
        Self {
            pool: Mutex::new(pool),
            path: library,
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
        &format!("/opds/author/nonserie/books/{fid}/{mid}/{lid}"),
    );
    feed.add("Книги по жанрам", &format!("/opds/nimpl"));
    feed.add(
        "Книги по алфавиту",
        &format!("/opds/author/alphabet/books/{fid}/{mid}/{lid}"),
    );

    opds::format_feed(feed)
}

#[get("/opds/nimpl")]
async fn root_opds_nimpl() -> impl Responder {
    let mut feed = opds::Feed::new("Not Implemented");
    feed.add("Пока не работает", "/opds/nimpl");
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

#[get("/opds/author/nonserie/books/{fid}/{mid}/{lid}")]
async fn root_opds_author_nonserie_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/nonserie/books/{fid}/{mid}/{lid}");

    let pool = ctx.pool.lock().unwrap();
    match impls::root_opds_author_nonserie_books(&pool, (fid, mid, lid)).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
}

#[get("/opds/author/alphabet/books/{fid}/{mid}/{lid}")]
async fn root_opds_author_alphabet_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/alphabet/books/{fid}/{mid}/{lid}");

    let pool = ctx.pool.lock().unwrap();
    match impls::root_opds_author_alphabet_books(&pool, (fid, mid, lid)).await {
        Ok(feed) => opds::format_feed(feed),
        Err(err) => format!("{err}"),
    }
}

#[get("/opds/book/{id}")]
async fn root_opds_book(
    ctx: web::Data<AppState>,
    param: web::Path<u32>,
) -> std::io::Result<NamedFile> {
    let id = param.into_inner();
    info!("/opds/book/{id})");

    let book = impls::extract_book(ctx.path.clone(), id)?;
    info!("root_opds_book =>: {}", book.display());
    Ok(actix_files::NamedFile::open_async(book).await?)
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    let (addr, port, database, library) = read_params();
    let pool = SqlitePool::connect(&database).await?;
    let ctx = web::Data::new(AppState::new(pool, library));

    info!("OPDS Server will ready at http://{addr}:{port}/opds");
    HttpServer::new(move || {
        App::new()
            .app_data(ctx.clone())
            .service(root_opds)
            .service(root_opds_nimpl)
            .service(root_opds_authors)
            .service(root_opds_authors_mask)
            .service(root_opds_author_id)
            .service(root_opds_author_series)
            .service(root_opds_author_serie_books)
            .service(root_opds_author_nonserie_books)
            .service(root_opds_author_alphabet_books)
            .service(root_opds_book)
    })
    .bind((addr.as_str(), port))?
    .run()
    .await
    .map_err(anyhow::Error::from)
}

/*********************************************************************************/

fn get_env<T: Into<String> + Display>(name: T, default: T) -> String {
    let name = name.into();
    let default = default.into();

    std::env::var(&name)
        .or_else(|err| {
            warn!("{name}: {err} will use '{default}'");
            Ok::<String, VarError>(default)
        })
        .expect(&format!("Can't configure {}", name))
}

fn read_params() -> (String, u16, String, PathBuf) {
    let addr = get_env("FB2S_ADDRESS", DEFAULT_ADDRESS);
    info!("FB2S_ADDRESS: {addr}");

    let port = get_env("FB2S_PORT", &format!("{DEFAULT_PORT}"))
        .as_str()
        .parse::<u16>()
        .unwrap_or(DEFAULT_PORT);
    info!("FB2S_PORT: {port}");

    let database = get_env("FB2S_DATABASE", DEFAULT_DATABASE);
    info!("FB2S_DATABASE: {database}");

    let library = PathBuf::from(get_env("FB2S_LIBRARY", DEFAULT_LIBRARY));
    info!("FB2S_LIBRARY: {}", library.display());

    return (addr, port, database, library);
}
