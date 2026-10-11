//! Gutendex metadata and bounded UTF-8 text downloads.
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Book {
    pub id: u64,
    pub title: String,
    pub author: String,
    pub text_url: Option<String>,
}

pub fn parse_books(json: &str) -> Result<Vec<Book>, String> {
    let data: Value = serde_json::from_str(json).map_err(|_| "Invalid catalogue response")?;
    let results = data["results"]
        .as_array()
        .ok_or("Missing catalogue results")?;
    Ok(results
        .iter()
        .filter_map(|book| {
            if book["copyright"].as_bool() != Some(false) {
                return None;
            }
            let formats = book["formats"].as_object()?;
            let text_url = ["text/plain; charset=utf-8", "text/plain"]
                .iter()
                .find_map(|mime| formats.get(*mime).and_then(Value::as_str))
                .map(str::to_string);
            Some(Book {
                id: book["id"].as_u64()?,
                title: book["title"].as_str()?.into(),
                author: book["authors"]
                    .as_array()
                    .map(|authors| {
                        authors
                            .iter()
                            .filter_map(|a| a["name"].as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default(),
                text_url,
            })
        })
        .collect())
}

fn get_limited(
    req: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
    limit: u64,
    seconds: u64,
) -> Result<String, String> {
    let mut response = req
        .header(
            "User-Agent",
            "Ferrite-Folio/1.0 (+https://github.com/rwetz/ferrite-folio)",
        )
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(seconds)))
        .build()
        .call()
        .map_err(|error| format!("Book service request failed: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_string()
        .map_err(|_| "Response exceeds the download limit or isn't UTF-8 text".into())
}

pub fn search(query: &str) -> Result<Vec<Book>, String> {
    let primary = get_limited(
        ureq::get("https://gutendex.com/books/")
            .query("search", query)
            .query("languages", "en")
            .query("mime_type", "text/plain")
            .query("copyright", "false"),
        4 * 1024 * 1024,
        8,
    )
    .and_then(|json| parse_books(&json));
    match primary {
        Ok(books) => Ok(books),
        Err(primary_error) => {
            // Gutenberg publishes this catalogue for automated/offline use.
            // Cache it so a third-party API outage never makes the reader blank.
            let catalogue = catalogue()
                .map_err(|error| format!("{primary_error}. Gutenberg fallback: {error}"))?;
            search_catalogue(&catalogue, query)
        }
    }
}

fn catalogue() -> Result<String, String> {
    let path = std::env::var_os(if cfg!(windows) {
        "LOCALAPPDATA"
    } else {
        "HOME"
    })
    .map(std::path::PathBuf::from)
    .map(|base| base.join("ferrite/folio/pg_catalog.csv"));
    if let Some(path) = &path
        && let Ok(metadata) = std::fs::metadata(path)
        && metadata.len() <= 32 * 1024 * 1024
        && let Ok(text) = std::fs::read_to_string(path)
        && text.starts_with("Text#,Type,")
    {
        return Ok(text);
    }
    let text = get_limited(
        ureq::get("https://www.gutenberg.org/cache/epub/feeds/pg_catalog.csv"),
        32 * 1024 * 1024,
        25,
    )?;
    if !text.starts_with("Text#,Type,") {
        return Err("Invalid Gutenberg catalogue".into());
    }
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        if std::fs::write(&temporary, &text).is_ok() {
            let _ = std::fs::rename(&temporary, &path);
            let _ = std::fs::remove_file(temporary);
        }
    }
    Ok(text)
}

fn search_catalogue(text: &str, query: &str) -> Result<Vec<Book>, String> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| "Invalid catalogue headers")?
        .clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|value| value == name)
            .ok_or("Missing catalogue column")
    };
    let (id, kind, title, languages, authors) = (
        column("Text#")?,
        column("Type")?,
        column("Title")?,
        column("Language")?,
        column("Authors")?,
    );
    let query = query.to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    let featured = [1342, 84, 11, 1661, 98, 2701, 345, 2600, 1232, 174];
    let mut books = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|_| "Invalid catalogue record")?;
        if record.get(kind) != Some("Text")
            || !record
                .get(languages)
                .unwrap_or_default()
                .split(';')
                .any(|language| language.trim() == "en")
        {
            continue;
        }
        let Some(id) = record.get(id).and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };
        let title = record
            .get(title)
            .unwrap_or_default()
            .replace(['\r', '\n'], " ");
        let author = record.get(authors).unwrap_or_default().to_string();
        if terms.is_empty() {
            if !featured.contains(&id) {
                continue;
            }
        } else {
            let haystack = format!("{title} {author}").to_lowercase();
            if !terms.iter().all(|term| haystack.contains(term)) {
                continue;
            }
        }
        books.push(Book {
            id,
            title,
            author,
            text_url: Some(format!("https://www.gutenberg.org/ebooks/{id}.txt.utf-8")),
        });
        if !terms.is_empty() && books.len() >= 30 {
            break;
        }
    }
    if terms.is_empty() {
        books.sort_by_key(|book| {
            featured
                .iter()
                .position(|id| *id == book.id)
                .unwrap_or(usize::MAX)
        });
    }
    Ok(books)
}

