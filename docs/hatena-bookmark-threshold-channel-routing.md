# Hatena Bookmark Threshold Channel Routing 設計チェックリスト

## 1. 目的

- [ ] RSSで検知した記事を候補として保持する
- [ ] はてなブックマーク数が `1`, `5`, `20` 以上に到達したら、それぞれ別のDiscordテキストチャンネルへ通知する
- [ ] 同じ記事でもthresholdごとに最大3回通知されうる
- [ ] 同じ記事の同じthresholdは二重通知しない
- [ ] Discord投稿成功後にだけ、そのthresholdを投稿済みとして保存する

前提: `thresholds = [1, 5, 20]` は確定。小型Botなので、最初から汎用ジョブキューやDBは入れず、現在のJSON状態ファイル方式を発展させる。

## 2. 現状整理

- [x] Discord Gatewayは使わず、Twilight HTTP clientのみで投稿している
- [x] `tokio::time::interval` でRSSをポーリングしている
- [x] `src/rss.rs` に `RssProcessor`, `RssItem`, `item_id()` がある
- [x] `RssItem` は `guid`, `title`, `link`, `description`, `pub_date`, `feed_title` を持つ
- [x] `item_id()` は `guid` → `link` → `title + pub_date` の順で記事IDを作る
- [x] `src/state.rs` は現在 `SeenState` として `id -> seen_at` をJSON保存している
- [x] 状態ファイルは `data/seen_items.json`
- [x] `src/formatter.rs` は投稿文を2000文字以内に整形し、メンション文字列もsanitizeしている
- [x] `src/main.rs` は現在「RSS新着 → 単一channelへ投稿 → 成功したらseen保存」の流れ

## 3. 新仕様概要

- [ ] RSS itemを見つけたら `ArticleStateStore` に候補として登録する
- [ ] 候補記事ごとに、記事URLからはてなブックマーク数を取得する
- [ ] `count >= threshold` で到達判定する
- [ ] 到達済みかつ未投稿のthresholdを昇順で処理する
- [ ] thresholdごとに設定されたDiscord channelへ投稿する
- [ ] 投稿成功したthresholdだけ `posted_thresholds` / `threshold_posts` に保存する
- [ ] API失敗、Discord投稿失敗ではBot全体を落とさず、次回ポーリングで再試行する

## 4. thresholds = [1, 5, 20] の意味

- [ ] `1`: 初回検知・新着検知ライン
  - 現在の「RSSに出た新着記事を通知する」挙動に相当
  - 今後は「1件threshold通知」として状態管理する
  - 投稿先は1件用チャンネル
- [ ] `5`: 初動注目ライン
  - 複数人から反応され始めた記事として通知する
  - 投稿先は5件用チャンネル
- [ ] `20`: 話題化ライン
  - 明確に伸びた記事として通知する
  - 投稿先は20件用チャンネル

判定は必ず「ちょうど」ではなく「以上」にする。

```text
count >= 1  -> 1件threshold対象
count >= 5  -> 5件threshold対象
count >= 20 -> 20件threshold対象
```

## 5. threshold別Discordチャンネル設計

推奨設定:

```toml
[hatena]
thresholds = [1, 5, 20]
count_api_timeout_seconds = 10
candidate_retention_days = 7

[[notification.channels]]
threshold = 1
channel_id = "111111111111111111"

[[notification.channels]]
threshold = 5
channel_id = "555555555555555555"

[[notification.channels]]
threshold = 20
channel_id = "202020202020202020"
```

- [ ] `hatena.thresholds` と `notification.channels` の対応を起動時に検証する
- [ ] `thresholds` にあるthresholdのchannel未設定は起動時エラー
- [ ] `notification.channels` に未知thresholdがあれば起動時エラー
- [ ] 同じthresholdが複数回設定されたら起動時エラー
- [ ] 同じchannel_idを複数thresholdに使うことは許可でよい
  - 理由: 運用上「1件と5件を同じチャンネルに流す」可能性はある
  - ただしREADMEで「別チャンネル推奨」と明記する

代替案:

```toml
[discord.channels]
threshold_1 = "111111111111111111"
threshold_5 = "555555555555555555"
threshold_20 = "202020202020202020"
```

比較:

