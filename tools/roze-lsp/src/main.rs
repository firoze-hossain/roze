// tools/roze-lsp/src/main.rs
//
// Roze Language Server Protocol (LSP) Engine ('roze-lsp')
// Full LSP v3.17 implementation delivering real-time diagnostics, semantic tokens,
// hover documentation, go-to-definition, context-aware autocomplete, signature help,
// hierarchical document symbols, and formatting for VS Code, Neovim, and Zed.

use dashmap::DashMap;
use ropey::Rope;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

mod analyzer;
mod completion;
mod definition;
mod diagnostics;
mod formatting;
mod hover;
mod parser;
mod semantic_tokens;
mod signature;
mod symbols;

use analyzer::Analyzer;
use diagnostics::DiagnosticEngine;

/// Resolves the filesystem base directory for a document URI,
/// used to resolve `import "...";` paths relative to the document.
fn base_dir_for(uri: &Url) -> PathBuf {
    uri.to_file_path()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Debug, Clone)]
pub struct Document {
    pub uri: Url,
    pub text: Rope,
    pub version: i32,
    pub ast: Option<parser::Ast>,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: Arc<RwLock<PathBuf>>,
    pub documents: Arc<DashMap<Url, Document>>,
    pub analyzer: Analyzer,
    pub diagnostic_engine: DiagnosticEngine,
}

impl Workspace {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: Arc::new(RwLock::new(root)),
            documents: Arc::new(DashMap::new()),
            analyzer: Analyzer::new(),
            diagnostic_engine: DiagnosticEngine::new(),
        }
    }

    pub fn set_root(&self, root: PathBuf) {
        if let Ok(mut r) = self.root.write() {
            *r = root;
        }
    }

    pub fn get_root(&self) -> PathBuf {
        self.root.read().map(|r| r.clone()).unwrap_or_else(|_| PathBuf::from("."))
    }

    pub fn open_document(&self, uri: Url, text: String, version: i32) {
        let rope = Rope::from_str(&text);
        let ast = parser::parse(&text);

        let doc = Document {
            uri: uri.clone(),
            text: rope,
            version,
            ast,
        };

        self.documents.insert(uri, doc);
    }

    pub fn update_document(&self, uri: &Url, text: String, version: i32) {
        if let Some(mut doc) = self.documents.get_mut(uri) {
            doc.text = Rope::from_str(&text);
            doc.version = version;
            doc.ast = parser::parse(&text);
        }
    }

    pub fn get_document(&self, uri: &Url) -> Option<Document> {
        self.documents.get(uri).map(|d| d.clone())
    }
}

struct Backend {
    client: Client,
    workspace: Workspace,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                self.workspace.set_root(path);
            }
        }

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "Roze Language Server (roze-lsp)".to_string(),
                version: Some("1.0.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        ":".to_string(),
                        "@".to_string(),
                    ]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: semantic_tokens::get_legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                        },
                    ),
                ),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "🌹 Roze Language Server v1.0.0 (LSP v3.17) initialized",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;

        self.workspace.open_document(uri.clone(), text.clone(), version);

        let diagnostics = self.workspace.diagnostic_engine.check(&text, &base_dir_for(&uri));
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        if let Some(change) = params.content_changes.first() {
            let text = change.text.clone();
            self.workspace.update_document(&uri, text.clone(), version);

            let diagnostics = self.workspace.diagnostic_engine.check(&text, &base_dir_for(&uri));
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(doc) = self.workspace.get_document(&uri) {
            let text = doc.text.to_string();
            let diagnostics = self.workspace.diagnostic_engine.check(&text, &base_dir_for(&uri));
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.workspace
            .documents
            .remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            return Ok(self.workspace.analyzer.get_hover(&doc, position));
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            let completions = self.workspace.analyzer.get_completions(&doc, position, &self.workspace);
            if !completions.is_empty() {
                return Ok(Some(CompletionResponse::Array(completions)));
            }
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            if let Some(location) = self.workspace.analyzer.get_definition(&doc, position, &self.workspace) {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            let refs = self.workspace.analyzer.get_references(&doc, position, &self.workspace);
            if !refs.is_empty() {
                return Ok(Some(refs));
            }
        }

        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        if let Some(doc) = self.workspace.get_document(&uri) {
            let symbols = self.workspace.analyzer.get_document_symbols(&doc);
            if !symbols.is_empty() {
                return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
            }
        }

        Ok(None)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;

        if let Some(doc) = self.workspace.get_document(&uri) {
            let tokens = self.workspace.analyzer.get_semantic_tokens(&doc);
            return Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data: tokens,
            })));
        }

        Ok(None)
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            return Ok(self.workspace.analyzer.get_signature_help(&doc, position, &self.workspace));
        }

        Ok(None)
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;

        if let Some(doc) = self.workspace.get_document(&uri) {
            return Ok(Some(self.workspace.analyzer.format_document(&doc, params.options)));
        }

        Ok(None)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let workspace = Workspace::new(PathBuf::from("."));

    let (service, socket) = LspService::new(|client| Backend {
        client,
        workspace: workspace.clone(),
    });

    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}
