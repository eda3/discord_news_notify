use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::Path;
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;
use url::Url;

pub const MIN_POLL_INTERVAL_SECONDS: u64 = 60;
pub const MIN_HATENA_TIMEOUT_SECONDS: u64 = 1;
pub const MAX_HATENA_TIMEOUT_SECONDS: u64 = 60;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub discord: DiscordConfig,
    pub rss: RssConfig,
    #[serde(default)]
    pub hatena: HatenaConfig,
    pub notification: NotificationConfig,
    #[serde(default)]
    pub state: StateConfig,
}

#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    #[serde(default = "empty_string")]
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct RssConfig {
    pub feed_url: String,
    pub poll_interval_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct StateConfig {
    #[serde(default = "default_state_file_path")]
    pub file_path: String,
    #[serde(default = "default_skip_existing_on_first_run")]
    pub skip_existing_on_first_run: bool,
}

#[derive(Debug, Deserialize)]
pub struct HatenaConfig {
    #[serde(default = "default_thresholds")]
    pub thresholds: Vec<u64>,
    #[serde(default = "default_count_api_timeout_seconds")]
    pub count_api_timeout_seconds: u64,
    #[serde(default = "default_candidate_retention_days")]
    pub candidate_retention_days: i64,
}

impl Default for HatenaConfig {
    fn default() -> Self {
        Self {
            thresholds: default_thresholds(),
            count_api_timeout_seconds: default_count_api_timeout_seconds(),
            candidate_retention_days: default_candidate_retention_days(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct NotificationConfig {
    pub channels: Vec<NotificationChannelConfig>,
}

#[derive(Debug, Deserialize)]
pub struct NotificationChannelConfig {
    pub threshold: u64,
    #[serde(deserialize_with = "deserialize_channel_id")]
    pub channel_id: Id<ChannelMarker>,
}

impl NotificationConfig {
    pub fn channel_for(&self, threshold: u64) -> anyhow::Result<Id<ChannelMarker>> {
        self.channels
            .iter()
            .find(|channel| channel.threshold == threshold)
            .map(|channel| channel.channel_id)
            .ok_or_else(|| {
                anyhow::anyhow!("notification channel for threshold {} not found", threshold)
            })
    }
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            file_path: default_state_file_path(),
            skip_existing_on_first_run: default_skip_existing_on_first_run(),
        }
    }
}

fn empty_string() -> String {
    String::new()
}

fn default_state_file_path() -> String {
    crate::state::default_state_path().to_string()
}

fn default_skip_existing_on_first_run() -> bool {
    true
}

fn default_thresholds() -> Vec<u64> {
    vec![1, 5, 20]
}

fn default_count_api_timeout_seconds() -> u64 {
    10
}

fn default_candidate_retention_days() -> i64 {
    7
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // .envファイルを読み込み
        dotenv::dotenv().ok();

        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::File::with_name("config/local").required(false))
            .build()?;

        let mut config: Config = settings.try_deserialize()?;

        // 環境変数からトークンを取得
        config.discord.token = discord_token_from_env(env::var("DISCORD_BOT_TOKEN").ok())?;

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.discord.token.trim().is_empty() {
            return Err(anyhow::anyhow!("DISCORD_BOT_TOKEN must not be empty"));
        }

        validate_feed_url(&self.rss.feed_url)?;

        if self.rss.poll_interval_seconds < MIN_POLL_INTERVAL_SECONDS {
            return Err(anyhow::anyhow!(
                "rss.poll_interval_seconds must be at least {} seconds",
                MIN_POLL_INTERVAL_SECONDS
            ));
        }

        validate_hatena_config(&self.hatena)?;
        validate_notification_channels(&self.hatena.thresholds, &self.notification)?;
        validate_state_file_path(&self.state.file_path)?;

        Ok(())
    }
}

fn discord_token_from_env(token: Option<String>) -> anyhow::Result<String> {
    token.ok_or_else(|| anyhow::anyhow!("DISCORD_BOT_TOKEN environment variable not set"))
}

fn deserialize_channel_id<'de, D>(deserializer: D) -> Result<Id<ChannelMarker>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    let id = value.parse::<u64>().map_err(|_| {
        serde::de::Error::custom(
            "notification.channels.channel_id must be a valid Discord snowflake",
        )
    })?;

    if id == 0 {
        return Err(serde::de::Error::custom(
            "notification.channels.channel_id must be a valid Discord snowflake",
        ));
    }

    Ok(Id::new(id))
}

