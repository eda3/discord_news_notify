# discord_news_notify

RSS feed を定期的に確認し、新着記事を Discord チャンネルへ投稿する小型Botです。

## 現在の機能

- 単一RSS feedの定期チェック
- はてなブックマーク数に応じたthreshold別Discord投稿
- JSON状態ファイルによるthreshold単位の重複投稿防止
- `.env`によるDiscord Bot Token管理
- `config/default.toml`と任意の`config/local.toml`によるthreshold別投稿先チャンネルとRSS設定

現時点ではRSSのみ対応しています。Atom feed対応は未実装です。

## 必要なもの

- Rust 1.85以上のtoolchain（edition 2024を使用）
- Discord Bot Token
- threshold別の投稿先DiscordチャンネルID

## セットアップ

```bash
git clone https://github.com/eda3/discord_news_notify.git
cd discord_news_notify
cargo build
```

Discord Developer PortalでBotを作成し、Bot Tokenを取得します。

1. <https://discord.com/developers/applications> でApplicationを作成する。
2. BotページでBotを作成し、Tokenを取得する。
3. OAuth2 URL Generatorで`bot` scopeを選ぶ。
4. Bot Permissionsで`Send Messages`を付け、投稿先サーバーへ招待する。

`.env_example`を`.env`にコピーし、Bot Tokenを設定します。`.env`はgit管理対象外です。

```env
DISCORD_BOT_TOKEN=replace_me
```

`config/default.example.toml`を参考に、`config/default.toml`または`config/local.toml`を設定します。個人設定を分けたい場合は`config/local.toml`を使ってください。`config/local.toml`はgit管理対象外です。

```toml
[discord]

[rss]
feed_url = "https://example.com/feed.xml"
poll_interval_seconds = 300

[hatena]
thresholds = [1, 5, 20]
count_api_timeout_seconds = 10
candidate_retention_days = 7

[[notification.channels]]
threshold = 1
channel_id = "your_threshold_1_channel_id"

[[notification.channels]]
threshold = 5
channel_id = "your_threshold_5_channel_id"

[[notification.channels]]
threshold = 20
channel_id = "your_threshold_20_channel_id"

[state]
file_path = "data/articles.json"
skip_existing_on_first_run = true
```

`notification.channels.channel_id`はDiscordの開発者モードを有効にし、投稿先チャンネルを右クリックしてコピーします。`discord.channel_id`は使いません。

## threshold別通知

RSSで見つけた記事を候補として保存し、記事URLのはてなブックマーク数を取得します。`count >= threshold`になった未投稿thresholdだけ、対応するDiscordチャンネルへ投稿します。

- `1`: これまでの新着通知に相当する初回検知ライン
- `5`: 初動注目ライン
- `20`: 話題化ライン

同じ記事でも`1`、`5`、`20`へ順に到達すれば最大3回通知されます。同じ記事の同じthresholdは、状態ファイルに保存されている限り二重通知されません。同じchannel_idを複数thresholdに設定することは可能ですが、運用上は別チャンネルを推奨します。

はてなAPIの取得に失敗した記事は投稿せず、候補として残して次回ポーリングで再試行します。Discord投稿に失敗したthresholdも投稿済みにしません。

## 実行

```bash
cargo run
```

終了するときは`Ctrl+C`を押します。

ログを増やしたい場合は`RUST_LOG`を指定します。

```bash
RUST_LOG=info cargo run
RUST_LOG=debug cargo run
```

## 初回起動と重複管理

`skip_existing_on_first_run = true`の場合、初回起動時はRSS feed内の既存記事を投稿せず、候補として`data/articles.json`へ保存します。その時点ではてなブックマーク数を取得できた記事は、到達済みthresholdを投稿済み扱いにして大量通知を避けます。

`skip_existing_on_first_run = false`にすると、初回起動時からthreshold到達済みの記事を投稿対象にします。

状態ファイルの保存先は`state.file_path`で変更できます。既定値は`data/articles.json`です。このファイルを削除するとthreshold投稿状態が失われ、次回起動時は初回起動として扱われます。

