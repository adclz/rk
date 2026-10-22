// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

#![allow(unused_variables)]
#![recursion_limit = "256"]
// Public for the fuzzer (crates/fuzz), which sends each request the way
// the server does.
pub mod capabilties;
mod file_events;

use auto_lsp::anyhow;
use auto_lsp::default::server::capabilities::TEXT_DOCUMENT_SYNC;
use auto_lsp::default::server::capabilities::WORKSPACE_PROVIDER;
use auto_lsp::lsp_server;
use auto_lsp::lsp_server::Connection;
use auto_lsp::lsp_server::Message;
use auto_lsp::lsp_types;
use auto_lsp::lsp_types::CallHierarchyServerCapability;
use auto_lsp::lsp_types::CodeActionProviderCapability;
use auto_lsp::lsp_types::CodeLensOptions;
use auto_lsp::lsp_types::CompletionOptions;
use auto_lsp::lsp_types::DeclarationCapability;
use auto_lsp::lsp_types::DiagnosticOptions;
use auto_lsp::lsp_types::DiagnosticServerCapabilities;
use auto_lsp::lsp_types::DidChangeWatchedFilesClientCapabilities;
use auto_lsp::lsp_types::DidChangeWatchedFilesRegistrationOptions;
use auto_lsp::lsp_types::DocumentLinkOptions;
use auto_lsp::lsp_types::FileChangeType;
use auto_lsp::lsp_types::FileSystemWatcher;
use auto_lsp::lsp_types::FoldingRangeProviderCapability;
use auto_lsp::lsp_types::GlobPattern;
use auto_lsp::lsp_types::HoverProviderCapability;
use auto_lsp::lsp_types::ImplementationProviderCapability;
use auto_lsp::lsp_types::PublishDiagnosticsParams;
use auto_lsp::lsp_types::Registration;
use auto_lsp::lsp_types::RegistrationParams;
use auto_lsp::lsp_types::ServerCapabilities;
use auto_lsp::lsp_types::SignatureHelpOptions;
use auto_lsp::lsp_types::TypeDefinitionProviderCapability;
use auto_lsp::lsp_types::Url;
use auto_lsp::lsp_types::WatchKind;
use auto_lsp::lsp_types::WorkDoneProgressOptions;
use auto_lsp::lsp_types::WorkspaceClientCapabilities;
use auto_lsp::lsp_types::notification::Cancel;
use auto_lsp::lsp_types::notification::DidChangeTextDocument;
use auto_lsp::lsp_types::notification::DidChangeWatchedFiles;
use auto_lsp::lsp_types::notification::DidCloseTextDocument;
use auto_lsp::lsp_types::notification::DidOpenTextDocument;
use auto_lsp::lsp_types::notification::DidSaveTextDocument;
use auto_lsp::lsp_types::notification::LogTrace;
use auto_lsp::lsp_types::notification::Notification;
use auto_lsp::lsp_types::notification::PublishDiagnostics;
use auto_lsp::lsp_types::notification::SetTrace;
use auto_lsp::lsp_types::notification::ShowMessage;
use auto_lsp::lsp_types::request::CallHierarchyIncomingCalls;
use auto_lsp::lsp_types::request::CallHierarchyOutgoingCalls;
use auto_lsp::lsp_types::request::CallHierarchyPrepare;
use auto_lsp::lsp_types::request::CodeActionRequest;
use auto_lsp::lsp_types::request::CodeLensRequest;
use auto_lsp::lsp_types::request::Completion;
use auto_lsp::lsp_types::request::DocumentDiagnosticRequest;
use auto_lsp::lsp_types::request::DocumentHighlightRequest;
use auto_lsp::lsp_types::request::DocumentLinkRequest;
use auto_lsp::lsp_types::request::DocumentSymbolRequest;
use auto_lsp::lsp_types::request::FoldingRangeRequest;
use auto_lsp::lsp_types::request::Formatting;
use auto_lsp::lsp_types::request::GotoDeclaration;
use auto_lsp::lsp_types::request::GotoDefinition;
use auto_lsp::lsp_types::request::GotoImplementation;
use auto_lsp::lsp_types::request::GotoTypeDefinition;
use auto_lsp::lsp_types::request::HoverRequest;
use auto_lsp::lsp_types::request::InlayHintRequest;
use auto_lsp::lsp_types::request::References;
use auto_lsp::lsp_types::request::RegisterCapability;
use auto_lsp::lsp_types::request::Rename;
use auto_lsp::lsp_types::request::SemanticTokensFullRequest;
use auto_lsp::lsp_types::request::SemanticTokensRangeRequest;
use auto_lsp::lsp_types::request::SignatureHelpRequest;
use auto_lsp::lsp_types::request::WorkspaceDiagnosticRequest;
use auto_lsp::lsp_types::request::WorkspaceSymbolRequest;
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
use db::workspace::{ConfigurationNotice, Workspace};
use ide_proto::SUPPORTED_MODIFIERS;
use ide_proto::SUPPORTED_TYPES;
use std::error::Error;
use std::panic::RefUnwindSafe;