fn validate_feed_url(feed_url: &str) -> anyhow::Result<()> {
    let url = Url::parse(feed_url)
        .map_err(|e| anyhow::anyhow!("rss.feed_url must be a valid URL: {}", e))?;

    match url.scheme() {
        "http" | "https" => Ok(()),
        _ => Err(anyhow::anyhow!(
            "rss.feed_url must use http or https scheme"
        )),
    }
}

fn validate_state_file_path(file_path: &str) -> anyhow::Result<()> {
    if file_path.trim().is_empty() {
        return Err(anyhow::anyhow!("state.file_path must not be empty"));
    }

    if Path::new(file_path).file_name().is_none() {
        return Err(anyhow::anyhow!("state.file_path must point to a file path"));
    }

    Ok(())
}

fn validate_hatena_config(hatena: &HatenaConfig) -> anyhow::Result<()> {
    if hatena.thresholds.is_empty() {
        return Err(anyhow::anyhow!("hatena.thresholds must not be empty"));
    }

    let mut seen = BTreeSet::new();
    let mut previous = None;
    for threshold in &hatena.thresholds {
        if *threshold == 0 {
            return Err(anyhow::anyhow!("hatena.thresholds must not contain 0"));
        }
        if !seen.insert(*threshold) {
            return Err(anyhow::anyhow!(
                "hatena.thresholds must not contain duplicates"
            ));
        }
        if previous.is_some_and(|value| value >= *threshold) {
            return Err(anyhow::anyhow!(
                "hatena.thresholds must be sorted ascending"
            ));
        }
        previous = Some(*threshold);
    }

    if !(MIN_HATENA_TIMEOUT_SECONDS..=MAX_HATENA_TIMEOUT_SECONDS)
        .contains(&hatena.count_api_timeout_seconds)
    {
        return Err(anyhow::anyhow!(
            "hatena.count_api_timeout_seconds must be between {} and {}",
            MIN_HATENA_TIMEOUT_SECONDS,
            MAX_HATENA_TIMEOUT_SECONDS
        ));
    }

    if hatena.candidate_retention_days < 1 {
        return Err(anyhow::anyhow!(
            "hatena.candidate_retention_days must be at least 1"
        ));
    }

    Ok(())
}