pub fn download(book: &Book) -> Result<Vec<String>, String> {
    let url = book
        .text_url
        .as_deref()
        .ok_or("This book has no UTF-8 text edition")?;
    if !(url.starts_with("https://www.gutenberg.org/") || url.starts_with("https://gutenberg.org/"))
    {
        return Err("Unsupported text host".into());
    }
    let text = get_limited(ureq::get(url), 4 * 1024 * 1024, 20)?;
    if text.trim_start().starts_with("<!DOCTYPE html") || text.trim_start().starts_with("<html") {
        return Err(
            "The text service returned a web page instead of a book. Try another edition.".into(),
        );
    }
    Ok(pages(&text))
}

pub fn pages(text: &str) -> Vec<String> {
    // Split at Unicode scalar boundaries, preserving the complete source.
    let mut result = Vec::new();
    let mut start = 0;
    for (count, (at, _)) in text.char_indices().enumerate() {
        if count > 0 && count % 4000 == 0 {
            result.push(text[start..at].into());
            start = at;
        }
    }
    if start < text.len() {
        result.push(text[start..].into());
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_fallback_handles_quotes_languages_and_queries() {
        let csv = "Text#,Type,Issued,Title,Language,Authors\n1342,Text,2001-01-01,\"Pride, and Prejudice\",en,Jane Austen\n2,Text,2001-01-01,Other,fr,Someone\n3,Sound,2001-01-01,Audio,en,Austen\n";
        let books = search_catalogue(csv, "pride austen").unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, 1342);
        assert!(
            books[0]
                .text_url
                .as_ref()
                .unwrap()
                .contains("1342.txt.utf-8")
        );
        assert_eq!(search_catalogue(csv, "").unwrap().len(), 1);
        assert!(search_catalogue("broken", "").is_err());
    }
    #[test]
    fn pages_preserve_unicode_and_every_character() {
        let text = "é🙂\n".repeat(5000);
        let parts = pages(&text);
        assert_eq!(parts.concat(), text);
        assert!(parts.iter().all(|p| p.chars().count() <= 4000));
    }
    #[test]
    fn catalogue_filters_copyright_and_selects_text() {
        let books = parse_books(r#"{"results":[{"id":1,"title":"Test","copyright":false,"authors":[{"name":"Writer"}],"formats":{"text/plain; charset=utf-8":"https://www.gutenberg.org/test.txt"}},{"id":2,"title":"Skip","copyright":true,"formats":{}}]}"#).unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].author, "Writer");
        assert!(books[0].text_url.is_some());
        assert!(parse_books("{}").is_err());
    }
}
