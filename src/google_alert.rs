use anyhow::Result;
use chrono::{DateTime, Utc};
use rss::Channel;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::BufReader;

#[derive(Debug, Serialize, Deserialize)]
pub struct AlertItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: DateTime<Utc>,
}

pub struct AlertProcessor {
    seen_items: HashSet<String>,
}

impl AlertProcessor {
    pub fn new() -> Self {
        Self {
            seen_items: HashSet::new(),
        }
    }

    pub async fn fetch_alerts(&self, rss_url: &str) -> Result<Vec<AlertItem>> {
        let response = ureq::get(rss_url).call()?;
        let text = response.into_string()?;
        
        // RSSフィードをパース
        let channel = Channel::read_from(BufReader::new(text.as_bytes()))?;
        
        let mut alerts = Vec::new();
        
        for item in channel.items() {
            let title = item.title().unwrap_or("No Title").to_string();
            let link = item.link().unwrap_or("#").to_string();
            let description = item.description().unwrap_or("No description available").to_string();
            
            // 日付をパース（フォーマットによって調整が必要かも）
            let pub_date_str = item.pub_date().unwrap_or_default();
            let pub_date = DateTime::parse_from_rfc2822(pub_date_str)
                .unwrap_or_else(|_| DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z").unwrap())
                .with_timezone(&Utc);
            
            alerts.push(AlertItem {
                title,
                link,
                description,
                pub_date,
            });
        }
        
        Ok(alerts)
    }

    pub fn is_new_item(&mut self, item: &AlertItem) -> bool {
        if self.seen_items.contains(&item.link) {
            false
        } else {
            self.seen_items.insert(item.link.clone());
            true
        }
    }
} 