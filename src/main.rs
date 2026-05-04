mod config;
mod formatter;
mod hatena;
mod rss;
mod state;

use anyhow::Result;
use log::{error, info, warn};
use state::{ArticleSnapshot, ArticleStateStore};
use std::collections::BTreeMap;
use std::time::Duration;
use twilight_http::Client as HttpClient;
use twilight_http::error::{Error as DiscordHttpError, ErrorType};
use twilight_model::channel::message::AllowedMentions;
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting Discord RSS Notify Bot...");

    let config = config::Config::load()?;
    info!("Configuration loaded successfully.");
    info!(
        "Runtime config: feed_url={}, poll_interval_seconds={}, thresholds={:?}, state_file={}, skip_existing_on_first_run={}",
        config.rss.feed_url,
        config.rss.poll_interval_seconds,
        config.hatena.thresholds,
        config.state.file_path,
        config.state.skip_existing_on_first_run
    );

    let http = HttpClient::new(config.discord.token.clone());
    let rss_processor = rss::RssProcessor::new();
    let hatena_client =
        hatena::HatenaClient::new(Duration::from_secs(config.hatena.count_api_timeout_seconds));
    let mut state = ArticleStateStore::load(
        &config.state.file_path,
        config.hatena.candidate_retention_days,
    )?;
    info!(
        "Article state loaded: first_run={}, articles={}",
        state.is_first_run(),
        state.len()
    );
    let mut interval = tokio::time::interval(Duration::from_secs(config.rss.poll_interval_seconds));
    info!("RSS polling loop started. Press Ctrl+C to stop.");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received.");
                if let Err(e) = state.save() {
                    warn!("Failed to save article state during shutdown: {}", e);
                }
                break;
            }
            _ = interval.tick() => {
                check_rss_feed(&http, &config, &rss_processor, &hatena_client, &mut state).await;
            }
        }
    }

    Ok(())
}

async fn check_rss_feed(
    http: &HttpClient,
    config: &config::Config,
    rss_processor: &rss::RssProcessor,
    hatena_client: &hatena::HatenaClient,
    state: &mut ArticleStateStore,
) {
    info!("RSS check started: {}", config.rss.feed_url);

    match rss_processor.fetch_items(&config.rss.feed_url).await {
        Ok(items) => {
            info!("RSS check fetched {} item(s).", items.len());
            for item in &items {
                state.upsert_rss_item(item);
            }

            if state.is_first_run() && config.state.skip_existing_on_first_run {
                mark_initial_articles_posted(config, hatena_client, state).await;
                state.finish_first_run();
                if let Err(e) = state.save() {
                    error!("Error saving initial article state: {}", e);
                } else {
                    info!(
                        "First run complete: registered {} RSS item(s) without posting.",
                        items.len()
                    );
                }
                return;
            }

            process_candidate_articles(http, config, hatena_client, state).await;
            state.prune();
            if let Err(e) = state.save() {
                warn!("Failed to save article state after RSS check: {}", e);
            }
        }
        Err(e) => {
            error!("Error fetching RSS items: {}", e);
        }
    }
}

async fn mark_initial_articles_posted(
    config: &config::Config,
    hatena_client: &hatena::HatenaClient,
    state: &mut ArticleStateStore,
) {
    let channels = notification_channels(config);

    for article in state.candidate_snapshots() {
        if article.url.trim().is_empty() {
            warn!(
                "Skipping Hatena count check for RSS item without link: {}",
                article.title
            );
            continue;
        }

        match hatena_client.fetch_count(&article.url).await {
            Ok(count) => {
                state.update_bookmark_count(&article.article_id, count);
                state.mark_reached_thresholds_posted(
                    &article.article_id,
                    &config.hatena.thresholds,
                    &channels,
                );
            }
            Err(e) => {
                warn!(
                    "Hatena count fetch failed during first run; article remains unposted for retry: {} ({})",
                    article.title, e
                );
            }
        }
    }
}

