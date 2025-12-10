//! Text extraction from various document formats using tree-sitter.
//!
//! This module extracts prose text (comments, markdown content, etc.)
//! from source code and documents for Japanese proofreading.

use anyhow::Result;

/// A span of extracted text with its position in the original document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSpan {
    /// The extracted text content
    pub text: String,
    /// Start byte offset in the original document
    pub start_byte: usize,
    /// End byte offset in the original document
    pub end_byte: usize,
    /// Start line (0-indexed)
    pub start_line: usize,
    /// Start column (0-indexed)
    pub start_col: usize,
    /// End line (0-indexed)
    pub end_line: usize,
    /// End column (0-indexed)
    pub end_col: usize,
}

impl TextSpan {
    pub fn new(
        text: String,
        start_byte: usize,
        end_byte: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            text,
            start_byte,
            end_byte,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}

/// Supported file types for text extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Markdown,
    Rust,
    Python,
    TypeScript,
    JavaScript,
    C,
    Cpp,
    Go,
    LaTeX,
    PlainText,
}

impl FileType {
    /// Detect file type from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "md" | "markdown" => FileType::Markdown,
            "rs" => FileType::Rust,
            "py" | "pyi" => FileType::Python,
            "ts" | "tsx" => FileType::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => FileType::JavaScript,
            "c" | "h" => FileType::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => FileType::Cpp,
            "go" => FileType::Go,
            "tex" | "latex" => FileType::LaTeX,
            _ => FileType::PlainText,
        }
    }
}

/// Text extractor that uses tree-sitter to parse documents
pub struct TextExtractor {
    // Parsers will be lazily initialized
}

impl TextExtractor {
    pub fn new() -> Self {
        Self {}
    }

    /// Extract text spans from a document based on its file type
    pub fn extract(&self, content: &str, file_type: FileType) -> Result<Vec<TextSpan>> {
        match file_type {
            FileType::PlainText => self.extract_plain_text(content),
            FileType::Markdown => self.extract_markdown(content),
            FileType::Rust => self.extract_rust_comments(content),
            FileType::Python => self.extract_python_comments(content),
            FileType::TypeScript | FileType::JavaScript => self.extract_js_comments(content),
            FileType::C | FileType::Cpp => self.extract_c_comments(content),
            FileType::Go => self.extract_go_comments(content),
            FileType::LaTeX => self.extract_plain_text(content), // TODO: LaTeX support disabled due to linker issues
        }
    }

    /// Extract entire content as a single span (for plain text)
    fn extract_plain_text(&self, content: &str) -> Result<Vec<TextSpan>> {
        if content.is_empty() {
            return Ok(vec![]);
        }

        let lines: Vec<&str> = content.lines().collect();
        let end_line = lines.len().saturating_sub(1);
        let end_col = lines.last().map(|l| l.len()).unwrap_or(0);

        Ok(vec![TextSpan::new(
            content.to_string(),
            0,
            content.len(),
            0,
            0,
            end_line,
            end_col,
        )])
    }

    /// Extract text from Markdown (paragraphs, headings, list items)
    fn extract_markdown(&self, content: &str) -> Result<Vec<TextSpan>> {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        let language = tree_sitter_md::LANGUAGE;
        parser.set_language(&language.into())?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Markdown"))?;

        let mut spans = Vec::new();
        self.collect_markdown_text(tree.root_node(), content.as_bytes(), &mut spans);
        Ok(spans)
    }

