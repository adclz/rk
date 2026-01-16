#![allow(unused_variables)]
#![recursion_limit = "256"]
mod capabilties;

use ast::RK_PARSER;
use auto_lsp::anyhow;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::server::capabilities::TEXT_DOCUMENT_SYNC;
use auto_lsp::default::server::capabilities::WORKSPACE_PROVIDER;
use auto_lsp::default::server::file_events::change_text_document;
use auto_lsp::default::server::file_events::changed_watched_files;
use auto_lsp::default::server::file_events::open_text_document;
use auto_lsp::default::server::workspace_init::WorkspaceInit;
use auto_lsp::lsp_server;
use auto_lsp::lsp_server::Connection;
use auto_lsp::lsp_server::Message;
use auto_lsp::lsp_types;
use auto_lsp::lsp_types::CodeActionProviderCapability;
use auto_lsp::lsp_types::CodeLensOptions;
use auto_lsp::lsp_types::CompletionOptions;
use auto_lsp::lsp_types::DeclarationCapability;
use auto_lsp::lsp_types::DiagnosticOptions;
use auto_lsp::lsp_types::DiagnosticServerCapabilities;
use auto_lsp::lsp_types::FoldingRangeProviderCapability;
use auto_lsp::lsp_types::HoverProviderCapability;
use auto_lsp::lsp_types::ImplementationProviderCapability;
use auto_lsp::lsp_types::ServerCapabilities;
use auto_lsp::lsp_types::Url;
use auto_lsp::lsp_types::WorkDoneProgressOptions;
use auto_lsp::lsp_types::notification::Cancel;
use auto_lsp::lsp_types::notification::DidChangeTextDocument;
use auto_lsp::lsp_types::notification::DidChangeWatchedFiles;
use auto_lsp::lsp_types::notification::DidCloseTextDocument;
use auto_lsp::lsp_types::notification::DidOpenTextDocument;
use auto_lsp::lsp_types::notification::DidSaveTextDocument;
use auto_lsp::lsp_types::notification::LogTrace;
use auto_lsp::lsp_types::notification::SetTrace;
use auto_lsp::lsp_types::request::CodeActionRequest;
use auto_lsp::lsp_types::request::CodeLensRequest;
use auto_lsp::lsp_types::request::Completion;
use auto_lsp::lsp_types::request::DocumentDiagnosticRequest;
use auto_lsp::lsp_types::request::DocumentSymbolRequest;
use auto_lsp::lsp_types::request::FoldingRangeRequest;
use auto_lsp::lsp_types::request::Formatting;
use auto_lsp::lsp_types::request::GotoDeclaration;
use auto_lsp::lsp_types::request::GotoDefinition;
use auto_lsp::lsp_types::request::GotoImplementation;
use auto_lsp::lsp_types::request::HoverRequest;
use auto_lsp::lsp_types::request::InlayHintRequest;
use auto_lsp::lsp_types::request::SemanticTokensFullRequest;
use auto_lsp::lsp_types::request::WorkspaceDiagnosticRequest;
use auto_lsp::lsp_types::{
    OneOf, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensServerCapabilities,
};
use auto_lsp::salsa;
use auto_lsp::server::Session;
use auto_lsp::server::notification_registry::NotificationRegistry;
use auto_lsp::server::options::InitOptions;
use auto_lsp::server::request_registry::RequestRegistry;
use auto_lsp::server::vendored::intent::ThreadIntent;
use db::RootDatabase;
use db::WorkspaceDataBase;
use db::configuration::Configuration;
use ide_proto::SUPPORTED_MODIFIERS;
use ide_proto::SUPPORTED_TYPES;
use salsa::EventKind;
use std::error::Error;
use std::panic::RefUnwindSafe;
use std::sync::LazyLock;
use std::sync::OnceLock;

use crate::capabilties::code_actions::code_actions;
use crate::capabilties::code_lens::code_lens;
use crate::capabilties::completions::completions;
use crate::capabilties::declaration::go_to_declaration;
use crate::capabilties::definition::go_to_definition;
use crate::capabilties::diagnostics::diagnostics;
use crate::capabilties::diagnostics::workspace_diagnostics;
use crate::capabilties::document_symbols::document_symbols;
use crate::capabilties::folding_ranges::folding_ranges;
use crate::capabilties::formatting::formatting;
use crate::capabilties::hover::hover;
use crate::capabilties::implementation::go_to_implementation;
use crate::capabilties::inlay_hints::inlay_hints;
use crate::capabilties::semantic_tokens;

