use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::analyzer::MorphologicalAnalyzer;
use crate::checker::GrammarChecker;
use crate::extractor::{FileType, TextExtractor};

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
}

impl MozukuServer {
    pub fn new(client: Client) -> Self {
        let analyzer = Arc::new(MorphologicalAnalyzer::new().expect("Failed to initialize analyzer"));
        let checker = Arc::new(GrammarChecker::new(analyzer.clone()));
        let extractor = Arc::new(TextExtractor::new());

        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            analyzer,
            checker,
            extractor,
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
}
