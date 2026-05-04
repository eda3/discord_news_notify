# discord_news_notify 改善チェックリスト

作成日: 2026-05-04  
対象リポジトリ: `https://github.com/eda3/discord_news_notify`

## 目的

`discord_news_notify` を、現在のMVP状態から **安全に常時運用できる小型RSS/ニュース通知Bot** へ改善する。

このドキュメントは、Codex-CLIに参照させるための改善チェックリストである。  
以降はこのMarkdownを編集しながら、改善項目の追加・分割・完了管理を行う。

---

# 0. 運用ルール

- [ ] Codexに投げる前に、このチェックリストから対象範囲を選ぶ。
- [ ] 1回のCodex実行では、できるだけ小さめのPhase単位に絞る。
- [ ] 完了した項目は `[x]` にする。
- [ ] 実装中に新しい課題が見つかったら「追加課題」に追記する。
- [ ] 仕様判断が必要な項目は、実装せず「判断待ち」に移す。
- [ ] READMEと実装の不一致を見つけた場合は、必ずどちらかを修正する。
- [ ] 機密情報、実トークン、Webhook URL、個人用チャンネルIDはコミットしない。
- [ ] 過剰設計を避ける。
- [ ] 小型Botとしての単純さを維持する。
- [ ] 実装していない機能をREADMEに書かない。

---

# 1. 現状把握メモ

## 1.1 現在の主な構成

- `Cargo.toml`
- `Cargo.lock`
- `.gitignore`
- `README.md`
- `LICENSE`
- `config/default.toml`
- `src/main.rs`
- `src/config.rs`
- `src/rss.rs`

## 1.2 現状の主な機能

- RSSフィードを取得する。
- 新規記事を検出する。
- Discordチャンネルへ記事を投稿する。
- `.env` からDiscord Bot Tokenを読む。
- `config/default.toml` から以下を読む。
  - Discord投稿先チャンネルID
  - RSS feed URL
  - ポーリング間隔

## 1.3 現状の主な懸念

- Discord Gateway待機とRSSチェックが直列になっている可能性がある。
- RSS巡回がDiscordイベントの有無に影響される恐れがある。
- `handle_message` が空実装で、Gateway接続が不要な可能性が高い。
- `MESSAGE_CONTENT` intent を使っているが、RSS通知用途だけなら不要な可能性が高い。
- RSS取得に同期HTTPクライアントを使っている。
- async関数内で同期HTTPを直接呼んでおり、Tokio runtimeをブロックする恐れがある。
- 重複管理がメモリ上の `HashSet` のみで、再起動後に既読状態が消える。
- 初回起動時に既存記事をまとめて投稿してしまう可能性がある。
- Discordの投稿文字数制限を考慮していない。
- RSS本文由来の `@everyone` / `@here` / role mention事故を防げていない可能性がある。
- READMEのclone URLや説明が実リポジトリと一致していない。
- `.env.example` がない。
- `config/default.toml` に環境依存値が入っている。
- テスト、CI、運用手順が不足している。

---

# 2. 改善方針

## 2.1 基本方針

- [ ] まず安全に常駐できることを優先する。
- [ ] 依存関係を増やしすぎない。
- [ ] 小型Rust Botとして自然な構成にする。
- [ ] 大規模フレームワーク化しない。
- [ ] 設定・状態・RSS取得・Discord投稿を適度に分離する。
- [ ] READMEと実装を一致させる。
- [ ] テストしやすい関数へ分割する。

## 2.2 優先順位

1. 現状のビルド確認
2. Gateway不要化、またはRSS巡回タスク分離
3. RSS取得処理の非同期/ブロッキング整理
4. 重複管理の永続化
5. Discord投稿の安全化
6. 設定管理の整理
7. テスト追加
8. CI追加
9. README更新
10. Docker/systemdなど運用補助

---

# Phase 0: 現状確認

## 0.1 ビルド・静的チェック

