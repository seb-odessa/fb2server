use actix_files::NamedFile;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder, ResponseError};
use log::{error, info, warn};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use lib::opds::Feed;
use lib::utils;
use opds_db_api::OpdsApi;

use std::env::VarError;
use std::fmt::{self, Display};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Mutex;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE: &'static str = "file:/lib.rus.ec/books.db?mode=ro";
const DEFAULT_STATISTIC: &'static str = "file:statistic.db?mode=rwc";
const DEFAULT_LIBRARY: &'static str = "/lib.rus.ec";

struct AppState {
    api: Mutex<OpdsApi>,
    storage: PathBuf,
}
impl AppState {
    pub fn new(api: OpdsApi, storage: PathBuf) -> Self {
        Self {
            api: Mutex::new(api),
            storage: storage,
        }
    }
}
#[derive(Debug)]
struct OpdsError(anyhow::Error);

impl fmt::Display for OpdsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ResponseError for OpdsError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().json("Error: ")
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    let address = get_env("FB2S_ADDRESS", DEFAULT_ADDRESS);
    info!("FB2S_ADDRESS: {address}");

    let port = get_env("FB2S_PORT", &format!("{DEFAULT_PORT}"))
        .as_str()
        .parse::<u16>()
        .unwrap_or(DEFAULT_PORT);
    info!("FB2S_PORT: {port}");

    let database = get_env("FB2S_DATABASE", DEFAULT_DATABASE);
    info!("FB2S_DATABASE: {database}");

    let statistic = get_env("FB2S_STATISTIC", DEFAULT_STATISTIC);
    info!("FB2S_STATISTIC: {statistic}");

    let storage = PathBuf::from(get_env("FB2S_LIBRARY", DEFAULT_LIBRARY));
    info!("FB2S_LIBRARY: {}", storage.display());

    let api = OpdsApi::try_from(&database)?;

    let ctx = web::Data::new(AppState::new(api, storage));

    info!("OPDS Server will ready at http://{address}:{port}/opds");
    HttpServer::new(move || {
        App::new()
            .app_data(ctx.clone())
            .service(root)
            // Books by Authors
            .service(root_authors)
            .service(root_authors_by_mask)
            .service(root_author_by_id)
            .service(root_author_series)
            // Books by Series
            .service(root_series)
            .service(root_series_by_mask)
            // Books by Genres
            .service(root_genres)
            .service(root_genres_by_meta)
            .service(root_genre_root)
            // .service(root_opds_genres_series)
            // .service(root_opds_genres_authors)
            // .service(root_opds_genres_dates)
            // // Favorite Books
            // .service(root_opds_favorite_authors)
            // // Books
            .service(root_books_by_author_and_serie)
            .service(root_books_by_author_nonserie)
            .service(root_books_by_author_and_genre)
            .service(root_books_by_author_alphabet)
            .service(root_books_by_author_datesort)
            .service(root_books_by_serie)
            .service(root_book_upload)
    })
    .bind((address.as_str(), port))?
    .run()
    .await
    .map_err(anyhow::Error::from)
}

#[get("/opds")]
async fn root(_ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds");
    let mut feed = Feed::new("Каталог");
    feed.catalog("Поиск по авторам", "/opds/authors");
    feed.catalog("Поиск по сериям", "/opds/series");
    feed.catalog("Поиск по жанрам", "/opds/genres");
    feed.catalog("Поиск по ниименованиям", "/opds/titles");
    feed.catalog("Любимые авторы ", "/opds/favorites");
    feed.format()
}

