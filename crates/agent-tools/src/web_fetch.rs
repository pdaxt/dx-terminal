use serde::{Deserialize, Serialize};

use dx_agent_core::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct WebFetchInput {
    pub url: String,
    pub max_length: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebFetchOutput {
    pub url: String,
    pub status: u16,
    pub content: String,
    pub content_type: String,
    pub truncated: bool,
}

pub fn web_fetch(input: WebFetchInput) -> Result<WebFetchOutput> {
    let max_length = input.max_length.unwrap_or(50_000);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("DXOS/0.1 (https://github.com/pdaxt/dxos)")
        .build()
        .map_err(|e| dx_agent_core::DxosError::Tool {
            tool: "web_fetch".to_string(),
            message: e.to_string(),
        })?;

    let response = client
        .get(&input.url)
        .send()
        .map_err(|e| dx_agent_core::DxosError::Tool {
            tool: "web_fetch".to_string(),
            message: format!("Failed to fetch {}: {e}", input.url),
        })?;

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let body = response
        .text()
        .map_err(|e| dx_agent_core::DxosError::Tool {
            tool: "web_fetch".to_string(),
            message: format!("Failed to read response: {e}"),
        })?;

    // Strip HTML tags for cleaner output (simple regex)
    let content = if content_type.contains("text/html") {
        strip_html(&body)
    } else {
        body
    };

    let truncated = content.len() > max_length;
    let content = if truncated {
        content[..max_length].to_string()
    } else {
        content
    };

    Ok(WebFetchOutput {
        url: input.url,
        status,
        content,
        content_type,
        truncated,
    })
}

/// Simple HTML tag stripper — removes tags, decodes common entities.
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if !in_tag && i + 7 < lower_chars.len() {
            let ahead: String = lower_chars[i..i + 7].iter().collect();
            if ahead == "<script" {
                in_script = true;
            }
            if ahead == "<style "
                || (i + 6 < lower_chars.len()
                    && lower_chars[i..i + 6].iter().collect::<String>() == "<style")
            {
                in_style = true;
            }
        }

        if chars[i] == '<' {
            in_tag = true;
            // Check for end of script/style
            if i + 9 < lower_chars.len() {
                let ahead: String = lower_chars[i..i + 9].iter().collect();
                if ahead == "</script>" {
                    in_script = false;
                }
            }
            if i + 8 < lower_chars.len() {
                let ahead: String = lower_chars[i..i + 8].iter().collect();
                if ahead == "</style>" {
                    in_style = false;
                }
            }
        } else if chars[i] == '>' {
            in_tag = false;
        } else if !in_tag && !in_script && !in_style {
            result.push(chars[i]);
        }

        i += 1;
    }

    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        // Collapse whitespace
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
