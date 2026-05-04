use anyhow::{Context, Result};
use std::time::Duration;
use url::form_urlencoded;

#[derive(Clone)]
pub struct HatenaClient {
    agent: ureq::Agent,
}

impl HatenaClient {
    pub fn new(timeout: Duration) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout(timeout)
            .user_agent("discord_news_notify/0.1")
            .build();

        Self { agent }
    }

    pub async fn fetch_count(&self, url: &str) -> Result<u64> {
        let agent = self.agent.clone();
        let url = url.to_string();

        tokio::task::spawn_blocking(move || fetch_count_blocking(agent, &url)).await?
    }
}

fn fetch_count_blocking(agent: ureq::Agent, url: &str) -> Result<u64> {
    let api_url = count_api_url(url);
    let response = agent.get(&api_url).call().map_err(|e| match e {
        ureq::Error::Status(status, _) => {
            anyhow::anyhow!("Hatena count API returned HTTP status {}", status)
        }
        ureq::Error::Transport(error) => {
            anyhow::anyhow!("Hatena count API request failed: {}", error)
        }
    })?;
    let body = response
        .into_string()
        .context("failed to read Hatena count API response")?;

    parse_count_response(&body)
}

fn count_api_url(url: &str) -> String {
    let encoded = form_urlencoded::byte_serialize(url.as_bytes()).collect::<String>();
    format!("https://bookmark.hatenaapis.com/count/entry?url={encoded}")
}

fn parse_count_response(body: &str) -> Result<u64> {
    let value = body.trim();
    if value.is_empty() {
        return Err(anyhow::anyhow!("Hatena count API returned empty response"));
    }

    value
        .parse::<u64>()
        .with_context(|| format!("Hatena count API returned non-numeric response: {value:?}"))
}

#[cfg(test)]
mod tests {
    use super::{count_api_url, parse_count_response};

    #[test]
    fn count_api_url_encodes_article_url() {
        let url = count_api_url("https://example.com/a b?x=1&y=2");

        assert_eq!(
            url,
            "https://bookmark.hatenaapis.com/count/entry?url=https%3A%2F%2Fexample.com%2Fa+b%3Fx%3D1%26y%3D2"
        );
    }

    #[test]
    fn parse_count_response_accepts_number() {
        assert_eq!(parse_count_response(" 123\n").unwrap(), 123);
    }

    #[test]
    fn parse_count_response_rejects_empty_body() {
        let err = parse_count_response(" ").unwrap_err().to_string();

        assert!(err.contains("empty"));
    }

    #[test]
    fn parse_count_response_rejects_non_numeric_body() {
        let err = parse_count_response("not-number").unwrap_err().to_string();

        assert!(err.contains("non-numeric"));
    }
}