    /// Recursively collect text nodes from Markdown AST
    fn collect_markdown_text(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        spans: &mut Vec<TextSpan>,
    ) {
        // Node types that contain prose text in tree-sitter-md
        let text_node_types = [
            "paragraph",
            "heading_content",
            "list_item",
            "atx_heading",
        ];

        // Skip code blocks and inline code
        let skip_types = ["code_block", "fenced_code_block", "code_span", "indented_code_block"];

        if skip_types.contains(&node.kind()) {
            return;
        }

        if text_node_types.contains(&node.kind()) {
            if let Ok(text) = node.utf8_text(source) {
                let text = text.trim();
                if !text.is_empty() {
                    spans.push(TextSpan::new(
                        text.to_string(),
                        node.start_byte(),
                        node.end_byte(),
                        node.start_position().row,
                        node.start_position().column,
                        node.end_position().row,
                        node.end_position().column,
                    ));
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_markdown_text(child, source, spans);
        }
    }

    /// Extract comments from Rust source code
    fn extract_rust_comments(&self, content: &str) -> Result<Vec<TextSpan>> {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser.set_language(&language.into())?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Rust"))?;

        let mut spans = Vec::new();
        self.collect_comments(tree.root_node(), content.as_bytes(), &mut spans, &["line_comment", "block_comment"]);
        Ok(spans)
    }

    /// Extract comments from Python source code
    fn extract_python_comments(&self, content: &str) -> Result<Vec<TextSpan>> {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        let language = tree_sitter_python::LANGUAGE;
        parser.set_language(&language.into())?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python"))?;

        let mut spans = Vec::new();
        self.collect_comments(tree.root_node(), content.as_bytes(), &mut spans, &["comment", "string"]);
        Ok(spans)
    }

    /// Extract comments from JavaScript/TypeScript source code
    fn extract_js_comments(&self, content: &str) -> Result<Vec<TextSpan>> {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
        parser.set_language(&language.into())?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse TypeScript/JavaScript"))?;

        let mut spans = Vec::new();
        self.collect_comments(tree.root_node(), content.as_bytes(), &mut spans, &["comment"]);
        Ok(spans)
    }

    /// Extract comments from C/C++ source code
    fn extract_c_comments(&self, content: &str) -> Result<Vec<TextSpan>> {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        let language = tree_sitter_cpp::LANGUAGE;
        parser.set_language(&language.into())?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse C/C++"))?;

        let mut spans = Vec::new();
        self.collect_comments(tree.root_node(), content.as_bytes(), &mut spans, &["comment"]);
        Ok(spans)
    }

    /// Extract comments from Go source code
    fn extract_go_comments(&self, content: &str) -> Result<Vec<TextSpan>> {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        let language = tree_sitter_go::LANGUAGE;
        parser.set_language(&language.into())?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Go"))?;

        let mut spans = Vec::new();
        self.collect_comments(tree.root_node(), content.as_bytes(), &mut spans, &["comment"]);
        Ok(spans)
    }

    /// Recursively collect comment nodes from AST
    fn collect_comments(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        spans: &mut Vec<TextSpan>,
        comment_types: &[&str],
    ) {
        if comment_types.contains(&node.kind()) {
            if let Ok(text) = node.utf8_text(source) {
                // Strip comment markers
                let cleaned = self.strip_comment_markers(text, node.kind());
                if !cleaned.trim().is_empty() {
                    spans.push(TextSpan::new(
                        cleaned,
                        node.start_byte(),
                        node.end_byte(),
                        node.start_position().row,
                        node.start_position().column,
                        node.end_position().row,
                        node.end_position().column,
                    ));
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_comments(child, source, spans, comment_types);
        }
    }

    /// Strip comment markers from comment text
    fn strip_comment_markers(&self, text: &str, kind: &str) -> String {
        match kind {
            "line_comment" => {
                // Rust // or /// or //!
                text.trim_start_matches("///")
                    .trim_start_matches("//!")
                    .trim_start_matches("//")
                    .trim()
                    .to_string()
            }
            "block_comment" => {
                // Rust /* */ or /** */
                text.trim_start_matches("/**")
                    .trim_start_matches("/*!")
                    .trim_start_matches("/*")
                    .trim_end_matches("*/")
                    .trim()
                    .to_string()
            }
            "comment" => {
                // Generic comment (Python #, C/C++ //, etc.)
                let trimmed = text.trim();
                if trimmed.starts_with('#') {
                    trimmed.trim_start_matches('#').trim().to_string()
                } else if trimmed.starts_with("//") {
                    trimmed.trim_start_matches("//").trim().to_string()
                } else if trimmed.starts_with("/*") {
                    trimmed
                        .trim_start_matches("/*")
                        .trim_end_matches("*/")
                        .trim()
                        .to_string()
                } else {
                    trimmed.to_string()
                }
            }
            "string" => {
                // Python docstring
                let trimmed = text.trim();
                if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                    trimmed[3..trimmed.len().saturating_sub(3)].trim().to_string()
                } else {
                    String::new() // Not a docstring
                }
            }
            _ => text.to_string(),
        }
    }
}

impl Default for TextExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ==========================================
    // TextSpan tests
    // ==========================================

    #[test]
    fn test_text_span_creation() {
        let span = TextSpan::new(
            "テスト".to_string(),
            0,
            9, // 3 chars * 3 bytes
            0,
            0,
            0,
            3,
        );
        assert_eq!(span.text, "テスト");
        assert_eq!(span.start_byte, 0);
        assert_eq!(span.end_byte, 9);
    }

    // ==========================================
    // FileType detection tests
    // ==========================================

    #[test]
    fn test_file_type_from_extension() {
        assert_eq!(FileType::from_extension("md"), FileType::Markdown);
        assert_eq!(FileType::from_extension("rs"), FileType::Rust);
        assert_eq!(FileType::from_extension("py"), FileType::Python);
        assert_eq!(FileType::from_extension("ts"), FileType::TypeScript);
        assert_eq!(FileType::from_extension("js"), FileType::JavaScript);
        assert_eq!(FileType::from_extension("c"), FileType::C);
        assert_eq!(FileType::from_extension("cpp"), FileType::Cpp);
        assert_eq!(FileType::from_extension("go"), FileType::Go);
        assert_eq!(FileType::from_extension("tex"), FileType::LaTeX);
        assert_eq!(FileType::from_extension("txt"), FileType::PlainText);
        assert_eq!(FileType::from_extension("unknown"), FileType::PlainText);
    }

    #[test]
    fn test_file_type_case_insensitive() {
        assert_eq!(FileType::from_extension("MD"), FileType::Markdown);
        assert_eq!(FileType::from_extension("Rs"), FileType::Rust);
        assert_eq!(FileType::from_extension("PY"), FileType::Python);
    }

    // ==========================================
    // Plain text extraction tests
    // ==========================================

    #[test]
    fn test_extract_plain_text() {
        let extractor = TextExtractor::new();
        let content = "これはテストです。\n二行目です。";
        let spans = extractor.extract(content, FileType::PlainText).unwrap();

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, content);
        assert_eq!(spans[0].start_byte, 0);
        assert_eq!(spans[0].start_line, 0);
    }

    #[test]
    fn test_extract_empty_plain_text() {
        let extractor = TextExtractor::new();
        let spans = extractor.extract("", FileType::PlainText).unwrap();
        assert!(spans.is_empty());
    }

    // ==========================================
    // Markdown extraction tests
    // ==========================================

    #[test]
    fn test_extract_markdown_paragraph() {
        let extractor = TextExtractor::new();
        let content = "これは段落です。";
        let spans = extractor.extract(content, FileType::Markdown).unwrap();

        assert!(!spans.is_empty());
        // Should contain the paragraph text
        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("これは段落です")));
    }

    #[test]
    fn test_extract_markdown_heading() {
        let extractor = TextExtractor::new();
        let content = "# 見出し\n\n本文です。";
        let spans = extractor.extract(content, FileType::Markdown).unwrap();

        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        // Should contain heading and paragraph
        assert!(texts.iter().any(|t| t.contains("見出し")));
        assert!(texts.iter().any(|t| t.contains("本文です")));
    }

    #[test]
    fn test_extract_markdown_skip_code_block() {
        let extractor = TextExtractor::new();
        let content = "説明文\n\n```rust\nlet x = 1;\n```\n\n続きの文";
        let spans = extractor.extract(content, FileType::Markdown).unwrap();

        let all_text: String = spans.iter().map(|s| s.text.as_str()).collect();
        // Code block content should NOT be included
        assert!(!all_text.contains("let x = 1"));
        // But surrounding text should be
        assert!(all_text.contains("説明文"));
    }

    // ==========================================
    // Rust comment extraction tests
    // ==========================================

    #[test]
    fn test_extract_rust_line_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
