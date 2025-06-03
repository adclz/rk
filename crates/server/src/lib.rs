#![recursion_limit = "256"]
mod capabilties;

use ast::RK_PARSER;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::server::capabilities::TEXT_DOCUMENT_SYNC;
use auto_lsp::default::server::capabilities::WORKSPACE_PROVIDER;
use auto_lsp::default::server::file_events::changed_watched_files;
use auto_lsp::default::server::file_events::open_text_document;
use auto_lsp::lsp_server;
use auto_lsp::lsp_server::Connection;
use auto_lsp::lsp_types::notification::Cancel;
use auto_lsp::lsp_types::notification::DidChangeTextDocument;
use auto_lsp::lsp_types::notification::DidChangeWatchedFiles;
use auto_lsp::lsp_types::notification::DidCloseTextDocument;
use auto_lsp::lsp_types::notification::DidOpenTextDocument;
use auto_lsp::lsp_types::notification::DidSaveTextDocument;
use auto_lsp::lsp_types::notification::LogTrace;
use auto_lsp::lsp_types::notification::SetTrace;
use auto_lsp::lsp_types::request::{DocumentDiagnosticRequest, SemanticTokensFullRequest, SemanticTokensRangeRequest};
use auto_lsp::lsp_types::request::DocumentSymbolRequest;
use auto_lsp::lsp_types::{DiagnosticOptions, DiagnosticServerCapabilities, OneOf, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities};
use auto_lsp::lsp_types::ServerCapabilities;
use auto_lsp::server::notification_registry::NotificationRegistry;
use auto_lsp::server::options::InitOptions;
use auto_lsp::server::request_registry::RequestRegistry;
use auto_lsp::server::Session;
use auto_lsp::default::db::FileManager;
use capabilties::semantic_tokens::{SUPPORTED_TYPES};
use db::RootDatabase;
use std::error::Error;
use std::panic::RefUnwindSafe;

use crate::capabilties::document_symbols::document_symbols;
use crate::capabilties::semantic_tokens;

pub fn boot() -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Starting IEC LSP");

    let (connection, io_threads) = Connection::stdio();
    let db = RootDatabase::default();
    let mut request_registry = RequestRegistry::<RootDatabase>::default();
    let mut notification_registry = NotificationRegistry::<RootDatabase>::default();

    let (mut session, params) = Session::create(
        InitOptions {
            parsers: &RK_PARSER,
            capabilities: ServerCapabilities {
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace: WORKSPACE_PROVIDER.clone(),
                /*diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                    workspace_diagnostics: false,
                    ..Default::default()
                })),*/
                text_document_sync: TEXT_DOCUMENT_SYNC.clone(),
                semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
                    SemanticTokensOptions {
                        legend: SemanticTokensLegend {
                            token_types: SUPPORTED_TYPES.to_vec(),
                            token_modifiers: vec![],
                        },
                        range: Some(false),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            server_info: None,
        },
        connection,
        db,
    )?;

    session.main_loop(
        on_requests(&mut request_registry),
        on_notifications(&mut notification_registry),
    )?;
    io_threads.join()?;

    // Shut down gracefully.
    eprintln!("Shutting down server");
    Ok(())
}

fn on_requests<Db: BaseDatabase + Clone + RefUnwindSafe>(
    registry: &mut RequestRegistry<Db>,
) -> &mut RequestRegistry<Db> {
    registry
        .on::<DocumentSymbolRequest, _>( document_symbols)
        .on::<SemanticTokensFullRequest, _>(semantic_tokens::semantic_tokens_full)

}

fn on_notifications<Db: BaseDatabase + Clone + RefUnwindSafe>(
    registry: &mut NotificationRegistry<Db>,
) -> &mut NotificationRegistry<Db> {
    registry
        .on_mut::<DidOpenTextDocument, _>(|s, p| Ok(open_text_document(s, p)?))
        .on_mut::<DidChangeTextDocument, _>(|s, p| -> Result<(), auto_lsp::anyhow::Error> {
            Ok(s.db.update(&p.text_document.uri, &p.content_changes)?)
        })
        .on_mut::<DidChangeWatchedFiles, _>(|s, p| Ok(changed_watched_files(s, p)?))
        .on_mut::<Cancel, _>(|s, p| {
            let id: lsp_server::RequestId = match p.id {
                auto_lsp::lsp_types::NumberOrString::Number(id) => id.into(),
                auto_lsp::lsp_types::NumberOrString::String(id) => id.into(),
            };
            if let Some(response) = s.req_queue.incoming.cancel(id) {
                s.connection.sender.send(response.into())?;
            }
            Ok(())
        })
        .on::<DidSaveTextDocument, _>(|_s, _p| Ok(()))
        .on::<DidCloseTextDocument, _>(|_s, _p| Ok(()))
        .on::<SetTrace, _>(|_s, _p| Ok(()))
        .on::<LogTrace, _>(|_s, _p| Ok(()))
}
