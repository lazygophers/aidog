<div align="center">

# 🐕 AiDog

**統合 AI API ゲートウェイ**

デスクトップアプリ · クラウド不要 · 50以上のプラットフォームを一元化 · スマートルーティング · 使用量分析

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/ja/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github&label=release)](https://github.com/lazygophers/aidog/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?logo=tauri&logoColor=white)](https://github.com/lazygophers/aidog/releases/latest)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)
[![Issues](https://img.shields.io/github/issues/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?logo=github)](https://github.com/lazygophers/aidog/pulls)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · `日本語`

</div>

---

> 📖 **完全なドキュメント**: <https://lazygophers.github.io/aidog/ja/>

AiDog は Tauri ベースの**デスクトップ AI API ゲートウェイ**です。50 以上の AI プラットフォームへのリクエストの管理・ルーティング・監視を統合し — 散在する API キー、モデルマッピング、ロードバランシング、使用量分析、コーディングアシスタント設定を1つのアプリに集約します。バックエンドサービス不要、クラウド不要、すべてのデータはローカル SQLite データベースに保存されます。

![AiDog — メインパネル](screenshots/dashboard.png)

## 解決する問題

| あなたの悩み | AiDog の対応 |
| --- | --- |
| API キーが十数のプラットフォームに散在、切り替えが面倒 | **マルチプラットフォーム集約** — 50以上のプリセット、すべてのキーを1箇所で管理 |
| 1つのプラットフォームが落ちると全体が停止 | **フェイルオーバー + ロードバランス** — プラットフォーム間で自動リトライ・サーキットブレーキ・スケジューリング |
| Claude Code / Codex / 各クライアントがバラバラに設定 | **ネイティブコーディングアシスタント連携** — ワンクリック設定エクスポート、全トラフィックがプロキシ経由 |
| 毎月いくら使うか、どのプラットフォームが切れそうか分からない | **使用量監視** — トークン + コスト見積もり + 残高 + Coding Plan クォータ |
| データをクラウドや第三者に渡したくない | **完全ローカル** — プロキシ + DB があなたのマシン上、漏洩ゼロ |

## コア機能

### 🌐 ゲートウェイ & ルーティング
- **マルチプラットフォーム集約** — 50以上のプラットフォームプリセット（Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen / SiliconFlow / OpenRouter など）、ワンクリック設定
- **スマートグルーピング** — Bearer トークン / パスでリクエストをマッチ、Failover と Load Balance
- **モデルマッピング** — モデル名の透過的置換（例 `claude-sonnet-4` → `deepseek-chat`）
- **プロトコル変換** — OpenAI Chat / Completions / Responses、Anthropic、Gemini プロトコル間の双方向変換
- **サーキットブレーキ & スケジューリング** — 異常プラットフォームの自動遮断、三状態管理、指数バックオフ、グループ内スマートスケジューリング
- **ミドルウェアルールエンジン** — インバウンド/アウトバウンドルール: 正規化・上書き・秘匿化・注入・機密語フィルタ・エラー検出、ビルトインプリセット付き

### 📊 監視 & 統計
- **使用量監視** — トークン統計、コスト見積もり（自動価格同期 + 手動予算）
- **残高照会** — 各プラットフォームの残高をリアルタイム取得
- **Coding Plan クォータ** — DeepSeek / Kimi / GLM Coding Plan クォータの表示とカウントダウン
- **リクエストログ** — 三段階の粒度（ユーザー生リクエスト / 上流リクエスト / サマリー）、それぞれ独立したトグルと保持期間

### 🤖 コーディングアシスタント連携
- **Claude Code** — ネイティブ連携: 設定編集、ワンクリックインポート/エクスポート、StatusLine スクリプト、Hooks、グループごとの設定同期
- **OpenAI Codex** — ネイティブ連携: `~/.codex/config.toml` エディタ、Responses API 自動ルーティング
- **MCP 管理** — DB 一元保存 + エージェントごとの有効化トグル + スキャンインポート + 機密フィールドマスキング
- **Skills 管理** — npx ベースのクロスプラットフォーム統一 skills リスト + アイテムごとの有効化トグル
- **システム通知** — TTS 読み上げ / ポップアップ / 受信トレイ + Claude Code/Codex フックのワンクリック注入

### 🎨 パーソナライズ
- **テーマシステム** — 3 軸: 9 スタイル（Liquid Glass / Flat / Soft / Sharp / Aurora / Paper / Terminal / Bento / Sketchy）× 12 命名パレット（Apple Blue / Nord / Dracula / Catppuccin / Gruvbox / Tokyo Night / One Dark / Material / GitHub / Night Owl など）× ライト/ダークモード
- **国際化** — 8 言語（アラビア語 RTL 含む）
- **インポート & エクスポート** — AES-256-GCM 暗号化単一ファイルコンテナ `.aidogx`、7 スコープ、アイテムごとの競合解決
- **トレイ + ステータスバー** — システムトレイからのクイック操作 + カスタマイズ可能なステータスバースクリプト（Python + uv）

## インストール

### システム要件

| OS | 最低バージョン | 備考 |
| --- | --- | --- |
| macOS | 12.0 (Monterey) | Intel + Apple Silicon |
| Windows | Windows 10 | x64 |
| Linux | x86_64 / aarch64 | WebKit2GTK 必要 |

**ダウンロード** 👉 <https://github.com/lazygophers/aidog/releases/latest>

### macOS

1. [Releases Latest](https://github.com/lazygophers/aidog/releases/latest) から `.dmg` をダウンロード
2. ダブルクリックで開き、**AiDog** を `Applications` フォルダにドラッグ
3. 初回起動時、アプリを**右クリック** → 「開く」を選択（Gatekeeper を回避 — アプリは未署名）

> ⚠️ 初回起動で「開発者を確認できません」と出たら: `システム設定 → プライバシーとセキュリティ → このまま開く`。

### Windows

1. [Releases Latest](https://github.com/lazygophers/aidog/releases/latest) から `.msi` インストーラをダウンロード
2. インストーラをダブルクリックし、案内に従う
3. SmartScreen がブロックしたら、「詳細情報 → 実行」をクリック

### Linux

```bash
# DEB パッケージ
sudo dpkg -i aidog_0.1.0_amd64.deb

# または AppImage
chmod +x aidog_0.1.0_amd64.AppImage
./aidog_0.1.0_amd64.AppImage
```

> Linux は先に WebKit2GTK 依存が必要: `sudo apt install libwebkit2gtk-4.1-dev`（Debian/Ubuntu）。

### 初回起動

インストール後、AiDog を起動すると自動的に:

1. ローカルプロキシサーバを起動（デフォルト `http://127.0.0.1:9876`）
2. ローカル SQLite データベースを作成（`~/.aidog/aidog.db`）
3. メイン UI を表示し、最初のプラットフォーム追加をガイド

## クイックスタート（3ステップ）

### ステップ 1: プラットフォームを追加

![AiDog — プラットフォーム追加](screenshots/add-platform.png)

1. 左ナビゲーションの **「プラットフォーム」** をクリック
2. **「+ プラットフォームを追加」** をクリック
3. 入力: **名前**（例 `My OpenAI`）、**Base URL**（例 `https://api.openai.com/v1`、`/v1` バージョンプレフィックス込み）、**API Key**
4. 保存

> 💡 Base URL はバージョンプレフィックス込み; AiDog が `/chat/completions` を自動付加 — パスの手動結合は不要。

### ステップ 2: クライアントをプロキシに向ける

AI API を消費するアプリで、API アドレスを AiDog プロキシアドレスに変更:

```
http://127.0.0.1:9876/proxy/v1
```

API キーは**任意の値**で OK — AiDog が設定した本物のキーで転送します。

### ステップ 3: 検証

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

正常な AI レスポンスが返れば設定完了。リクエストは自動的にルーティング・計量・記録されます。

## クライアント連携の詳細

### Claude Code

AiDog は **「設定 → Claude Code」** で完全な連携を提供（モデル/権限/サンドボックス/プラグイン/Hooks/StatusLine 編集、ワンクリックインポート/エクスポート）。

**方法1: 環境変数（最速）**

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:9876"
export ANTHROPIC_API_KEY="any-value"
claude
```

**方法2: ワンクリック設定エクスポート**

「設定 → Claude Code」で「Claude Code へエクスポート」をクリック; AiDog が `~/.claude.json` に書き込み:

```json
{ "apiBaseUrl": "http://127.0.0.1:9876" }
```

**グループごとの隔離** — 「グループ設定を同期」をクリックするとグループごとに独立設定を生成（`~/.aidog/settings.<グループ名>.json`）; グループカードの「Claude」ボタンが起動コマンドをコピー。

### OpenAI Codex

`~/.codex/config.toml` を編集（または「設定 → Codex」タブ内で編集）:

```toml
[provider]
name = "openai"
base_url = "http://127.0.0.1:9876/proxy/v1"
api_key = "any-value"

[model]
name = "o3"
```

> Codex は Responses API（`/v1/responses`）を使用; AiDog が自動検出してルーティング。

### 任意の OpenAI / Anthropic 互換クライアント

クライアントの `base_url` / `OPENAI_API_BASE` / `ANTHROPIC_BASE_URL` を `http://127.0.0.1:9876/proxy/v1` に向け、キーに任意の値を使用。

> 🔐 **グループ認証** — プロキシアドレスのキーに**グループ名**を指定; AiDog が Bearer トークンで対応グループにルーティング: `Authorization: Bearer <グループ名>`。

![AiDog — 設定](screenshots/settings.png)

## ソースからビルド

```bash
# クローン
git clone https://github.com/lazygophers/aidog.git
cd aidog

# 依存インストール
yarn install

# 開発モード
yarn tauri dev

# プロダクションビルド
yarn tauri build
```

**前提条件** — Node.js ≥ 18、Yarn 4.x、Rust toolchain（rustup）、Tauri CLI、OS ごとのシステム依存（[Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/) 参照）。

## 技術スタック

| レイヤー | 技術 |
| --- | --- |
| デスクトップフレームワーク | Tauri 2.0 |
| フロントエンド | React 19 + TypeScript + Vite |
| バックエンド | Rust + Axum プロキシ + SQLite ストレージ |
| ドキュメント | Rspress（8言語サイト） |
| ビルド | Yarn 4 + Vite + cargo |

## ドキュメント

完全なドキュメントサイト 👉 <https://lazygophers.github.io/aidog/ja/>

| トピック | リンク |
| --- | --- |
| クイックスタート | [/getting-started/quick-start](https://lazygophers.github.io/aidog/ja/getting-started/quick-start) |
| インストールガイド | [/getting-started/installation](https://lazygophers.github.io/aidog/ja/getting-started/installation) |
| プラットフォームプロトコル | [/platforms/protocols](https://lazygophers.github.io/aidog/ja/platforms/protocols) |
| グループ & ルーティング | [/groups/routing-rules](https://lazygophers.github.io/aidog/ja/groups/routing-rules) |
| スマートスケジューリング | [/groups/scheduling](https://lazygophers.github.io/aidog/ja/groups/scheduling) |
| Codex 連携 | [/proxy/codex-integration](https://lazygophers.github.io/aidog/ja/proxy/codex-integration) |
| ミドルウェアルール | [/middleware](https://lazygophers.github.io/aidog/ja/middleware/) |
| 使用量統計 & 価格 | [/stats/usage-stats](https://lazygophers.github.io/aidog/ja/stats/usage-stats) |
| API リファレンス | [/api/api-reference](https://lazygophers.github.io/aidog/ja/api/api-reference) |

## 多言語 README

| 言語 | ファイル |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## 推奨 IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## 謝辞

[![LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)

[LINUX DO](https://linux.do) コミュニティに感謝します。

## ライセンス

MIT
