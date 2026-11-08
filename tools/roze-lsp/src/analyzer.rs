// tools/roze-lsp/src/analyzer.rs
//
// Central Semantic Analysis Coordinator for Roze LSP v3.17.
// Delegates IDE capabilities to specialized engines.

use tower_lsp::lsp_types::*;
use crate::completion::CompletionEngine;
use crate::definition::DefinitionEngine;
use crate::formatting::FormattingEngine;
use crate::hover::HoverEngine;
use crate::semantic_tokens::SemanticTokensEngine;
use crate::signature::SignatureHelpEngine;
use crate::symbols::SymbolsEngine;
use crate::Document;
use crate::Workspace;

#[derive(Debug, Clone)]
pub struct Analyzer;

impl Analyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn get_hover(&self, doc: &Document, position: Position) -> Option<Hover> {
        HoverEngine::get_hover(doc, position)
    }

    pub fn get_hover_info(&self, doc: &Document, position: Position) -> Option<String> {
        let hover = self.get_hover(doc, position)?;
        match hover.contents {
            HoverContents::Markup(m) => Some(m.value),
            HoverContents::Scalar(MarkedString::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn get_completions(
        &self,
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Vec<CompletionItem> {
        CompletionEngine::get_completions(doc, position, workspace)
    }

    pub fn get_definition(
        &self,
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Option<Location> {
        DefinitionEngine::get_definition(doc, position, workspace)
    }

    pub fn get_references(
        &self,
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Vec<Location> {
        DefinitionEngine::get_references(doc, position, workspace)
    }

    pub fn get_document_symbols(&self, doc: &Document) -> Vec<DocumentSymbol> {
        SymbolsEngine::get_document_symbols(doc)
    }

    pub fn get_semantic_tokens(&self, doc: &Document) -> Vec<SemanticToken> {
        let source = doc.text.to_string();
        SemanticTokensEngine::compute_tokens(&source, doc.ast.as_ref())
    }

    pub fn get_signature_help(
        &self,
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Option<SignatureHelp> {
        SignatureHelpEngine::get_signature_help(doc, position, workspace)
    }

    pub fn format_document(&self, doc: &Document, options: FormattingOptions) -> Vec<TextEdit> {
        FormattingEngine::format_document(doc, options)
    }
}
