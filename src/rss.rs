use anyhow::Result;
use chrono::{DateTime, Utc};
use rss::Channel;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::BufReader;

#[derive(Debug, Serialize, Deserialize)]
pub struct RssItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: DateTime<Utc>,
}

pub struct RssProcessor {
    seen_items: HashSet<String>,
}

impl RssProcessor {
    pub fn new() -> Self {
        Self {
            seen_items: HashSet::new(),
        }
    }

    pub async fn fetch_items(&self, feed_url: &str) -> Result<Vec<RssItem>> {
        let response = ureq::get(feed_url).call()?;
        let text = response.into_string()?;
        
        // RSSフィードをパース
        let channel = Channel::read_from(BufReader::new(text.as_bytes()))?;
        
        let mut items = Vec::new();
        
        for item in channel.items() {
            let title = item.title().unwrap_or("No Title").to_string();
            let link = item.link().unwrap_or("#").to_string();
            let description = item.description().unwrap_or("No description available").to_string();
            
            // 日付をパース（フォーマットによって調整が必要かも）
            let pub_date_str = item.pub_date().unwrap_or_default();
            let pub_date = DateTime::parse_from_rfc2822(pub_date_str)
                .unwrap_or_else(|_| DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z").unwrap())
                .with_timezone(&Utc);
            
            items.push(RssItem {
                title,
                link,
                description,
                pub_date,
            });
        }
        
        Ok(items)
    }

    pub fn is_new_item(&mut self, item: &RssItem) -> bool {
        if self.seen_items.contains(&item.link) {
            false
        } else {
            self.seen_items.insert(item.link.clone());
            true
        }
    }
} 