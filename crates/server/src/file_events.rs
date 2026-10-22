// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The editor's documents and the files it watches, added to and removed
//! from rk's [`db::FileSet`]. auto-lsp has handlers for them, but they keep
//! the files in auto-lsp's own map, which salsa does not see.

use ast::RK_PARSER;
use auto_lsp::core::errors::{DataBaseError, RuntimeError};
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidOpenTextDocumentParams,
    FileChangeType,
};
use auto_lsp::salsa::Setter;
use auto_lsp::server::Session;
use db::{RootDatabase, WorkspaceDataBase};

/// A document the editor opened: the workspace's file at its URL takes its
/// version, and a document the workspace does not hold is added.
pub(crate) fn open_text_document(
    session: &mut Session<RootDatabase>,
    params: DidOpenTextDocumentParams,
) -> Result<(), RuntimeError> {
    match session.db.get_file(&params.text_document.uri) {
        Some(file) => {
            file.set_version(&mut session.db)
                .to(Some(params.text_document.version));
            Ok(())
        }
        None => {
            let file = File::from_text_doc()
                .doc(&params.text_document)
                .session(session)
                .parser(&RK_PARSER)
                .call()?;
            Ok(session.db.add_file(file)?)
        }
    }
}

/// An edit of a document, applied to the workspace's file at its URL.
pub(crate) fn change_text_document(
    session: &mut Session<RootDatabase>,
    params: DidChangeTextDocumentParams,
) -> Result<(), RuntimeError> {
    let file = session
        .db
        .get_file(&params.text_document.uri)
        .ok_or_else(|| DataBaseError::FileNotFound {
            uri: params.text_document.uri.clone(),
        })?;
    Ok(file.update_edit(&mut session.db, &params)?)
}

/// The files the editor watches: one created is read and added, one changed
/// is read again, and one deleted is emptied and removed. A file created
/// that the workspace holds already, or one changed or deleted that it does
/// not hold, is left as it is. Every change is applied, and the first one
/// that failed is the error.
pub(crate) fn changed_watched_files(
    session: &mut Session<RootDatabase>,
    params: DidChangeWatchedFilesParams,
) -> Result<(), RuntimeError> {
    let mut first_error = None;
    for change in &params.changes {
        let url = &change.uri;
        if url.scheme() != "file" {
            continue;
        }
        let held = session.db.get_file(url);
        let parser = url
            .to_file_path()
            .ok()
            .and_then(|path| db::loader::rk_parser(&path));
        let applied = match (change.typ, held, parser) {
            (FileChangeType::CREATED, None, Some(parser)) => File::from_fs()
                .session(session)
                .url(url)
                .parser(parser)
                .call()
                .and_then(|file| Ok(session.db.add_file(file)?)),
            (FileChangeType::CHANGED, Some(file), Some(parser)) => {
                file.update_full_fs(session, parser)
            }
            (FileChangeType::DELETED, Some(file), _) => file
                .reset(&mut session.db)
                .and_then(|()| session.db.remove_file(url))
                .map_err(RuntimeError::from),
            _ => Ok(()),
        };
        if let Err(error) = applied {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}
