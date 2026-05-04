use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{info, warn};
use rss::Channel;
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct RssItem {
    pub guid: Option<String>,
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: Option<DateTime<Utc>>,
    pub feed_title: Option<String>,
}

pub struct RssProcessor;

impl RssProcessor {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_items(&self, feed_url: &str) -> Result<Vec<RssItem>> {
        let feed_url = feed_url.to_string();

        tokio::task::spawn_blocking(move || fetch_items_blocking(&feed_url)).await?
    }
}

fn fetch_items_blocking(feed_url: &str) -> Result<Vec<RssItem>> {
    info!("Fetching RSS feed: {}", feed_url);

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .user_agent("discord_news_notify/0.1")
        .build();

    let response = agent.get(feed_url).call().map_err(|e| match e {
        ureq::Error::Status(status, _) => {
            anyhow::anyhow!(
                "failed to fetch RSS feed {}: HTTP status {}",
                feed_url,
                status
            )
        }
        ureq::Error::Transport(error) => {
            anyhow::anyhow!("failed to fetch RSS feed {}: {}", feed_url, error)
        }
    })?;
    info!("RSS feed response received: {}", feed_url);

    let text = response.into_string()?;
    info!("RSS feed body loaded: {} bytes", text.len());

    let channel = Channel::read_from(BufReader::new(text.as_bytes()))?;
    let feed_title = channel.title().trim();
    info!(
        "RSS feed parsed: title={:?}, raw_items={}",
        feed_title,
        channel.items().len()
    );
    let feed_title = if feed_title.is_empty() {
        None
    } else {
        Some(feed_title.to_string())
    };
    let mut items = Vec::new();

    for item in channel.items() {
        let guid = item.guid().map(|guid| guid.value().to_string());
        let title = item.title().unwrap_or("No Title").to_string();
        let link = item.link().unwrap_or_default().to_string();
        let description = item
            .description()
            .unwrap_or("No description available")
            .to_string();
        let pub_date = parse_pub_date(item.pub_date().unwrap_or_default());

        items.push(RssItem {
            guid,
            title,
            link,
            description,
            pub_date,
            feed_title: feed_title.clone(),
        });
    }

    Ok(items)
}

fn parse_pub_date(value: &str) -> Option<DateTime<Utc>> {
    if value.trim().is_empty() {
        return None;
    }

    match DateTime::parse_from_rfc2822(value) {
        Ok(date) => Some(date.with_timezone(&Utc)),
        Err(e) => {
            warn!("Failed to parse RSS pub_date {:?}: {}", value, e);
            None
        }
    }
}

pub fn item_id(item: &RssItem) -> String {
    if let Some(guid) = item.guid.as_deref().filter(|guid| !guid.trim().is_empty()) {
        return format!("guid:{guid}");
    }

    if !item.link.trim().is_empty() {
        return format!("link:{}", item.link);
    }

    let pub_date = item
        .pub_date
        .map(|date| date.to_rfc3339())
        .unwrap_or_else(|| "unknown-date".to_string());

    format!("fallback:{}:{pub_date}", item.title)
}

#[cfg(test)]
mod tests {
    use super::{RssItem, item_id, parse_pub_date};

    #[test]
    fn parse_pub_date_returns_none_for_invalid_date() {
        assert!(parse_pub_date("not a date").is_none());
    }

    #[test]
    fn parse_pub_date_parses_rfc2822_date() {
        assert!(parse_pub_date("Mon, 04 May 2026 12:00:00 +0900").is_some());
    }

    #[test]
    fn item_id_prefers_guid() {
        let item = RssItem {
            guid: Some("guid-1".to_string()),
            title: "Title".to_string(),
            link: "https://example.com/item".to_string(),
            description: String::new(),
            pub_date: None,
            feed_title: None,
        };

        assert_eq!(item_id(&item), "guid:guid-1");
    }

    #[test]
    fn item_id_uses_link_without_guid() {
        let item = RssItem {
            guid: None,
            title: "Title".to_string(),
            link: "https://example.com/item".to_string(),
            description: String::new(),
            pub_date: None,
            feed_title: None,
        };

        assert_eq!(item_id(&item), "link:https://example.com/item");
    }

    #[test]
    fn item_id_falls_back_to_title_and_date() {
        let item = RssItem {
            guid: None,
            title: "Title".to_string(),
            link: String::new(),
            description: String::new(),
            pub_date: parse_pub_date("Mon, 04 May 2026 12:00:00 +0900"),
            feed_title: None,
        };

        assert_eq!(item_id(&item), "fallback:Title:2026-05-04T03:00:00+00:00");
    }
}
