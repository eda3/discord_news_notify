mod config;
mod formatter;
mod rss;
mod state;

use anyhow::Result;
use log::{error, info, warn};
use state::SeenState;
use std::time::Duration;
use twilight_http::Client as HttpClient;
use twilight_http::error::{Error as DiscordHttpError, ErrorType};
use twilight_model::channel::message::AllowedMentions;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting Discord RSS Notify Bot...");

    let config = config::Config::load()?;
    info!("Configuration loaded successfully.");
    info!(
        "Runtime config: channel_id={}, feed_url={}, poll_interval_seconds={}, state_file={}, max_seen_items={}, skip_existing_on_first_run={}",
        config.discord.channel_id.get(),
        config.rss.feed_url,
        config.rss.poll_interval_seconds,
        config.state.file_path,
        config.state.max_seen_items,
        config.state.skip_existing_on_first_run
    );

    let http = HttpClient::new(config.discord.token.clone());
    let rss_processor = rss::RssProcessor::new();
    let mut seen_state = SeenState::load(&config.state.file_path, config.state.max_seen_items)?;
    info!(
        "Seen state loaded: first_run={}, seen_items={}",
        seen_state.is_first_run(),
        seen_state.len()
    );
    let mut interval = tokio::time::interval(Duration::from_secs(config.rss.poll_interval_seconds));
    info!("RSS polling loop started. Press Ctrl+C to stop.");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received.");
                break;
            }
            _ = interval.tick() => {
                check_rss_feed(&http, &config, &rss_processor, &mut seen_state).await;
            }
        }
    }

    Ok(())
}

async fn check_rss_feed(
    http: &HttpClient,
    config: &config::Config,
    rss_processor: &rss::RssProcessor,
    seen_state: &mut SeenState,
) {
    info!("RSS check started: {}", config.rss.feed_url);

    match rss_processor.fetch_items(&config.rss.feed_url).await {
        Ok(items) => {
            info!("RSS check fetched {} item(s).", items.len());

            if seen_state.is_first_run() && config.state.skip_existing_on_first_run {
                let item_count = items.len();
                for item in items {
                    seen_state.mark_seen(rss::item_id(&item));
                }

                if let Err(e) = seen_state.save() {
                    error!("Error saving initial seen state: {}", e);
                } else {
                    seen_state.finish_first_run();
                    info!(
                        "First run complete: marked {} existing RSS item(s) as seen without posting.",
                        item_count
                    );
                }

                return;
            }

            let mut skipped_count = 0;
            let mut posted_count = 0;
            let mut failed_count = 0;

            for item in items {
                let item_id = rss::item_id(&item);
                if seen_state.is_seen(&item_id) {
                    skipped_count += 1;
                    info!("Skipping already seen RSS item: {}", item.title);
                    continue;
                }

                info!("New RSS item detected: {} ({})", item.title, item.link);
                if let Err(e) = post_rss_item(http, config, &item).await {
                    failed_count += 1;
                    error!("Error posting RSS item: {}", e);
                    continue;
                }

                seen_state.mark_seen(item_id);
                posted_count += 1;
                if let Err(e) = seen_state.save() {
                    warn!("RSS item was posted, but failed to save seen state: {}", e);
                } else {
                    info!("Seen state saved after posting: {}", config.state.file_path);
                }
            }

            info!(
                "RSS check finished: posted={}, skipped_seen={}, failed={}",
                posted_count, skipped_count, failed_count
            );
        }
        Err(e) => {
            error!("Error fetching RSS items: {}", e);
        }
    }
}

async fn post_rss_item(
    http: &HttpClient,
    config: &config::Config,
    item: &rss::RssItem,
) -> Result<()> {
    let content = formatter::format_item_message(item);
    let allowed_mentions = AllowedMentions::default();

    info!(
        "Posting RSS item to Discord channel {}: {}",
        config.discord.channel_id.get(),
        item.title
    );
    let request = http
        .create_message(config.discord.channel_id)
        .content(&content)
        .allowed_mentions(Some(&allowed_mentions));

    if let Err(e) = request.await {
        error!(
            "Discord post failed ({}): {}",
            classify_discord_error(&e),
            e
        );
        return Err(e.into());
    }

    info!("Discord post succeeded: {}", item.title);

    Ok(())
}

fn classify_discord_error(error: &DiscordHttpError) -> &'static str {
    match error.kind() {
        ErrorType::Unauthorized => "token invalid",
        ErrorType::Response { status, .. } => match status.get() {
            403 => "permission denied",
            404 => "channel not found",
            429 => "rate limited",
            _ => "unknown error",
        },
        ErrorType::Validation => "message too long",
        ErrorType::RequestCanceled | ErrorType::RequestError | ErrorType::RequestTimedOut => {
            "network error"
        }
        _ => "unknown error",
    }
}
