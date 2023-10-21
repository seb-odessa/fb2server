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
use lib::impls;
use lib::opds;
use lib::utils;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE: &'static str = "sqlite://books.db?mode=ro";
const DEFAULT_STATISTIC: &'static str = "sqlite://statistic.db?mode=rwc";
const DEFAULT_LIBRARY: &'static str = "/lib.rus.ec";

struct AppState {
    pool: Mutex<SqlitePool>,
    statistic: Mutex<SqlitePool>,
    path: PathBuf,
    api: Mutex<Box<dyn database::LibraryProvider>>,
}
impl AppState {
    pub fn new(
        pool: SqlitePool,
        statistic: SqlitePool,
        path: PathBuf,
        api: Box<dyn database::LibraryProvider>,
    ) -> Self {
        Self {
            pool: Mutex::new(pool),
            statistic: Mutex::new(statistic),
            path: path,
            api: Mutex::new(api),
        }
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    let (addr, port, database, statistic, lib_root) = read_params();
    let pool = SqlitePool::connect(&database).await?;
    let statistic = SqlitePool::connect(&statistic).await?;
    database::init_statistic_db(&statistic).await?;

    let api = Box::new(database::LibRusEcOffline::new(pool.clone()));
    let app = AppState::new(pool, statistic, lib_root, api);
    let ctx = web::Data::new(app);

    info!("OPDS Server will ready at http://{addr}:{port}/opds");
    HttpServer::new(move || {
        App::new()
            .app_data(ctx.clone())
            .service(opds_root)
            .service(opds_not_impl)
            // Books by Authors
            .service(opds_authors_catalogue)
            .service(opds_authors_name_starts_with)
            .service(opds_author_root_by_id)
            .service(opds_books_by_author_in_serie)
            .service(opds_books_by_author_wo_serie)
            .service(opds_books_by_author_sorted_by_alphabet)
            .service(opds_books_by_author_sorted_by_date)
            // Books by Series
            .service(opds_series)
            .service(opds_series_name_starts_with)
            .service(opds_series_by_author)
            .service(opds_serie_root_by_id)
            .service(opds_serie_books)
            // Books by Genres
            .service(opds_meta)
            .service(opds_genres_by_meta)
            .service(opds_genre_authors_starts_with)
            .service(opds_genre_series_starts_with)
            .service(opds_genres_genre)
            .service(opds_genre_series)
            .service(opds_genres_authors)
            // Favorite Books
            .service(root_opds_favorite_authors)
            // Books
            .service(opds_book_by_id)
    })
    .bind((addr.as_str(), port))?
    .run()
    .await
    .map_err(anyhow::Error::from)
}

#[get("/opds/nimpl")]
async fn opds_not_impl() -> impl Responder {
    // Not Implemented placeholder
    let mut feed = opds::Feed::new("Not Implemented");
    feed.add("Пока не работает", "/opds/nimpl");

    opds::format_feed(feed)
}

#[get("/opds")]
async fn opds_root() -> impl Responder {
    info!("/opds");
    let mut feed = opds::Feed::new("Catalog Root");
    feed.add("Поиск книг по авторам", "/opds/authors");
    feed.add("Поиск книг по сериям", "/opds/series");
    feed.add("Поиск книг по жанрам", "/opds/meta");
    feed.add("Любимые авторы ", "/opds/favorites");

    opds::format_feed(feed)
}

#[get("/opds/authors")]
async fn opds_authors_catalogue(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/authors");

    let title = String::from("Поиск книг по авторам");
    let root = String::from("/opds/authors/mask");

    let api = ctx.api.lock().unwrap();
    let mut feed = opds::Feed::new(title);
    if let Ok((_, lines)) = api.get_authors_names_starts_with(&String::new()).await {
        impls::fill_feed_from(lines, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/series")]
async fn opds_series(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/series");

    let title = String::from("Поиск книг по сериям");
    let root = String::from("/opds/series/mask");

    let api = ctx.api.lock().unwrap();
    let mut feed = opds::Feed::new(title);
    if let Ok((_, lines)) = api.get_series_names_starts_with(&String::new()).await {
        impls::fill_feed_from(lines, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/meta")]
async fn opds_meta(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/meta");

    let title = String::from("Поиск книг жанрам");
    let root = String::from("/opds/genres");
    let api = ctx.api.lock().unwrap();
    let lines = api.get_meta_genres().await;
    let feed = impls::make_feed_from(lines, title, root);

    opds::handle_feed(feed)
}

#[get("/opds/genres/{meta}")]
async fn opds_genres_by_meta(ctx: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let meta = utils::decode_with_lossy(path.as_str());
    info!("/opds/genres/{} - {meta}", path.as_str());

    let title = String::from("Поиск книг жанрам");
    let root = String::from("/opds/genre");
    let api = ctx.api.lock().unwrap();
    let qp = database::GenresByMeta::new(&meta);
    let lines = api.get_genres_by_meta(qp).await;
    let feed = impls::make_feed_from(lines, title, root);

    opds::handle_feed(feed)
}

#[get("/opds/genre/{genre}")]
async fn opds_genres_genre(path: web::Path<String>) -> impl Responder {
    let genre = utils::decode_with_lossy(path.as_str());
    info!("/opds/genres/{} - {genre}", path.as_str());

    use impls::FeedSrc;

    let mut feed = opds::Feed::new(format!("Книги в жанре '{genre}'"));
    impls::add_to_feed(
        FeedSrc::new("По авторам", "/opds/genre/authors", &genre),
        &mut feed,
    );
    impls::add_to_feed(
        FeedSrc::new("По сериям", "/opds/genre/series", &genre),
        &mut feed,
    );
    impls::add_to_feed(
        FeedSrc::new("По дате (not impl)", "/opds/genre/date", &genre),
        &mut feed,
    );
    opds::format_feed(feed)
}

#[get("/opds/genre/series/{genre}")]
async fn opds_genre_series(ctx: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let genre = utils::decode_with_lossy(path.as_str());
    info!("/opds/genre/series/{} - {genre}", path.as_str());

    let title = format!("Поиск книг по сериям в жанре {genre}");
    let root = format!("/opds/genre/series/{genre}");

    let mut feed = opds::Feed::new(title);
    let api = ctx.api.lock().unwrap();

    if let Ok((_, lines)) = api
        .get_series_names_by_genre_starts_with(&genre, &String::new())
        .await
    {
        impls::fill_feed_from(lines, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/genre/series/{genre}/{mask}")]
async fn opds_genre_series_starts_with(
    ctx: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (genre, mask_encoded) = path.into_inner();

    let mask = utils::decode_with_lossy(mask_encoded.as_str());
    info!("/opds/genre/series/{genre}/{mask_encoded} - {mask}");

    let title = format!("Поиск книг по сериям в жанре {genre}");
    let root = format!("/opds/genre/series/{genre}");

    let mut feed = opds::Feed::new(title);

    let api = ctx.api.lock().unwrap();
    if let Ok((exact, rest)) = api
        .get_series_names_by_genre_starts_with(&genre, &mask)
        .await
    {
        for mask in utils::sorted(exact) {
            info!("mask: {:?}", mask);
            let qp = database::SeriesByName::new(&mask);
            if let Ok(mut series) = api.get_series_by_name(qp).await {
                series.sort_by(|a, b| utils::fb2sort(&a.value, &b.value));

                for serie in series {
                    let desc = format!("{}", serie.value);
                    let link = format!("/opds/serie/id/{}", serie.id);

                    info!("{desc} -> {link}");
                    feed.add(desc, link);
                }
            }
        }
        impls::fill_feed_from(rest, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/genre/authors/{genre}")]
async fn opds_genres_authors(ctx: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let genre = utils::decode_with_lossy(path.as_str());
    info!("/opds/genre/authors/{} - {genre}", path.as_str());

    let title = format!("Поиск книг по авторам в жанре {genre}");
    let root = format!("/opds/genre/authors/{genre}");

    let mut feed = opds::Feed::new(title);
    let api = ctx.api.lock().unwrap();

    if let Ok((_, lines)) = api
        .get_authors_names_by_genre_starts_with(&genre, &String::new())
        .await
    {
        impls::fill_feed_from(lines, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/genre/authors/{genre}/{mask}")]
async fn opds_genre_authors_starts_with(
    ctx: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (genre, mask_encoded) = path.into_inner();

    let mask = utils::decode_with_lossy(mask_encoded.as_str());
    info!("/opds/genre/authors/{genre}/{mask_encoded} - {mask}");

    let title = format!("Поиск книг по сериям в жанре {genre}");
    let root = format!("/opds/genre/authors/{genre}");

    let mut feed = opds::Feed::new(title);

    let api = ctx.api.lock().unwrap();
    if let Ok((exact, rest)) = api
        .get_authors_names_by_genre_starts_with(&genre, &mask)
        .await
    {
        for mask in utils::sorted(exact) {
            let qp = database::AuthorsByLastName::new(&mask);
            if let Ok(mut authors) = api.get_authors_by_last_name(qp).await {
                authors.sort_by(|a, b| utils::fb2sort(&a.first_name.value, &b.first_name.value));
                for author in authors {
                    let desc = format!(
                        "{} {} {}",
                        author.last_name.value, author.first_name.value, author.middle_name.value,
                    );
                    let link = format!(
                        "/opds/author/id/{}/{}/{}",
                        author.first_name.id, author.middle_name.id, author.last_name.id
                    );
                    feed.add(desc, link);
                }
            }
        }
        impls::fill_feed_from(rest, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/favorites")]
async fn root_opds_favorite_authors(ctx: web::Data<AppState>) -> impl Responder {
    info!("/opds/favorites");

    let books = ctx.pool.lock().unwrap();
    let stats = ctx.statistic.lock().unwrap();
    let feed = impls::root_opds_favorite_authors(&books, &stats).await;
    opds::handle_feed(feed)
}

#[get("/opds/authors/mask/{pattern}")]
async fn opds_authors_name_starts_with(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let pattern = utils::decode_with_lossy(path.as_str());
    info!("/opds/authors/mask/<{pattern}>");

    let title = String::from("Поиск книг по авторам");
    let root = String::from("/opds/authors/mask");

    let mut feed = opds::Feed::new(title);
    let api = ctx.api.lock().unwrap();
    let maybe_substrings = api.get_authors_names_starts_with(&pattern).await;

    if let Ok((exact, rest)) = maybe_substrings {
        for mask in utils::sorted(exact) {
            let qp = database::AuthorsByLastName::new(&mask);
            if let Ok(mut authors) = api.get_authors_by_last_name(qp).await {
                authors.sort_by(|a, b| utils::fb2sort(&a.first_name.value, &b.first_name.value));
                for author in authors {
                    let desc = format!(
                        "{} {} {}",
                        author.last_name.value, author.first_name.value, author.middle_name.value,
                    );
                    let link = format!(
                        "/opds/author/id/{}/{}/{}",
                        author.first_name.id, author.middle_name.id, author.last_name.id
                    );
                    feed.add(desc, link);
                }
            }
        }

        impls::fill_feed_from(rest, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/series/mask/{pattern}")]
async fn opds_series_name_starts_with(
    ctx: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let pattern = utils::decode_with_lossy(path.as_str());
    info!("/opds/series/mask/{} - {pattern}", path.as_str());

    let title = String::from("Поиск книг сериям");
    let root = String::from("/opds/series/mask");

    let mut feed = opds::Feed::new(title);
    let api = ctx.api.lock().unwrap();
    let maybe_substrings = api.get_series_names_starts_with(&pattern).await;

    if let Ok((exact, rest)) = maybe_substrings {
        for mask in utils::sorted(exact) {
            info!("mask: {:?}", mask);
            let qp = database::SeriesByName::new(&mask);
            if let Ok(mut series) = api.get_series_by_name(qp).await {
                series.sort_by(|a, b| utils::fb2sort(&a.value, &b.value));

                for serie in series {
                    let desc = format!("{}", serie.value);
                    let link = format!("/opds/serie/id/{}", serie.id);

                    info!("{desc} -> {link}");
                    feed.add(desc, link);
                }
            }
        }
        impls::fill_feed_from(rest, root, &mut feed);
    }

    opds::format_feed(feed)
}

#[get("/opds/author/id/{fid}/{mid}/{lid}")]
async fn opds_author_root_by_id(path: web::Path<(u32, u32, u32)>) -> impl Responder {
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
    opds::format_feed(feed)
}

#[get("/opds/serie/id/{id}")]
async fn opds_serie_root_by_id(path: web::Path<u32>) -> impl Responder {
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
    opds::format_feed(feed)
}

#[get("/opds/serie/books/{id}/{sort}")]
async fn opds_serie_books(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, String)>,
) -> impl Responder {
    let (id, sort) = path.into_inner();
    info!("/opds/serie/{id}/{sort}");

    let mut feed = opds::Feed::new("Книги в серии");
    let api = ctx.api.lock().unwrap();
    let sid = database::BooksBySerieId::new(id);
    if let Ok(mut values) = api.books_by_serie_id(sid).await {
        match sort.as_str() {
            "numbered" => values.sort_by(|a, b| a.num.cmp(&b.num)),
            "alphabet" => values.sort_by(|a, b| utils::fb2sort(&a.name, &b.name)),
            "added" => values.sort_by(|a, b| utils::fb2sort(&a.date, &b.date)),
            _ => {}
        }
        for book in values {
            let id = book.id;
            let num = book.num;
            let name = book.name;
            let date = book.date;
            let author = book.author;
            feed.book(
                format!("{num} {name} - {author} ({date})"),
                format!("/opds/book/{id}"),
            );
        }
    }

    opds::format_feed(feed)
}

#[get("/opds/author/series/{fid}/{mid}/{lid}")]
async fn opds_series_by_author(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/series/{fid}/{mid}/{lid}");

    let ids = database::AuthorIds::from((fid, mid, lid));
    let api = ctx.api.lock().unwrap();

    let author = match api.author_full_name(ids.clone()).await {
        Ok(author_full_name) => author_full_name,
        Err(err) => format!("{}", err),
    };

    let mut feed = opds::Feed::new(&author);
    if let Ok(mut series) = api.series_by_author(ids).await {
        series.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));
        for serie in series {
            let sid = serie.id;
            let name = serie.name;
            let count = serie.count;
            feed.add(
                format!("{name} [{count} книг]"),
                format!("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}"),
            );
        }
        feed.add(author, format!("/opds/author/id/{fid}/{mid}/{lid}"));
    }
    opds::format_feed(feed)
}

#[get("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}")]
async fn opds_books_by_author_in_serie(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid, sid) = path.into_inner();
    info!("/opds/author/serie/books/{fid}/{mid}/{lid}/{sid}");

    let author = database::AuthorIds::from((fid, mid, lid));
    let filter = database::BooksFilter::ByTheSerieOnly(sid);
    let author_query = database::BooksByAuthor::new(author.clone(), filter);

    let mut feed = opds::Feed::new("Книги");
    let api = ctx.api.lock().unwrap();
    if let Ok(mut books) = api.books_by_author(author_query).await {
        books.sort_unstable_by_key(|book| (book.num, book.name.clone(), book.date.clone()));
        for book in books {
            let id = book.id;
            let num = book.num;
            let name = book.name;
            let date = book.date;
            let title = format!("{num} - {name} ({date})");
            let link = format!("/opds/book/{id}");
            feed.book(title, link);
        }
    }
    opds::format_feed(feed)
}

#[get("/opds/author/nonserie/books/{fid}/{mid}/{lid}")]
async fn opds_books_by_author_wo_serie(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/nonserie/books/{fid}/{mid}/{lid}");

    let author = database::AuthorIds::from((fid, mid, lid));
    let filter = database::BooksFilter::WithoutSerieOnly;
    let author_query = database::BooksByAuthor::new(author.clone(), filter);

    let mut feed = opds::Feed::new("Книги");
    let api = ctx.api.lock().unwrap();
    if let Ok(mut books) = api.books_by_author(author_query).await {
        books.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

        for book in books {
            let id = book.id;
            let num = book.num;
            let name = book.name;
            let date = book.date;
            let title = format!("{num} - {name} ({date})");
            let link = format!("/opds/book/{id}");
            feed.book(title, link);
        }
    }
    opds::format_feed(feed)
}

#[get("/opds/author/alphabet/books/{fid}/{mid}/{lid}")]
async fn opds_books_by_author_sorted_by_alphabet(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/alphabet/books/{fid}/{mid}/{lid}");

    let author = database::AuthorIds::from((fid, mid, lid));
    let filter = database::BooksFilter::All;
    let author_query = database::BooksByAuthor::new(author.clone(), filter);

    let mut feed = opds::Feed::new("Книги");
    let api = ctx.api.lock().unwrap();
    if let Ok(mut books) = api.books_by_author(author_query).await {
        books.sort_by(|a, b| utils::fb2sort(&a.name, &b.name));

        for book in books {
            let id = book.id;
            let num = book.num;
            let name = book.name;
            let date = book.date;
            let title = format!("{num} - {name} ({date})");
            let link = format!("/opds/book/{id}");
            feed.book(title, link);
        }
    }
    opds::format_feed(feed)
}

#[get("/opds/author/added/books/{fid}/{mid}/{lid}")]
async fn opds_books_by_author_sorted_by_date(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/added/books/{fid}/{mid}/{lid}");

    let author = database::AuthorIds::from((fid, mid, lid));
    let filter = database::BooksFilter::All;
    let author_query = database::BooksByAuthor::new(author.clone(), filter);

    let mut feed = opds::Feed::new("Книги");
    let api = ctx.api.lock().unwrap();
    if let Ok(mut books) = api.books_by_author(author_query).await {
        books.sort_by(|a, b| b.date.cmp(&a.date));

        for book in books {
            let id = book.id;
            let num = book.num;
            let name = book.name;
            let date = book.date;
            let title = format!("{num} - {name} ({date})");
            let link = format!("/opds/book/{id}");
            feed.book(title, link);
        }
    }
    opds::format_feed(feed)
}

#[get("/opds/book/{id}")]
async fn opds_book_by_id(
    ctx: web::Data<AppState>,
    param: web::Path<u32>,
) -> std::io::Result<NamedFile> {
    let id = param.into_inner();
    info!("/opds/book/{id})");

    match impls::extract_book(ctx.path.clone(), id) {
        Ok(book) => {
            let pool = ctx.statistic.lock().unwrap();

            if let Err(err) = database::insert_book(&pool, id).await {
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
