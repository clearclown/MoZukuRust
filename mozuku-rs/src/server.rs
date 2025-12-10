use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::analyzer::MorphologicalAnalyzer;
use crate::checker::GrammarChecker;
use crate::config::Config;
use crate::extractor::{FileType, TextExtractor};
use crate::llm::{LlmClient, ProofreadRequest};

/// Document state stored for each open file
#[derive(Debug, Clone)]
pub struct DocumentState {
    pub content: String,
    pub version: i32,
    pub file_type: FileType,
}

/// MoZuku Language Server implementation
pub struct MozukuServer {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
    analyzer: Arc<MorphologicalAnalyzer>,
    checker: Arc<GrammarChecker>,
    extractor: Arc<TextExtractor>,
    /// Configuration (stored for future use with dynamic checker settings)
    #[allow(dead_code)]
    config: Arc<Config>,
    llm_client: Arc<LlmClient>,
}

impl MozukuServer {
    pub fn new(client: Client) -> Self {
        let config = Config::load_from_default();
        let analyzer = Arc::new(MorphologicalAnalyzer::new().expect("Failed to initialize analyzer"));
        let checker = Arc::new(GrammarChecker::new(analyzer.clone()));
        let extractor = Arc::new(TextExtractor::new());
        let llm_client = Arc::new(LlmClient::new(config.clone()));

        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            analyzer,
            checker,
            extractor,
            config: Arc::new(config),
            llm_client,
        }
    }

    /// Detect file type from URI
    fn detect_file_type(uri: &Url) -> FileType {
        uri.path()
            .rsplit('.')
            .next()
            .map(FileType::from_extension)
            .unwrap_or(FileType::PlainText)
    }

    /// Analyze document and publish diagnostics
    async fn analyze_document(&self, uri: &Url) {
        let documents = self.documents.read().await;
        if let Some(doc) = documents.get(uri) {
            // Extract text spans based on file type
            let spans = match self.extractor.extract(&doc.content, doc.file_type) {
                Ok(spans) => spans,
                Err(e) => {
                    tracing::warn!("Failed to extract text from {}: {}", uri, e);
                    // Fall back to full document analysis
                    let diagnostics = self.checker.check(&doc.content);
                    self.client
                        .publish_diagnostics(uri.clone(), diagnostics, Some(doc.version))
                        .await;
                    return;
                }
            };

            // Check each extracted text span
            let mut all_diagnostics = Vec::new();
            for span in spans {
                let span_diagnostics = self.checker.check(&span.text);

                // Adjust diagnostic positions based on span offset
                for mut diag in span_diagnostics {
                    // Store original line values before modification
                    let orig_start_line = diag.range.start.line;
                    let orig_end_line = diag.range.end.line;

                    diag.range.start.line += span.start_line as u32;
                    diag.range.end.line += span.start_line as u32;

                    // If on the first line of the span, add column offset
                    if orig_start_line == 0 {
                        diag.range.start.character += span.start_col as u32;
                    }
                    if orig_end_line == 0 {
                        diag.range.end.character += span.start_col as u32;
                    }

                    all_diagnostics.push(diag);
                }
            }

            self.client
                .publish_diagnostics(uri.clone(), all_diagnostics, Some(doc.version))
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for MozukuServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        tracing::info!("MoZuku server initializing...");

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                // Semantic tokens for part-of-speech highlighting
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::KEYWORD,    // 名詞
                                    SemanticTokenType::FUNCTION,   // 動詞
                                    SemanticTokenType::PROPERTY,   // 形容詞
                                    SemanticTokenType::VARIABLE,   // 副詞
                                    SemanticTokenType::OPERATOR,   // 助詞
                                    SemanticTokenType::STRING,     // 助動詞
                                    SemanticTokenType::NUMBER,     // 接続詞
                                    SemanticTokenType::COMMENT,    // その他
                                ],
                                token_modifiers: vec![],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: Some(false),
                            ..Default::default()
                        },
                    ),
                ),
                // Hover support for word information
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Code actions for AI suggestions
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR_REWRITE,
                        ]),
                        resolve_provider: Some(true),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "MoZuku".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("MoZuku server initialized!");
        self.client
            .log_message(MessageType::INFO, "MoZuku Language Server started")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("MoZuku server shutting down...");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let version = params.text_document.version;
        let file_type = Self::detect_file_type(&uri);

        tracing::debug!("Document opened: {} (type: {:?})", uri, file_type);

        {
            let mut documents = self.documents.write().await;
            documents.insert(uri.clone(), DocumentState { content, version, file_type });
        }

        self.analyze_document(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let file_type = Self::detect_file_type(&uri);

        if let Some(change) = params.content_changes.into_iter().last() {
            let content = change.text;

            {
                let mut documents = self.documents.write().await;
                documents.insert(uri.clone(), DocumentState { content, version, file_type });
            }

            self.analyze_document(&uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        tracing::debug!("Document saved: {}", uri);
        self.analyze_document(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        tracing::debug!("Document closed: {}", uri);

        let mut documents = self.documents.write().await;
        documents.remove(&uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let documents = self.documents.read().await;
        if let Some(doc) = documents.get(uri) {
            if let Some(hover_info) = self.analyzer.get_hover_info(&doc.content, position) {
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: hover_info,
                    }),
                    range: None,
                }));
            }
        }

        Ok(None)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;

        let documents = self.documents.read().await;
        if let Some(doc) = documents.get(uri) {
            let tokens = self.analyzer.get_semantic_tokens(&doc.content);
            return Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data: tokens,
            })));
        }

        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let range = params.range;

        let documents = self.documents.read().await;
        let doc = match documents.get(uri) {
            Some(doc) => doc,
            None => return Ok(None),
        };

        // Get diagnostics in the range
        let diagnostics_in_range: Vec<_> = params
            .context
            .diagnostics
            .iter()
            .filter(|d| ranges_overlap(&d.range, &range))
            .collect();

        if diagnostics_in_range.is_empty() {
            return Ok(None);
        }

        let mut actions = Vec::new();

        for diag in diagnostics_in_range {
            // Get the text at the diagnostic range
            let text = self.get_text_at_range(&doc.content, &diag.range);

            // Create quick fix action
            let quick_fix = CodeAction {
                title: format!("修正: {}", diag.message),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diag.clone()]),
                is_preferred: Some(true),
                data: Some(serde_json::json!({
                    "uri": uri.to_string(),
                    "range": diag.range,
                    "text": text,
                    "message": diag.message,
                    "type": "quickfix"
                })),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(quick_fix));

            // If LLM is available, add AI suggestion action
            if self.llm_client.is_available() {
                let ai_action = CodeAction {
                    title: format!("🤖 AIによる修正提案: {}", diag.message),
                    kind: Some(CodeActionKind::REFACTOR_REWRITE),
                    diagnostics: Some(vec![diag.clone()]),
                    is_preferred: Some(false),
                    data: Some(serde_json::json!({
                        "uri": uri.to_string(),
                        "range": diag.range,
                        "text": text,
                        "message": diag.message,
                        "type": "ai_suggestion"
                    })),
                    ..Default::default()
                };
                actions.push(CodeActionOrCommand::CodeAction(ai_action));
            }
        }

        Ok(Some(actions))
    }

    async fn code_action_resolve(&self, mut action: CodeAction) -> Result<CodeAction> {
        let data = match action.data.as_ref() {
            Some(data) => data.clone(),
            None => return Ok(action),
        };

        let uri_str = data.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        let text = data.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let message = data.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let action_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let range: Range = serde_json::from_value(data.get("range").cloned().unwrap_or_default())
            .unwrap_or_default();

        let uri = match Url::parse(uri_str) {
            Ok(uri) => uri,
            Err(_) => return Ok(action),
        };

        // Generate the edit based on action type
        let new_text = if action_type == "ai_suggestion" {
            // Use LLM to generate suggestion
            match self
                .llm_client
                .proofread(ProofreadRequest {
                    text: text.to_string(),
                    context: None,
                    issue: Some(message.to_string()),
                })
                .await
            {
                Ok(response) => {
                    // Update action title with explanation
                    action.title = format!(
                        "🤖 {} (確信度: {:.0}%)",
                        response.explanation,
                        response.confidence * 100.0
                    );
                    response.suggestion
                }
                Err(e) => {
                    tracing::warn!("LLM request failed: {}", e);
                    return Ok(action);
                }
            }
        } else {
            // For quickfix, extract suggestion from message
            self.extract_suggestion_from_message(text, message)
        };

        // Create the workspace edit
        let edit = WorkspaceEdit {
            changes: Some(HashMap::from([(
                uri,
                vec![TextEdit {
                    range,
                    new_text,
                }],
            )])),
            ..Default::default()
        };

        action.edit = Some(edit);
        Ok(action)
    }
}

