use actix_files::NamedFile;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder, ResponseError};
use chrono::{Datelike, Duration, Utc};
use log::{error, info, warn};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use lib::books;
use lib::search;
use lib::opds::Feed;
use lib::statistic::StatisticApi;
use opds_api::OpdsApi;

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

type AppCtx = web::Data<AppState>;

struct AppState {
    api: Mutex<OpdsApi>,
    stat: Mutex<StatisticApi>,
    storage: PathBuf,
}
impl AppState {
    pub fn new(api: OpdsApi, stat: StatisticApi, storage: PathBuf) -> Self {
        Self {
            api: Mutex::new(api),
            stat: Mutex::new(stat),
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
    let stat = StatisticApi::try_from(&statistic)?;

    let ctx = web::Data::new(AppState::new(api, stat, storage));

    info!("OPDS Server will ready at http://{address}:{port}/opds");
    HttpServer::new(move || {
        App::new()
            .app_data(ctx.clone())
            .service(opds)
            // Books by Authors
            .service(opds_authors)
            .service(opds_authors_by_mask)
            .service(opds_authors_by_genre)
            .service(opds_author_by_id)
            // Books by Series
            .service(opds_series)
            .service(opds_series_by_mask)
            .service(opds_series_by_author)
            .service(opds_series_by_genre)
            // Books by Genres
            .service(opds_genres)
            .service(opds_genres_by_meta)
            .service(opds_genre_by_id)
            // Books
            .service(opds_books_by_author_and_serie)
            .service(opds_books_by_author_nonserie)
            .service(opds_books_by_author_and_genre)
            .service(opds_books_by_author_alphabet)
            .service(opds_books_by_author_datesort)
            .service(opds_books_by_serie)
            .service(opds_books_by_genre_year_month)
            .service(opds_book_upload)
            // Favorite Books
            .service(opds_authors_favorits)
    })
    .bind((address.as_str(), port))?
    .run()
    .await
    .map_err(anyhow::Error::from)
}

#[get("/opds")]
async fn opds() -> impl Responder {
    info!("/opds");
    let mut feed = Feed::new("Каталог");
    feed.catalog("Поиск по авторам", "/opds/authors");
    feed.catalog("Поиск по сериям", "/opds/series");
    feed.catalog("Поиск по жанрам", "/opds/genres");
    feed.catalog("Поиск по наименованиям", "/opds/titles");
    feed.catalog("Любимые авторы 10 дей", "/opds/authors/favorits/days/10");
    feed.catalog("Любимые авторы 30 дей ", "/opds/authors/favorits/days/30");
    feed.catalog("Любимые авторы 90 дей ", "/opds/authors/favorits/days/90");
    feed.format()
}

#[get("/opds/authors")]
async fn opds_authors(ctx: AppCtx) -> impl Responder {
    info!("/opds/authors");
    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по авторам");
        feed.catalog("[Home]", "/opds");
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
async fn opds_authors_by_mask(ctx: AppCtx, args: web::Path<String>) -> impl Responder {
    let pattern = args.into_inner();
    info!("/opds/authors/mask/{pattern}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по авторам");
        feed.catalog("[Home]", "/opds");

        let fetcher = |s: &String| api.authors_next_char_by_prefix(s);
        let (exact, tail) = search::search_by_mask(&pattern, fetcher).map_err(OpdsError)?;

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
async fn opds_author_by_id(args: web::Path<(u32, u32, u32)>) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/author/id/{fid}/{mid}/{lid}");

    let ids = &format!("{fid}/{mid}/{lid}");
    let mut feed = Feed::new("Книги автора");
    feed.catalog("Cерии", &format!("/opds/series/author/{ids}"));
    feed.catalog(
        "Книги без серий",
        &format!("/opds/books/author/nonserie/{ids}"),
    );
    feed.catalog("Книги жанрам", &format!("/opds/books/author/genre/{ids}"));
    feed.catalog(
        "Книги алфавиту",
        &format!("/opds/books/author/alphabet/{ids}"),
    );
    feed.catalog("Книги по дате", &format!("/opds/books/author/added/{ids}"));

    feed.format()
}

#[get("/opds/series/author/{fid}/{mid}/{lid}")]
async fn opds_series_by_author(ctx: AppCtx, args: web::Path<(u32, u32, u32)>) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/series/author/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Серии автора");
        feed.catalog("[Home]", "/opds");
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

#[get("/opds/books/author/nonserie/{fid}/{mid}/{lid}")]
async fn opds_books_by_author_nonserie(
    ctx: AppCtx,
    args: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/books/author/nonserie/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги без серий");
        feed.catalog("[Home]", "/opds");
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

#[get("/opds/books/author/genre/{genre}")]
async fn opds_books_by_author_and_genre(ctx: AppCtx, args: web::Path<u32>) -> impl Responder {
    let genre = args.into_inner();
    info!("/opds/books/author/genre/{genre}");

    let mut feed;
    if let Ok(_api) = ctx.api.lock() {
        feed = Feed::new("Книги по жанрам");
        feed.catalog("[Home]", "/opds");
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/books/author/alphabet/{fid}/{mid}/{lid}")]
async fn opds_books_by_author_alphabet(
    ctx: AppCtx,
    args: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/books/author/alphabet/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги по алфавиту");
        feed.catalog("[Home]", "/opds");
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

#[get("/opds/books/author/added/{fid}/{mid}/{lid}")]
async fn opds_books_by_author_datesort(
    ctx: AppCtx,
    args: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = args.into_inner();
    info!("/opds/books/author/added/{fid}/{mid}/{lid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги по дате поступления");
        feed.catalog("[Home]", "/opds");
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
async fn opds_series(ctx: AppCtx) -> impl Responder {
    info!("/opds/series");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по сериям");
        feed.catalog("[Home]", "/opds");
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
async fn opds_series_by_mask(ctx: AppCtx, args: web::Path<String>) -> impl Responder {
    let pattern = args.into_inner();
    info!("/opds/series/mask/{pattern}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поиск книг по сериям");
        feed.catalog("[Home]", "/opds");
        let fetcher = |s: &String| api.series_next_char_by_prefix(s);
        let (exact, tail) = search::search_by_mask(&pattern, fetcher).map_err(OpdsError)?;

        for name in exact.into_iter() {
            let series = api.series_by_serie_name(&name).map_err(OpdsError)?;
            for serie in series.iter() {
                let title = format!("[{serie}]");
                let link = format!("/opds/books/serie/id/{}", serie.id);
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

#[get("/opds/books/serie/id/{id}")]
async fn opds_books_by_serie(ctx: AppCtx, args: web::Path<u32>) -> impl Responder {
    let id = args.into_inner();
    info!("/opds/books/serie/id/{id}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги в серии");
        feed.catalog("[Home]", "/opds");
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
async fn opds_genres(ctx: AppCtx) -> impl Responder {
    info!("/opds/genres");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Жанры");
        feed.catalog("[Home]", "/opds");
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
async fn opds_genres_by_meta(ctx: AppCtx, args: web::Path<String>) -> impl Responder {
    let meta = args.into_inner();
    info!("/opds/genres/meta/{meta}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Поджанры");
        let genres = api.genres_by_meta(&meta).map_err(OpdsError)?;
        for genre in genres.into_iter() {
            let title = genre.value;
            let link = format!("/opds/genre/id/{}", genre.id);
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/genre/id/{gid}")]
async fn opds_genre_by_id(path: web::Path<u32>) -> impl Responder {
    let gid = path.into_inner();
    info!("/opds/genre/id/{gid}");

    let mut feed = Feed::new(format!("Книги по жанру"));
    feed.catalog("[Home]", "/opds");
    feed.catalog("Список авторов", &format!("/opds/authors/genre/{gid}"));
    feed.catalog("Список серий", &format!("/opds/series/genre/{gid}"));

    let today = Utc::now().date_naive();
    for i in 0..12 {
        let date = today - Duration::days(30 * i);
        let year = date.year();
        let month = date.month();
        let title = format!("Книги за {year} {} ", date.format("%B"));
        let link = format!("/opds/books/genre/id/{gid}/year/{year}/month/{month}");
        feed.catalog(title, link);
    }
    feed.format()
}

#[get("/opds/authors/genre/{gid}")]
async fn opds_authors_by_genre(ctx: AppCtx, args: web::Path<u32>) -> impl Responder {
    let gid = args.into_inner();
    info!("/opds/authors/series/{gid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Авторы по жанру");
        feed.catalog("[Home]", "/opds");
        let authors = api.authors_by_genre_id(gid).map_err(OpdsError)?;
        for author in authors.into_iter() {
            let title = format!("{author}");
            let link = format!(
                "/opds/author/id/{}/{}/{}",
                author.first_name.id, author.middle_name.id, author.last_name.id
            );
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }
    feed.format()
}

#[get("/opds/series/genre/{gid}")]
async fn opds_series_by_genre(ctx: AppCtx, args: web::Path<u32>) -> impl Responder {
    let gid = args.into_inner();
    info!("/opds/series/genre/{gid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Серии по жанру");
        feed.catalog("[Home]", "/opds");
        let series = api.series_by_genre_id(gid).map_err(OpdsError)?;
        for serie in series.iter() {
            let title = format!("{serie}");
            let link = format!("/opds/books/serie/id/{}", serie.id);
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }

    feed.format()
}

#[get("/opds/books/genre/id/{gid}/year/{year}/month/{month}")]
async fn opds_books_by_genre_year_month(
    ctx: AppCtx,
    args: web::Path<(u32, u16, u8)>,
) -> impl Responder {
    let (gid, year, month) = args.into_inner();
    info!("/opds/books/genre/id/{gid}/year/{year}/month/{month}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Книги в серии по месяцам");
        feed.catalog("[Home]", "/opds");
        let date = format!("{}-{:02}-%", year, month);
        let books = api
            .books_by_genre_id_and_date(gid, date)
            .map_err(OpdsError)?;
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

#[get("/opds/authors/favorits/days/{days}")]
async fn opds_authors_favorits(ctx: AppCtx, args: web::Path<u8>,) -> impl Responder {
    let days = args.into_inner();
    info!("/opds/authors/favorits/days/{days}");

    let mut feed;
    let ids;
    if let Ok(stat) = ctx.stat.lock() {
        ids = stat.load_last(days).map_err(OpdsError)?;
    }else {
        ids = vec![];
    }

    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Авторы за {days} дней");
        feed.catalog("[Home]", "/opds");
        let authors = api.authors_by_books_ids(ids).map_err(OpdsError)?;
        for author in authors.into_iter() {
            let title = format!("{author}");
            let link = format!(
                "/opds/author/id/{}/{}/{}",
                author.first_name.id, author.middle_name.id, author.last_name.id
            );
            feed.catalog(title, link);
        }
    } else {
        feed = Feed::new("Can't lock API");
    }
    feed.format()
}

#[get("/opds/serie/books/id/{fid}/{mid}/{lid}/{sid}")]
async fn opds_books_by_author_and_serie(
    ctx: AppCtx,
    args: web::Path<(u32, u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid, sid) = args.into_inner();
    info!("/opds/serie/books/id/{fid}/{mid}/{lid}/{sid}");

    let mut feed;
    if let Ok(api) = ctx.api.lock() {
        feed = Feed::new("Все книги по алфавиту");
        feed.catalog("[Home]", "/opds");
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
async fn opds_book_upload(ctx: AppCtx, args: web::Path<u32>) -> std::io::Result<NamedFile> {
    let id = args.into_inner();
    info!("/opds/book/id/{id})");

    match books::extract_book(ctx.storage.clone(), id) {
        Ok(path) => {
            let stat = ctx.stat.lock().unwrap();

            if let Err(err) = stat.save(id) {
                let msg = format!("{err}");
                error!("{}", msg);
                return Err(io::Error::new(io::ErrorKind::Other, msg));
            }
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
