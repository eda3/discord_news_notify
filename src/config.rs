use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub discord: DiscordConfig,
    pub rss: RssConfig,
}

#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    #[serde(default = "empty_string")]
    pub token: String,
    pub channel_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RssConfig {
    pub feed_url: String,
    pub poll_interval_seconds: u64,
}

fn empty_string() -> String {
    String::new()
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // .envファイルを読み込み
        dotenv::dotenv().ok();
        
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .build()?;
        
        let mut config: Config = settings.try_deserialize()?;
        
        // 環境変数からトークンを取得
        if let Ok(token) = env::var("DISCORD_BOT_TOKEN") {
            config.discord.token = token;
        } else {
            return Err(anyhow::anyhow!("DISCORD_BOT_TOKEN environment variable not set"));
        }
        
        Ok(config)
    }
} 