- [ ] メリット: 設定が短い
- [ ] デメリット: threshold固定感が強い
- [ ] デメリット: `[3, 10, 30]` へ変更しづらい
- [ ] デメリット: validation対象を構造的に扱いにくい

結論: `[[notification.channels]]` 形式を推奨。

## 6. 全体アーキテクチャ

```text
起動
  -> Config読み込み
  -> thresholds/channel routing validation
  -> Twilight HTTP client初期化
  -> RssProcessor初期化
  -> HatenaClient初期化
  -> ArticleStateStore読み込み
  -> interval loop:
       RSS取得
       RSS itemを候補として登録
       候補記事を走査
       はてブcount取得
       threshold到達判定
       threshold別channelへ投稿
       成功thresholdのみ状態更新
       retention pruning
       状態保存
```

- [ ] `main.rs` は制御フローだけを持つ
- [ ] `hatena.rs` はcount取得だけを持つ
- [ ] `state.rs` は候補・投稿済みthreshold・pruningを持つ
- [ ] `formatter.rs` は投稿文生成だけを持つ
- [ ] 設定検証は `config.rs` に寄せる

## 7. データモデル設計

推奨: `SeenState` を `ArticleStateStore` へ発展させる。

```rust
ArticleState {
    article_id: String,
    title: String,
    url: String,
    feed_title: Option<String>,
    first_seen_at: DateTime<Utc>,
    last_checked_at: Option<DateTime<Utc>>,
    last_bookmark_count: Option<u64>,
    posted_thresholds: Vec<u64>,
    threshold_posts: BTreeMap<u64, ThresholdPostState>,
    last_posted_at: Option<DateTime<Utc>>,
    pub_date: Option<DateTime<Utc>>,
}

ThresholdPostState {
    channel_id: String,
    posted_at: DateTime<Utc>,
}
```

JSON例:

```json
{
  "article_id": "link:https://example.com/article",
  "title": "Example Article",
  "url": "https://example.com/article",
  "feed_title": "Hatena AI",
  "first_seen_at": "2026-05-05T00:00:00Z",
  "last_checked_at": "2026-05-05T00:10:00Z",
  "last_bookmark_count": 23,
  "posted_thresholds": [1, 20],
  "threshold_posts": {
    "1": {
      "channel_id": "111111111111111111",
      "posted_at": "2026-05-05T00:01:00Z"
    },
    "20": {
      "channel_id": "202020202020202020",
      "posted_at": "2026-05-05T00:10:00Z"
    }
  },
  "pub_date": "2026-05-05T00:00:00Z"
}
```

`posted_thresholds` のみ:

- [ ] メリット: 判定だけなら十分
- [ ] デメリット: どのchannelへいつ投稿したか追えない
- [ ] デメリット: 設定変更後の調査が難しい

`threshold_posts` あり:

- [ ] メリット: thresholdごとの投稿実績を監査できる
- [ ] メリット: 部分成功状態を説明しやすい
- [ ] デメリット: JSONが少し大きくなる

結論: 小型Botでも `threshold_posts` まで持つ。`posted_thresholds` は判定用の冗長フィールドとして残すか、`threshold_posts.keys()` から導出する。実装の単純さを優先するなら両方保存して、更新処理で整合性を保つ。

既存 `data/seen_items.json` 互換:

- [ ] 既存形式を検出したら、各 `id` を最低限のArticleStateへ変換できる
- [ ] ただし旧形式には `title`, `url`, `feed_title` がないため、完全移行はできない
- [ ] 推奨: 初回実装では `data/articles.json` など新ファイルへ切り替える
- [ ] 旧 `seen_items.json` を読む移行は別ステップにしてもよい
- [ ] 互換重視なら、旧seen itemは `posted_thresholds = [1]` 相当として扱うが、URL不明の場合は候補追跡不可

## 8. 設定設計

追加設定:

```toml
[hatena]
thresholds = [1, 5, 20]
count_api_timeout_seconds = 10
candidate_retention_days = 7

[[notification.channels]]
threshold = 1
channel_id = "replace_me_1"

[[notification.channels]]
threshold = 5
channel_id = "replace_me_5"

[[notification.channels]]
threshold = 20
channel_id = "replace_me_20"
```

