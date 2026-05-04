use serde::Deserialize;
use std::env;
use std::path::Path;
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;
use url::Url;

pub const MIN_POLL_INTERVAL_SECONDS: u64 = 60;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub discord: DiscordConfig,
    pub rss: RssConfig,
    #[serde(default)]
    pub state: StateConfig,
}

#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    #[serde(default = "empty_string")]
    pub token: String,
    #[serde(deserialize_with = "deserialize_channel_id")]
    pub channel_id: Id<ChannelMarker>,
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
    #[serde(default = "default_max_seen_items")]
    pub max_seen_items: usize,
    #[serde(default = "default_skip_existing_on_first_run")]
    pub skip_existing_on_first_run: bool,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            file_path: default_state_file_path(),
            max_seen_items: default_max_seen_items(),
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

fn default_max_seen_items() -> usize {
    10_000
}

fn default_skip_existing_on_first_run() -> bool {
    true
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
        if let Ok(token) = env::var("DISCORD_BOT_TOKEN") {
            config.discord.token = token;
        } else {
            return Err(anyhow::anyhow!(
                "DISCORD_BOT_TOKEN environment variable not set"
            ));
        }

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

        validate_state_file_path(&self.state.file_path)?;

        Ok(())
    }
}

fn deserialize_channel_id<'de, D>(deserializer: D) -> Result<Id<ChannelMarker>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    let id = value.parse::<u64>().map_err(|_| {
        serde::de::Error::custom("discord.channel_id must be a valid Discord snowflake")
    })?;

    if id == 0 {
        return Err(serde::de::Error::custom(
            "discord.channel_id must be a valid Discord snowflake",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> Config {
        Config {
            discord: DiscordConfig {
                token: "token".to_string(),
                channel_id: Id::new(1),
            },
            rss: RssConfig {
                feed_url: "https://example.com/feed.xml".to_string(),
                poll_interval_seconds: MIN_POLL_INTERVAL_SECONDS,
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
    fn deserialize_rejects_invalid_channel_id() {
        let source = r#"
[discord]
channel_id = "replace_me"

[rss]
feed_url = "https://example.com/feed.xml"
poll_interval_seconds = 300
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
}
