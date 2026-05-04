use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

use crate::rss::{RssItem, item_id};

#[derive(Debug, Serialize, Deserialize)]
struct ArticleStateFile {
    articles: Vec<ArticleState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleState {
    pub article_id: String,
    pub title: String,
    pub url: String,
    pub feed_title: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_bookmark_count: Option<u64>,
    pub posted_thresholds: Vec<u64>,
    pub threshold_posts: BTreeMap<u64, ThresholdPostState>,
    pub last_posted_at: Option<DateTime<Utc>>,
    pub pub_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdPostState {
    pub channel_id: String,
    pub posted_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ArticleSnapshot {
    pub article_id: String,
    pub title: String,
    pub url: String,
}

pub struct ArticleStateStore {
    path: PathBuf,
    retention_days: i64,
    articles: HashMap<String, ArticleState>,
    first_run: bool,
}

impl ArticleStateStore {
    pub fn load(path: impl Into<PathBuf>, retention_days: i64) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self {
                path,
                retention_days,
                articles: HashMap::new(),
                first_run: true,
            });
        }

        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read state file {}", path.display()))?;
        let file: ArticleStateFile = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse state file {}", path.display()))?;
        let articles = file
            .articles
            .into_iter()
            .map(|article| (article.article_id.clone(), article))
            .collect();

        Ok(Self {
            path,
            retention_days,
            articles,
            first_run: false,
        })
    }

    pub fn is_first_run(&self) -> bool {
        self.first_run
    }

    pub fn finish_first_run(&mut self) {
        self.first_run = false;
    }

    pub fn len(&self) -> usize {
        self.articles.len()
    }

    pub fn upsert_rss_item(&mut self, item: &RssItem) -> String {
        let article_id = item_id(item);
        self.articles
            .entry(article_id.clone())
            .and_modify(|article| {
                article.title = item.title.clone();
                article.url = item.link.clone();
                article.feed_title = item.feed_title.clone();
                article.pub_date = item.pub_date;
            })
            .or_insert_with(|| ArticleState {
                article_id: article_id.clone(),
                title: item.title.clone(),
                url: item.link.clone(),
                feed_title: item.feed_title.clone(),
                first_seen_at: Utc::now(),
                last_checked_at: None,
                last_bookmark_count: None,
                posted_thresholds: Vec::new(),
                threshold_posts: BTreeMap::new(),
                last_posted_at: None,
                pub_date: item.pub_date,
            });

        article_id
    }

    pub fn candidate_snapshots(&self) -> Vec<ArticleSnapshot> {
        let mut snapshots = self
            .articles
            .values()
            .map(|article| ArticleSnapshot {
                article_id: article.article_id.clone(),
                title: article.title.clone(),
                url: article.url.clone(),
            })
            .collect::<Vec<_>>();
        snapshots.sort_by(|a, b| a.article_id.cmp(&b.article_id));
        snapshots
    }

    pub fn update_bookmark_count(&mut self, article_id: &str, count: u64) {
        if let Some(article) = self.articles.get_mut(article_id) {
            article.last_checked_at = Some(Utc::now());
            article.last_bookmark_count = Some(count);
        }
    }

    pub fn unposted_reached_thresholds(&self, article_id: &str, thresholds: &[u64]) -> Vec<u64> {
        let Some(article) = self.articles.get(article_id) else {
            return Vec::new();
        };
        let Some(count) = article.last_bookmark_count else {
            return Vec::new();
        };

        thresholds
            .iter()
            .copied()
            .filter(|threshold| count >= *threshold)
            .filter(|threshold| !article.posted_thresholds.contains(threshold))
            .collect()
    }

    pub fn mark_threshold_posted(
        &mut self,
        article_id: &str,
        threshold: u64,
        channel_id: Id<ChannelMarker>,
    ) {
        if let Some(article) = self.articles.get_mut(article_id) {
            if !article.posted_thresholds.contains(&threshold) {
                article.posted_thresholds.push(threshold);
                article.posted_thresholds.sort_unstable();
            }

            let posted_at = Utc::now();
            article.threshold_posts.insert(
                threshold,
                ThresholdPostState {
                    channel_id: channel_id.get().to_string(),
                    posted_at,
                },
            );
            article.last_posted_at = Some(posted_at);
        }
    }

    pub fn mark_reached_thresholds_posted(
        &mut self,
        article_id: &str,
        thresholds: &[u64],
        channels: &BTreeMap<u64, Id<ChannelMarker>>,
    ) {
        let reached = self.unposted_reached_thresholds(article_id, thresholds);
        for threshold in reached {
            if let Some(channel_id) = channels.get(&threshold) {
                self.mark_threshold_posted(article_id, threshold, *channel_id);
            }
        }
    }

    pub fn prune(&mut self) {
        let cutoff = Utc::now() - Duration::days(self.retention_days);
        self.articles.retain(|_, article| {
            let anchor = article.last_checked_at.unwrap_or(article.first_seen_at);
            anchor >= cutoff
        });
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create state directory {}", parent.display())
            })?;
        }

        let file = ArticleStateFile {
            articles: self.sorted_articles(),
        };
        let text = serde_json::to_string_pretty(&file)?;

        fs::write(&self.path, text)
            .with_context(|| format!("failed to write state file {}", self.path.display()))
    }

    fn sorted_articles(&self) -> Vec<ArticleState> {
        let mut articles = self.articles.values().cloned().collect::<Vec<_>>();
        articles.sort_by(|a, b| {
            a.first_seen_at
                .cmp(&b.first_seen_at)
                .then_with(|| a.article_id.cmp(&b.article_id))
        });
        articles
    }
}

pub fn default_state_path() -> &'static str {
    "data/articles.json"
}