- [ ] `HatenaConfig` を追加する
- [ ] `NotificationConfig` を追加する
- [ ] `NotificationChannelConfig` を追加する
- [ ] `thresholds` は空配列禁止
- [ ] `thresholds` に `0` は禁止
- [ ] `thresholds` の重複は禁止
- [ ] `thresholds` は設定時に昇順必須にすることを推奨
  - 理由: 設定ミスを早く発見できる
  - 代替: 読み込み後にsortする。ただし設定ミスが隠れる
- [ ] `notification.channels.threshold` は `hatena.thresholds` と完全一致必須
- [ ] `notification.channels.channel_id` はDiscord snowflakeとしてparseする
- [ ] `count_api_timeout_seconds` は `1..=60` を許可、デフォルト `10`
- [ ] `candidate_retention_days` は `1` 以上、デフォルト `7`
- [ ] `thresholds` に `1` が含まれる場合、RSS新着通知は「1件threshold通知」として扱う
- [ ] `state.file_path` は新形式でも使い続けるか、`data/articles.json` に変更するか決める

既存 `discord.channel_id` の扱い:

案A: 廃止

- [ ] メリット: 設定の意味が明確
- [ ] メリット: 単一投稿先との混乱がない
- [ ] デメリット: 既存設定との互換性が落ちる

案C: 今回は残すがthreshold通知では使わない

- [ ] メリット: 差分が小さい
- [ ] メリット: 既存コードへの影響が少ない
- [ ] デメリット: 使われない設定が残り、混乱しやすい

推奨: 案A。理由は、この機能の中心が「threshold別channel routing」なので、使われない `discord.channel_id` を残すと設定の意味が曖昧になるため。段階移行を優先する場合だけ案C。

## 9. はてなAPI連携設計

追加ファイル: `src/hatena.rs`

責務:

- [ ] URLごとのはてなブックマーク数を取得する
- [ ] `https://bookmark.hatenaapis.com/count/entry?url=<urlencoded_url>` を呼ぶ
- [ ] URL encodeする
- [ ] HTTP timeoutを設定する
- [ ] User-Agentを設定する
- [ ] 2xx以外をエラーにする
- [ ] 空レスポンスをエラーにする
- [ ] 数値でないレスポンスをエラーにする
- [ ] レスポンスを `u64` にparseする
- [ ] HTTPエラー、timeout、不正レスポンスを分類する
- [ ] API失敗時にBot全体を落とさない

設計:

```rust
pub struct HatenaClient {
    agent: ureq::Agent,
}

impl HatenaClient {
    pub fn new(timeout: Duration) -> Self;
    pub async fn fetch_count(&self, url: &str) -> Result<u64>;
}
```

既存 `rss.rs` と同じく、`ureq` を使うなら `spawn_blocking` でasync loopを塞がない設計にする。

## 10. 投稿判定・チャンネルルーティングロジック

疑似コード:

```text
reached_thresholds =
  thresholds
    .filter(threshold <= bookmark_count)

unposted_reached_thresholds =
  reached_thresholds
    .filter(threshold not in article.posted_thresholds)

target_thresholds =
  unposted_reached_thresholds sorted ascending

for threshold in target_thresholds:
  channel_id = notification.channel_for(threshold)
  post to channel_id
  if post succeeded:
    article.posted_thresholds.add(threshold)
    article.threshold_posts[threshold] = {
      channel_id,
      posted_at
    }
    article.last_posted_at = posted_at
  else:
    keep threshold unposted
```

同時到達時:

```text
thresholds = [1, 5, 20]
bookmark_count = 23
posted_thresholds = []
```

投稿対象:

```text
1件用チャンネルへ投稿
5件用チャンネルへ投稿
20件用チャンネルへ投稿
```

- [ ] 投稿順序は `1 -> 5 -> 20` の昇順
- [ ] 各thresholdの成功/失敗は独立管理する
- [ ] 途中thresholdが失敗しても後続thresholdは処理する
- [ ] 成功したthresholdだけ状態更新する

例:

```text
1件チャンネル投稿成功
5件チャンネル投稿失敗
20件チャンネル投稿成功
```

保存状態:

```text
posted_thresholds = [1, 20]
```

状態保存タイミング:

