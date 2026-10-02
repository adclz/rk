// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::{
    anyhow,
    lsp_types::{DocumentFormattingParams, TextEdit},
};
use db::WorkspaceDataBase;
use formatter::format;

pub fn formatting(
    db: &impl WorkspaceDataBase,
    params: DocumentFormattingParams,
) -> anyhow::Result<Option<Vec<TextEdit>>> {
    let uri = &params.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    format(db, file)
}
