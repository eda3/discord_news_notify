mod config;
mod rss;

use anyhow::Result;
use log::{error, info};
use std::time::Duration;
use twilight_gateway::{Event, Intents, Shard, ShardId, StreamExt};
use twilight_http::Client as HttpClient;
use twilight_model::gateway::payload::incoming::MessageCreate;
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

#[tokio::main]
async fn main() -> Result<()> {
    // ログの初期化
    env_logger::init();
    info!("Starting Discord RSS Notify Bot... 🚀");

    // 設定の読み込み
    let config = config::Config::load()?;
    info!("Configuration loaded successfully! 📝");

    // Discordクライアントの初期化
    let http = HttpClient::new(config.discord.token.clone());
    let intents = Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT;
    let mut shard = Shard::new(ShardId::ONE, config.discord.token.clone(), intents);

    // RSSプロセッサーの初期化
    let mut rss_processor = rss::RssProcessor::new();

    // メインループ
    loop {
        // Discordイベントの処理
        if let Some(Ok(event)) = shard.next_event(twilight_gateway::EventTypeFlags::all()).await {
            match event {
                Event::Ready(_) => {
                    info!("Shard is ready! 🎉");
                }
                Event::MessageCreate(msg) => {
                    if let Err(e) = handle_message(&http, &config, msg).await {
                        error!("Error handling message: {}", e);
                    }
                }
                _ => {}
            }
        }

        // RSSフィードのチェック
        match rss_processor.fetch_items(&config.rss.feed_url).await {
            Ok(items) => {
                for item in items {
                    if rss_processor.is_new_item(&item) {
                        if let Err(e) = post_rss_item(&http, &config, &item).await {
                            error!("Error posting RSS item: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error fetching RSS items: {}", e);
            }
        }

        // ポーリング間隔待機
        tokio::time::sleep(Duration::from_secs(config.rss.poll_interval_seconds)).await;
    }
}

async fn handle_message(
    _http: &HttpClient,
    _config: &config::Config,
    _msg: Box<MessageCreate>,
) -> Result<()> {
    // ここにメッセージ処理ロジックを実装
    Ok(())
}

async fn post_rss_item(
    http: &HttpClient,
    config: &config::Config,
    item: &rss::RssItem,
) -> Result<()> {
    let channel_id = Id::<ChannelMarker>::new(config.discord.channel_id.parse::<u64>()?);
    let content = format!(
        "📰 **New RSS Item!**\n\n**Title:** {}\n**Description:** {}\n**Link:** {}",
        item.title, item.description, item.link
    );

    let request = http.create_message(channel_id)
        .content(&content);
    
    request.await?;

    Ok(())
}