use crate::capabilties::call_hierarchy::{incoming_calls, outgoing_calls, prepare_call_hierarchy};
use crate::capabilties::code_actions::code_actions;
use crate::capabilties::code_lens::code_lens;
use crate::capabilties::completions::completions;
use crate::capabilties::declaration::go_to_declaration;
use crate::capabilties::definition::go_to_definition;
use crate::capabilties::diagnostics::diagnostics;
use crate::capabilties::diagnostics::workspace_diagnostics;
use crate::capabilties::document_highlight::highlights;
use crate::capabilties::document_links::document_links;
use crate::capabilties::document_symbols::document_symbols;
use crate::capabilties::folding_ranges::folding_ranges;
use crate::capabilties::formatting::formatting;
use crate::capabilties::hover::hover;
use crate::capabilties::implementation::go_to_implementation;
use crate::capabilties::inlay_hints::inlay_hints;
use crate::capabilties::inline_value::inline_value;
use crate::capabilties::references::references;
use crate::capabilties::rename::rename;
use crate::capabilties::semantic_tokens;
use crate::capabilties::signature_help::signature_help;
use crate::capabilties::type_definition::go_to_type_definition;
use crate::capabilties::workspace_symbols::workspace_symbols;

/// `textDocument/inlineValue`, with the result the SPEC gives it.
///
/// `lsp_types::request::InlineValueRequest` declares `Option<InlineValue>`;
/// the protocol says `InlineValue[] | null`. A client handed a bare object
/// where it expects an array shows nothing at all, so the request is declared
/// here rather than taken from the crate.
enum InlineValues {}

impl auto_lsp::lsp_types::request::Request for InlineValues {
    type Params = auto_lsp::lsp_types::InlineValueParams;
    type Result = Option<Vec<auto_lsp::lsp_types::InlineValue>>;
    const METHOD: &'static str = "textDocument/inlineValue";
}

pub fn boot() -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Starting IEC LSP");

    let (connection, io_threads) = Connection::stdio();
    serve(connection)?;
    io_threads.join()?;

    // Shut down gracefully.
    eprintln!("Shutting down server");
    Ok(())
}