- [x] `cargo check` を実行する。
- [x] `cargo fmt --check` を実行する。
- [x] `cargo clippy --all-targets --all-features -- -D warnings` を実行する。
- [x] `cargo test` を実行する。
- [x] 各コマンドの結果を記録する。

## 0.2 失敗分類

失敗がある場合、原因を分類する。

- [ ] コンパイルエラー
- [ ] API変更・依存関係不整合
- [ ] 未使用import
- [ ] 未使用dependency
- [ ] 設定ファイル不足
- [ ] テスト不足
- [ ] その他

## 0.3 現状フローの記録

- [x] 設定読み込みの流れを確認する。
- [x] Discord client初期化の流れを確認する。
- [x] Gateway event loopの必要性を確認する。
- [x] RSS fetchの流れを確認する。
- [x] 重複判定の流れを確認する。
- [x] Discord投稿の流れを確認する。
- [x] 現状フローをREADMEまたは `docs/current-flow.md` に短く記録する。

---

# Phase 1: Discord Gateway設計の見直し

## 1.1 Gatewayが必要か判断する

- [x] `handle_message` の実装状況を確認する。
- [x] Botが受信メッセージを処理しているか確認する。
- [x] RSS通知だけが目的か確認する。
- [x] Gateway接続が不要か判断する。

## 1.2 Gateway不要の場合

RSS通知だけが目的なら、Discord Gatewayは不要な可能性が高い。

- [x] `twilight-gateway` を削除する。
- [x] `twilight-model` のGatewayイベント関連importを削除する。
- [x] `Intents::GUILD_MESSAGES` を削除する。
- [x] `Intents::MESSAGE_CONTENT` を削除する。
- [x] `Shard` 初期化を削除する。
- [x] `handle_message` を削除する。
- [x] Discord投稿はHTTP clientのみで行う。
- [x] READMEからGatewayやMessage Content Intentが必要に見える記述を削る。

## 1.3 Gatewayを残す場合

将来的にBotコマンドやメッセージ受信を扱うなら、Gatewayを残してもよい。  
ただし、RSS巡回とGateway処理は分離する。

- [ ] RSS巡回とGatewayイベント処理を別タスクに分離する。
- [x] `tokio::spawn` または `tokio::select!` を使う。
- [x] RSS巡回には `tokio::time::interval` を使う。
- [x] Discordイベントが来なくてもRSSチェックが動くことを確認する。
- [x] Ctrl+Cでgraceful shutdownできるようにする。

## 1.4 受け入れ条件

- [x] RSS巡回がDiscordイベント到着に依存しない。
- [x] 不要なGateway関連依存が残っていない。
- [x] Message Content Intentが不要なら削除されている。
- [x] `cargo check` が通る。
- [x] `cargo clippy` が通る。

---

# Phase 2: RSS取得処理の改善

## 2.1 HTTPクライアント方針

現状は同期HTTPクライアントをasync関数内で呼んでいる可能性がある。

- [x] 現在のHTTP取得処理を確認する。
- [x] 以下のどちらかを選ぶ。
  - [ ] A案: `reqwest` を導入して完全async化する。
  - [x] B案: 現在の同期HTTPを維持し、`tokio::task::spawn_blocking` に隔離する。
- [ ] 小規模Botとして自然ならA案を優先する。
- [x] 依存を増やしたくない場合はB案を検討する。

## 2.2 HTTP取得の堅牢化

- [x] request timeoutを設定する。
- [x] connect timeoutを設定する。
- [x] User-Agentを設定する。
- [x] HTTPステータスコードが2xx以外の場合はエラーにする。
- [x] エラー時にfeed URLとstatusをログに出す。
- [x] ネットワーク失敗時にBot全体が落ちないようにする。
- [ ] 必要なら連続失敗回数をログに出す。
- [x] RSS取得失敗後、次回ポーリングで復帰できるようにする。

## 2.3 RSS/Atom対応方針