impl MozukuServer {
    /// Get text at a specific range
    fn get_text_at_range(&self, content: &str, range: &Range) -> String {
        let lines: Vec<&str> = content.lines().collect();

        let start_line = range.start.line as usize;
        let end_line = range.end.line as usize;

        if start_line >= lines.len() {
            return String::new();
        }

        if start_line == end_line {
            // Single line
            let line = lines[start_line];
            let start_char = range.start.character as usize;
            let end_char = range.end.character as usize;
            line.chars()
                .skip(start_char)
                .take(end_char - start_char)
                .collect()
        } else {
            // Multiple lines
            let mut result = String::new();

            // First line
            if let Some(line) = lines.get(start_line) {
                result.push_str(
                    &line
                        .chars()
                        .skip(range.start.character as usize)
                        .collect::<String>(),
                );
            }

            // Middle lines
            for i in start_line + 1..end_line {
                if let Some(line) = lines.get(i) {
                    result.push('\n');
                    result.push_str(line);
                }
            }

            // Last line
            if let Some(line) = lines.get(end_line) {
                result.push('\n');
                result.push_str(
                    &line
                        .chars()
                        .take(range.end.character as usize)
                        .collect::<String>(),
                );
            }

            result
        }
    }

    /// Extract suggestion from diagnostic message
    fn extract_suggestion_from_message(&self, original: &str, message: &str) -> String {
        // Common patterns: 「X」→「Y」 or X→Y
        if let Some(arrow_idx) = message.find('→') {
            let after = &message[arrow_idx + '→'.len_utf8()..];

            // Extract text between 「」
            if let Some(start) = after.find('「') {
                if let Some(end) = after[start + '「'.len_utf8()..].find('」') {
                    return after[start + '「'.len_utf8()..start + '「'.len_utf8() + end].to_string();
                }
            }

            // Or just return trimmed text after arrow
            return after.trim().to_string();
        }

        // If no suggestion found, return original
        original.to_string()
    }
}

