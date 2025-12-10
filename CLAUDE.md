# CLAUDE.md

このファイルは、Claude Codeがプロジェクトを理解するためのガイドである。

## プロジェクト概要

MoZuku は日本語校正のための Language Server Protocol (LSP) 実装である。C++版からRust版への移行を進めており、LLM連携機能を強化した次世代の日本語校正ツールを目指している。

### プロジェクトの目的

本プロジェクトは以下の3つの課題解決を目的としている：

1. **システム依存性の排除**:
   C++版で必須であったMeCabやCaboChaのシステムインストールを不要とし、RustおよびLinderaを採用することで、ポータブルなシングルバイナリとして提供する。

2. **LLMによる相互補完**:
   - **LLMを用いた推敲**: 従来のルールベース手法では困難な「文脈理解」や「トーン＆マナーの統一」をLLMで実現
   - **LLM出力の校正**: Claude CodeやChatGPT等が生成する日本語の「不自然な表現」や「文法上の誤り」を高速なローカル解析で検出・修正

3. **マルチフォーマット対応**:
   ソースコード内のコメント、学術論文（LaTeX）、Web記事（HTML/Markdown）など多様な形式に対応。

### 現在の状態

- **Rust版 LSP サーバー**: `mozuku-rs/` に実装中（Phase 3 完了）
- **C++版 LSP サーバー**: `mozuku-lsp/` に実装済み（レガシー）
- **VS Code 拡張機能**: `vscode-mozuku/` に実装済み

## ディレクトリ構造

```
MoZukuRust/
├── mozuku-rs/           # Rust LSP サーバー（推奨）
│   ├── src/
│   │   ├── main.rs      # エントリーポイント
│   │   ├── lib.rs       # ライブラリモジュール定義
│   │   ├── server.rs    # LSP サーバー実装
│   │   ├── analyzer.rs  # Lindera形態素解析
│   │   ├── checker.rs   # 文法チェック
│   │   └── extractor.rs # tree-sitterテキスト抽出
│   └── Cargo.toml
├── mozuku-lsp/          # C++ LSP サーバー（レガシー）
│   ├── include/         # ヘッダファイル
│   ├── src/             # ソースファイル
│   └── third-party/     # 依存ライブラリ
├── vscode-mozuku/       # VS Code 拡張機能 (TypeScript)
│   ├── src/             # TypeScript ソース
│   └── out/             # コンパイル済み JS
├── patches/             # 依存ライブラリ用パッチ
└── docs/                # ドキュメント
```

## ビルド手順

### Rust LSP サーバー (mozuku-rs) - 推奨

```bash
cd mozuku-rs
cargo build --release
```

**依存関係**: なし（Linderaの辞書がバイナリに埋め込まれる）

生成バイナリ: `target/release/mozuku-rs`

### C++ LSP サーバー (mozuku-lsp) - レガシー

```bash
cd mozuku-lsp
mkdir build && cd build
cmake ..
make
```

**依存関係**: MeCab, CaboChaが必要

### VS Code 拡張機能

```bash
cd vscode-mozuku
npm install
npm run compile
```

## 重要なコンポーネント

### mozuku-rs (Rust) - 推奨

| ファイル | 役割 |
|---------|------|
| `main.rs` | LSP サーバー起動、ログ初期化 |
| `lib.rs` | ライブラリモジュール公開 |
| `server.rs` | LSP プロトコル処理、ドキュメント管理、ファイルタイプ検出 |
| `analyzer.rs` | Lindera形態素解析、ホバー情報、セマンティックトークン |
| `checker.rs` | 文法チェック（ら抜き、い抜き、二重助詞、二重敬語、冗長表現、連続文末、たり並列、の連続） |
| `extractor.rs` | tree-sitterテキスト抽出（Markdown, Rust, Python, JS/TS, C/C++, Go） |

### mozuku-lsp (C++) - レガシー

| ファイル | 役割 |
|---------|------|
| `lsp.cpp/hpp` | LSP プロトコル処理 |
| `analyzer.cpp/hpp` | 日本語解析メインロジック |
| `grammar_checker.cpp/hpp` | 文法チェック（ら抜き言葉等） |
| `mecab_manager.cpp/hpp` | MeCab連携（形態素解析） |
| `comment_extractor.cpp/hpp` | ソースコードからコメント抽出 |
| `pos_analyzer.cpp/hpp` | 品詞解析 |

### vscode-mozuku (TypeScript)

| ファイル | 役割 |
|---------|------|
| `extension.ts` | 拡張機能エントリーポイント |
| `client.ts` | LSP クライアント設定 |

## 開発ガイドライン

### コーディング規約

- Rust: `cargo fmt` と `cargo clippy` に準拠
- C++: `.clang-format` に準拠
- TypeScript: `eslint.config.mjs` に準拠

### 日本語処理の注意点

- UTF-8 エンコーディングを前提とする
- LSP では UTF-16 オフセットを使用するため、変換処理に注意
- 日本語文字列の長さ計算は文字数ベースで行う

### Rust版の実装状況

- [x] **Phase 1: Rust 基盤の確立**
  - tower-lsp を用いた LSP サーバーの構築
  - Lindera 統合による形態素解析の実装
  - 基礎的な文法チェック機能（ら抜き、い抜き、二重助詞）
  - セマンティックトークン（品詞ハイライト）
  - ホバー情報（品詞詳細表示）
- [x] **Phase 2: ドキュメント構造解析**
  - tree-sitter の統合（Markdown, Rust, Python, JS/TS, C/C++, Go）
  - コメント/Markdownからのテキスト抽出
  - ファイルタイプ自動検出
  - 28個のユニットテスト（TDD実装）
- [x] **Phase 3: ルールベース診断の拡充**
  - 二重敬語検出（おっしゃられる、ご覧になられる等）
  - 冗長表現検出（することができる→できる、ことが可能→できる）
  - 連続文末検出（です。です。です。）
  - たり並列検出（歩いたり走る→歩いたり走ったりする）
  - の連続検出（私の友達の本の...）
  - 37個のユニットテスト（TDD実装）
- [ ] **Phase 4: LLM 統合**
  - 設定ファイル (`mozuku.toml`) による API キー管理
  - 非同期での LLM 問い合わせ
  - Code Action による AI 修正の適用

## テスト

### Rust LSP サーバー

```bash
cd mozuku-rs
cargo test
```

### VS Code 拡張機能

```bash
cd vscode-mozuku
npm test
```

## CI/CD

GitHub Actions で CI を実行（`.github/workflows/ci.yml`）。

## 関連リンク

- [Lindera](https://github.com/lindera/lindera) - Pure Rust形態素解析器
- [tower-lsp](https://github.com/ebkalderon/tower-lsp) - Rust LSPフレームワーク
- [tree-sitter](https://tree-sitter.github.io/tree-sitter/) - パーサージェネレーター