- [x] READMEがRSS/Atom対応を謳っているか確認する。
- [x] 実装がRSSのみか、Atomにも対応しているか確認する。
- [x] RSSのみならREADMEをRSSのみへ修正する。
- [ ] Atom対応も必要なら `feed-rs` 等の導入を検討する。
- [ ] Atom対応を入れる場合、Atom用テストフィードを追加する。

## 2.4 日付パース改善

- [x] `pub_date` パース失敗時に1970年固定へ黙って落とさない。
- [x] `pub_date` は `Option<DateTime<Utc>>` にするか検討する。
- [x] 日付不明の場合の投稿表示を決める。
- [x] パース失敗時はdebug/warnログに出す。

## 2.5 受け入れ条件

- [x] RSS取得がTokio runtimeを不必要にブロックしない。
- [x] HTTP timeoutが設定されている。
- [x] RSS取得失敗時もBotが継続する。
- [x] RSS/Atom対応状況がREADMEと一致している。
- [x] 日付パース失敗時に不自然な1970年扱いにならない。

---

# Phase 3: 重複管理の永続化

## 3.1 状態管理モジュール追加

- [x] `src/state.rs` を追加する。
- [x] 既読記事IDを保存する構造体を定義する。
- [x] JSONファイルへ保存する。
- [x] JSONファイルから読み込む。
- [x] 保存先パスを設定可能にする。
- [x] デフォルト保存先を決める。
  - 候補: `data/seen_items.json`

## 3.2 記事ID生成ルール

- [x] RSS itemの `guid` を取得できるようにする。
- [x] `guid` があればIDに使う。
- [x] `guid` がなければ `link` を使う。
- [x] `link` もなければ `title + pub_date` を使う。
- [ ] 必要なら最終的にhash化する。
- [x] ID生成ロジックを関数化する。
- [x] ID生成のテストを追加する。

## 3.3 既読化タイミング

- [x] 投稿成功後に既読化する。
- [x] 投稿失敗時には既読化しない。
- [x] 状態保存失敗時の扱いを決める。
- [x] 状態保存失敗時はエラーとしてログに出す。

## 3.4 初回起動ポリシー

- [x] 初回起動時の挙動を設定可能にする。
- [x] `skip_existing_on_first_run = true` を検討する。
- [x] 初回は既存記事を既読化だけして投稿しないモードを用意する。
- [x] READMEに初回起動時の挙動を書く。

## 3.5 保存データの肥大化対策

- [x] 保存件数の上限を設定する。
- [x] 保存期間の上限を設定するか検討する。
- [x] 古いIDをpruneする。
- [x] pruneのテストを追加する。

## 3.6 受け入れ条件

- [x] 再起動後も重複投稿しない。
- [x] 投稿失敗時に既読化されない。
- [x] 初回起動時に過去記事を大量投稿しない設定がある。
- [x] 状態ファイルが壊れている場合の挙動が明確である。
- [x] 状態管理のユニットテストがある。

---

# Phase 4: Discord投稿の安全化

## 4.1 投稿フォーマット分離

- [x] `src/formatter.rs` を追加するか検討する。
- [x] 投稿本文生成を `main.rs` から分離する。
- [x] `format_item_message` のような関数を作る。
- [x] 表示項目を整理する。
  - [x] title
  - [x] link
  - [x] description
  - [x] pub_date
  - [x] feed name

## 4.2 文字数制限対策

- [x] Discord contentの文字数制限を守る。
- [x] titleを適切に短縮する。
- [x] descriptionを適切に短縮する。
- [x] linkを必ず残す。
- [x] 長文RSSでも投稿失敗しないようにする。
- [x] 文字数制限のユニットテストを追加する。

## 4.3 メンション事故対策

- [x] `allowed_mentions` を明示的に空にする。
- [x] `@everyone` を無効化する。
- [x] `@here` を無効化する。
- [x] role mentionを無効化する。
- [x] user mentionを無効化する。
- [x] RSS本文側のsanitizeも検討する。
- [x] メンション混入テストを追加する。

