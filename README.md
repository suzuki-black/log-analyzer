<div align="center">

# ⚡ Log Analyzer

**Drop NDJSON logs. Query them in seconds. No schema required.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Backend: Rust](https://img.shields.io/badge/backend-Rust%20%2B%20Axum-orange.svg)](https://axum.rs)
[![Frontend: React](https://img.shields.io/badge/frontend-React%20%2B%20TypeScript-61DAFB.svg)](https://react.dev)
[![Database: MySQL 8](https://img.shields.io/badge/database-MySQL%208.4-4479A1.svg)](https://www.mysql.com)
[![Deploy: Docker Compose](https://img.shields.io/badge/deploy-Docker%20Compose-2496ED.svg)](docker-compose.yml)

<!-- Replace with actual demo GIF: docs/demo.gif
     Capture: drag files to drop zone → real-time SSE progress bars → completion modal (~3s) -->
![Demo](docs/demo.gif)

| 🏗️ Zero Schema | 🔁 Cross-Job Dedup | ⚠️ Type Safety | 📡 Live Progress | 🌐 i18n |
|:---:|:---:|:---:|:---:|:---:|
| Schema evolves automatically | SHA-256 across all past jobs | Companion columns preserve bad data | SSE streams every record | EN / JA built-in |

</div>

---

**[🇯🇵 日本語版はこちら](#-log-analyzer-日本語)**

---

## ⚡ Quick Start

```bash
git clone https://github.com/suzuki-black/log-analyzer.git
cd log-analyzer
docker compose up --build
```

Open **http://localhost:5173** — drag a log file — done.
> Sample logs are in [`samples/`](#-try-with-sample-logs). No test data needed.

---

## Why Log Analyzer?

Most log tools require you to define a schema, configure a pipeline, or learn a query language before you see any data.
Log Analyzer takes the opposite approach: **ingest first, explore later.**

| Challenge | Log Analyzer's answer |
|---|---|
| Log format keeps changing | Schema evolves automatically via `ALTER TABLE ADD COLUMN` |
| Re-ingesting the same files | Cross-job SHA-256 dedup — never double-count |
| Bad values break imports | `_te_{col}` companion columns preserve every raw value |
| 100k-line files are slow | Rust backend streams records directly to MySQL without loading into memory |
| Setting up a pipeline takes hours | One `docker compose up` — backend, DB, and UI all included |

---

## 👤 Who Is This For?

- **Backend engineers** investigating API errors from access logs
- **Data engineers** loading ETL pipeline output into a queryable store
- **SREs** correlating authentication and error events across services
- **Any developer** who wants to run SQL on structured logs without a data warehouse

---

## ✨ Features

🏗️ **Dynamic Schema** — Discovers new JSON keys at ingest time and issues `ALTER TABLE ADD COLUMN` automatically. Your schema always matches your data.

🔍 **Type Inference** — Maps values to `TINYINT(1)` / `BIGINT` / `DOUBLE` / `DATETIME` / `TEXT`. ISO 8601, `YYYY-MM-DD HH:MM:SS`, and other common datetime formats are detected automatically.

⚠️ **Type-Error Companion Columns** — When a value cannot be coerced (e.g., a string in a numeric column), the original value is preserved in `_te_{col}` so no data is ever silently dropped.

🔁 **Cross-Job Duplicate Detection** — Each row is fingerprinted with SHA-256. Duplicates are detected not just within a job but across all historical jobs. Three modes: `warn` / `flag_column` / `skip`.

📦 **gzip + Recursive Directory** — `.gz` files decompress on the fly. Selecting a folder recursively collects all `.log` and `.gz` files beneath it.

📡 **Real-Time SSE Progress** — File count, line count, insert count, and schema changes stream to the browser via Server-Sent Events as they happen.

🛠️ **Table Management** — TRUNCATE or DROP any ingested table directly from the UI without touching MySQL.

🌐 **i18n (EN / JA)** — UI defaults to English. Switch to Japanese from Settings; preference is persisted in `localStorage`.

---

## 💡 Use Cases

**API access log analysis**
Import `api-access.log` into a table and run `SELECT status, COUNT(*) FROM access_la GROUP BY status` to see error distribution instantly.

**Authentication event investigation**
Correlate `auth.log` and `errors.log` in separate tables, then JOIN on `user_id` to trace failed login chains.

**ETL pipeline anomaly detection**
The 100k-line `etl-job.log` in `samples/batch/` loads in seconds. Filter for `"level": "error"` rows to surface pipeline failures.

**Scheduler log analysis**
Track job durations over time by querying the `_line_no` and `duration_ms` columns that Log Analyzer infers automatically.

---

## 🧪 Try with Sample Logs

The `samples/` directory contains ready-to-use log files — no test data setup required.

```
samples/
├── app.log               # Varied keys + duplicate rows  →  tests dedup modes
├── access.log            # New keys appear mid-file      →  tests dynamic schema
├── type_error_demo.log   # Malformed timestamps          →  shows _te_ columns
│
├── webapp/
│   ├── api-access.log    # REST access log  (1,000 lines)
│   ├── api-access.log.gz # Same file, gzip  →  triggers pair-warning dialog
│   ├── auth.log          # Auth events      (800 lines)
│   ├── errors.log        # Component errors (400 lines)
│   └── worker.log        # Background jobs  (600 lines)
│
└── batch/
    ├── etl-job.log       # ETL pipeline     (100,000 lines, ~22 MB)
    ├── scheduler.log     # Cron jobs        (500 lines)
    └── scheduler.log.gz  # Same, gzip, 250 lines  →  gz-only demo
```

> **Tip:** Drag the entire `samples/webapp/` folder onto the drop zone to ingest all five files in one job.

---

## 🏛️ Architecture

```
┌─────────────────────────────────────────────┐
│            Browser  (React + TS)            │
│                                             │
│  Drop Zone ──▶ Upload ──▶ Job Config        │
│                              │              │
│       Progress Page ◀── SSE │              │
└──────────────────────────────┼──────────────┘
                               │ HTTP / SSE
┌──────────────────────────────▼──────────────┐
│           Rust Backend  (Axum + sqlx)        │
│                                             │
│  /api/upload   →  save files to disk        │
│  /api/jobs     →  spawn ingest worker       │
│  /api/jobs/:id/progress  →  SSE stream      │
│                                             │
│  Ingest Worker                              │
│    ├─ reader.rs   decompress + parse NDJSON │
│    ├─ schema.rs   ALTER TABLE as needed     │
│    ├─ dedup.rs    SHA-256 fingerprint       │
│    └─ worker.rs   batch INSERT + SSE emit   │
└──────────────────────────────┬──────────────┘
                               │ sqlx (MySQL protocol)
┌──────────────────────────────▼──────────────┐
│              MySQL 8.4                       │
│                                             │
│  {name}_la   — ingested log tables          │
│  _la_files   — uploaded file registry       │
│  _la_jobs    — job history                  │
└─────────────────────────────────────────────┘
```

| Service | Technology | Port |
|---|---|---|
| Frontend | React 18 + TypeScript + Vite → nginx | 5173 |
| Backend | Rust (Axum + sqlx) | 8080 |
| Database | MySQL 8.4 | 3306 |
| DB Admin | phpMyAdmin | 8081 |

---

## 🚀 Getting Started

### Prerequisites

- Docker and Docker Compose

### Start

```bash
docker compose up --build
```

- UI: **http://localhost:5173**
- phpMyAdmin: **http://localhost:8081**

### Production credentials

The default credentials in `docker-compose.yml` are for local development only.
**Change all passwords before exposing the service to any external network.**

```yaml
mysql:
  environment:
    MYSQL_ROOT_PASSWORD: rootpass   # ← change
    MYSQL_PASSWORD: logpass         # ← change

backend:
  environment:
    DATABASE_URL: mysql://loguser:logpass@mysql:3306/logdb  # ← update logpass

phpmyadmin:
  environment:
    PMA_PASSWORD: rootpass          # ← must match MYSQL_ROOT_PASSWORD
```

After editing:

```bash
docker compose down -v && docker compose up --build
```

---

## 📖 Usage

1. Drop `.log` or `.gz` files onto the drop zone, or click **Select Files** / **Select Folder**
2. Enter a **table base name** — the actual table will be created as `{name}_la`
3. Choose a **duplicate mode**: `warn`, `flag_column`, or `skip`
4. Click **Start Import** and watch real-time progress via SSE
5. When complete, use **Table Management** to reset (TRUNCATE) or remove (DROP) tables

---

## 🗂️ System Columns

Every ingested table automatically receives these system columns:

| Column | Type | Description |
|---|---|---|
| `_id` | `BIGINT AUTO_INCREMENT` | Primary key |
| `_job_id` | `VARCHAR(36)` | Ingestion job ID |
| `_line_no` | `BIGINT` | Original line number in the source file |
| `_is_dup` | `TINYINT(1)` | Duplicate flag (`flag_column` mode) |
| `_content_hash` | `CHAR(64)` | SHA-256 of the row content |
| `_raw` | `MEDIUMTEXT` | Original JSON string |
| `_te_{col}` | `TEXT` | Raw value when type coercion fails (added on demand) |

Management tables created automatically:

| Table | Description |
|---|---|
| `_la_files` | Uploaded file registry |
| `_la_jobs` | Ingestion job history |

---

## 🗺️ Roadmap

The core ingestion engine is stable. Here is what is coming next:

**Automation**
- 📂 **Directory watch** — Monitor a path and auto-ingest new files as they arrive
- ⏰ **Cron scheduling** — Trigger ingestion jobs on a cron expression

**Exploration**
- 🔎 **Search & filter UI** — Browse ingested rows without writing SQL
- 📊 **Dashboard** — Time-series charts for record counts, error rates, and level breakdowns
- 📈 **Column statistics** — NULL rate, distinct count, min/max for every column

**Export & Integration**
- 📥 **CSV / JSON export** — Download any ingested table or query result as a file
- 🔔 **Slack / webhook notifications** — Alert on job completion or error

**Ingestion**
- 🗜️ **Sampling** — Ingest every N-th row or a random N%
- 🔧 **Custom parsers** — Support non-JSON logs via regex or delimiter config
- ✏️ **Manual column type editor** — Correct mis-inferred types after ingestion

---

## 📄 License

[MIT](LICENSE) © 2026 suzuki-black

---

> If you find this project useful, please consider giving it a ⭐
> Stars help support the project and guide future development.

---
---

<div align="center">

# ⚡ Log Analyzer（日本語）

**NDJSON ログをドロップするだけで、即座にクエリ可能になる。スキーマ定義不要。**

</div>

---

**[🇺🇸 English version (above)](#-log-analyzer)**

---

## ⚡ クイックスタート

```bash
git clone https://github.com/suzuki-black/log-analyzer.git
cd log-analyzer
docker compose up --build
```

ブラウザで **http://localhost:5173** を開き、ログファイルをドロップするだけ。
> すぐ試せる [`samples/`](#-サンプルログ) が同梱されています。

---

## なぜ Log Analyzer か

ほとんどのログツールはスキーマ定義・パイプライン構築・独自クエリ言語の習得が必要です。
Log Analyzer は逆の発想で動きます。**まず取り込んで、あとから探索する。**

| 課題 | Log Analyzer の解決策 |
|---|---|
| ログの形式が頻繁に変わる | `ALTER TABLE ADD COLUMN` でスキーマを自動進化 |
| 同じファイルを再取り込みしてしまう | クロスジョブ SHA-256 重複検出で二重カウントを防止 |
| 不正な値が取り込みを壊す | `_te_{col}` コンパニオンカラムで生文字列を保存 |
| 10 万行超のファイルが遅い | Rust バックエンドがメモリにロードせず MySQL へ直接ストリーム |
| パイプライン構築に時間がかかる | `docker compose up` 一発でバックエンド・DB・UI がすべて起動 |

---

## 👤 このツールを使う人

- API エラーをアクセスログから調査する**バックエンドエンジニア**
- ETL パイプラインの出力をクエリ可能なストアに取り込む**データエンジニア**
- 認証・エラーイベントを横断的に調査する **SRE**
- データウェアハウスなしでログに SQL を実行したい**すべての開発者**

---

## ✨ 機能

🏗️ **動的スキーマ** — 取り込み時に新しい JSON キーを発見すると `ALTER TABLE ADD COLUMN` を自動発行。スキーマは常にデータに追随します。

🔍 **型推論** — `TINYINT(1)` / `BIGINT` / `DOUBLE` / `DATETIME` / `TEXT` へ自動マッピング。ISO 8601 等の日時文字列も自動判定。

⚠️ **型エラーコンパニオンカラム** — 型不一致の値は `_te_{col}` カラムに生文字列として保存。データが無音で消えることはありません。

🔁 **クロスジョブ重複検出** — 各行を SHA-256 でフィンガープリント。過去の全ジョブを横断して重複を検出。`warn` / `flag_column` / `skip` の 3 モード。

📦 **gzip + 再帰ディレクトリ** — `.gz` はオンザフライ解凍。フォルダを指定すると配下の `.log` / `.gz` を再帰取得。

📡 **SSE リアルタイム進捗** — ファイル数・行数・挿入数・スキーマ変更が Server-Sent Events でブラウザにリアルタイム配信。

🛠️ **テーブル管理** — TRUNCATE / DROP を MySQL を直接触らずに UI から実行。

🌐 **多言語対応（EN / JA）** — デフォルトは英語。設定画面から日本語に切り替え可能。選択は `localStorage` に保存。

---

## 💡 ユースケース

**API アクセスログ解析**
`api-access.log` を取り込んで `SELECT status, COUNT(*) FROM access_la GROUP BY status` を実行するだけで、エラー分布を即座に把握できます。

**認証ログの調査**
`auth.log` と `errors.log` を別テーブルに取り込み、`user_id` で JOIN してログイン失敗チェーンを追跡します。

**ETL パイプラインの異常検知**
`samples/batch/` の 10 万行 `etl-job.log` は数秒で取り込み完了。`"level": "error"` の行を即座に抽出してパイプライン障害を特定できます。

**スケジューラログの分析**
Log Analyzer が自動推論した `_line_no` や `duration_ms` カラムを使ってジョブ実行時間の推移を SQL でクエリできます。

---

## 🧪 サンプルログ

`samples/` にすぐ試せるログファイルが同梱されています。テストデータの準備は不要です。

```
samples/
├── app.log               # 多様なキー + 重複行     →  重複検出モードのデモ
├── access.log            # 途中から新規キー登場    →  動的スキーマのデモ
├── type_error_demo.log   # 不正な日時文字列        →  _te_ カラムのデモ
│
├── webapp/
│   ├── api-access.log    # REST アクセスログ（1,000 行）
│   ├── api-access.log.gz # 同上 gzip 版  →  ペア警告ダイアログのデモ
│   ├── auth.log          # 認証イベント（800 行）
│   ├── errors.log        # コンポーネント別エラー（400 行）
│   └── worker.log        # バックグラウンドジョブ（600 行）
│
└── batch/
    ├── etl-job.log       # ETL パイプライン（100,000 行・約 22 MB）
    ├── scheduler.log     # スケジューラ（500 行）
    └── scheduler.log.gz  # 同上 gzip 版・先頭 250 行  →  gz 単独のデモ
```

> **ヒント:** `samples/webapp/` フォルダをドロップゾーンにドラッグすると、5 ファイルを 1 ジョブで一括取り込みできます。

---

## 🏛️ アーキテクチャ

```
┌─────────────────────────────────────────────┐
│       ブラウザ  (React + TypeScript)          │
│                                             │
│  ドロップゾーン → アップロード → ジョブ設定    │
│                              │              │
│      進捗ページ ◀──── SSE    │              │
└──────────────────────────────┼──────────────┘
                               │ HTTP / SSE
┌──────────────────────────────▼──────────────┐
│        Rust バックエンド  (Axum + sqlx)       │
│                                             │
│  /api/upload   →  ファイルをディスクに保存    │
│  /api/jobs     →  取り込みワーカーを起動      │
│  /api/jobs/:id/progress  →  SSE ストリーム   │
│                                             │
│  取り込みワーカー                            │
│    ├─ reader.rs   解凍 + NDJSON パース       │
│    ├─ schema.rs   必要に応じ ALTER TABLE     │
│    ├─ dedup.rs    SHA-256 フィンガープリント  │
│    └─ worker.rs   バッチ INSERT + SSE 送信   │
└──────────────────────────────┬──────────────┘
                               │ sqlx (MySQL プロトコル)
┌──────────────────────────────▼──────────────┐
│                MySQL 8.4                     │
│                                             │
│  {name}_la   — 取り込みログテーブル          │
│  _la_files   — アップロードファイル一覧       │
│  _la_jobs    — ジョブ履歴                    │
└─────────────────────────────────────────────┘
```

---

## 🚀 起動方法

### 前提条件

- Docker および Docker Compose

### 起動

```bash
docker compose up --build
```

- UI: **http://localhost:5173**
- phpMyAdmin: **http://localhost:8081**

### 接続情報（本番環境向け）

`docker-compose.yml` のデフォルト接続情報はローカル開発用です。
**外部に公開する場合は必ず変更してください。**

```yaml
mysql:
  environment:
    MYSQL_ROOT_PASSWORD: rootpass   # ← 変更推奨
    MYSQL_PASSWORD: logpass         # ← 変更推奨

backend:
  environment:
    DATABASE_URL: mysql://loguser:logpass@mysql:3306/logdb  # ← logpass を上記に合わせて変更

phpmyadmin:
  environment:
    PMA_PASSWORD: rootpass          # ← MYSQL_ROOT_PASSWORD に合わせて変更
```

変更後:

```bash
docker compose down -v && docker compose up --build
```

---

## 📖 使い方

1. `.log` または `.gz` ファイルをドロップ、または **ファイルを選択** / **フォルダを選択** をクリック
2. **テーブルベース名**を入力（実際のテーブル名は `{name}_la`）
3. **重複行の扱い**を選択: `warn`、`flag_column`、または `skip`
4. **取り込み開始** をクリックして SSE でリアルタイム進捗を確認
5. 完了後、**テーブル管理** でリセット（TRUNCATE）または削除（DROP）が可能

---

## 🗂️ システムカラム

各取り込みテーブルに自動追加されるシステムカラム:

| カラム | 型 | 説明 |
|---|---|---|
| `_id` | `BIGINT AUTO_INCREMENT` | 主キー |
| `_job_id` | `VARCHAR(36)` | 取り込みジョブ ID |
| `_line_no` | `BIGINT` | 元ファイルの行番号 |
| `_is_dup` | `TINYINT(1)` | 重複フラグ（`flag_column` モード時） |
| `_content_hash` | `CHAR(64)` | 行内容の SHA-256 ハッシュ |
| `_raw` | `MEDIUMTEXT` | 元の JSON 生文字列 |
| `_te_{col}` | `TEXT` | 型エラー時の生値（自動追加） |

自動作成される管理テーブル:

| テーブル | 説明 |
|---|---|
| `_la_files` | アップロード済みファイル一覧 |
| `_la_jobs` | 取り込みジョブ履歴 |

---

## 🗺️ ロードマップ

取り込みエンジンのコアは安定しています。次のステップとして以下を計画しています:

**自動化**
- 📂 **ディレクトリ監視** — 指定パスを監視し、新規ファイルを自動インジェスト
- ⏰ **cron スケジューリング** — cron 式で取り込みジョブを定期実行

**データ探索**
- 🔎 **検索・フィルタ UI** — SQL なしで取り込み済みデータを閲覧・絞り込み
- 📊 **ダッシュボード** — レコード件数推移・エラー率・レベル別集計グラフ
- 📈 **カラム統計** — NULL 率・ユニーク数・最大/最小値のサマリー

**エクスポート・連携**
- 📥 **CSV / JSON エクスポート** — テーブルやクエリ結果をファイルでダウンロード
- 🔔 **Slack / webhook 通知** — ジョブ完了・エラー時にアラート送信

**取り込み強化**
- 🗜️ **サンプリング** — N 行ごと・ランダム N% 取り込み
- 🔧 **カスタムパーサー** — 正規表現や区切り文字で非 JSON ログに対応
- ✏️ **カラム型の手動変更** — 誤推論されたカラムの型を後から修正

---

## 📄 ライセンス

[MIT](LICENSE) © 2026 suzuki-black

---

> このプロジェクトが役に立ったら ⭐ をいただけると嬉しいです。
> スターはプロジェクトの継続と今後の開発の指針になります。
