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
    let mut pattern = path.into_inner();
    info!("/opds/authors/mask/{pattern}");

    let pool = ctx.pool.lock().unwrap();
    let mut feed = opds::Feed::new("Поиск книг по авторам");
    loop {
        match database::find_authors(&pool, &pattern).await {
            Ok(authors) => {
                for author in authors {
                    feed.add(fmt_name(&author), fmt_link(&author));
                }
            }
            Err(err) => return format!("{err}"),
        }

        match database::make_patterns(&pool, &pattern).await {
            Ok(patterns) => {
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
                        feed.add(format!("{prefix}..."), format!("/opds/authors/mask/{prefix}"));
                    }
                    break;
                }
            }
            Err(err) => return format!("{err}"),
        }
    }

    opds::format_feed(feed)
}

#[get("/opds/author/id/{fid}/{mid}/{lid}")]
async fn root_opds_author_id(
    ctx: web::Data<AppState>,
    path: web::Path<(u32, u32, u32)>,
) -> impl Responder {
    let (fid, mid, lid) = path.into_inner();
    info!("/opds/author/id/{fid}/{mid}/{lid}");

    let pool = ctx.pool.lock().unwrap();
    let mut feed = opds::Feed::new("Книги автора");

    opds::format_feed(feed)
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

fn fmt_name(author: &database::Author) -> String {
    if author.middle_name.is_empty() {
        format!("{} {}", author.first_name, author.last_name)
    } else {
        format!(
            "{} {} {} [{}/{}/{}]",
            author.first_name,
            author.middle_name,
            author.last_name,
            author.first_id,
            author.middle_id,
            author.last_id
        )
    }
}

fn fmt_link(author: &database::Author) -> String {
    format!(
        "/opds/author/id/{}/{}/{}",
        author.first_id, author.middle_id, author.last_id
    )
}