- [ ] 推奨: 各投稿成功後に保存を試みる
- [ ] 理由: Discord投稿済みなのにプロセス終了して未保存になる二重投稿リスクを小さくする
- [ ] デメリット: 書き込み回数が増える
- [ ] 小型Botでは許容範囲

状態保存失敗時:

- [ ] Discord投稿成功後に保存失敗した場合、ログは `warn` または `error`
- [ ] メモリ上の状態は更新済みにする
- [ ] プロセスが生きている間は二重投稿を避けられる
- [ ] 再起動後は二重投稿リスクがあるためREADMEに明記する

## 11. 初回起動時の挙動

既存 `skip_existing_on_first_run = true` は維持する。

推奨: 案A。

```text
初回起動時はRSS内の記事をArticleStateに登録するが、投稿はしない。
その時点のbookmark_countを取得し、到達済みthresholdをposted扱いにする。
```

例:

```text
count = 23
posted_thresholds = [1, 5, 20]
```

- [ ] メリット: 初回起動時の大量通知事故を防げる
- [ ] デメリット: 初回時点で伸びている記事は通知されない
- [ ] 小型Botでは通知事故回避を優先する

代替案B:

- [ ] 登録だけして `posted_thresholds` は空
- [ ] 次回ポーリングで大量通知される可能性があるため非推奨

代替案C:

- [ ] 初回から投稿する
- [ ] RSS内の既存記事が大量投稿される可能性があるため非推奨

## 12. retention / pruning 設計

- [ ] `candidate_retention_days = 7` をデフォルトにする
- [ ] `first_seen_at` または `last_checked_at` が古い候補を削除する
- [ ] すべてのthresholdを投稿済みの記事も削除候補にする
- [ ] RSSに再登場した記事は再登録される可能性がある
- [ ] 完全削除すると、再発見時に再通知される可能性がある
- [ ] tombstone保存なら再通知を防げるが、状態管理が重くなる

推奨: 最初は単純なprune。

理由:

- 小型Botとして自然
- 状態ファイルが肥大化しにくい
- `candidate_retention_days = 7` ならRSS再登場による再通知リスクは限定的

注意:

- [ ] READMEに「状態削除・prune後に同じ記事が再通知される可能性」を書く
- [ ] 将来必要になったら `tombstones` を追加する

## 13. 既存コードへの変更チェックリスト

### `src/config.rs`

- [ ] `HatenaConfig` を追加する
- [ ] `NotificationConfig` を追加する
- [ ] `NotificationChannelConfig` を追加する
- [ ] `thresholds` を読み込む
- [ ] thresholdsをvalidateする
- [ ] thresholdごとのchannel_idを読み込む
- [ ] thresholdとchannel_idの対応をvalidateする
- [ ] channel_idを `Id<ChannelMarker>` へparseする
- [ ] `count_api_timeout_seconds` をvalidateする
- [ ] `candidate_retention_days` をvalidateする
- [ ] 既存 `discord.channel_id` の扱いを案Aまたは案Cで決める
- [ ] テストを追加する

### `src/rss.rs`

- [ ] 現在の `RssItem` を維持する
- [ ] はてブ数取得に使うURLは原則 `link` を使う
- [ ] `item_id()` はArticleStateのIDとして使える
- [ ] `link` が空の記事はcount取得不可として扱う
- [ ] `link` が空の記事は候補登録するが、threshold通知対象外にするか検討する
- [ ] 推奨: `link` 空はwarnログを出してcount取得をskipする

### `src/hatena.rs`

- [ ] 新規追加する
- [ ] count API clientを実装する
- [ ] timeoutを設定する
- [ ] User-Agentを設定する
- [ ] URL encodeする
- [ ] 2xx以外をエラーにする
- [ ] レスポンスをu64にparseする
- [ ] テスト可能な構造にする

### `src/state.rs`

- [ ] `SeenState` を `ArticleStateStore` へ置き換えるか、段階移行するか決める
- [ ] 推奨: 小型Botなので置き換え。ただし型名互換は不要
- [ ] articleごとの状態を保存する
- [ ] posted_thresholdsを保存する
- [ ] threshold_postsを保存する
- [ ] thresholdごとの投稿先channel_idを保存する
- [ ] last_bookmark_countを保存する
- [ ] first_seen_atを保存する
- [ ] last_checked_atを保存する
- [ ] retention pruningを実装する
- [ ] 既存 `seen_items.json` からの移行方針を決める
- [ ] thresholdごとの投稿成功/失敗を独立して扱えるようにする

