# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## プロジェクト概要

Discord RSS Notify Botは、RSSフィードを定期的に監視し、新着記事をDiscordチャンネルに自動投稿するRust製のボット。

## ビルド・実行コマンド

```bash
# ビルド
cargo build
cargo build --release

# 実行
cargo run

# テスト
cargo test

# 単一テスト実行
cargo test <test_name>

# リント
cargo clippy
```

## 設定

起動前に以下が必要：

1. `.env_example` を `.env` にコピーし、`DISCORD_BOT_TOKEN` を設定
2. `config/default.toml` で `channel_id`（Discord チャンネルID）と `feed_url`（RSS URL）を設定

```toml
# config/default.toml
[discord]
channel_id = "..."

[rss]
feed_url = "https://..."
poll_interval_seconds = 300
```

## アーキテクチャ

### データフロー

```
起動 → Config読み込み (.env + config/default.toml + 任意の config/local.toml)
     → Twilight HTTP クライアント初期化
     → RssProcessor 初期化
     → SeenState 読み込み
     → 無限ループ:
         RSS取得 → 既読フィルタ → Discord投稿 → 状態保存 → interval待機
```

### 主要モジュール

- **`main.rs`**: エントリポイント。RSS ポーリング、既読判定、Discord投稿、Ctrl+C終了を管理。
- **`config.rs`**: `config/default.toml`、任意の`config/local.toml`、`DISCORD_BOT_TOKEN` 環境変数を読み込み、設定値を検証する。
- **`rss.rs`**: `ureq` で RSS フィードを取得・パース。RSS item IDを生成する。
- **`state.rs`**: JSON状態ファイルで既読itemを永続化し、再起動後の重複投稿を防ぐ。
- **`formatter.rs`**: Discord投稿本文を生成し、文字数制限とメンション抑止を扱う。
- **`google_alert.rs`**: 現在未使用のスタブ。将来の Google Alerts 対応用。

### Discord ライブラリ

Twilight SDK を使用（serenity や poise ではない）。
- `twilight-http`: REST API 呼び出し
- `twilight-model`: Discord のデータ型