## 4.4 HTML/Markdown整形

- [x] description内のHTMLタグをどう扱うか決める。
- [x] HTMLを除去するか、Discord向けに整形する。
- [x] 改行を整える。
- [x] Markdown崩れを防ぐ。
- [x] 空description時の表示を決める。

## 4.5 Embed投稿の検討

- [x] contentのみで十分か判断する。
- [ ] Embedを使う場合の構造を決める。
  - [ ] embed title
  - [ ] embed url
  - [ ] embed description
  - [ ] timestamp
  - [ ] footer
- [ ] Embedの文字数制限も考慮する。
- [x] まずはcontent方式を安全化し、Embedは次フェーズでもよい。

## 4.6 Discord APIエラー分類

- [x] token invalid
- [x] permission denied
- [x] channel not found
- [x] rate limited
- [x] message too long
- [x] network error
- [x] unknown error

## 4.7 受け入れ条件

- [x] RSS本文にメンションが含まれていてもDiscordで通知事故が起きない。
- [x] 長文記事でもDiscord投稿制限に引っかからない。
- [x] 投稿本文生成のテストがある。
- [x] 投稿失敗時に原因分類がログに出る。

---

# Phase 5: 設定管理の整理

## 5.1 サンプル設定

- [x] `.env.example` を追加する。
- [x] `DISCORD_BOT_TOKEN=replace_me` を書く。
- [x] Webhook方式は未採用のため `DISCORD_WEBHOOK_URL` は追加しない。
- [x] `config/default.example.toml` を追加する。
- [x] 実チャンネルIDをサンプルから外す。
- [x] `.gitignore` に `config/local.toml` などを追加するか検討する。

## 5.2 設定値検証

- [x] tokenが空でないことを検証する。
- [x] channel_idがDiscord snowflakeとしてparseできることを検証する。
- [x] feed_urlがURLとして妥当なことを検証する。
- [x] poll_interval_secondsが短すぎないことを検証する。
- [x] state file pathが妥当なことを検証する。
- [x] 設定不備時のエラー文を分かりやすくする。

## 5.3 型の改善

- [x] `channel_id` を起動時にparseする。
- [x] 内部では可能なら `Id<ChannelMarker>` で保持する。
- [x] `poll_interval_seconds` の最小値を定義する。
- [x] 設定構造体に `Clone` が必要か確認する。

## 5.4 複数フィード対応の判断

- [x] MVPでは単一feed維持でよいか判断する。
- [x] 複数feed対応を入れるか判断する。
- [x] 複数feed対応を今入れない場合、future workに残す。

## 5.5 受け入れ条件

- [x] `.env.example` がある。
- [x] サンプル設定に実運用値が含まれていない。
- [x] 不正設定では起動時に分かりやすく失敗する。
- [x] READMEの設定手順と実装が一致している。

---

# Phase 6: 依存関係とRustプロジェクト整備

方針メモ:

- `twilight` aggregate crate と `twilight-gateway` は現在の `Cargo.toml` に存在しないため、追加削除は不要。
- `serde_json` は `src/state.rs` のJSON状態保存・読み込みで使用しているため維持する。
- `anyhow` はアプリ境界と小規模な内部処理で維持する。現時点では独自エラー型や `thiserror` は導入しない。
- `log/env_logger` は小型Botとして十分なため維持する。現時点では `tracing` へ移行しない。
- Rust 2024 edition のため、最小Rustバージョンは `rust-version = "1.85"` とREADMEで明示する。
- `rust-toolchain.toml` はローカルtoolchain固定を増やすため、この段階では追加しない。

## 6.1 未使用依存の整理

- [x] `twilight` aggregate crateが必要か確認する。
- [x] `twilight-gateway` が必要か確認する。
- [x] `serde_json` が必要か確認する。
- [x] その他の未使用依存を確認する。
- [x] 不要な依存を削除する。
- [x] `Cargo.lock` を更新する、または更新不要を確認する。

## 6.2 Rustバージョン明示

