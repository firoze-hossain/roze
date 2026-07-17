// src/main.rs - FINAL FIXED VERSION
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use dashmap::DashMap;
use ropey::Rope;
use std::path::PathBuf;

mod parser;
mod analyzer;
mod diagnostics;

use analyzer::Analyzer;
use diagnostics::DiagnosticEngine;

#[derive(Debug, Clone)]
struct Document {
    uri: Url,
    text: Rope,
    version: i32,
    ast: Option<parser::Ast>,
}

#[derive(Debug, Clone)]
struct Workspace {
    root: PathBuf,
    documents: DashMap<Url, Document>,
    analyzer: Analyzer,
    diagnostic_engine: DiagnosticEngine,
}

impl Workspace {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            documents: DashMap::new(),
            analyzer: Analyzer::new(),
            diagnostic_engine: DiagnosticEngine::new(),
        }
    }

    fn open_document(&self, uri: Url, text: String, version: i32) {
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

    fn update_document(&self, uri: &Url, text: String, version: i32) {
        if let Some(mut doc) = self.documents.get_mut(uri) {
            doc.text = Rope::from_str(&text);
            doc.version = version;
            doc.ast = parser::parse(&text);
        }
    }

    fn get_document(&self, uri: &Url) -> Option<Document> {
        self.documents.get(uri).map(|d| d.clone())
    }
}

struct Backend {
    client: Client,
    workspace: Workspace,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "Roze Language Server".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "🌹 Roze Language Server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text.clone();
        let version = params.text_document.version;

        self.workspace.open_document(uri.clone(), text.clone(), version);

        let diagnostics = self.workspace.diagnostic_engine.check(&text);
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

            let diagnostics = self.workspace.diagnostic_engine.check(&text);
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.workspace.documents.remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            if let Some(info) = self.workspace.analyzer.get_hover_info(&doc, position) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(info)),
                    range: None,
                }));
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            let completions = self.workspace.analyzer.get_completions(&doc, position);
            if !completions.is_empty() {
                return Ok(Some(CompletionResponse::Array(completions)));
            }
        }

        Ok(None)
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            if let Some(location) = self.workspace.analyzer.get_definition(&doc, position) {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(doc) = self.workspace.get_document(&uri) {
            let refs = self.workspace.analyzer.get_references(&doc, position);
            if !refs.is_empty() {
                return Ok(Some(refs));
            }
        }

        Ok(None)
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        if let Some(doc) = self.workspace.get_document(&uri) {
            let symbols = self.workspace.analyzer.get_document_symbols(&doc);
            if !symbols.is_empty() {
                return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
            }
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