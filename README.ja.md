<div align="center">

# 🐕 AiDog

**統合 AI API ゲートウェイ**

マルチプラットフォーム統合 · スマートルーティング · 使用量分析 — すべての AI プラットフォームのキー、リクエスト、支出を管理するクロスプラットフォームデスクトップアプリ

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/ja/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

> 📖 **完全なドキュメント**: <https://lazygophers.github.io/aidog/ja/>

AiDog は Tauri ベースのデスクトップ AI API ゲートウェイで、50以上の AI プラットフォーム全体のリクエストの管理、ルーティング、監視を統合します。散在する API キー、モデルマッピング、ロードバランシング、使用量分析を一つのアプリに集約 — バックエンドサービス不要、クラウド不要、すべてのデータはローカル保存。

## ✨ 特徴

- **マルチプラットフォーム統合** — 50以上のプラットフォームプリセット（Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen 等）、ワンクリック設定
- **スマートグループ化** — Bearer トークン / パスでリクエストをマッチング、フェイルオーバーとロードバランスルーティング
- **モデルマッピング** — 透過的なモデル名置換（例：`claude-sonnet-4` → `deepseek-chat`）
- **プロトコル変換** — OpenAI Chat / Completions / Responses、Anthropic、Gemini プロトコル間の双方向変換
- **ロードバランシングとフェイルオーバー** — 障害時にプラットフォーム間で自動リトライ、サーキットブレーキング / 三状態管理 / 指数バックオフ
- **使用量監視** — トークン統計、コスト推定、プラットフォーム別残高照会、Coding Plan クォータ表示
- **リクエストログ** — 三段階の粒度（ユーザー原本リクエスト / 上流リクエスト / サマリー）、それぞれ独立トグルと保持期間
- **ミドルウェアルールエンジン** — 入出力ルール：正規化、上書き、マスキング、インジェクション、センシティブワードフィルタ、エラー検出
- **コーディングアシスタント統合** — Claude Code、OpenAI Codex 等のネイティブワンクリックサポート
- **i18n とテーマ** — 8言語（アラビア語 RTL 含む）、Liquid Glass 等のテーマとライト/ダークモード

## 🚀 クイックスタート

### ダウンロードとインストール

[GitHub Releases](https://github.com/lazygophers/aidog/releases) からプラットフォーム（macOS / Windows / Linux）用インストーラを取得。

[インストールガイド](https://lazygophers.github.io/aidog/ja/getting-started/installation)参照。

### 3ステップで開始

1. **プラットフォーム追加** — AI プラットフォームの API キーとエンドポイントを入力
2. **プロキシ設定** — クライアントの API ベース URL をローカルプロキシに設定：
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **使用開始** — リクエストが自動的にルーティング、計量、ログ記録

curl で検証：

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 プロキシ URL の API キーは任意の値で構いません — AiDog は設定済みの実際のキーで転送します。

📖 完全チュートリアル：[クイックスタート](https://lazygophers.github.io/aidog/ja/getting-started/quick-start)。

## 🧩 技術スタック

| レイヤー | 技術 |
| --- | --- |
| デスクトップフレームワーク | Tauri 2.0 |
| フロントエンド | React 19 + TypeScript + Vite |
| バックエンド | Rust + Axum プロキシ + SQLite ストレージ |
| ビルド | Yarn + Vite |

## 🛠️ 開発

```bash
yarn                          # フロントエンド依存関係インストール
yarn tauri dev                # デスクトップアプリ起動（開発）
yarn build                    # フロントエンドビルド（tsc && vite build）
cd src-tauri && cargo build   # Rust バックエンドビルド
cd src-tauri && cargo clippy  # Rust リント（警告クリーン必須）
cd src-tauri && cargo test    # Rust テスト
```

前提条件：Node.js ≥ 18、Yarn 4.x、Rust ツールチェーン、Tauri CLI。

## 📚 ドキュメント

完全なドキュメントサイト：<https://lazygophers.github.io/aidog>

- [クイックスタート](https://lazygophers.github.io/aidog/ja/getting-started/quick-start)
- [プラットフォームプロトコル](https://lazygophers.github.io/aidog/ja/platforms/protocols)
- [グループとルーティング](https://lazygophers.github.io/aidog/ja/groups/routing-rules)
- [Codex 統合](https://lazygophers.github.io/aidog/ja/proxy/codex-integration)
- [使用統計と料金](https://lazygophers.github.io/aidog/ja/stats/usage-stats)

## 🌍 言語

| 言語 | README |
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

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)。

## License

MIT
