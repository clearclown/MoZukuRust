# MoZuku-RS

Rust実装の日本語校正Language Server

## 概要

MoZuku-RSは、日本語テキストの文法チェックとLLMによる校正支援を提供するLanguage Server Protocol (LSP) サーバーである。

## 機能

### 文法チェック（ローカル処理）

| ルール | 例 |
|--------|-----|
| ら抜き言葉 | 食べれる → 食べられる |
| い抜き言葉 | 食べてる → 食べている |
| 助詞の重複 | 私はは → 私は |
| 二重敬語 | おっしゃられる → おっしゃる |
| 冗長表現 | することができる → できる |
| 連続文末 | です。です。です。 |
| たり並列不完全 | 歩いたり走る → 歩いたり走ったり |
| の連続 | 私の友達の本の内容 |

### LLM連携（オプション）

- Claude (Anthropic) API
- OpenAI API
- Code Actionによる修正提案

### 対応ファイル形式

- Markdown (.md)
- Rust (.rs)
- Python (.py)
- TypeScript/JavaScript (.ts, .tsx, .js, .jsx)
- C/C++ (.c, .cpp, .h, .hpp)
- Go (.go)
- プレーンテキスト

## ビルド

```bash
cargo build --release
```

生成バイナリ: `target/release/mozuku-rs`

## 設定

### 設定ファイル (mozuku.toml)

```toml
[llm]
# LLMプロバイダー: "claude", "openai", "none"
provider = "claude"

# APIキー（環境変数でも設定可能）
api_key = "your-api-key"

# モデル名（省略時はデフォルト）
model = "claude-3-5-sonnet-20241022"

# カスタムエンドポイント（省略可）
# base_url = "https://api.anthropic.com"

# 最大トークン数
max_tokens = 1024

[checker]
# 各チェックルールの有効/無効
ra_nuki = true
i_nuki = true
double_particle = true
double_honorific = true
redundant_expression = true
consecutive_endings = true
tari_parallel = true
consecutive_no = true
```

### 環境変数

APIキーは環境変数でも設定可能：

- `ANTHROPIC_API_KEY` - Claude API用
- `OPENAI_API_KEY` - OpenAI API用

### 設定ファイルの読み込み順序

1. カレントディレクトリの `mozuku.toml`
2. ユーザー設定ディレクトリ（`~/.config/mozuku/mozuku.toml`）
3. デフォルト設定

## 使用方法

### VSCode

VSCode拡張機能 `vscode-mozuku` を使用する。

### Neovim

```lua
-- nvim-lspconfig
require('lspconfig').mozuku.setup {
  cmd = { "/path/to/mozuku-rs" },
  filetypes = { "markdown", "text", "rust", "python" },
}
```

### Helix

```toml
# ~/.config/helix/languages.toml
[[language]]
name = "markdown"
language-servers = ["mozuku"]

[language-server.mozuku]
command = "/path/to/mozuku-rs"
```

## テスト

```bash
cargo test
```

## ライセンス

MIT License