### `src/formatter.rs`

- [ ] threshold別投稿文を生成する
- [ ] 1件通知、5件通知、20件通知で文面を変える
- [ ] bookmark_countを表示する
- [ ] thresholdを表示する
- [ ] 投稿先チャンネルごとの文面差分は不要にする
- [ ] linkを必ず残す
- [ ] 2000文字制限を守る
- [ ] メンションsanitizeを維持する

### `src/main.rs`

- [ ] 現在の「RSS新着即投稿」フローを見直す
- [ ] RSS itemをArticleStateStoreに登録する
- [ ] 候補記事ごとにはてブ数を取得する
- [ ] threshold到達判定を行う
- [ ] thresholdごとの投稿先チャンネルを解決する
- [ ] 到達済みかつ未投稿thresholdを昇順に処理する
- [ ] Discord投稿成功後に該当thresholdだけposted_thresholdsへ追加する
- [ ] API失敗時は候補を残して次回再試行する
- [ ] Discord投稿失敗時は該当thresholdを更新しない
- [ ] 一部threshold投稿成功・一部失敗を保存する
- [ ] Ctrl+C終了時に状態保存する
- [ ] `allowed_mentions` 空設定を維持する

## 14. テストチェックリスト

### 設定validation

- [ ] thresholdsが `[1, 5, 20]` で読み込める
- [ ] thresholdsが空配列ならエラー
- [ ] thresholdsに0が含まれるとエラー
- [ ] thresholdsに重複があるとエラー
- [ ] thresholdsが昇順でなければエラー
- [ ] thresholdsにある値のchannel_idが未設定ならエラー
- [ ] notification.channelsに未知thresholdがあるとエラー
- [ ] 同じthresholdが複数回設定されたらエラー
- [ ] channel_idが不正ならエラー
- [ ] 同じchannel_idを複数thresholdに使う場合は許可される

### threshold判定

- [ ] count = 0 の場合、投稿対象なし
- [ ] count = 1 の場合、1件通知対象
- [ ] count = 4 の場合、1件通知済みなら投稿対象なし
- [ ] count = 5 の場合、5件通知対象
- [ ] count = 19 の場合、5件通知済みなら投稿対象なし
- [ ] count = 20 の場合、20件通知対象
- [ ] count = 25 かつ未投稿の場合、1, 5, 20すべてが投稿対象
- [ ] count = 25 かつ1件投稿済みの場合、5, 20が投稿対象
- [ ] count = 25 かつ1, 5投稿済みの場合、20のみ投稿対象
- [ ] count = 25 かつすべて投稿済みの場合、投稿対象なし

### channel routing

- [ ] threshold 1 が1件用チャンネルに解決される
- [ ] threshold 5 が5件用チャンネルに解決される
- [ ] threshold 20 が20件用チャンネルに解決される
- [ ] 未知thresholdのchannel解決はエラーになる
- [ ] Discord投稿時に正しいchannel_idが使われる

### 状態管理

- [ ] ArticleStateを保存できる
- [ ] ArticleStateを再読み込みできる
- [ ] posted_thresholdsが永続化される
- [ ] threshold_postsが永続化される
- [ ] last_bookmark_countが永続化される
- [ ] retentionを過ぎた候補がpruneされる
- [ ] 壊れたJSONで分かりやすく失敗する
- [ ] 1件投稿成功、5件失敗、20件成功の部分成功状態を保存できる

### Hatena API

- [ ] 正常レスポンスをu64にparseできる
- [ ] 空レスポンスをエラーにする
- [ ] 数値でないレスポンスをエラーにする
- [ ] 404/500等をエラーにする
- [ ] timeoutをエラーにする
- [ ] API失敗時にBot全体は落ちない

### Discord投稿

