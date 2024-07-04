use actix_files::NamedFile;
use actix_web::{get, web, App, HttpServer, Responder};

use log::{error, info, warn};
use sqlx::sqlite::SqlitePool;

use std::env::VarError;
use std::fmt::Display;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use lib::database;
use lib::database::QueryType;
use lib::impls;
use lib::impls::authors;
use lib::opds;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE: &'static str = "sqlite://books.db?mode=ro";
const DEFAULT_STATISTIC: &'static str = "sqlite://statistic.db?mode=rwc";
const DEFAULT_LIBRARY: &'static str = "/lib.rus.ec";

struct AppState {
    catalog: Mutex<SqlitePool>,
    statistic: Mutex<SqlitePool>,
    path: PathBuf,
}
impl AppState {
    pub fn new(catalog: SqlitePool, statistic: SqlitePool, library: PathBuf) -> Self {
        Self {
            catalog: Mutex::new(catalog),
            statistic: Mutex::new(statistic),
            path: library,
        }
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    let (addr, port, database, statistic, library) = read_params();
    let catalog = SqlitePool::connect(&database).await?;
    let statistic = SqlitePool::connect(&statistic).await?;
    database::init_statistic_db(&statistic).await?;

    let ctx = web::Data::new(AppState::new(catalog, statistic, library));

    info!("OPDS Server will ready at http://{addr}:{port}/opds");
    HttpServer::new(move || {
        App::new()
            .app_data(ctx.clone())
            .service(root_opds)
            .service(root_opds_nimpl)
            // Books by Authors
            .service(root_opds_authors)
            .service(root_opds_authors_mask)
            .service(root_opds_author_id)
            .service(root_opds_author_series)
            .service(root_opds_author_serie_books)
            .service(root_opds_author_nonserie_books)
            .service(root_opds_author_alphabet_books)
            .service(root_opds_author_added_books)
            // Books by Series
            .service(root_opds_series)
            .service(root_opds_series_mask)
            .service(root_opds_serie_id)
            .service(root_opds_serie_books)
            // Books by Genres
            .service(root_opds_meta)
            .service(root_opds_genres_meta)
            .service(root_opds_genres_genre)
            .service(root_opds_genres_series)
            .service(root_opds_genres_authors)
            .service(root_opds_genres_dates)
            // Favorite Books
            .service(root_opds_favorite_authors)
            // Books
            .service(root_opds_book)
    })
    .bind((addr.as_str(), port))?
    .run()
    .await
    .map_err(anyhow::Error::from)
}

#[get("/opds/nimpl")]
async fn root_opds_nimpl() -> impl Responder {
    // Not Implemented placeholder
    let mut feed = opds::Feed::new("Not Implemented");
    feed.add("Пока не работает", "/opds/nimpl");
    opds::handle_feed(Ok(feed))
}

#[get("/opds")]
async fn root_opds(_ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds");
    let mut feed = opds::Feed::new("Catalog Root");
    feed.add("Поиск книг по авторам", "/opds/authors");
    feed.add("Поиск книг по сериям", "/opds/series");
    feed.add("Поиск книг по жанрам", "/opds/meta");
    feed.add("Любимые авторы ", "/opds/favorites");
    opds::handle_feed(Ok(feed))
}

#[get("/opds/authors")]
async fn root_opds_authors(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/authors");

    let title = String::from("Поиск книг по авторам");
    let root = String::from("/opds/authors/mask");
    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_by_mask(&catalog, QueryType::Author, title, root).await;
    opds::handle_feed(feed)
}

#[get("/opds/series")]
async fn root_opds_series(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/series");

    let title = String::from("Поиск книг сериям");
    let root = String::from("/opds/series/mask");
    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_by_mask(&catalog, QueryType::Serie, title, root).await;
    opds::handle_feed(feed)
}

#[get("/opds/meta")]
async fn root_opds_meta(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/meta");

    let title = String::from("Поиск книг жанрам");
    let root = String::from("/opds/genres");
    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_meta(&catalog, &title, &root).await;
    opds::handle_feed(feed)
}

#[get("/opds/genres/{meta}")]
async fn root_opds_genres_meta(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let meta = path.into_inner();
    info!("/opds/genres/{meta}");

    let title = String::from("Поиск книг жанрам");
    let root = String::from("/opds/genre");
    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_genres_meta(&catalog, &title, &root, &meta).await;
    opds::handle_feed(feed)
}

#[get("/opds/genre/{genre}")]
async fn root_opds_genres_genre(path: web::Path<String>) -> impl Responder {
    let genre = path.into_inner();
    info!("/opds/genre/{genre}");

    let mut feed = opds::Feed::new(format!("Книги '{genre}'"));
    feed.add("По сериям", &format!("/opds/genre/series/{genre}"));
    feed.add("По авторам", &format!("/opds/genre/authors/{genre}"));
    feed.add("Последние поступления (45)", &format!("/opds/genre/dates/{genre}"));
    opds::handle_feed(Ok(feed))
}

#[get("/opds/genre/series/{genre}")]
async fn root_opds_genres_series(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let genre = path.into_inner();
    info!("/opds/genre/series/{genre}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_genres_series(&catalog, &genre).await;
    opds::handle_feed(feed)
}

#[get("/opds/genre/authors/{genre}")]
async fn root_opds_genres_authors(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let genre = path.into_inner();
    info!("/opds/genre/authors/{genre}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_genres_authors(&catalog, &genre).await;
    opds::handle_feed(feed)
}

#[get("/opds/genre/dates/{genre}")]
async fn root_opds_genres_dates(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let genre = path.into_inner();
    info!("/opds/genre/dates/{genre}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_genres_dates(&catalog, &genre).await;
    opds::handle_feed(feed)
}

#[get("/opds/favorites")]
async fn root_opds_favorite_authors(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/favorites");

    let books = ctx.catalog.lock().unwrap();
    let stats = ctx.statistic.lock().unwrap();
    let feed = impls::root_opds_favorite_authors(&books, &stats).await;
    opds::handle_feed(feed)
}

#[get("/opds/authors/mask/{pattern}")]
async fn root_opds_authors_mask(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let pattern = path.into_inner();
    info!("/opds/authors/mask/{pattern}");

    let title = String::from("Поиск книг по авторам");
    let root = String::from("/opds/authors/mask");
    let catalog = ctx.catalog.lock().unwrap();
    let feed =
        impls::root_opds_search_by_mask(&catalog, QueryType::Author, title, root, pattern).await;
    opds::handle_feed(feed)
}

#[get("/opds/series/mask/{pattern}")]
async fn root_opds_series_mask(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let pattern = path.into_inner();
    info!("/opds/series/mask/{pattern}");

    let title = String::from("Поиск книг сериям");
    let root = String::from("/opds/series/mask");
    let catalog = ctx.catalog.lock().unwrap();
    let feed =
        impls::root_opds_search_by_mask(&catalog, QueryType::Serie, title, root, pattern).await;
    opds::handle_feed(feed)
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
    feed.add(
        "Книги по дате поступления",
        &format!("/opds/author/added/books/{fid}/{mid}/{lid}"),
    );
    opds::handle_feed(Ok(feed))
}

#[get("/opds/serie/id/{id}")]
async fn root_opds_serie_id(path: web::Path<u32>) -> impl Responder {
    let id = path.into_inner();
    info!("/opds/serie/id/{id}");

    let mut feed = opds::Feed::new("Книги в серии");
    feed.add(
        "Книги по номеру в серии",
        &format!("/opds/serie/books/{id}/numbered"),
    );
    feed.add(
        "Книги по алфавиту",
        &format!("/opds/serie/books/{id}/alphabet"),
    );
    feed.add(
        "Книги по дате поступления",
        &format!("/opds/serie/books/{id}/added"),
    );
    opds::handle_feed(Ok(feed))
}

#[get("/opds/serie/books/{id}/{sort}")]
async fn root_opds_serie_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, String)>,
) -> impl Responder {
    let (id, sort) = path.into_inner();
    info!("/opds/serie/{id}/{sort}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_serie_books(&catalog, id, sort).await;
    opds::handle_feed(feed)
}

#[get("/opds/author/series/{fid}/{mid}/{lid}")]
async fn root_opds_author_series(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/series/{fid}/{mid}/{lid}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_author_series(&catalog, (fid, mid, lid)).await;
    opds::handle_feed(feed)
}

#[get("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}")]
async fn root_opds_author_serie_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid, sid) = path.into_inner();
    info!("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}");

    let sort = authors::Sort::BySerie(sid);
    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_author_books(&catalog, (fid, mid, lid), sort).await;
    opds::handle_feed(feed)
}

#[get("/opds/author/nonserie/books/{fid}/{mid}/{lid}")]
async fn root_opds_author_nonserie_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/nonserie/books/{fid}/{mid}/{lid}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed =
        impls::root_opds_author_books(&catalog, (fid, mid, lid), authors::Sort::NoSerie).await;
    opds::handle_feed(feed)
}