- [x] `Cargo.toml` に `rust-version` を追加するか検討する。
- [x] `rust-toolchain.toml` を追加するか検討する。
- [x] `edition = "2024"` を維持する場合、READMEに必要Rustバージョンを書く。

## 6.3 ログ・エラー方針

- [x] `anyhow` をアプリ境界で使う方針は維持してよい。
- [x] 内部エラー型に `thiserror` が必要か検討する。
- [x] `log/env_logger` を維持するか、`tracing` へ寄せるか判断する。
- [x] この段階での `tracing` 移行は必須にしない。

## 6.4 受け入れ条件

- [x] 未使用依存が整理されている。
- [x] `cargo clippy --all-targets --all-features -- -D warnings` が通る。
- [x] Rustバージョン要件がREADMEか設定ファイルに明示されている。

---

# Phase 7: テスト追加

## 7.1 RSSパーステスト

- [ ] 正常なRSSをパースできる。
- [ ] titleなしitemを扱える。
- [ ] linkなしitemを扱える。
- [ ] descriptionなしitemを扱える。
- [ ] pubDate不正itemを扱える。
- [ ] Atom対応するならAtomもテストする。

## 7.2 重複判定テスト

- [ ] 初回itemは新規扱いになる。
- [ ] 同一IDは重複扱いになる。
- [ ] 再起動後も重複扱いになる。
- [ ] 投稿失敗時は既読化されない。

## 7.3 状態保存テスト

- [ ] 空状態から保存できる。
- [ ] 保存後に再読み込みできる。
- [ ] 壊れたJSONを扱える。
- [ ] 保存件数上限が効く。
- [ ] pruningが効く。

## 7.4 設定読み込みテスト

- [ ] envありで読み込める。
- [ ] envなしで分かりやすく失敗する。
- [ ] channel_id不正で失敗する。
- [ ] feed_url不正で失敗する。
- [ ] interval不正で失敗する。

## 7.5 投稿フォーマットテスト

- [ ] 長文descriptionを短縮する。
- [ ] titleを短縮する。
- [ ] linkを残す。
- [ ] HTML混入を扱う。
- [ ] メンション混入を扱う。
- [ ] 空title/descriptionを扱う。

## 7.6 受け入れ条件

- [ ] `cargo test` が通る。
- [ ] ネットワークを使うテストはmock化されている。
- [ ] 主要ロジックがユニットテストで保護されている。

---

# Phase 8: CI追加

## 8.1 GitHub Actions

- [ ] `.github/workflows/ci.yml` を追加する。
- [ ] pushで実行する。
- [ ] pull_requestで実行する。

## 8.2 CI内容

