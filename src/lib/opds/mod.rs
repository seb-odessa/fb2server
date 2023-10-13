use chrono;
use quick_xml::events::{BytesText, Event};
use quick_xml::writer::Writer;

use std::io::Cursor;

const XML_HEAD: &'static str = r#"xml version="1.0" encoding="utf-8""#;
pub struct Entry {
    pub id: String,
    pub title: String,
    pub link: String,
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
        let entry = Entry::new(title, link);
        self.entries.push(entry);
    }
}

pub fn format_feed(feed: Feed) -> String {
    match make_feed(feed) {
        Ok(xml) => xml,
        Err(err) => format!("{err}"),
    }
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
