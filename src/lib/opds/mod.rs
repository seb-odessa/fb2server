use actix_web::Responder;
use chrono;
use log::error;
use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::writer::Writer;

use std::io::Cursor;

#[derive(Debug)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub href: String,
    pub htype: String,
}
impl Entry {
    pub fn catalog<T: Into<String>>(title: T, link: T) -> Self {
        let href = link.into();
        let id = String::from("root") + &href.clone().as_str().replace("/", ":");
        Self {
            id: id.into(),
            title: title.into(),
            href: href.into(),
            htype: String::from("application/atom+xml;profile=opds-catalog"),
        }
    }

    pub fn book<T: Into<String>>(title: T, link: T) -> Self {
        let href = link.into();
        let id = String::from("root") + &href.clone().as_str().replace("/", ":");
        Self {
            id: id.into(),
            title: title.into(),
            href: href.into(),
            htype: String::from("application/fb2+zip"),
        }
    }
}

#[derive(Debug)]
pub struct Feed {
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
        let entry = Entry::catalog(title, link);
        self.entries.push(entry);
    }

    pub fn book<T: Into<String>>(&mut self, title: T, link: T) {
        let entry = Entry::book(title, link);
        self.entries.push(entry);
    }
}

fn format_feed(feed: Feed) -> String {
    match make_feed(feed) {
        Ok(xml) => xml,
        Err(err) => format!("{err}"),
    }
}

pub fn handle_feed(feed_result: anyhow::Result<Feed>) -> impl Responder {
    match feed_result {
        Ok(feed) => format_feed(feed),
        Err(err) => {
            let msg = format!("{}", err);
            error!("failure: {}", msg);
            return msg;
        }
    }
}

fn make_feed(feed: Feed) -> anyhow::Result<String> {
    let mut w = Writer::new(Cursor::new(Vec::new()));

    const XML_VERSION: &'static str = "1.0";
    const XML_ENCODING: &'static str = "utf-8";

    w.write_event(Event::Decl(BytesDecl::new(
        XML_VERSION,
        Some(XML_ENCODING),
        None,
    )))?;

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
                        .with_attribute(("href", entry.href.as_str()))
                        .with_attribute(("type", entry.htype.as_str()))
                        .write_empty()?;

                    Ok::<(), quick_xml::Error>(())
                })?;
            }

            Ok::<(), quick_xml::Error>(())
        })?;

    Ok(String::from_utf8_lossy(&w.into_inner().into_inner()).into_owned())
}