#[get("/opds/authors")]
async fn root_authors(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/authors");
    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по авторам");
        let all = String::from("");
        let patterns = api.authors_next_char_by_prefix(&all).map_err(OpdsError)?;
        for prefix in patterns.into_iter() {
            let title = format!("{prefix}...");
            let encoded = utf8_percent_encode(prefix.as_str(), NON_ALPHANUMERIC).to_string();
            let link = format!("/opds/authors/mask/{encoded}");
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/authors/mask/{pattern}")]
async fn root_authors_by_mask(ctx: web::Data<AppState>, args: web::Path<String>) -> impl Responder {
    let pattern = args.into_inner();
    info!("/opds/authors/mask/{pattern}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по авторам");

        let fetcher = |s: &String| api.authors_next_char_by_prefix(s);
        let (exact, tail) = lib::search_by_mask(&pattern, fetcher).map_err(OpdsError)?;

        for name in exact.into_iter() {
            let authors = api.authors_by_last_name(&name).map_err(OpdsError)?;
            for author in authors.iter() {
                let title = format!("[{author}]");
                let link = format!(
                    "/opds/author/id/{}/{}/{}",
                    author.first_name.id, author.middle_name.id, author.last_name.id
                );
                feed.catalog(title, link);
            }
        }
        for prefix in tail.into_iter() {
            let title = format!("{prefix}...");
            let encoded = utf8_percent_encode(prefix.as_str(), NON_ALPHANUMERIC).to_string();
            let link = format!("/opds/authors/mask/{encoded}");
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/author/id/{fid}/{mid}/{lid}")]
async fn root_author_by_id(args: web::Path<(u32, u32, u32)>) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/author/id/{fid}/{mid}/{lid}");

    let uri = "/opds/author";
    let ids = &format!("{fid}/{mid}/{lid}");
    let mut feed = Feed::new("Книги автора");
    feed.catalog("По сериям", &format!("{uri}/series/{ids}"));
    feed.catalog("Вне серий", &format!("{uri}/books/nonserie/{ids}"));
    feed.catalog("По жанрам", &format!("{uri}/books/genre/{ids}"));
    feed.catalog("По алфавиту", &format!("{uri}/books/alphabet/{ids}"));
    feed.catalog("По дате", &format!("{uri}/books/added/{ids}"));

    feed.format()
}

#[get("/opds/author/series/{fid}/{mid}/{lid}")]
async fn root_author_series(
    ctx: web::Data<AppState>,
    args: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/author/series/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Серии автора");
        let series = api.series_by_author_ids(fid, mid, lid).map_err(OpdsError)?;
        for serie in series.iter() {
            let title = format!("{serie}");
            let link = format!("/opds/serie/books/id/{}/{}/{}/{}", fid, mid, lid, serie.id);
            feed.catalog(title, link);
        }
        if series.is_empty() {
            let title = format!("Вернуться к автору");
            let link = format!("/opds/author/id/{}/{}/{}", fid, mid, lid);
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/author/books/nonserie/{fid}/{mid}/{lid}")]
async fn root_books_by_author_nonserie(
    ctx: web::Data<AppState>,
    args: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/author/books/nonserie/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги без серий");
        let books = api
            .books_by_author_ids_without_serie(fid, mid, lid)
            .map_err(OpdsError)?;
        for book in books.iter() {
            let title = format!("{book}");
            let link = format!("/opds/book/id/{}", book.id);
            feed.book(title, link);
        }
        if books.is_empty() {
            let title = format!("Вернуться к автору");
            let link = format!("/opds/author/id/{}/{}/{}", fid, mid, lid);
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/author/books/genre/{genre}")]
async fn root_books_by_author_and_genre(
    ctx: web::Data<AppState>,
    args: web::Path<u32>,
) -> impl Responder {
    let genre = args.into_inner();
    info!("/opds/author/books/genre/{genre}");

    let feed;
    if let Ok(_api) = ctx.api.lock() {
        feed = Feed::new("Книги по жанрам");
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/author/books/alphabet/{fid}/{mid}/{lid}")]
async fn root_books_by_author_alphabet(
    ctx: web::Data<AppState>,
    args: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/author/books/alphabet/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Все книги по алфавиту");
        let books = api.books_by_author_ids(fid, mid, lid).map_err(OpdsError)?;
        for book in books.iter() {
            let title = format!("{book}");
            let link = format!("/opds/book/id/{}", book.id);
            feed.book(title, link);
        }
        if books.is_empty() {
            let title = format!("Вернуться к автору");
            let link = format!("/opds/author/id/{}/{}/{}", fid, mid, lid);
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/author/books/added/{fid}/{mid}/{lid}")]
async fn root_books_by_author_datesort(
    ctx: web::Data<AppState>,
    args: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/author/books/added/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Все книги по алфавиту");
        let mut books = api.books_by_author_ids(fid, mid, lid).map_err(OpdsError)?;
        books.sort_by(|a, b| b.added.cmp(&a.added));
        for book in books.iter() {
            let title = format!("{book}");
            let link = format!("/opds/book/id/{}", book.id);
            feed.book(title, link);
        }
        if books.is_empty() {
            let title = format!("Вернуться к автору");
            let link = format!("/opds/author/id/{}/{}/{}", fid, mid, lid);
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/series")]
async fn root_series(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/series");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по сериям");
        let all = String::from("");
        let patterns = api.series_next_char_by_prefix(&all).map_err(OpdsError)?;
        for prefix in patterns.into_iter() {
            let title = format!("{prefix}...");
            let encoded = utf8_percent_encode(prefix.as_str(), NON_ALPHANUMERIC).to_string();
            let link = format!("/opds/series/mask/{encoded}");
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/series/mask/{pattern}")]
async fn root_series_by_mask(ctx: web::Data<AppState>, args: web::Path<String>) -> impl Responder {
    let pattern = args.into_inner();
    info!("/opds/series/mask/{pattern}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по сериям");

        let fetcher = |s: &String| api.series_next_char_by_prefix(s);
        let (exact, tail) = lib::search_by_mask(&pattern, fetcher).map_err(OpdsError)?;

        for name in exact.into_iter() {
            let series = api.series_by_serie_name(&name).map_err(OpdsError)?;
            for serie in series.iter() {
                let title = format!("[{serie}]");
                let link = format!("/opds/serie/id/{}", serie.id);
                feed.catalog(title, link);
            }
        }
        for prefix in tail.into_iter() {
            let title = format!("{prefix}...");
            let encoded = utf8_percent_encode(prefix.as_str(), NON_ALPHANUMERIC).to_string();
            let link = format!("/opds/series/mask/{encoded}");
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/serie/id/{id}")]
async fn root_books_by_serie(ctx: web::Data<AppState>, args: web::Path<u32>) -> impl Responder {
    let id = args.into_inner();
    info!("/opds/serie/id/{id}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги в серии");
        let books = api.books_by_serie_id(id).map_err(OpdsError)?;
        for book in books.iter() {
            let title = format!("{book}");
            let link = format!("/opds/book/id/{}", book.id);
            feed.book(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/genres")]
async fn root_genres(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/genres");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги по жанрам");
        let metas = api.meta_genres().map_err(OpdsError)?;
        for meta in metas.into_iter() {
            let encoded = utf8_percent_encode(meta.as_str(), NON_ALPHANUMERIC).to_string();
            let link = format!("/opds/genres/meta/{encoded}");
            feed.catalog(meta, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/genres/meta/{meta}")]
async fn root_genres_by_meta(ctx: web::Data<AppState>, args: web::Path<String>) -> impl Responder {
    let meta = args.into_inner();
    info!("/opds/genres/meta/{meta}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги по поджанрам");
        let genres = api.genres_by_meta(&meta).map_err(OpdsError)?;
        for genre in genres.into_iter() {
            let encoded = utf8_percent_encode(genre.as_str(), NON_ALPHANUMERIC).to_string();
            let link = format!("/opds/genre/{encoded}");
            feed.catalog(genre, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/genre/{genre}")]
async fn root_genre_root(path: web::Path<String>) -> impl Responder {
    let genre = path.into_inner();
    info!("/opds/genre/{genre}");

    let mut feed = Feed::new(format!("Книги '{genre}'"));
    feed.catalog("По авторам", &format!("/opds/genre/authors/{genre}"));
    feed.catalog("По сериям", &format!("/opds/genre/series/{genre}"));
    feed.catalog("Последние 45 книг", &format!("/opds/genre/dates/{genre}"));
    feed.format()
}

// #[get("/opds/genre/series/{genre}")]
// async fn root_opds_genres_series(
//     ctx: web::Data<AppState>,
//     path: web::Path<String>,
// ) -> impl Responder {
//     let genre = path.into_inner();
//     info!("/opds/genre/series/{genre}");

//     let catalog = ctx.catalog.lock().unwrap();
//     let feed = impls::root_opds_genres_series(&catalog, &genre).await;
//     opds::handle_feed(feed)
// }

// #[get("/opds/genre/authors/{genre}")]
// async fn root_opds_genres_authors(
//     ctx: web::Data<AppState>,
//     path: web::Path<String>,
// ) -> impl Responder {
//     let genre = path.into_inner();
//     info!("/opds/genre/authors/{genre}");

//     let catalog = ctx.catalog.lock().unwrap();
//     let feed = impls::root_opds_genres_authors(&catalog, &genre).await;
//     opds::handle_feed(feed)
// }

// #[get("/opds/genre/dates/{genre}")]
// async fn root_opds_genres_dates(
//     ctx: web::Data<AppState>,
//     path: web::Path<String>,
// ) -> impl Responder {
//     let genre = path.into_inner();
//     info!("/opds/genre/dates/{genre}");

//     let catalog = ctx.catalog.lock().unwrap();
//     let feed = impls::root_opds_genres_dates(&catalog, &genre).await;
//     opds::handle_feed(feed)
// }

// #[get("/opds/favorites")]
// async fn root_opds_favorite_authors(ctx: web::Data<AppState>) -> impl Responder {
//     info!("/opds/favorites");

//     let books = ctx.catalog.lock().unwrap();
//     let stats = ctx.statistic.lock().unwrap();
//     let feed = impls::root_opds_favorite_authors(&books, &stats).await;
//     opds::handle_feed(feed)
// }

// #[get("/opds/author/nonserie/books/{fid}/{mid}/{lid}")]
// async fn root_author_books_nonserie(
//     ctx: web::Data<AppState>,
//     path: web::Path<(u32, u32, u32)>,
// ) -> impl Responder {
//     let (fid, mid, lid) = path.into_inner();
//     info!("/opds/author/nonserie/books/{fid}/{mid}/{lid}");

//     let catalog = ctx.catalog.lock().unwrap();
//     let feed =
//         impls::root_opds_author_books(&catalog, (fid, mid, lid), authors::Sort::NoSerie).await;
//     opds::handle_feed(feed)
// }

// #[get("/opds/author/alphabet/books/{fid}/{mid}/{lid}")]
// async fn root_author_books_alphabet(
//     ctx: web::Data<AppState>,
//     path: web::Path<(u32, u32, u32)>,
// ) -> impl Responder {
//     let (fid, mid, lid) = path.into_inner();
//     info!("/opds/author/alphabet/books/{fid}/{mid}/{lid}");

//     let catalog = ctx.catalog.lock().unwrap();
//     let feed =
//         impls::root_opds_author_books(&catalog, (fid, mid, lid), authors::Sort::Alphabet).await;
//     opds::handle_feed(feed)
// }

#[get("/opds/serie/books/id/{fid}/{mid}/{lid}/{sid}")]
async fn root_books_by_author_and_serie(
    ctx: web::Data<AppState>,
    args: web::Path<(u32, u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid, sid) = args.into_inner();
    info!("/opds/serie/books/id/{fid}/{mid}/{lid}/{sid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Все книги по алфавиту");
        let books = api
            .books_by_author_ids_and_serie_id(fid, mid, lid, sid)
            .map_err(OpdsError)?;
        for book in books.iter() {
            let title = format!("{book}");
            let link = format!("/opds/book/id/{}", book.id);
            feed.book(title, link);
        }
        if books.is_empty() {
            let title = format!("Вернуться к автору");
            let link = format!("/opds/author/id/{}/{}/{}", fid, mid, lid);
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/book/id/{id}")]
async fn root_book_upload(
    ctx: web::Data<AppState>,
    args: web::Path<u32>,
) -> std::io::Result<NamedFile> {
    let id = args.into_inner();
    info!("/opds/book/id/{id})");

    match utils::extract_book(ctx.storage.clone(), id) {
        Ok(path) => {
            // let catalog = ctx.statistic.lock().unwrap();

            // if let Err(err) = database::insert_book(&catalog, id).await {
            //     let msg = format!("{err}");
            //     error!("{}", msg);
            //     return Err(io::Error::new(io::ErrorKind::Other, msg));
            // }
            match actix_files::NamedFile::open_async(path).await {
                Ok(file) => {
                    info!("Uploading {} B", file.metadata().size());
                    Ok(file)
                }
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

// /*********************************************************************************/
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

// fn read_params() -> (String, u16, String, String, PathBuf) {
//     let addr = get_env("FB2S_ADDRESS", DEFAULT_ADDRESS);
//     info!("FB2S_ADDRESS: {addr}");

//     let port = get_env("FB2S_PORT", &format!("{DEFAULT_PORT}"))
//         .as_str()
//         .parse::<u16>()
//         .unwrap_or(DEFAULT_PORT);
//     info!("FB2S_PORT: {port}");

//     let database = get_env("FB2S_DATABASE", DEFAULT_DATABASE);
//     info!("FB2S_DATABASE: {database}");

//     let statistic = get_env("FB2S_STATISTIC", DEFAULT_STATISTIC);
//     info!("FB2S_STATISTIC: {statistic}");

//     let library = PathBuf::from(get_env("FB2S_LIBRARY", DEFAULT_LIBRARY));
//     info!("FB2S_LIBRARY: {}", library.display());

//     return (addr, port, database, statistic, library);
// }