- [ ] 1件通知文にthresholdとbookmark_countが含まれる
- [ ] 5件通知文にthresholdとbookmark_countが含まれる
- [ ] 20件通知文にthresholdとbookmark_countが含まれる
- [ ] 1件通知が1件用channel_idへ投稿される
- [ ] 5件通知が5件用channel_idへ投稿される
- [ ] 20件通知が20件用channel_idへ投稿される
- [ ] 長文でも2000文字制限を超えない
- [ ] linkが残る
- [ ] mentionが無効化される
- [ ] 投稿失敗時に該当thresholdがposted_thresholdsへ追加されない
- [ ] 投稿成功時に該当thresholdがposted_thresholdsへ追加される

## 15. README更新チェックリスト

- [ ] 新しいthreshold別チャンネル通知仕様を書く
- [ ] `thresholds = [1, 5, 20]` の意味を書く
- [ ] 1件通知が現在の新着通知に相当することを書く
- [ ] 1件、5件、20件で投稿先チャンネルが異なることを書く
- [ ] 5件通知、20件通知では同じ記事が別チャンネルへ再通知されうることを書く
- [ ] 同じthresholdは二重通知されないことを書く
- [ ] `[[notification.channels]]` の設定例を書く
- [ ] `discord.channel_id` を廃止または未使用にする方針を書く
- [ ] 状態ファイルを削除した場合の挙動を書く
- [ ] prune後に同じ記事が再通知される可能性を書く
- [ ] はてなAPI取得失敗時の挙動を書く
- [ ] 初回起動時 `skip_existing_on_first_run = true` の挙動を書く

## 16. リスク・未決事項

- [ ] 同時に `1`, `5`, `20` へ到達した記事は3チャンネルへ短時間に連続投稿される
- [ ] 各チャンネルの読者が異なる前提では正しいが、Discord上では通知が多く見える可能性がある
- [ ] はてなAPIが失敗・遅延すると、候補記事の確認が遅れる
- [ ] 状態保存失敗後にプロセスが落ちると、Discord投稿済み記事が再通知される可能性がある
- [ ] `link` が空の記事ははてブcountを取得できない
- [ ] 旧 `seen_items.json` との完全互換は難しい
- [ ] pruneで削除した記事がRSSに再登場すると再通知される可能性がある
- [ ] `discord.channel_id` を廃止する場合、既存ユーザーの設定変更が必要

未決事項として実装前に確認したい点:

- [ ] `discord.channel_id` は案Aで廃止してよいか
- [ ] 状態ファイル名を `data/seen_items.json` のまま新形式にするか、`data/articles.json` に変えるか
- [ ] 旧 `seen_items.json` の自動移行を初回実装に含めるか
- [ ] `link` 空記事を候補保存だけするか、保存もskipするか

## 17. 推奨実装順序

1. [ ] `config.rs` に `HatenaConfig` / `NotificationConfig` を追加する
   検証: 設定validationテストが通る

2. [ ] `state.rs` を `ArticleStateStore` 設計へ発展させる
   検証: 保存・読み込み・部分成功・pruneテストが通る

3. [ ] `hatena.rs` を追加する
   検証: 正常レスポンス、異常レスポンス、timeout相当のテストが通る

4. [ ] `formatter.rs` にthreshold別投稿文を追加する
   検証: 1/5/20件通知文、2000文字制限、link保持、mention sanitizeテストが通る

5. [ ] `main.rs` の制御フローをthreshold routingへ差し替える
   検証: 投稿成功thresholdのみ状態更新されるテストが通る

6. [ ] `README.md` と `config/default.toml` を更新する
   検証: READMEの設定例でConfigが読み込める

7. [ ] `cargo fmt -- --check`, `cargo clippy`, `cargo test` を実行する
   検証: 全チェックが通る

## 18. Codex実装投入前の確認事項

- [ ] `discord.channel_id` は廃止する案Aで進めるか
- [ ] 新状態ファイル名は `data/articles.json` にするか
- [ ] 旧 `data/seen_items.json` の移行を今回含めるか
- [ ] 同じchannel_idを複数thresholdで使うことを許可でよいか
- [ ] 初回起動時は案A、つまり到達済みthresholdをposted扱いにして大量通知を防ぐ方針でよいか
- [ ] `link` 空の記事はwarnしてthreshold判定対象外でよいか
- [ ] 状態保存は各投稿成功後に行う方針でよいか