fn main() {
    // これはコメントです
    let x = 1;
}
"#;
        let spans = extractor.extract(content, FileType::Rust).unwrap();

        assert!(!spans.is_empty());
        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("これはコメントです")));
    }

    #[test]
    fn test_extract_rust_doc_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
/// ドキュメントコメント
fn foo() {}
"#;
        let spans = extractor.extract(content, FileType::Rust).unwrap();

        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("ドキュメントコメント")));
    }

    #[test]
    fn test_extract_rust_block_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
/*
 * ブロックコメント
 * 複数行
 */
fn main() {}
"#;
        let spans = extractor.extract(content, FileType::Rust).unwrap();

        let all_text: String = spans.iter().map(|s| &s.text).cloned().collect();
        assert!(all_text.contains("ブロックコメント"));
    }

    #[test]
    fn test_extract_rust_no_code() {
        let extractor = TextExtractor::new();
        let content = r#"
fn main() {
    // コメント
    let message = "文字列リテラル";
}
"#;
        let spans = extractor.extract(content, FileType::Rust).unwrap();

        let all_text: String = spans.iter().map(|s| &s.text).cloned().collect();
        // Should include comment
        assert!(all_text.contains("コメント"));
        // Should NOT include string literal or code
        assert!(!all_text.contains("文字列リテラル"));
        assert!(!all_text.contains("let message"));
    }

    // ==========================================
    // Python comment extraction tests
    // ==========================================

    #[test]
    fn test_extract_python_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
