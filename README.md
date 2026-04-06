# Log Analyzer v1.0

NDJSON ログファイルを MySQL 8 へ動的スキーマで取り込む Web アプリ。

## 構成

| サービス | 技術 | ポート |
|---|---|---|
| フロントエンド | React + TypeScript + Vite → nginx | 5173 |
| バックエンド | Rust (Axum + sqlx) | 8080 |
| データベース | MySQL 8.4 | 3306 |
| phpMyAdmin | phpmyadmin:latest | 8081 |

## 起動

```bash
docker compose up --build
```

ブラウザで http://localhost:5173 を開く。
phpMyAdmin は http://localhost:8081 でアクセス可能。

## 接続情報について

`docker-compose.yml` に記載のデフォルト接続情報はローカル開発用です。
**本番環境や外部公開する場合は必ず変更してください。**

`docker-compose.yml` の以下の値を任意のものに変更します:

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

変更後は `docker compose down -v && docker compose up --build` で再構築してください。

## 使い方

1. `.log` または `.gz` ファイルをドロップ、またはフォルダを選択してアップロード
2. テーブルベース名を入力（実テーブル名: `{name}_la`）
3. 重複行の扱いを選択
4. 「取り込み開始」→ リアルタイム進捗を確認
5. 完了後、ホーム画面の「テーブル管理」でリセット・削除が可能

## 機能

- **動的スキーマ**: 新しいキーを見つけ次第 `ALTER TABLE ADD COLUMN` を発行
- **型推論**: `TINYINT(1)` / `BIGINT` / `DOUBLE` / `DATETIME` / `TEXT`
  - 日時文字列 (ISO 8601, `YYYY-MM-DD HH:MM:SS` 等) は自動で `DATETIME` 型
- **型エラーコンパニオンカラム**: 型不一致の値は `_te_{col}` カラムに生文字列を保存
- **重複検出 (クロスジョブ対応)**: SHA-256 ハッシュで行単位の重複を検出。別ジョブで取り込み済みの行も重複として扱う。`warn` / `flag_column` / `skip` の 3 モード
- **gzip 対応**: `.gz` ファイルはオンザフライで解凍
- **再帰ディレクトリ選択**: フォルダを指定すると配下の `.log` / `.gz` を再帰取得
- **同名ファイル警告**: `.log` と `.log.gz` が同時指定された場合に確認ダイアログ
- **SSE 進捗**: ファイル数・レコード数をリアルタイム表示
- **テーブル管理**: 取り込みテーブルのリセット (TRUNCATE) / 削除 (DROP) を UI から実行

## テーブル構造

各取り込みテーブルには以下のシステムカラムが自動追加される:

| カラム | 型 | 説明 |
|---|---|---|
| `_id` | BIGINT AUTO_INCREMENT | PK |
| `_job_id` | VARCHAR(36) | 取り込みジョブ ID |
| `_line_no` | BIGINT | 元ファイルの行番号 |
| `_is_dup` | TINYINT(1) | 重複フラグ (flag_column モード時) |
| `_content_hash` | CHAR(64) | 行内容の SHA-256 ハッシュ（重複検出用） |
| `_raw` | MEDIUMTEXT | 元の JSON 生文字列 |
| `_te_{col}` | TEXT | 型エラー時の生値 (自動追加) |

## サンプルファイル

`samples/` ディレクトリに以下のサンプルが含まれる:

```
samples/
├── app.log                      # アプリログ（重複行あり・多様なキー）
├── access.log                   # HTTP アクセスログ（途中から新規キー）
├── type_error_demo.log          # _te_ts コンパニオンカラムのデモ
├── webapp/
│   ├── api-access.log           # REST API アクセスログ（1,000 行）
│   ├── api-access.log.gz        # 同上 gzip 版 ← ペア警告のデモ
│   ├── auth.log                 # 認証イベントログ（800 行）
│   ├── errors.log               # エラーログ・コンポーネント別（400 行）
│   └── worker.log               # バックグラウンドジョブログ（600 行）
└── batch/
    ├── etl-job.log              # ETL パイプラインログ（100,000 行・約 22 MB）
    ├── scheduler.log            # スケジューラーログ（500 行）
    └── scheduler.log.gz         # 同上 gzip 版・先頭 250 行 ← gz 単独のデモ
```

## 管理テーブル

| テーブル | 説明 |
|---|---|
| `_la_files` | アップロード済みファイル一覧 |
| `_la_jobs` | 取り込みジョブ履歴 |

## 今後の機能追加候補

### 取り込み強化
- [ ] **ディレクトリ監視による自動取り込み** — 指定ディレクトリを watch で監視し、新規ファイルを自動インジェスト
- [ ] **取り込みスケジューリング** — cron 式で定期実行
- [ ] **サンプリング取り込み** — 大量ログの N 行ごと・ランダム N% 取り込みオプション
- [ ] **カスタムパーサー** — 正規表現や区切り文字指定で非 JSON ログにも対応

### スキーマ・データ管理
- [ ] **カラム型の手動変更 UI** — 誤推論されたカラムの型を後から変更
- [ ] **取り込み済みデータの検索・フィルタ UI** — SQL を書かずにデータを閲覧・絞り込み

### エクスポート・連携
- [ ] **CSV / JSON エクスポート** — 取り込み済みテーブルをファイル出力
- [ ] **クエリ結果のダウンロード** — 任意の SELECT 結果をエクスポート

### 可視化・分析
- [ ] **ダッシュボード** — 件数推移・エラー率・レベル別集計をグラフ表示
- [ ] **カラム統計** — NULL 率・ユニーク数・最大/最小値のサマリー表示

### 運用
- [ ] **ユーザー認証** — ログイン機能・操作ログ
- [ ] **取り込みジョブの再実行・キャンセル** — 失敗ジョブのリトライ UI
- [ ] **通知連携** — 取り込み完了・エラー時の Slack / webhook 通知

## ライセンス

[MIT](LICENSE) © 2026 suzuki-black