pub static WORKSPACE_FOLDER: OnceLock<Url> = OnceLock::new();

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
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        workspace_diagnostics: true,
                        inter_file_dependencies: true,
                        ..Default::default()
                    },
                )),
                text_document_sync: TEXT_DOCUMENT_SYNC.clone(),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: SUPPORTED_TYPES.to_vec(),
                                token_modifiers: SUPPORTED_MODIFIERS.to_vec(),
                            },
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                inlay_hint_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: None,
                    trigger_characters: Some(vec![".".to_owned(), "(".to_owned()]),
                    all_commit_characters: None,
                    completion_item: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: None,
        },
        connection,
        db,
    )?;

    params.root_uri.as_ref().map(|uri| {
        Configuration::init_or_update(&mut session.db, Some(uri.clone()));
    });

    session.init_workspace(params)?;

    session.main_loop(
        on_requests(&mut request_registry),
        on_notifications(&mut notification_registry),
    )?;
    io_threads.join()?;

    // Shut down gracefully.
    eprintln!("Shutting down server");
    Ok(())
}

fn on_requests<Db: WorkspaceDataBase + Clone + RefUnwindSafe>(
    registry: &mut RequestRegistry<Db>,
) -> &mut RequestRegistry<Db> {
    registry
        .on::<SemanticTokensFullRequest, _>(
            ThreadIntent::LatencySensitive,
            semantic_tokens::semantic_tokens_full,
        )
        .on::<Completion, _>(ThreadIntent::LatencySensitive, completions)
        .on::<DocumentDiagnosticRequest, _>(ThreadIntent::Worker, diagnostics)
        .on::<WorkspaceDiagnosticRequest, _>(ThreadIntent::Worker, workspace_diagnostics)
        .on::<DocumentSymbolRequest, _>(ThreadIntent::Worker, document_symbols)
        .on::<HoverRequest, _>(ThreadIntent::Worker, hover)
        .on::<CodeActionRequest, _>(ThreadIntent::Worker, code_actions)
        .on::<CodeLensRequest, _>(ThreadIntent::Worker, code_lens)
        .on::<FoldingRangeRequest, _>(ThreadIntent::Worker, folding_ranges)
        .on::<InlayHintRequest, _>(ThreadIntent::Worker, inlay_hints)
        .on::<Formatting, _>(ThreadIntent::Worker, formatting)
        .on::<GotoDeclaration, _>(ThreadIntent::Worker, go_to_declaration)
        .on::<GotoDefinition, _>(ThreadIntent::Worker, go_to_definition)
        .on::<GotoImplementation, _>(ThreadIntent::Worker, go_to_implementation)
}

fn on_notifications<Db: WorkspaceDataBase + Clone + RefUnwindSafe>(
    registry: &mut NotificationRegistry<Db>,
) -> &mut NotificationRegistry<Db> {
    registry
        // DidOpenTextDocument events are also emitted when a LLM / Agent creates temporary files.
        // We only want to process files with the .st extension that are part of the workspace.
        .on_mut::<DidOpenTextDocument, _>(|s, p| {
            match p.text_document.uri.as_str().ends_with(".st") {
                true => Ok(open_text_document(s, p)?),
                false => {
                    //log::warn!("Ignored opening file: {}", p.text_document.uri);
                    Ok(())
                }
            }
        })
        .on_mut::<DidChangeTextDocument, _>(|s, p| {
            match p.text_document.uri.as_str().ends_with(".st") {
                true => Ok(change_text_document(s, p)?),
                false => {
                    //log::warn!("Ignored opening file: {}", p.text_document.uri);
                    Ok(())
                }
            }
        })
        .on_mut::<DidChangeWatchedFiles, _>(|s, p| {
            changed_watched_files(s, p)?;
            send_request::<lsp_types::request::WorkspaceDiagnosticRefresh>(s, ())
        })
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
        .on::<DidSaveTextDocument, _>(ThreadIntent::Worker, |_s, _p| Ok(()))
        .on::<DidCloseTextDocument, _>(ThreadIntent::Worker, |_s, _p| Ok(()))
        .on::<SetTrace, _>(ThreadIntent::Worker, |_s, _p| Ok(()))
        .on::<LogTrace, _>(ThreadIntent::Worker, |_s, _p| Ok(()))
}

pub fn send_request<N: lsp_types::request::Request>(
    session: &Session<impl salsa::Database>,
    params: N::Params,
) -> anyhow::Result<()> {
    let params = serde_json::to_value(&params)?;
    let n = lsp_server::Request {
        method: N::METHOD.into(),
        id: lsp_server::RequestId::from(0),
        params,
    };
    session.connection.sender.send(Message::Request(n))?;
    Ok(())
}