# Pythonのコメント
x = 1
"#;
        let spans = extractor.extract(content, FileType::Python).unwrap();

        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("Pythonのコメント")));
    }

    #[test]
    fn test_extract_python_docstring() {
        let extractor = TextExtractor::new();
        let content = r#"
def foo():
    """
    これはdocstringです。
    関数の説明。
    """
    pass
"#;
        let spans = extractor.extract(content, FileType::Python).unwrap();

        let all_text: String = spans.iter().map(|s| &s.text).cloned().collect();
        assert!(all_text.contains("docstring") || all_text.contains("関数の説明"));
    }

    // ==========================================
    // JavaScript/TypeScript comment extraction tests
    // ==========================================

    #[test]
    fn test_extract_js_line_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
// JSのコメント
const x = 1;
"#;
        let spans = extractor.extract(content, FileType::JavaScript).unwrap();

        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("JSのコメント")));
    }

    #[test]
    fn test_extract_ts_block_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
/**
 * TypeScriptのJSDocコメント
 */
function foo() {}
"#;
        let spans = extractor.extract(content, FileType::TypeScript).unwrap();

        let all_text: String = spans.iter().map(|s| &s.text).cloned().collect();
        assert!(all_text.contains("TypeScript") || all_text.contains("JSDoc"));
    }

    // ==========================================
    // C/C++ comment extraction tests
    // ==========================================

    #[test]
    fn test_extract_c_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
// Cのコメント
int main() { return 0; }
"#;
        let spans = extractor.extract(content, FileType::C).unwrap();

        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("Cのコメント")));
    }

    #[test]
    fn test_extract_cpp_block_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
/* C++のブロックコメント */
int main() { return 0; }
"#;
        let spans = extractor.extract(content, FileType::Cpp).unwrap();

        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("C++のブロックコメント")));
    }

    // ==========================================
    // Go comment extraction tests
    // ==========================================

    #[test]
    fn test_extract_go_comment() {
        let extractor = TextExtractor::new();
        let content = r#"
// Goのコメント
package main
func main() {}
"#;
        let spans = extractor.extract(content, FileType::Go).unwrap();

        let texts: Vec<&str> = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("Goのコメント")));
    }

    // ==========================================
    // Integration tests
    // ==========================================

    #[test]
    fn test_extractor_default() {
        let extractor = TextExtractor::default();
        let spans = extractor.extract("テスト", FileType::PlainText).unwrap();
        assert_eq!(spans.len(), 1);
    }
}