#[get("/opds/author/alphabet/books/{fid}/{mid}/{lid}")]
async fn root_opds_author_alphabet_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/alphabet/books/{fid}/{mid}/{lid}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed =
        impls::root_opds_author_books(&catalog, (fid, mid, lid), authors::Sort::Alphabet).await;
    opds::handle_feed(feed)
}

#[get("/opds/author/added/books/{fid}/{mid}/{lid}")]
async fn root_opds_author_added_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/added/books/{fid}/{mid}/{lid}");

    let catalog = ctx.catalog.lock().unwrap();
    let feed = impls::root_opds_author_books(&catalog, (fid, mid, lid), authors::Sort::Added).await;
    opds::handle_feed(feed)
}

#[get("/opds/book/{id}")]
async fn root_opds_book(
    ctx: web::Data<AppState>,
    param: web::Path<u32>,
) -> std::io::Result<NamedFile> {
    let id = param.into_inner();
    info!("/opds/book/{id})");

    match impls::extract_book(ctx.path.clone(), id) {
        Ok(book) => {
            let catalog = ctx.statistic.lock().unwrap();

            if let Err(err) = database::insert_book(&catalog, id).await {
                let msg = format!("{err}");
                error!("{}", msg);
                return Err(io::Error::new(io::ErrorKind::Other, msg));
            }
            match actix_files::NamedFile::open_async(book).await {
                Ok(file) => Ok(file),
                Err(err) => {
                    let msg = format!("{err}");
                    error!("{}", msg);
                    return Err(io::Error::new(io::ErrorKind::Other, msg));
                }
            }
        }
        Err(err) => {
            let msg = format!("{err}");
            error!("{}", msg);
            return Err(io::Error::new(io::ErrorKind::Other, msg));
        }
    }
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

fn read_params() -> (String, u16, String, String, PathBuf) {
    let addr = get_env("FB2S_ADDRESS", DEFAULT_ADDRESS);
    info!("FB2S_ADDRESS: {addr}");

    let port = get_env("FB2S_PORT", &format!("{DEFAULT_PORT}"))
        .as_str()
        .parse::<u16>()
        .unwrap_or(DEFAULT_PORT);
    info!("FB2S_PORT: {port}");

    let database = get_env("FB2S_DATABASE", DEFAULT_DATABASE);
    info!("FB2S_DATABASE: {database}");

    let statistic = get_env("FB2S_STATISTIC", DEFAULT_STATISTIC);
    info!("FB2S_STATISTIC: {statistic}");

    let library = PathBuf::from(get_env("FB2S_LIBRARY", DEFAULT_LIBRARY));
    info!("FB2S_LIBRARY: {}", library.display());

    return (addr, port, database, statistic, library);
}
