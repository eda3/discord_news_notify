# Discord RSS Notifier 📰 🤖

RSSフィードの最新情報をDiscordのチャンネルに自動投稿するBotだよ〜！マジ便利✨

## 機能 🎯

- RSS/Atomフィードを定期的にチェック
- 新しい記事を見つけたらDiscordに通知
- 重複投稿を防止する仕組み搭載
- 環境変数で安全にトークン管理
- 設定ファイルで簡単カスタマイズ

## インストール方法 💿

### 前提条件

- Rust toolchain (rustc, cargo)
- Discord Bot トークン

### インストール手順

```bash
# リポジトリをクローン
git clone https://github.com/yourusername/discord_rss_notify.git
cd discord_rss_notify

# 依存関係をインストール
cargo build
```

## 設定方法 ⚙️

### Discord Bot の作成

1. [Discord Developer Portal](https://discord.com/developers/applications) にアクセス
2. 「New Application」をクリック
3. Bot タブを開いて「Add Bot」をクリック
4. トークンをコピー（あとで使うよ！）
5. OAuth2 タブでBotのスコープを設定
6. 必要な権限（メッセージの送信権限など）を付与
7. 生成されたURLを使ってBotをサーバーに招待

### 環境設定

`.env` ファイルを作成して以下の内容を記述：

```
DISCORD_BOT_TOKEN=あなたのボットトークンをここに貼り付け
```

`config/default.toml` ファイルを設定：

```toml
[discord]
# トークンは.envファイルから読み込みます
channel_id = "投稿先のDiscordチャンネルID"

[rss]
feed_url = "購読したいRSSフィードのURL"
poll_interval_seconds = 300  # チェック間隔（秒）
```

## 起動方法 🚀

```bash
cargo run
```

プロセスを継続的に実行するなら：

```bash
nohup cargo run > bot.log 2>&1 &
```

## カスタマイズ 🎨

### メッセージフォーマットの変更

`src/main.rs` の `post_rss_item` 関数内の `format!` 部分を編集すると、投稿メッセージの形式を変更できるよ！

### ポーリング間隔の変更

`config/default.toml` の `poll_interval_seconds` の値を変更して、RSSフィードをチェックする間隔を調整できるよ！

## トラブルシューティング 🔧

### ボットが応答しない場合

1. `.env` ファイルにトークンが正しく設定されているか確認
2. チャンネルIDが正しいか確認
3. ボットがチャンネルにメッセージを送信する権限を持っているか確認

### RSSフィードが取得できない場合

1. フィードURLが有効かブラウザで確認
2. ネットワーク接続を確認
3. フィードの形式がRSS/Atomに対応しているか確認

## ライセンス 📄

MITライセンスだよ！詳細は `LICENSE` ファイルを見てね！

---

マジ使いやすいボットだから、ぜひ楽しんでね〜！🎉 何か質問があればIssueで教えてね！ 