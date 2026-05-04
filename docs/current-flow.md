# Current Flow

This document records the current runtime flow after the Gateway removal.

## Startup

1. Initialize logging with `env_logger`.
2. Load `config/default.toml`, then optional `config/local.toml`.
3. Load `DISCORD_BOT_TOKEN` from `.env` or the process environment.
4. Validate configuration, including token, channel ID, feed URL, poll interval, and state file path.
5. Create a Twilight HTTP client.
6. Create an in-memory `RssProcessor`.
7. Load seen item state from `state.file_path`.
8. Start a `tokio::time::interval` using `poll_interval_seconds`.

If the configured state file exists but cannot be parsed, startup fails with an error. If the file does not exist, the bot treats the run as a first run.

## Main Loop

The bot waits on two async events with `tokio::select!`:

- `Ctrl+C`: log shutdown and exit the loop.
- RSS interval tick: fetch the configured feed and post new items.

There is no Discord Gateway connection in the current flow. Discord is used only through HTTP message creation.

## RSS Fetch

1. `RssProcessor::fetch_items` runs the blocking `ureq` HTTP request inside `tokio::task::spawn_blocking`.
2. The HTTP client uses a connect timeout, overall timeout, and User-Agent.
3. Non-2xx HTTP responses and transport failures are returned as errors that include the feed URL.
4. The response body is parsed as RSS with the `rss` crate.
5. Feed title, item title, link, description, and optional RFC 2822 `pub_date` are extracted.

The current implementation supports RSS. Atom support is not implemented.

## Duplicate Detection

Duplicate detection is backed by a JSON state file:

1. The item ID is generated from `guid`, then `link`, then `title + pub_date`.
2. The item ID is checked against the JSON-backed seen state.
3. If the ID is already present, the item is skipped.
4. If the ID is new, the item is posted.
5. After posting succeeds, the ID is marked as seen and saved.

Posting failures do not mark items as seen. State save failures are logged after a successful post.

When `skip_existing_on_first_run = true`, the first run marks fetched items as seen without posting them. This prevents a new deployment from posting all existing feed entries at once.

## Discord Posting

1. Startup has already parsed `channel_id` into a Twilight `Id<ChannelMarker>`.
2. `formatter::format_item_message` builds a plain content message from feed title, item title, description, optional published date, and link.
3. HTML tags are stripped from text fields, whitespace is normalized, long fields are truncated, and the final content is kept within Discord's 2000-character limit.
4. The message is sent with the Twilight HTTP client and `AllowedMentions::default()` so RSS content cannot trigger `@everyone`, `@here`, role, or user pings.

If posting fails, the error is logged with a coarse category and the bot continues to the next item or next polling tick.

## Validation

Recorded on 2026-05-04:

- `cargo check`: passed.
- `cargo fmt --check`: passed after running `cargo fmt`.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test`: passed. The project currently has 17 unit tests.
