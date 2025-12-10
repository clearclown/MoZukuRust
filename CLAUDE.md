# CLAUDE.md

このファイルは、Claude Codeがプロジェクトを理解するためのガイドである。

## プロジェクト概要

MoZuku は日本語校正のための Language Server Protocol (LSP) 実装である。C++版からRust版への移行を計画しており、LLM連携機能を強化した次世代の日本語校正ツールを目指している。

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

- **C++版 LSP サーバー**: `mozuku-lsp/` に実装済み
- **VS Code 拡張機能**: `vscode-mozuku/` に実装済み
- **Rust版**: 未着手（README.mdのロードマップ参照）

## ディレクトリ構造

```
MoZukuRust/
├── mozuku-lsp/          # C++ LSP サーバー
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

### C++ LSP サーバー (mozuku-lsp)

```bash
cd mozuku-lsp
mkdir build && cd build
cmake ..
make
```

**依存関係**: MeCab, CaboChが必要（Rust版では不要になる予定）

### VS Code 拡張機能

```bash
cd vscode-mozuku
npm install
npm run compile
```

## 重要なコンポーネント

### mozuku-lsp (C++)

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

- C++: `.clang-format` に準拠
- TypeScript: `eslint.config.mjs` に準拠

### 日本語処理の注意点

- UTF-8 エンコーディングを前提とする
- LSP では UTF-16 オフセットを使用するため、変換処理に注意（`utf16.cpp/hpp`）
- 日本語文字列の長さ計算は文字数ベースで行う

### Rust 移行計画

Phase 1〜4 のロードマップに従って移行を進める（README.md参照）。主な変更点：

1. **MeCab → Lindera**: Pure Rust形態素解析器への置き換え
2. **tree-sitter 統合**: コメント抽出の汎用化
3. **tower-lsp**: Rust用LSPフレームワークの採用
4. **LLM API 連携**: OpenAI/Anthropic API との非同期通信

## テスト

### VS Code 拡張機能のテスト

```bash
cd vscode-mozuku
npm test
```

## CI/CD

GitHub Actions で CI を実行（`.github/workflows/ci.yml`）。

## 関連リンク

- [Lindera](https://github.com/lindera-morphology/lindera) - Pure Rust形態素解析器
- [tower-lsp](https://github.com/ebkalderon/tower-lsp) - Rust LSPフレームワーク
- [tree-sitter](https://tree-sitter.github.io/tree-sitter/) - パーサージェネレーター
