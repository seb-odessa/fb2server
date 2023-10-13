use actix_web::{get, App, HttpServer, Responder};
use chrono;
use log::{info, warn};
use quick_xml::events::{BytesText, Event};
use quick_xml::writer::Writer;

use std::convert::Into;
use std::io::Cursor;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const XML_HEAD: &'static str = r#"xml version="1.0" encoding="utf-8""#;

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

#[get("/opds")]
async fn opds() -> impl Responder {
    info!("opds");
    let mut feed = Feed::new("Catalog Root");
    feed.add("Поиск книг по авторам", "/opds/authors");
    feed.add("Поиск книг по сериям", "/opds/series");
    feed.add("Поиск книг по жанрам", "/opds/genres");

    match make_feed(feed) {
        Ok(xml) => xml,
        Err(err) => format!("{err}"),
    }
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let (addr, port) = read_params();

    info!("Try http://{addr}:{port}/opds");

    HttpServer::new(|| App::new().service(opds))
        .bind((addr.as_str(), port))?
        .run()
        .await
        .map_err(anyhow::Error::from)
}

/*********************************************************************************/

fn read_params() -> (String, u16) {
    let addr = match std::env::var("FB2SERVER") {
        Ok(addr) => addr,
        Err(e) => {
            warn!("FB2SERVER: {e}");
            String::from(DEFAULT_ADDRESS)
        }
    };

    let port = match std::env::var("FB2PORT") {
        Ok(string) => string.as_str().parse::<u16>().unwrap_or(DEFAULT_PORT),
        Err(e) => {
            warn!("FB2PORT: {e}");
            DEFAULT_PORT
        }
    };

    return (addr, port);
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
