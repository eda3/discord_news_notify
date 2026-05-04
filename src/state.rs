use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct SeenStateFile {
    seen_items: Vec<SeenItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SeenItem {
    id: String,
    seen_at: DateTime<Utc>,
}

pub struct SeenState {
    path: PathBuf,
    max_items: usize,
    items: HashMap<String, DateTime<Utc>>,
    first_run: bool,
}

impl SeenState {
    pub fn load(path: impl Into<PathBuf>, max_items: usize) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self {
                path,
                max_items,
                items: HashMap::new(),
                first_run: true,
            });
        }

        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read state file {}", path.display()))?;
        let file: SeenStateFile = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse state file {}", path.display()))?;
        let items = file
            .seen_items
            .into_iter()
            .map(|item| (item.id, item.seen_at))
            .collect();

        Ok(Self {
            path,
            max_items,
            items,
            first_run: false,
        })
    }

    pub fn is_first_run(&self) -> bool {
        self.first_run
    }

    pub fn finish_first_run(&mut self) {
        self.first_run = false;
    }

    pub fn is_seen(&self, id: &str) -> bool {
        self.items.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn mark_seen(&mut self, id: String) {
        self.items.insert(id, Utc::now());
        self.prune();
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create state directory {}", parent.display())
            })?;
        }

        let file = SeenStateFile {
            seen_items: self.sorted_items(),
        };
        let text = serde_json::to_string_pretty(&file)?;

        fs::write(&self.path, text)
            .with_context(|| format!("failed to write state file {}", self.path.display()))
    }

    fn prune(&mut self) {
        if self.items.len() <= self.max_items {
            return;
        }

        let keep_ids: Vec<String> = self
            .sorted_items()
            .into_iter()
            .rev()
            .take(self.max_items)
            .map(|item| item.id)
            .collect();

        self.items.retain(|id, _| keep_ids.contains(id));
    }

    fn sorted_items(&self) -> Vec<SeenItem> {
        let mut items: Vec<SeenItem> = self
            .items
            .iter()
            .map(|(id, seen_at)| SeenItem {
                id: id.clone(),
                seen_at: *seen_at,
            })
            .collect();

        items.sort_by(|a, b| a.seen_at.cmp(&b.seen_at).then_with(|| a.id.cmp(&b.id)));
        items
    }
}

pub fn default_state_path() -> &'static str {
    "data/seen_items.json"
}

#[cfg(test)]
mod tests {
    use super::SeenState;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn missing_state_file_is_first_run() {
        let state = SeenState::load(test_path("missing"), 100).unwrap();

        assert!(state.is_first_run());
        assert!(!state.is_seen("item-1"));
    }

    #[test]
    fn mark_seen_records_item() {
        let mut state = SeenState::load(test_path("mark"), 100).unwrap();

        state.mark_seen("item-1".to_string());

        assert!(state.is_seen("item-1"));
    }

    #[test]
    fn prune_keeps_newest_items() {
        let mut state = SeenState::load(test_path("prune"), 2).unwrap();

        state.mark_seen("item-1".to_string());
        state.mark_seen("item-2".to_string());
        state.mark_seen("item-3".to_string());

        assert!(!state.is_seen("item-1"));
        assert!(state.is_seen("item-2"));
        assert!(state.is_seen("item-3"));
    }

    #[test]
    fn empty_state_can_be_saved_and_reloaded() {
        let path = test_path("empty-save");
        let state = SeenState::load(&path, 100).unwrap();

        state.save().unwrap();
        let reloaded = SeenState::load(&path, 100).unwrap();

        assert_eq!(reloaded.len(), 0);
        assert!(!reloaded.is_first_run());
    }

    #[test]
    fn saved_items_can_be_reloaded() {
        let path = test_path("reload");
        let mut state = SeenState::load(&path, 100).unwrap();
        state.mark_seen("item-1".to_string());

        state.save().unwrap();
        let reloaded = SeenState::load(&path, 100).unwrap();

        assert!(reloaded.is_seen("item-1"));
    }

    #[test]
    fn corrupted_json_returns_error() {
        let path = test_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not json").unwrap();

        let err = match SeenState::load(&path, 100) {
            Ok(_) => panic!("corrupted state file should fail to load"),
            Err(err) => err.to_string(),
        };

        assert!(err.contains("failed to parse state file"));
    }

    #[test]
    fn saved_state_respects_max_items_after_pruning() {
        let path = test_path("max-items");
        let mut state = SeenState::load(&path, 2).unwrap();
        state.mark_seen("item-1".to_string());
        state.mark_seen("item-2".to_string());
        state.mark_seen("item-3".to_string());

        state.save().unwrap();
        let reloaded = SeenState::load(&path, 2).unwrap();

        assert_eq!(reloaded.len(), 2);
        assert!(!reloaded.is_seen("item-1"));
        assert!(reloaded.is_seen("item-2"));
        assert!(reloaded.is_seen("item-3"));
    }
}