/// Serves the client at the other end of `connection` until it shuts the
/// server down: over stdio from [`boot`], in memory in the tests.
fn serve(connection: Connection) -> Result<(), Box<dyn Error + Send + Sync>> {
    let db = RootDatabase::default();
    let mut request_registry = RequestRegistry::<RootDatabase>::default();
    let mut notification_registry = NotificationRegistry::<RootDatabase>::default();

    let (mut session, params) = Session::create(
        InitOptions {
            capabilities: ServerCapabilities {
                document_symbol_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
                inline_value_provider: Some(OneOf::Left(true)),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                workspace: WORKSPACE_PROVIDER.clone(),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("rk".to_string()),
                        workspace_diagnostics: true,
                        inter_file_dependencies: true,
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: Some(true),
                        },
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
                            range: Some(true),
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
                    trigger_characters: Some(vec![
                        ".".to_owned(),
                        "#".to_owned(),
                        "(".to_owned(),
                        // Opens a pragma, whose name is all that can follow.
                        "{".to_owned(),
                        // Opens an `{allow}` argument, which names a lint rule.
                        "'".to_owned(),
                    ]),
                    all_commit_characters: None,
                    completion_item: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: Default::default(),
                }),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                    // What keeps the popup alive once it is showing. Without
                    // these it closed on the first argument separator the user
                    // wrote, so help was only ever visible on the empty call.
                    retrigger_characters: Some(vec![
                        ",".to_owned(),
                        "=".to_owned(),
                        " ".to_owned(),
                    ]),
                    work_done_progress_options: Default::default(),
                }),
                workspace_symbol_provider: Some(OneOf::Left(true)),

                ..Default::default()
            },
            server_info: None,
        },
        connection,
        db,
    )?;

    // The folder the editor opened, which the configuration and the files
    // both come from. `root_uri` is what a client always sends; the first
    // workspace folder is the same place when it sends those instead.
    let workspace = params
        .root_uri
        .clone()
        .or_else(|| {
            params
                .workspace_folders
                .as_ref()?
                .first()
                .map(|f| f.uri.clone())
        })
        .and_then(|uri| uri.to_file_path().ok());
    refresh_configuration(&mut session, workspace.as_deref())?;

    // Register file watchers after workspace initialization
    setup_file_watcher_if_necessary(&mut session, &params.capabilities);

    session.main_loop(
        on_requests(&mut request_registry),
        on_notifications(&mut notification_registry),
    )?;
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
        .on::<SemanticTokensRangeRequest, _>(
            ThreadIntent::LatencySensitive,
            semantic_tokens::semantic_tokens_range,
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
        .on::<CallHierarchyPrepare, _>(ThreadIntent::Worker, prepare_call_hierarchy)
        .on::<CallHierarchyIncomingCalls, _>(ThreadIntent::Worker, incoming_calls)
        .on::<CallHierarchyOutgoingCalls, _>(ThreadIntent::Worker, outgoing_calls)
        .on::<InlineValues, _>(ThreadIntent::Worker, inline_value)
        .on::<GotoTypeDefinition, _>(ThreadIntent::Worker, go_to_type_definition)
        .on::<DocumentHighlightRequest, _>(ThreadIntent::LatencySensitive, highlights)
        .on::<GotoDeclaration, _>(ThreadIntent::Worker, go_to_declaration)
        .on::<GotoDefinition, _>(ThreadIntent::Worker, go_to_definition)
        .on::<GotoImplementation, _>(ThreadIntent::Worker, go_to_implementation)
        .on::<References, _>(ThreadIntent::Worker, references)
        .on::<Rename, _>(ThreadIntent::Worker, rename)
        .on::<SignatureHelpRequest, _>(ThreadIntent::LatencySensitive, signature_help)
        .on::<DocumentLinkRequest, _>(ThreadIntent::Worker, document_links)
        .on::<WorkspaceSymbolRequest, _>(ThreadIntent::Worker, workspace_symbols)
}

/// Whether the editor's document is one of rk's own: the language server
/// reads rk's files only, on disk and in the editor.
fn is_rk_document(uri: &Url) -> bool {
    db::loader::is_st_file(std::path::Path::new(uri.path()))
}

/// Whether the editor's document is the workspace's. The editor hands the
/// server documents from anywhere, so it asks before adding one: a `file:`
/// one is when the loader reads it, and one with no file, `untitled:`,
/// always is.
fn is_workspace_document(db: &RootDatabase, uri: &Url) -> bool {
    is_rk_document(uri)
        && match uri.scheme() {
            "file" => uri
                .to_file_path()
                .is_ok_and(|path| db::loader::is_workspace_file(db, &path)),
            _ => true,
        }
}