- [ ] checkout
- [ ] Rust toolchain setup
- [ ] Cargo cache
- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo check`

## 8.3 追加チェック

- [ ] `cargo audit` を追加するか検討する。
- [ ] Dependabotを追加するか検討する。

## 8.4 受け入れ条件

- [ ] CIが追加されている。
- [ ] CIが成功する。
- [ ] READMEにCIバッジを追加するか検討する。

---

# Phase 9: README更新

## 9.1 基本情報修正

- [ ] clone URLを実リポジトリに修正する。
- [ ] ディレクトリ名を実リポジトリに合わせる。
- [ ] プロジェクト名を統一する。
- [ ] 実装していない機能説明を削る。
- [ ] RSS/Atom対応状況を正確に書く。

## 9.2 セットアップ手順

- [ ] 必要Rustバージョンを書く。
- [ ] `.env.example` から `.env` を作る手順を書く。
- [ ] `config/default.example.toml` からローカル設定を作る手順を書く。
- [ ] Discord Bot Tokenの作り方を書く。
- [ ] 必要権限を書く。
- [ ] `cargo run` での起動手順を書く。

## 9.3 初回起動・重複管理

- [ ] 初回起動時に既存記事を投稿するのか書く。
- [ ] 初回起動時に既存記事をスキップするのか書く。
- [ ] 状態ファイルの場所を書く。
- [ ] 状態ファイルを削除した場合の挙動を書く。

## 9.4 常時運用

- [ ] systemd運用例を書くか検討する。
- [ ] Docker運用例を書くか検討する。
- [ ] `RUST_LOG=info` の使い方を書く。
- [ ] ログ確認方法を書く。

## 9.5 トラブルシューティング

- [ ] token invalid
- [ ] missing permissions
- [ ] channel not found
- [ ] feed fetch failed
- [ ] duplicate state file broken
- [ ] message too long
- [ ] rate limit

## 9.6 セキュリティ注意

- [ ] tokenをcommitしない。
- [ ] webhook URLを公開しない。
- [ ] RSS本文由来のmention事故に注意する。
- [ ] `.env` は `.gitignore` に含める。

## 9.7 受け入れ条件

- [ ] READMEだけ読めばセットアップできる。
- [ ] READMEの説明と実装が一致している。
- [ ] clone URLが正しい。
- [ ] サンプル設定が安全である。

---

# Phase 10: 運用補助

## 10.1 Docker

- [ ] Dockerfileを追加するか判断する。
- [ ] マルチステージビルドにする。
- [ ] 実行用イメージを小さくする。
- [ ] state保存用volumeを考慮する。
- [ ] docker-composeサンプルを追加するか判断する。

## 10.2 systemd

- [ ] systemd unitサンプルを追加するか判断する。
- [ ] WorkingDirectoryを明示する。
- [ ] EnvironmentFileを使う。
- [ ] Restart=alwaysを設定する。
- [ ] ログ確認方法を書く。

## 10.3 受け入れ条件

- [ ] 少なくとも1つの常時運用方法がREADMEに書かれている。
- [ ] 再起動後も重複投稿しない。
- [ ] 状態ファイルが永続化される。

---

# 判断待ち

現時点で、実装前に方針を決めたい項目。

- [ ] Discord Bot Token方式を維持するか、Webhook方式へ寄せるか。
- [ ] RSSのみ対応にするか、Atom対応も正式に入れるか。
- [ ] 複数フィード対応を今入れるか、future workに回すか。
- [ ] Embed投稿を今入れるか、content投稿を安全化するだけにするか。
- [ ] Docker運用を入れるか、systemd運用を入れるか。

---

# 追加課題

作業中に見つかった課題をここに追記する。

- [ ] 未記入

---

# 完了ログ

## 2026-05-04

- [ ] 初版作成。

---

# Codex-CLI実行用プロンプト雛形

以下は、Codex-CLIへ投げるときの基本テンプレート。  
実際に投げるときは、対象Phaseだけ残して使う。

```markdown
あなたは熟練Rustエンジニア兼コードレビュアーです。
対象リポジトリは `https://github.com/eda3/discord_news_notify` です。

このリポジトリを、提示された改善チェックリストに従って改善してください。
今回は以下のPhaseだけを対象にしてください。

対象Phase:
- Phase X: <ここに対象Phase名を書く>

作業ルール:
- まず現状確認コマンドを実行してください。
- 変更は最小限かつ安全にしてください。
- 不要な大規模リファクタリングは避けてください。
- 機密情報を追加しないでください。
- 実装していない機能をREADMEに書かないでください。
- 判断が必要なものは勝手に大きく決めず、理由付きで保留してください。
- 作業後、実行したテストコマンドと結果を報告してください。

完了条件:
- 対象Phaseのチェック項目が満たされていること。
- `cargo fmt --check` が成功すること。
- `cargo clippy --all-targets --all-features -- -D warnings` が成功すること。
- `cargo test` が成功すること。
- 変更内容と残課題を報告すること。

最終報告形式:

### Summary
- 何を変更したか

### Behavior Changes
- 実行時の挙動がどう変わったか

### Tests
- 実行したコマンド
- 結果

### Remaining Issues
- 残した課題
- 理由

### Files Changed
- 主要変更ファイル
