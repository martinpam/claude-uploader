use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;

#[derive(Clone, Default)]
pub struct CurlParser {
    pub headers: Option<HeaderMap>,
    pub organization_id: Option<String>,
    pub project_id: Option<String>,
}

impl CurlParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(&mut self, curl_text: &str) -> Result<(), String> {
        // Extract organization ID
        let org_id = curl_text
            .find("/organizations/")
            .and_then(|start_idx| {
                let start = start_idx + "/organizations/".len();
                let remaining = &curl_text[start..];
                remaining
                    .find('/')
                    .map(|end_idx| remaining[..end_idx].to_string())
            })
            .ok_or("Could not find organization ID in curl command".to_string())?;

        // Extract project ID
        let proj_id = curl_text
            .find("/projects/")
            .and_then(|start_idx| {
                let start = start_idx + "/projects/".len();
                let remaining = &curl_text[start..];
                remaining
                    .find('/')
                    .map(|end_idx| remaining[..end_idx].to_string())
            })
            .ok_or("Could not find project ID in curl command".to_string())?;

        // Extract headers
        let mut headers = HeaderMap::new();
        for line in curl_text.lines() {
            if !line.starts_with("  -H '") && !line.starts_with(" -H '") {
                continue;
            }

            let content = line
                .trim_start_matches("  -H '")
                .trim_start_matches(" -H '")
                .trim_end_matches('\'')
                .to_string();

            let parts: Vec<&str> = content.split(": ").collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0].to_lowercase();
            let value = parts[1];

            if let Ok(header_name) = HeaderName::from_str(&key) {
                if let Ok(header_value) = HeaderValue::from_str(value) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        // Extract cookies separately if needed
        if !headers.contains_key("cookie") {
            for line in curl_text.lines() {
                if line.contains("--cookie") || line.contains("-b ") {
                    if let Some(cookie_start) = line.find('\'') {
                        if let Some(cookie_end) = line[cookie_start + 1..].find('\'') {
                            let cookie_value = &line[cookie_start + 1..cookie_start + 1 + cookie_end];
                            if let Ok(header_value) = HeaderValue::from_str(cookie_value) {
                                headers.insert(HeaderName::from_static("cookie"), header_value);
                            }
                        }
                    }
                }
            }
        }

        // Add essential headers
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            HeaderName::from_static("origin"),
            HeaderValue::from_static("https://claude.ai"),
        );
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_str(&format!("https://claude.ai/project/{}", proj_id)).unwrap(),
        );
        
        // Make sure user-agent is set
        if !headers.contains_key("user-agent") {
            headers.insert(
                HeaderName::from_static("user-agent"),
                HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"),
            );
        }

        self.organization_id = Some(org_id);
        self.project_id = Some(proj_id);
        self.headers = Some(headers);

        Ok(())
    }
}