/// Check if two ranges overlap
fn ranges_overlap(r1: &Range, r2: &Range) -> bool {
    !(r1.end.line < r2.start.line
        || r1.start.line > r2.end.line
        || (r1.end.line == r2.start.line && r1.end.character < r2.start.character)
        || (r1.start.line == r2.end.line && r1.start.character > r2.end.character))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_file_type_markdown() {
        let uri = Url::parse("file:///path/to/README.md").unwrap();
        assert_eq!(MozukuServer::detect_file_type(&uri), FileType::Markdown);
    }

    #[test]
    fn test_detect_file_type_rust() {
        let uri = Url::parse("file:///path/to/main.rs").unwrap();
        assert_eq!(MozukuServer::detect_file_type(&uri), FileType::Rust);
    }

    #[test]
    fn test_detect_file_type_python() {
        let uri = Url::parse("file:///path/to/script.py").unwrap();
        assert_eq!(MozukuServer::detect_file_type(&uri), FileType::Python);
    }

    #[test]
    fn test_detect_file_type_typescript() {
        let uri = Url::parse("file:///path/to/app.tsx").unwrap();
        assert_eq!(MozukuServer::detect_file_type(&uri), FileType::TypeScript);
    }

    #[test]
    fn test_detect_file_type_unknown() {
        let uri = Url::parse("file:///path/to/file.unknown").unwrap();
        assert_eq!(MozukuServer::detect_file_type(&uri), FileType::PlainText);
    }

    #[test]
    fn test_detect_file_type_no_extension() {
        let uri = Url::parse("file:///path/to/Makefile").unwrap();
        assert_eq!(MozukuServer::detect_file_type(&uri), FileType::PlainText);
    }

    #[test]
    fn test_ranges_overlap_same_line() {
        let r1 = Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 10 },
        };
        let r2 = Range {
            start: Position { line: 0, character: 5 },
            end: Position { line: 0, character: 15 },
        };
        assert!(ranges_overlap(&r1, &r2));
    }

    #[test]
    fn test_ranges_overlap_no_overlap() {
        let r1 = Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 5 },
        };
        let r2 = Range {
            start: Position { line: 0, character: 10 },
            end: Position { line: 0, character: 15 },
        };
        assert!(!ranges_overlap(&r1, &r2));
    }

    #[test]
    fn test_ranges_overlap_different_lines() {
        let r1 = Range {
            start: Position { line: 1, character: 0 },
            end: Position { line: 3, character: 10 },
        };
        let r2 = Range {
            start: Position { line: 2, character: 5 },
            end: Position { line: 2, character: 15 },
        };
        assert!(ranges_overlap(&r1, &r2));
    }

    #[test]
    fn test_ranges_overlap_contained() {
        let r1 = Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 5, character: 0 },
        };
        let r2 = Range {
            start: Position { line: 2, character: 0 },
            end: Position { line: 3, character: 0 },
        };
        assert!(ranges_overlap(&r1, &r2));
    }
}