fn on_notifications(
    registry: &mut NotificationRegistry<RootDatabase>,
) -> &mut NotificationRegistry<RootDatabase> {
    registry
        .on_mut::<DidOpenTextDocument, _>(|s, p| {
            if is_workspace_document(&s.db, &p.text_document.uri) {
                file_events::open_text_document(s, p)?;
            }
            Ok(())
        })
        // A document left out when it was opened stays out.
        .on_mut::<DidChangeTextDocument, _>(|s, p| match s.db.get_file(&p.text_document.uri) {
            Some(_) => Ok(file_events::change_text_document(s, p)?),
            None => Ok(()),
        })
        .on_mut::<DidChangeWatchedFiles, _>(|s, mut p| {
            let config_changed = Workspace::try_get(&s.db)
                .and_then(|c| c.workspace_folder(&s.db).cloned())
                .and_then(|ws| Url::from_file_path(ws.join("config.toml")).ok())
                .is_some_and(|url| p.changes.iter().any(|e| e.uri == url));

            if config_changed {
                let workspace =
                    Workspace::try_get(&s.db).and_then(|c| c.workspace_folder(&s.db).cloned());
                refresh_configuration(s, workspace.as_deref())?;
            } else {
                // A file created is taken when the workspace reads it; one
                // changed or deleted is the workspace's when it holds it.
                p.changes.retain(|change| {
                    change.typ != FileChangeType::CREATED
                        || is_workspace_document(&s.db, &change.uri)
                });
                file_events::changed_watched_files(s, p)?;
            }

            send_request::<lsp_types::request::WorkspaceDiagnosticRefresh>(s, ())?;
            Ok(())
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

/// Initializes or refreshes workspace configuration.
///
/// File-attached errors (e.g. config.toml parse errors) are pushed via
/// `textDocument/publishDiagnostics`. An empty list is always sent so the
/// client clears stale squiggles when errors are fixed. Errors without a
/// file location (e.g. no library configured) are reported via `window/showMessage`.
fn refresh_configuration(
    session: &mut Session<RootDatabase>,
    workspace: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    let mut file_errors = vec![];
    let mut notices: Vec<ConfigurationNotice> = vec![];
    db::loader::open_workspace(
        &mut session.db,
        workspace,
        session.encoding.clone(),
        db::loader::rk_parser,
        // The editor reads: a library the cache lacks is reported, and
        // `rk fetch` fetches it.
        None,
        &mut file_errors,
        &mut notices,
    );

    // Push diagnostics for the config file via publishDiagnostics.
    // Always send (even an empty list) so the client clears stale squiggles.
    if let Some(config_uri) = db::workspace::Workspace::try_get(&session.db)
        .and_then(|w| w.config_file(&session.db).cloned())
        .and_then(|path| Url::from_file_path(path).ok())
    {
        for error in &file_errors {
            log::warn!("Configuration: {}", error.diagnostic.message);
        }
        let diags: Vec<lsp_types::Diagnostic> = file_errors
            .iter()
            .map(|e| e.to_lsp_diagnostic(&session.db))
            .collect();
        let notification = lsp_server::Notification::new(
            PublishDiagnostics::METHOD.to_string(),
            PublishDiagnosticsParams {
                uri: config_uri,
                diagnostics: diags,
                version: None,
            },
        );
        session
            .connection
            .sender
            .send(Message::Notification(notification))?;
    }

    // Locationless notices → window/showMessage
    for notice in &notices {
        log::warn!("Configuration: {}", notice);
        let params = lsp_types::ShowMessageParams {
            typ: lsp_types::MessageType::WARNING,
            message: notice.to_string(),
        };
        let notification = lsp_server::Notification::new(ShowMessage::METHOD.to_string(), params);
        session
            .connection
            .sender
            .send(Message::Notification(notification))?;
    }

    // Send server status notification for status bar
    let (status, message) = if !file_errors.is_empty() {
        (
            "error",
            format!(
                "config.toml: {}",
                file_errors
                    .iter()
                    .map(|e| e.diagnostic.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )
    } else if !notices.is_empty() {
        (
            "warning",
            notices
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("; "),
        )
    } else {
        ("ok", String::new())
    };

    let status_notification = lsp_server::Notification::new(
        "rk/serverStatus".to_string(),
        serde_json::json!({ "status": status, "message": message }),
    );
    session
        .connection
        .sender
        .send(Message::Notification(status_notification))?;

    Ok(())
}

pub fn send_request<N: lsp_types::request::Request>(
    session: &Session<impl salsa::Database>,
    params: N::Params,
) -> anyhow::Result<()> {
    use std::sync::atomic::{AtomicI32, Ordering};
    static NEXT_ID: AtomicI32 = AtomicI32::new(1);

    let params_value = serde_json::to_value(&params)?;
    let id = lsp_server::RequestId::from(NEXT_ID.fetch_add(1, Ordering::Relaxed));

    let n = lsp_server::Request {
        method: N::METHOD.into(),
        id,
        params: params_value,
    };
    session.connection.sender.send(Message::Request(n))?;
    Ok(())
}

fn setup_file_watcher_if_necessary(
    session: &mut Session<impl salsa::Database>,
    capabilities: &lsp_types::ClientCapabilities,
) {
    match capabilities.workspace {
        Some(WorkspaceClientCapabilities {
            did_change_watched_files:
                Some(DidChangeWatchedFilesClientCapabilities {
                    dynamic_registration: Some(true),
                    relative_pattern_support,
                    ..
                }),
            ..
        }) => {
            let watchers = vec![
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.st".to_string()),
                    kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
                },
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/config.toml".to_string()),
                    kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
                },
            ];

            let registration_params = RegistrationParams {
                registrations: vec![Registration {
                    id: "FILE_WATCHER".to_owned(),
                    method: DidChangeWatchedFiles::METHOD.to_owned(),
                    register_options: Some(
                        serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                            watchers: watchers.clone(),
                        })
                        .unwrap(),
                    ),
                }],
            };

            if let Err(e) = send_request::<RegisterCapability>(session, registration_params) {
                tracing::error!("Failed to register file watchers: {}", e);
            }
        }
        Some(WorkspaceClientCapabilities {
            did_change_watched_files: Some(caps),
            ..
        }) => {
            tracing::error!(
                "Client has did_change_watched_files capability but dynamic_registration is not true: {:?}",
                caps
            );
        }
        _ => {
            tracing::error!("Client does not support did_change_watched_files capability");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_lsp::lsp_types::notification::{Exit, Initialized};
    use auto_lsp::lsp_types::request::{Initialize, Shutdown};
    use auto_lsp::lsp_types::{
        DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidOpenTextDocumentParams,
        FileEvent, InitializeParams, InitializedParams, TextDocumentContentChangeEvent,
        TextDocumentItem, VersionedTextDocumentIdentifier, WorkspaceDiagnosticParams,
        WorkspaceFolder,
    };

    /// A document is rk's by its extension, in any case, as the workspace's
    /// loader reads it: `MAIN.ST` was loaded from disk and its edits were
    /// dropped.
    #[test]
    fn a_document_is_rk_s_by_its_extension() {
        let is_rk = |uri: &str| is_rk_document(&Url::parse(uri).unwrap());
        assert!(is_rk("file:///plant/main.st"));
        assert!(is_rk("file:///plant/MAIN.ST"));
        assert!(is_rk("untitled:/draft.st"));
        assert!(!is_rk("file:///plant/config.toml"));
        assert!(!is_rk("file:///plant/main.st.bak"));
        assert!(!is_rk("file:///plant/st"));
    }

    /// The client's end of an in-memory connection to the server.
    struct Client {
        connection: Connection,
        id: i32,
        /// The errors the server showed, as it does for a notification it
        /// failed to handle.
        errors: Vec<String>,
    }

    impl Client {
        /// A server serving the workspace folder `workspace`, once its files
        /// are loaded, and the client's end of its connection.
        fn start(workspace: Url) -> (Client, std::thread::JoinHandle<()>) {
            let (server, connection) = Connection::memory();
            let server = std::thread::spawn(move || serve(server).unwrap());
            let mut client = Client {
                connection,
                id: 0,
                errors: vec![],
            };
            client.request::<Initialize>(InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: workspace,
                    name: "ws".into(),
                }]),
                ..Default::default()
            });
            client.notify::<Initialized>(InitializedParams {});
            (client, server)
        }

        fn stop(mut self, server: std::thread::JoinHandle<()>) {
            self.request::<Shutdown>(());
            self.notify::<Exit>(());
            server.join().unwrap();
        }

        /// Sends a request and waits for its answer.
        fn request<R: lsp_types::request::Request>(
            &mut self,
            params: R::Params,
        ) -> serde_json::Value {
            self.id += 1;
            let id = lsp_server::RequestId::from(self.id);
            let request = lsp_server::Request::new(id.clone(), R::METHOD.to_string(), params);
            self.connection
                .sender
                .send(Message::Request(request))
                .unwrap();
            let error = serde_json::json!(lsp_types::MessageType::ERROR);
            loop {
                let timeout = std::time::Duration::from_secs(60);
                match self.connection.receiver.recv_timeout(timeout).unwrap() {
                    Message::Response(response) if response.id == id => {
                        assert!(response.error.is_none(), "{:?}", response.error);
                        return response.result.unwrap_or_default();
                    }
                    Message::Notification(shown)
                        if shown.method == ShowMessage::METHOD && shown.params["type"] == error =>
                    {
                        self.errors.push(shown.params["message"].to_string());
                    }
                    _ => {}
                }
            }
        }

        fn notify<N: Notification>(&self, params: N::Params) {
            let notification = lsp_server::Notification::new(N::METHOD.to_string(), params);
            self.connection
                .sender
                .send(Message::Notification(notification))
                .unwrap();
        }

        /// The files the server reports, each with the number of its errors.
        fn report(&mut self) -> Vec<(String, usize)> {
            let report = self.request::<WorkspaceDiagnosticRequest>(WorkspaceDiagnosticParams {
                identifier: None,
                previous_result_ids: vec![],
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            });
            let mut files: Vec<(String, usize)> = report["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|file| {
                    let items = file["items"].as_array().unwrap();
                    let errors = items.iter().filter(|item| item["severity"] == 1).count();
                    (file["uri"].as_str().unwrap().to_string(), errors)
                })
                .collect();
            files.sort();
            files
        }
    }

    /// A document outside the workspace folder is left out, opened, edited or
    /// reported created, and a file created in the folder is taken. An older
    /// copy of the standard library in the cache, opened again from the
    /// editor's history, was read as the workspace's and declared every POU
    /// of the library a second time.
    #[test]
    fn a_document_outside_the_workspace_is_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let run = "FUNCTION Run : INT\n    Run := 1;\nEND_FUNCTION\n";
        let later = "FUNCTION Later : INT\n    Later := 1;\nEND_FUNCTION\n";
        write(
            "ws/config.toml",
            "[project]\nname = \"app\"\nversion = \"1\"\n",
        );
        // No standard library: the test reads none of the developer's.
        write("ws/.env", "RK_STDLIB_PATH=\n");
        write("ws/main.st", run);
        // The workspace's file again, elsewhere, as the old copy was.
        write("elsewhere/main.st", run);
        write("elsewhere/other.st", later);
        let url = |path: &str| Url::from_file_path(root.join(path)).unwrap();

        let (mut client, server) = Client::start(url("ws"));
        // Answered once the folder is loaded, which a file written before
        // that would be part of.
        assert_eq!(client.report(), [(url("ws/main.st").to_string(), 0)]);

        let outside = url("elsewhere/main.st");
        client.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(outside.clone(), "st".into(), 1, run.into()),
        });
        client.notify::<DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(outside, 2),
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: run.into(),
            }],
        });
        write("elsewhere/later.st", later);
        write("ws/later.st", later);
        client.notify::<DidChangeWatchedFiles>(DidChangeWatchedFilesParams {
            changes: vec![
                // A file the workspace does not hold: rereading it failed,
                // and the changes after it were dropped.
                FileEvent::new(url("elsewhere/other.st"), FileChangeType::CHANGED),
                FileEvent::new(url("elsewhere/later.st"), FileChangeType::CREATED),
                FileEvent::new(url("ws/later.st"), FileChangeType::CREATED),
            ],
        });

        assert_eq!(
            client.report(),
            [
                (url("ws/later.st").to_string(), 0),
                (url("ws/main.st").to_string(), 0)
            ]
        );
        assert_eq!(client.errors, [] as [String; 0]);
        client.stop(server);
    }

    /// What the editor and the watcher add, change and remove is what the
    /// other files resolve: the server keeps the files in rk's file set,
    /// which salsa sees, where auto-lsp's handlers kept them in a map of
    /// their own. A change that fails leaves the rest of its batch applied.
    #[test]
    fn the_files_follow_the_editor_and_the_watcher() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("ws")).unwrap();
        let write = |path: &str, text: &str| std::fs::write(root.join(path), text).unwrap();
        let remove = |path: &str| std::fs::remove_file(root.join(path)).unwrap();
        let declaring =
            |name: &str| format!("FUNCTION {name} : INT\n    {name} := 1;\nEND_FUNCTION\n");
        let run = "FUNCTION Run : INT\n    Run := Helper();\nEND_FUNCTION\n";
        write(
            "ws/config.toml",
            "[project]\nname = \"app\"\nversion = \"1\"\n",
        );
        // No standard library: the test reads none of the developer's.
        write("ws/.env", "RK_STDLIB_PATH=\n");
        write("ws/main.st", run);
        let url = |path: &str| Url::from_file_path(root.join(path)).unwrap();
        let main = url("ws/main.st").to_string();
        let helper = url("ws/helper.st").to_string();
        let watched = |client: &Client, changes: &[(&str, FileChangeType)]| {
            client.notify::<DidChangeWatchedFiles>(DidChangeWatchedFilesParams {
                changes: changes
                    .iter()
                    .map(|(path, typ)| FileEvent::new(url(path), *typ))
                    .collect(),
            });
        };

        let (mut client, server) = Client::start(url("ws"));
        assert_eq!(client.report(), [(main.clone(), 1)]);
        // A document of the workspace, opened: its file takes the version.
        client.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(url("ws/main.st"), "st".into(), 1, run.into()),
        });
        assert_eq!(client.report(), [(main.clone(), 1)]);

        write("ws/helper.st", &declaring("Helper"));
        watched(&client, &[("ws/helper.st", FileChangeType::CREATED)]);
        assert_eq!(client.report(), [(helper.clone(), 0), (main.clone(), 0)]);

        write("ws/helper.st", &declaring("Other"));
        watched(&client, &[("ws/helper.st", FileChangeType::CHANGED)]);
        assert_eq!(client.report(), [(helper.clone(), 0), (main.clone(), 1)]);

        remove("ws/helper.st");
        watched(&client, &[("ws/helper.st", FileChangeType::DELETED)]);
        assert_eq!(client.report(), [(main.clone(), 1)]);

        // A document no file holds, opened, then edited.
        let draft = Url::parse("untitled:/draft.st").unwrap();
        client.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                draft.clone(),
                "st".into(),
                1,
                declaring("Helper"),
            ),
        });
        assert_eq!(client.report(), [(main.clone(), 0), (draft.to_string(), 0)]);
        client.notify::<DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(draft.clone(), 2),
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: declaring("Other"),
            }],
        });
        assert_eq!(client.report(), [(main.clone(), 1), (draft.to_string(), 0)]);

        // The file read again is gone from disk, and the file created after
        // it in the batch is added all the same.
        write("ws/gone.st", &declaring("Gone"));
        watched(&client, &[("ws/gone.st", FileChangeType::CREATED)]);
        // Answered once the server read the file.
        assert_eq!(
            client.report(),
            [
                (url("ws/gone.st").to_string(), 0),
                (main.clone(), 1),
                (draft.to_string(), 0)
            ]
        );
        remove("ws/gone.st");
        write("ws/helper.st", &declaring("Helper"));
        watched(
            &client,
            &[
                ("ws/gone.st", FileChangeType::CHANGED),
                ("ws/helper.st", FileChangeType::CREATED),
            ],
        );
        assert_eq!(
            client.report(),
            [
                (url("ws/gone.st").to_string(), 0),
                (helper, 0),
                (main, 0),
                (draft.to_string(), 0)
            ]
        );
        assert_eq!(client.errors.len(), 1, "{:?}", client.errors);
        client.stop(server);
    }
}
