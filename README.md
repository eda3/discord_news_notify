# discord_news_notify

RSS feed を定期的に確認し、新着記事を Discord チャンネルへ投稿する小型Botです。

## 現在の機能

- 単一RSS feedの定期チェック
- 新着記事のDiscord投稿
- JSON状態ファイルによる重複投稿防止
- `.env`によるDiscord Bot Token管理
- `config/default.toml`と任意の`config/local.toml`による投稿先チャンネルとRSS設定

現時点ではRSSのみ対応しています。Atom feed対応は未実装です。

## 必要なもの

- Rust toolchain
- Discord Bot Token
- 投稿先DiscordチャンネルID

## セットアップ

```bash
git clone https://github.com/eda3/discord_news_notify.git
cd discord_news_notify
cargo build
```

`.env_example`を`.env`にコピーし、Bot Tokenを設定します。

```env
DISCORD_BOT_TOKEN=replace_me
```

`config/default.example.toml`を参考に、`config/default.toml`または`config/local.toml`を設定します。
`config/local.toml`はgit管理対象外です。

```toml
[discord]
channel_id = "your_discord_channel_id"

[rss]
feed_url = "https://example.com/feed.xml"
poll_interval_seconds = 300

[state]
file_path = "data/seen_items.json"
max_seen_items = 10000
skip_existing_on_first_run = true
```

## 実行

```bash
cargo run
```

終了するときは`Ctrl+C`を押します。

## 実装メモ

- Discord Gatewayは使っていません。
- Discord投稿はTwilight HTTP clientのみで行います。
- 起動時に設定値を検証し、不備がある場合は分かりやすいエラーで停止します。
- `channel_id`は起動時にTwilightの`Id<ChannelMarker>`へ変換します。
- `poll_interval_seconds`の最小値は60秒です。
- RSS取得は`ureq`を`tokio::task::spawn_blocking`内で実行します。
- HTTP取得にはconnect timeout、全体timeout、User-Agentを設定しています。
- `pub_date`のパースに失敗した場合は日付不明として扱い、1970年固定値にはしません。
- 記事IDは`guid`、`link`、`title + pub_date`の順で生成します。
- 投稿本文は`src/formatter.rs`で生成し、Discordのcontent文字数制限内に収めます。
- 投稿時は`allowed_mentions`を空にして、RSS本文由来のメンション通知を無効化します。
- RSS description内のHTMLタグは投稿前に除去し、空の場合は既定文を表示します。
- 投稿成功後に既読状態を`data/seen_items.json`へ保存します。
- 投稿失敗時は既読化しません。
- `skip_existing_on_first_run = true`の場合、初回起動時は既存記事を既読化するだけで投稿しません。
- 状態ファイルが壊れている場合は、起動時にエラーとして停止します。
- 複数feed対応はfuture workとして残し、MVPでは単一feedを維持します。

## 検証コマンド

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
