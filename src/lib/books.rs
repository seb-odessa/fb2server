use std::fs;
use std::io;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use log::info;
use regex::Regex;

pub fn extract_book(root: PathBuf, id: u32) -> std::io::Result<PathBuf> {
    let rx = Regex::new("fb2-([0-9]+)-([0-9]+)")
        .map_err(|e| Error::new(ErrorKind::Other, format!("{e}")))?;
    let book_name = format!("{id}.fb2");
    info!("book_name: {book_name}");

    if root.is_dir() {
        for entry in fs::read_dir(&root)? {
            let path = entry?.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    let name = name.to_string_lossy();
                    if let Some(caps) = rx.captures(&name) {
                        let min = caps
                            .get(1)
                            .map_or("", |m| m.as_str())
                            .parse::<u32>()
                            .map_err(|err| Error::new(ErrorKind::Other, format!("{err}")))?;

                        let max = caps
                            .get(2)
                            .map_or("", |m| m.as_str())
                            .parse::<u32>()
                            .map_err(|err| Error::new(ErrorKind::Other, format!("{err}")))?;

                        if min <= id && id <= max {
                            let file = fs::File::open(&path)?;
                            let mut archive = zip::ZipArchive::new(file)?;
                            if let Ok(mut file) = archive.by_name(&book_name) {
                                let crc32 = file.crc32();
                                let size = file.size();
                                let outname = PathBuf::from(std::env::temp_dir())
                                    .join(format!("{crc32}"))
                                    .with_extension("fb2");
                                info!(
                                    "Found {} -> crc32: {crc32}, path: {} {size} B",
                                    file.name(),
                                    outname.display()
                                );
                                let mut outfile = fs::File::create(&outname)?;
                                io::copy(&mut file, &mut outfile)?;
                                return Ok(outname);
                            };
                        }
                    }
                }
            }
        }
    }
    Err(Error::new(
        ErrorKind::Other,
        format!("The book {id} was not found in {}", root.display()),
    ))
}