#[cfg(test)]
mod tests {
    use super::{ArticleStateStore, ThresholdPostState};
    use crate::rss::RssItem;
    use chrono::{Duration, Utc};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use twilight_model::id::Id;

    fn test_path(name: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        PathBuf::from(format!(
            "target/test-state/{name}-{}-{now}.json",
            std::process::id()
        ))
    }

    fn rss_item(link: &str) -> RssItem {
        RssItem {
            guid: Some(format!("guid-{link}")),
            title: "Title".to_string(),
            link: link.to_string(),
            description: "Description".to_string(),
            pub_date: None,
            feed_title: Some("Feed".to_string()),
        }
    }

    #[test]
    fn missing_state_file_is_first_run() {
        let state = ArticleStateStore::load(test_path("missing"), 7).unwrap();

        assert!(state.is_first_run());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn upsert_records_article() {
        let mut state = ArticleStateStore::load(test_path("upsert"), 7).unwrap();

        let id = state.upsert_rss_item(&rss_item("https://example.com/item"));

        assert_eq!(state.len(), 1);
        assert_eq!(state.candidate_snapshots()[0].article_id, id);
    }

    #[test]
    fn thresholds_use_greater_than_or_equal() {
        let mut state = ArticleStateStore::load(test_path("thresholds"), 7).unwrap();
        let id = state.upsert_rss_item(&rss_item("https://example.com/item"));
        state.update_bookmark_count(&id, 25);

        assert_eq!(
            state.unposted_reached_thresholds(&id, &[1, 5, 20]),
            vec![1, 5, 20]
        );
    }

    #[test]
    fn posted_thresholds_are_not_returned_again() {
        let mut state = ArticleStateStore::load(test_path("posted"), 7).unwrap();
        let id = state.upsert_rss_item(&rss_item("https://example.com/item"));
        state.update_bookmark_count(&id, 25);
        state.mark_threshold_posted(&id, 1, Id::new(11));
        state.mark_threshold_posted(&id, 5, Id::new(55));

        assert_eq!(
            state.unposted_reached_thresholds(&id, &[1, 5, 20]),
            vec![20]
        );
    }

    #[test]
    fn partial_success_state_can_be_saved_and_reloaded() {
        let path = test_path("partial");
        let mut state = ArticleStateStore::load(&path, 7).unwrap();
        let id = state.upsert_rss_item(&rss_item("https://example.com/item"));
        state.update_bookmark_count(&id, 25);
        state.mark_threshold_posted(&id, 1, Id::new(11));
        state.mark_threshold_posted(&id, 20, Id::new(2020));
        state.save().unwrap();

        let reloaded = ArticleStateStore::load(&path, 7).unwrap();

        assert_eq!(
            reloaded.unposted_reached_thresholds(&id, &[1, 5, 20]),
            vec![5]
        );
    }

    #[test]
    fn threshold_posts_are_persisted() {
        let path = test_path("threshold-posts");
        let mut state = ArticleStateStore::load(&path, 7).unwrap();
        let id = state.upsert_rss_item(&rss_item("https://example.com/item"));
        state.update_bookmark_count(&id, 5);
        state.mark_threshold_posted(&id, 5, Id::new(55));
        state.save().unwrap();

        let text = fs::read_to_string(path).unwrap();

        assert!(text.contains("\"posted_thresholds\""));
        assert!(text.contains("\"5\""));
        assert!(text.contains("\"channel_id\": \"55\""));
    }

    #[test]
    fn mark_reached_thresholds_posted_uses_channel_map() {
        let mut state = ArticleStateStore::load(test_path("initial-posted"), 7).unwrap();
        let id = state.upsert_rss_item(&rss_item("https://example.com/item"));
        state.update_bookmark_count(&id, 20);
        let channels = BTreeMap::from([(1, Id::new(11)), (5, Id::new(55)), (20, Id::new(2020))]);

        state.mark_reached_thresholds_posted(&id, &[1, 5, 20], &channels);

        assert!(
            state
                .unposted_reached_thresholds(&id, &[1, 5, 20])
                .is_empty()
        );
    }

    #[test]
    fn prune_removes_old_candidates() {
        let mut state = ArticleStateStore::load(test_path("prune"), 7).unwrap();
        let old_id = state.upsert_rss_item(&rss_item("https://example.com/old"));
        let new_id = state.upsert_rss_item(&rss_item("https://example.com/new"));
        state.articles.get_mut(&old_id).unwrap().last_checked_at =
            Some(Utc::now() - Duration::days(8));
        state.articles.get_mut(&new_id).unwrap().last_checked_at = Some(Utc::now());

        state.prune();

        assert_eq!(state.len(), 1);
        assert_eq!(state.candidate_snapshots()[0].article_id, new_id);
    }

    #[test]
    fn corrupted_json_returns_error() {
        let path = test_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not json").unwrap();

        let err = match ArticleStateStore::load(&path, 7) {
            Ok(_) => panic!("corrupted state file should fail to load"),
            Err(err) => err.to_string(),
        };

        assert!(err.contains("failed to parse state file"));
    }

    #[test]
    fn saved_empty_state_is_not_first_run() {
        let path = test_path("empty");
        let state = ArticleStateStore::load(&path, 7).unwrap();
        state.save().unwrap();

        let reloaded = ArticleStateStore::load(&path, 7).unwrap();

        assert!(!reloaded.is_first_run());
    }

    #[test]
    fn threshold_post_state_serializes_channel() {
        let post = ThresholdPostState {
            channel_id: "123".to_string(),
            posted_at: Utc::now(),
        };

        let text = serde_json::to_string(&post).unwrap();

        assert!(text.contains("123"));
    }
}
