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
    let results = data["results"].as_array().ok_or("Missing catalogue results")?;
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
                    .map(|authors| authors.iter().filter_map(|a| a["name"].as_str()).collect::<Vec<_>>().join(", "))
                    .unwrap_or_default(),
                text_url,
            })
        })
        .collect())
}

fn get(req: ureq::RequestBuilder<ureq::typestate::WithoutBody>) -> Result<String, String> {
    let mut response = req
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .build()
        .call()
        .map_err(|_| "Couldn't reach the book service")?;
    response.body_mut().with_config().limit(4 * 1024 * 1024).read_to_string().map_err(|_| "Book exceeds 4 MB or isn't UTF-8 text".into())
}

pub fn search(query: &str) -> Result<Vec<Book>, String> {
    parse_books(&get(ureq::get("https://gutendex.com/books/")
        .query("search", query)
        .query("languages", "en")
        .query("mime_type", "text/plain")
        .query("copyright", "false"))?)
}

pub fn download(book: &Book) -> Result<Vec<String>, String> {
    let url = book.text_url.as_deref().ok_or("This book has no UTF-8 text edition")?;
    if !(url.starts_with("https://www.gutenberg.org/") || url.starts_with("https://gutenberg.org/")) {
        return Err("Unsupported text host".into());
    }
    let text = get(ureq::get(url))?;
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
