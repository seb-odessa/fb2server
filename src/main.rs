use actix_web::{get, web, App, HttpServer, Responder};
use chrono;
use log::{info, warn};
use quick_xml::events::{BytesText, Event};
use quick_xml::writer::Writer;
use std::io::Cursor;

const DEFAULT_ADDRESS: &'static str = "localhost";
const DEFAULT_PORT: u16 = 8080;
const XML_HEAD: &'static str = r#"xml version="1.0" encoding="utf-8""#;

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    info!("Name: {name}");
    format!("Hello {name}!")
}

fn make_root_xml() -> anyhow::Result<Vec<u8>> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    // <?xml version="1.0" encoding="utf-8"?>
    writer
        .write_event(Event::PI(BytesText::from_escaped(XML_HEAD)))
        .unwrap();

    writer
        .create_element("feed")
        .with_attribute(("xmlns", "http://www.w3.org/2005/Atom"))
        .with_attribute(("xmlns:dc", "http://purl.org/dc/terms/"))
        .with_attribute(("xmlns:os", "http://a9.com/-/spec/opensearch/1.1/"))
        .with_attribute(("xmlns:opds", "http://opds-spec.org/2010/catalog"))
        .write_inner_content(|w| {
            w.create_element("title")
                .write_text_content(BytesText::new("Catalog fb2"))?;

            let updated = format!("{:?}", chrono::Utc::now());
            w.create_element("updated")
                .write_text_content(BytesText::new(&updated))?;

            w.create_element("link")
                .with_attribute(("href", "/opds"))
                .with_attribute(("rel", "/start"))
                .with_attribute(("type", "application/atom+xml;profile=opds-catalog"))
                .write_empty()?;

            w.create_element("link")
                .with_attribute(("href", "/opds"))
                .with_attribute(("self", "/start"))
                .with_attribute(("type", "application/atom+xml;profile=opds-catalog"))
                .write_empty()?;

            w.create_element("entry").write_inner_content(|w| {
                w.create_element("id")
                    .write_text_content(BytesText::new("tag:root:authors"))?;

                w.create_element("title")
                    .write_text_content(BytesText::new("По авторам"))?;

                w.create_element("link")
                    .with_attribute(("href", "/opds/authors"))
                    .with_attribute(("type", "application/atom+xml;profile=opds-catalog"))
                    .write_empty()?;

                Ok(())
            })?;

            Ok(())
        })?;

    return Ok(writer.into_inner().into_inner());
}

#[get("/opds")]
async fn root() -> impl Responder {
    info!("opds");
    match make_root_xml() {
        Ok(vec) => String::from_utf8_lossy(&vec).into_owned(),
        Err(err) => format!("{err}"),
    }
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let (addr, port) = read_params();

    info!("Try http://{addr}:{port}/opds");

    HttpServer::new(|| App::new().service(greet).service(root))
        .bind((addr.as_str(), port))?
        .run()
        .await
        .map_err(anyhow::Error::from)
}

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
