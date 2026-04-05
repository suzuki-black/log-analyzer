# Log Analyzer v0.1

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

### 変更方法

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

変更後、`docker compose down -v && docker compose up --build` で再構築してください。
(`-v` で既存ボリュームを削除してパスワードを反映させます)

## 使い方

1. `.log` または `.gz` ファイルをドロップしてアップロード
2. テーブルベース名を入力（実テーブル名: `{name}_la`）
3. 重複行の扱いを選択
4. 「取り込み開始」→ リアルタイム進捗を確認

## 機能

- **動的スキーマ**: 新しいキーを見つけ次第 `ALTER TABLE ADD COLUMN` を発行
- **型推論**: `TINYINT(1)` / `BIGINT` / `DOUBLE` / `DATETIME` / `TEXT`
  - 日時文字列 (ISO 8601, `YYYY-MM-DD HH:MM:SS` 等) は自動で `DATETIME` 型
- **型エラーコンパニオンカラム**: `DATETIME` 型カラムにパース不能な値が来た場合、`_te_{col}` カラムに生文字列を保存し元カラムは `NULL`
- **重複検出**: SHA-256 ハッシュで行単位の重複を検出。`warn` / `flag_column` / `skip` の 3 モード
- **gzip 対応**: `.gz` ファイルはオンザフライで解凍
- **同名ファイル警告**: `.log` と `.log.gz` が同時指定された場合に確認ダイアログ
- **SSE 進捗**: ファイル数・レコード数をリアルタイム表示

## テーブル構造

各取り込みテーブルには以下のシステムカラムが自動追加される:

| カラム | 型 | 説明 |
|---|---|---|
| `_id` | BIGINT AUTO_INCREMENT | PK |
| `_job_id` | VARCHAR(36) | 取り込みジョブ ID |
| `_line_no` | BIGINT | 元ファイルの行番号 |
| `_is_dup` | TINYINT(1) | 重複フラグ (flag_column モード時) |
| `_raw` | MEDIUMTEXT | 元の JSON 生文字列 |
| `_te_{col}` | TEXT | 型エラー時の生値 (自動追加) |

## サンプルファイル

`samples/` ディレクトリに以下のサンプルが含まれる:

| ファイル | 内容 |
|---|---|
| `app.log` | アプリケーションログ。重複行あり、多様なキー |
| `access.log` | HTTP アクセスログ。途中から新規キー (`bot`, `error`) |
| `type_error_demo.log` | `ts` カラムへの不正値を含む。`_te_ts` 自動追加のデモ |

## 管理テーブル

| テーブル | 説明 |
|---|---|
| `_la_files` | アップロード済みファイル一覧 |
| `_la_jobs` | 取り込みジョブ履歴 |

## ライセンス

[MIT License](LICENSE)