`candidate_retention_days`を過ぎた候補はpruneされます。prune後に同じ記事がRSSに再登場した場合、再通知される可能性があります。

状態ファイルが壊れている場合、Botは起動時に停止します。復旧するにはファイル内容を修正するか、必要に応じて状態ファイルを削除してください。

## 常時運用

常時運用する場合は、作業ディレクトリをこのリポジトリに固定し、`.env`と設定ファイルを読める状態で`cargo run --release`またはビルド済みバイナリを起動してください。

systemdやDockerの設定例はまだ同梱していません。Phase 9では運用方針の説明に留め、具体的なunit/Dockerfile追加はPhase 10の対象です。

ログは標準出力/標準エラーに出ます。systemdで動かす場合は`journalctl`、Dockerで動かす場合は`docker logs`など、起動方法に合わせて確認してください。

## トラブルシューティング

- `token invalid`: `.env`の`DISCORD_BOT_TOKEN`が正しいか確認してください。Tokenを再発行した場合は`.env`も更新します。
- `missing permissions`: Botに投稿先チャンネルで`Send Messages`権限があるか確認してください。
- `channel not found`: `notification.channels.channel_id`が投稿先チャンネルIDか、Botがそのサーバー/チャンネルへアクセスできるか確認してください。
- `feed fetch failed`: `rss.feed_url`がHTTP/HTTPS URLか、RSS feedとして取得できるか確認してください。
- `Hatena count API request failed`: はてなAPIへの接続に失敗しています。Botは停止せず、次回ポーリングで再試行します。
- `failed to parse state file`: 状態ファイルのJSONが壊れています。内容を修正するか、初回扱いに戻してよい場合は状態ファイルを削除してください。
- `message too long`: 投稿本文はDiscordのcontent制限内に収める実装です。発生した場合はRSS本文やリンクが想定外に長くないか確認してください。
- `rate limited`: Discord APIの制限です。通常は時間を置いて再試行されます。短すぎるポーリング間隔は避けてください。

## セキュリティ注意

- Bot Tokenをcommitしないでください。
- `.env`は`.gitignore`に含まれています。
- webhook URLは現在使っていません。将来導入する場合も公開リポジトリへcommitしないでください。
- RSS由来の本文にはメンションが含まれる可能性があります。投稿時は`allowed_mentions`を空にし、本文中のメンション文字列も無効化しています。

## 実装メモ

- Discord Gatewayは使っていません。
- Discord投稿はTwilight HTTP clientのみで行います。
- 起動時に設定値を検証し、不備がある場合は分かりやすいエラーで停止します。
- `notification.channels.channel_id`は起動時にTwilightの`Id<ChannelMarker>`へ変換します。
- `hatena.thresholds`と`notification.channels`は完全一致が必要です。
- thresholdは空配列、0、重複、降順を起動時にエラーにします。
- `poll_interval_seconds`の最小値は60秒です。
- RSS取得は`ureq`を`tokio::task::spawn_blocking`内で実行します。
- HTTP取得にはconnect timeout、全体timeout、User-Agentを設定しています。
- `pub_date`のパースに失敗した場合は日付不明として扱い、1970年固定値にはしません。
- 記事IDは`guid`、`link`、`title + pub_date`の順で生成します。
- 投稿本文は`src/formatter.rs`で生成し、Discordのcontent文字数制限内に収めます。
- 投稿時は`allowed_mentions`を空にして、RSS本文由来のメンション通知を無効化します。
- RSS description内のHTMLタグは投稿前に除去し、空の場合は既定文を表示します。
- 投稿成功後に該当thresholdだけを`data/articles.json`へ保存します。
- 投稿失敗時は該当thresholdを投稿済みにしません。
- `skip_existing_on_first_run = true`の場合、初回起動時は既存記事を投稿せず、到達済みthresholdを投稿済み扱いにします。
- 状態ファイルが壊れている場合は、起動時にエラーとして停止します。
- 複数feed対応はfuture workとして残し、MVPでは単一feedを維持します。

## 検証コマンド

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