async fn process_candidate_articles(
    http: &HttpClient,
    config: &config::Config,
    hatena_client: &hatena::HatenaClient,
    state: &mut ArticleStateStore,
) {
    let mut posted_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;

    for article in state.candidate_snapshots() {
        if article.url.trim().is_empty() {
            skipped_count += 1;
            warn!(
                "Skipping Hatena count check for RSS item without link: {}",
                article.title
            );
            continue;
        }

        let count = match hatena_client.fetch_count(&article.url).await {
            Ok(count) => count,
            Err(e) => {
                failed_count += 1;
                warn!("Hatena count fetch failed: {} ({})", article.title, e);
                continue;
            }
        };

        state.update_bookmark_count(&article.article_id, count);
        let thresholds =
            state.unposted_reached_thresholds(&article.article_id, &config.hatena.thresholds);
        if thresholds.is_empty() {
            skipped_count += 1;
            continue;
        }

        for threshold in thresholds {
            let channel_id = match config.notification.channel_for(threshold) {
                Ok(channel_id) => channel_id,
                Err(e) => {
                    failed_count += 1;
                    error!("Notification routing failed: {}", e);
                    continue;
                }
            };

            match post_threshold_item(http, channel_id, &article, threshold, count).await {
                Ok(()) => {
                    posted_count += 1;
                    state.mark_threshold_posted(&article.article_id, threshold, channel_id);
                    if let Err(e) = state.save() {
                        warn!(
                            "Threshold was posted, but failed to save article state: {}",
                            e
                        );
                    }
                }
                Err(e) => {
                    failed_count += 1;
                    error!("Error posting threshold notification: {}", e);
                }
            }
        }
    }

    info!(
        "RSS check finished: threshold_posts={}, skipped={}, failed={}",
        posted_count, skipped_count, failed_count
    );
}

async fn post_threshold_item(
    http: &HttpClient,
    channel_id: Id<ChannelMarker>,
    article: &ArticleSnapshot,
    threshold: u64,
    bookmark_count: u64,
) -> Result<()> {
    let content = formatter::format_threshold_message(article, threshold, bookmark_count);
    let allowed_mentions = AllowedMentions::default();

    info!(
        "Posting threshold {} item to Discord channel {}: {}",
        threshold,
        channel_id.get(),
        article.title
    );
    let request = http
        .create_message(channel_id)
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

    info!(
        "Discord threshold post succeeded: threshold={}, title={}",
        threshold, article.title
    );

    Ok(())
}

fn notification_channels(config: &config::Config) -> BTreeMap<u64, Id<ChannelMarker>> {
    config
        .notification
        .channels
        .iter()
        .map(|channel| (channel.threshold, channel.channel_id))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::notification_channels;
    use crate::config::{
        Config, DiscordConfig, HatenaConfig, NotificationChannelConfig, NotificationConfig,
        RssConfig, StateConfig,
    };
    use twilight_model::id::Id;

    fn valid_config() -> Config {
        Config {
            discord: DiscordConfig {
                token: "token".to_string(),
            },
            rss: RssConfig {
                feed_url: "https://example.com/feed.xml".to_string(),
                poll_interval_seconds: 300,
            },
            hatena: HatenaConfig {
                thresholds: vec![1, 5, 20],
                count_api_timeout_seconds: 10,
                candidate_retention_days: 7,
            },
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
            state: StateConfig {
                file_path: "target/test-main/articles.json".to_string(),
                skip_existing_on_first_run: true,
            },
        }
    }

    #[test]
    fn notification_channels_routes_thresholds() {
        let channels = notification_channels(&valid_config());

        assert_eq!(channels.get(&1).unwrap().get(), 11);
        assert_eq!(channels.get(&5).unwrap().get(), 55);
        assert_eq!(channels.get(&20).unwrap().get(), 2020);
    }
}
