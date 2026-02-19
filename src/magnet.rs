use std::collections::HashMap;

pub struct MagnetLink {
    pub tracker_url: Option<String>,
    pub info_hash: String,
    #[allow(dead_code)]
    pub name: Option<String>,
}

impl MagnetLink {
    pub fn parse(magnet_url: &str) -> Self {
        if !magnet_url.starts_with("magnet:?") {
            panic!("Invalid magnet link format");
        }

        let query_string = &magnet_url[8..];
        let params = parse_query_params(query_string);

        let info_hash = params
            .get("xt")
            .and_then(|xt| extract_info_hash(xt))
            .expect("Info hash (xt) is required");

        let tracker_url = params.get("tr").map(|tr| url_decode(tr));
        let name = params.get("dn").map(|dn| url_decode(dn));

        Self { tracker_url, info_hash, name }
    }
}

fn parse_query_params(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|param| {
            let mut parts = param.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next()?;
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn extract_info_hash(xt: &str) -> Option<String> {
    xt.strip_prefix("urn:btih:").map(|hash| hash.to_string())
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '%' => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                } else {
                    result.push('%');
                    result.push_str(&hex);
                }
            }
            '+' => result.push(' '),
            _ => result.push(ch),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_magnet_link_with_all_params() {
        let link = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        let magnet = MagnetLink::parse(link);

        assert_eq!(magnet.info_hash, "ad42ce8109f54c99613ce38f9b4d87e70f24a165");
        assert_eq!(
            magnet.tracker_url,
            Some("http://bittorrent-test-tracker.codecrafters.io/announce".to_string())
        );
        assert_eq!(magnet.name, Some("magnet1.gif".to_string()));
    }

    #[test]
    fn test_parse_magnet_link_minimal() {
        let link = "magnet:?xt=urn:btih:d69f91e6b2ae4c542468d1073a71d4ea13879a7f";
        let magnet = MagnetLink::parse(link);

        assert_eq!(magnet.info_hash, "d69f91e6b2ae4c542468d1073a71d4ea13879a7f");
        assert_eq!(magnet.tracker_url, None);
        assert_eq!(magnet.name, None);
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(
            url_decode("http%3A%2F%2Fexample.com%2Fannounce"),
            "http://example.com/announce"
        );
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("no%20encoding"), "no encoding");
    }

    #[test]
    fn test_parse_query_params() {
        let params = parse_query_params("key1=value1&key2=value2&key3=value3");
        assert_eq!(params.get("key1"), Some(&"value1".to_string()));
        assert_eq!(params.get("key2"), Some(&"value2".to_string()));
        assert_eq!(params.get("key3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_extract_info_hash() {
        let hash = extract_info_hash("urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165");
        assert_eq!(hash, Some("ad42ce8109f54c99613ce38f9b4d87e70f24a165".to_string()));

        let invalid = extract_info_hash("invalid");
        assert_eq!(invalid, None);
    }

    #[test]
    #[should_panic(expected = "Invalid magnet link format")]
    fn test_invalid_magnet_format() {
        MagnetLink::parse("not_a_magnet_link");
    }

    #[test]
    #[should_panic(expected = "Info hash (xt) is required")]
    fn test_missing_info_hash() {
        MagnetLink::parse("magnet:?dn=test.txt&tr=http://tracker.com");
    }
}