fn validate_notification_channels(
    thresholds: &[u64],
    notification: &NotificationConfig,
) -> anyhow::Result<()> {
    let threshold_set = thresholds.iter().copied().collect::<BTreeSet<_>>();
    let mut channels = BTreeMap::new();

    for channel in &notification.channels {
        if !threshold_set.contains(&channel.threshold) {
            return Err(anyhow::anyhow!(
                "notification.channels contains unknown threshold {}",
                channel.threshold
            ));
        }
        if channels
            .insert(channel.threshold, channel.channel_id)
            .is_some()
        {
            return Err(anyhow::anyhow!(
                "notification.channels contains duplicate threshold {}",
                channel.threshold
            ));
        }
    }

    for threshold in thresholds {
        if !channels.contains_key(threshold) {
            return Err(anyhow::anyhow!(
                "notification channel for threshold {} is missing",
                threshold
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> Config {
        Config {
            discord: DiscordConfig {
                token: "token".to_string(),
            },
            rss: RssConfig {
                feed_url: "https://example.com/feed.xml".to_string(),
                poll_interval_seconds: MIN_POLL_INTERVAL_SECONDS,
            },
            hatena: HatenaConfig::default(),
            notification: NotificationConfig {
                channels: vec![
                    NotificationChannelConfig {
                        threshold: 1,
                        channel_id: Id::new(11),
                    },
                    NotificationChannelConfig {
                        threshold: 5,
                        channel_id: Id::new(55),
                    },
                    NotificationChannelConfig {
                        threshold: 20,
                        channel_id: Id::new(2020),
                    },
                ],
            },
            state: StateConfig::default(),
        }
    }

    #[test]
    fn validate_rejects_empty_token() {
        let mut config = valid_config();
        config.discord.token = " ".to_string();

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("DISCORD_BOT_TOKEN"));
    }

    #[test]
    fn discord_token_from_env_accepts_present_value() {
        let token = discord_token_from_env(Some("token".to_string())).unwrap();

        assert_eq!(token, "token");
    }

    #[test]
    fn discord_token_from_env_rejects_missing_value() {
        let err = discord_token_from_env(None).unwrap_err().to_string();

        assert!(err.contains("DISCORD_BOT_TOKEN"));
    }

    #[test]
    fn deserialize_rejects_invalid_channel_id() {
        let source = r#"
[rss]
feed_url = "https://example.com/feed.xml"
poll_interval_seconds = 300

[hatena]
thresholds = [1]

[[notification.channels]]
threshold = 1
channel_id = "replace_me"
"#;

        let result = config::Config::builder()
            .add_source(config::File::from_str(source, config::FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize::<Config>();

        assert!(result.unwrap_err().to_string().contains("channel_id"));
    }

    #[test]
    fn validate_rejects_invalid_feed_url() {
        let mut config = valid_config();
        config.rss.feed_url = "not a url".to_string();

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("rss.feed_url"));
    }

    #[test]
    fn validate_rejects_short_poll_interval() {
        let mut config = valid_config();
        config.rss.poll_interval_seconds = MIN_POLL_INTERVAL_SECONDS - 1;

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("poll_interval_seconds"));
    }

    #[test]
    fn validate_rejects_empty_state_file_path() {
        let mut config = valid_config();
        config.state.file_path = " ".to_string();

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("state.file_path"));
    }

    #[test]
    fn validate_accepts_threshold_channels() {
        let config = valid_config();

        config.validate().unwrap();
        assert_eq!(config.notification.channel_for(5).unwrap().get(), 55);
    }

    #[test]
    fn validate_rejects_empty_thresholds() {
        let mut config = valid_config();
        config.hatena.thresholds = Vec::new();

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("hatena.thresholds"));
    }

    #[test]
    fn validate_rejects_zero_threshold() {
        let mut config = valid_config();
        config.hatena.thresholds = vec![1, 0, 5];

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("0"));
    }

    #[test]
    fn validate_rejects_duplicate_thresholds() {
        let mut config = valid_config();
        config.hatena.thresholds = vec![1, 5, 5];

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("duplicates"));
    }

    #[test]
    fn validate_rejects_unsorted_thresholds() {
        let mut config = valid_config();
        config.hatena.thresholds = vec![1, 20, 5];

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("sorted"));
    }

    #[test]
    fn validate_rejects_missing_notification_channel() {
        let mut config = valid_config();
        config
            .notification
            .channels
            .retain(|channel| channel.threshold != 20);

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("threshold 20"));
    }

    #[test]
    fn validate_rejects_unknown_notification_threshold() {
        let mut config = valid_config();
        config
            .notification
            .channels
            .push(NotificationChannelConfig {
                threshold: 99,
                channel_id: Id::new(99),
            });

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("unknown threshold 99"));
    }

    #[test]
    fn validate_rejects_duplicate_notification_threshold() {
        let mut config = valid_config();
        config
            .notification
            .channels
            .push(NotificationChannelConfig {
                threshold: 5,
                channel_id: Id::new(555),
            });

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("duplicate threshold 5"));
    }

    #[test]
    fn validate_allows_same_channel_for_multiple_thresholds() {
        let mut config = valid_config();
        for channel in &mut config.notification.channels {
            channel.channel_id = Id::new(123);
        }

        config.validate().unwrap();
    }

    #[test]
    fn validate_rejects_invalid_hatena_timeout() {
        let mut config = valid_config();
        config.hatena.count_api_timeout_seconds = 61;

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("count_api_timeout_seconds"));
    }

    #[test]
    fn validate_rejects_invalid_retention_days() {
        let mut config = valid_config();
        config.hatena.candidate_retention_days = 0;

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("candidate_retention_days"));
    }
}
