// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The language server's requests, sent the way the editor sends them.
//!
//! The server runs all day on half-typed code, so a request that panics
//! costs more there than anywhere. Every request the server answers goes
//! through its own handler (`server::capabilties`), at positions spread
//! over the text: word starts, just after `.`, `:` and `(`, the first and
//! the last position. What comes back must fit the documents: every range
//! and position in it, whatever the request, lies inside the text of the
//! file it names, the input or the library it is read beside, a
//! symbol's `selectionRange` lies inside its `range`, semantic tokens are
//! sorted, apart and within their line, a completion edit is on one line
//! and holds the cursor, and a rename's edits do not overlap. VS Code
//! rejects a response that breaks one of these, and the feature just stops
//! working for the user.

use auto_lsp::lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CodeActionContext, CodeActionParams, CodeLensParams, CompletionParams, CompletionResponse,
    CompletionTextEdit, DocumentDiagnosticParams, DocumentFormattingParams,
    DocumentHighlightParams, DocumentLinkParams, DocumentSymbolParams, FoldingRangeParams,
    FormattingOptions, GotoDefinitionParams, HoverParams, InlayHintParams, Position, Range,
    ReferenceContext, ReferenceParams, RenameParams, SemanticTokensParams,
    SemanticTokensRangeParams, SemanticTokensRangeResult, SemanticTokensResult,
    SignatureHelpParams, TextDocumentIdentifier, TextDocumentPositionParams, TextEdit,
    WorkspaceEdit, WorkspaceSymbolParams,
};
use serde_json::Value;
use server::capabilties as cap;

use crate::{Finding, session};

/// How many positions a text is probed at, at most.
const POSITIONS: usize = 32;

pub fn check(source: &str) -> Result<(), Finding> {
    let Some((db, _file)) = session::load(source) else {
        return Ok(());
    };
    let docs = Docs::new(source);
    let doc = &docs.input;
    let uri = session::url();
    let id = || TextDocumentIdentifier { uri: uri.clone() };
    let whole = Range::new(Position::new(0, 0), doc.end());
    let wd = Default::default;
    let pr = Default::default;

    // The requests about the whole document.
    let r = "documentSymbol";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::document_symbols::document_symbols(
                &db,
                DocumentSymbolParams {
                    text_document: id(),
                    work_done_progress_params: wd(),
                    partial_result_params: pr(),
                },
            )
        })?,
    )?;
    let r = "foldingRange";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::folding_ranges::folding_ranges(
                &db,
                FoldingRangeParams {
                    text_document: id(),
                    work_done_progress_params: wd(),
                    partial_result_params: pr(),
                },
            )
        })?,
    )?;
    let r = "inlayHint";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::inlay_hints::inlay_hints(
                &db,
                InlayHintParams {
                    text_document: id(),
                    range: whole,
                    work_done_progress_params: wd(),
                },
            )
        })?,
    )?;
    let r = "codeLens";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::code_lens::code_lens(
                &db,
                CodeLensParams {
                    text_document: id(),
                    work_done_progress_params: wd(),
                    partial_result_params: pr(),
                },
            )
        })?,
    )?;
    let r = "documentLink";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::document_links::document_links(
                &db,
                DocumentLinkParams {
                    text_document: id(),
                    work_done_progress_params: wd(),
                    partial_result_params: pr(),
                },
            )
        })?,
    )?;
    let r = "formatting";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::formatting::formatting(
                &db,
                DocumentFormattingParams {
                    text_document: id(),
                    options: FormattingOptions {
                        tab_size: 4,
                        insert_spaces: true,
                        ..Default::default()
                    },
                    work_done_progress_params: wd(),
                },
            )
        })?,
    )?;
    let r = "diagnostic";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::diagnostics::diagnostics(
                &db,
                DocumentDiagnosticParams {
                    text_document: id(),
                    identifier: None,
                    previous_result_id: None,
                    work_done_progress_params: wd(),
                    partial_result_params: pr(),
                },
            )
        })?,
    )?;
    let r = "workspaceSymbol";
    fits(
        &docs,
        doc,
        r,
        &ask(r, || {
            cap::workspace_symbols::workspace_symbols(
                &db,
                WorkspaceSymbolParams {
                    query: doc.first_word().to_string(),
                    work_done_progress_params: wd(),
                    partial_result_params: pr(),
                },
            )
        })?,
    )?;
    let r = "semanticTokens/full";
    if let Ok(Some(SemanticTokensResult::Tokens(tokens))) = ask(r, || {
        cap::semantic_tokens::semantic_tokens_full(
            &db,
            SemanticTokensParams {
                text_document: id(),
                work_done_progress_params: wd(),
                partial_result_params: pr(),
            },
        )
    })? {
        doc.tokens(r, &tokens.data)?;
    }
    let r = "semanticTokens/range";
    if let Ok(Some(SemanticTokensRangeResult::Tokens(tokens))) = ask(r, || {
        cap::semantic_tokens::semantic_tokens_range(
            &db,
            SemanticTokensRangeParams {
                text_document: id(),
                range: whole,
                work_done_progress_params: wd(),
                partial_result_params: pr(),
            },
        )
    })? {
        doc.tokens(r, &tokens.data)?;
    }

    // The requests at a position.
    for position in doc.probes() {
        let at = || TextDocumentPositionParams {
            text_document: id(),
            position,
        };
        let name = |request: &str| format!("{request} at {}:{}", position.line, position.character);
        let goto = || GotoDefinitionParams {
            text_document_position_params: at(),
            work_done_progress_params: wd(),
            partial_result_params: pr(),
        };

        let r = name("hover");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || {
                cap::hover::hover(
                    &db,
                    HoverParams {
                        text_document_position_params: at(),
                        work_done_progress_params: wd(),
                    },
                )
            })?,
        )?;
        let r = name("definition");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || cap::definition::go_to_definition(&db, goto()))?,
        )?;
        let r = name("declaration");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || cap::declaration::go_to_declaration(&db, goto()))?,
        )?;
        let r = name("typeDefinition");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || {
                cap::type_definition::go_to_type_definition(&db, goto())
            })?,
        )?;
        let r = name("implementation");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || {
                cap::implementation::go_to_implementation(&db, goto())
            })?,
        )?;
        let r = name("references");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || {
                cap::references::references(
                    &db,
                    ReferenceParams {
                        text_document_position: at(),
                        context: ReferenceContext {
                            include_declaration: true,
                        },
                        work_done_progress_params: wd(),
                        partial_result_params: pr(),
                    },
                )
            })?,
        )?;
        let r = name("documentHighlight");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || {
                cap::document_highlight::highlights(
                    &db,
                    DocumentHighlightParams {
                        text_document_position_params: at(),
                        work_done_progress_params: wd(),
                        partial_result_params: pr(),
                    },
                )
            })?,
        )?;
        let r = name("signatureHelp");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || {
                cap::signature_help::signature_help(
                    &db,
                    SignatureHelpParams {
                        text_document_position_params: at(),
                        context: None,
                        work_done_progress_params: wd(),
                    },
                )
            })?,
        )?;
        let r = name("codeAction");
        fits(
            &docs,
            doc,
            &r,
            &ask(&r, || {
                cap::code_actions::code_actions(
                    &db,
                    CodeActionParams {
                        text_document: id(),
                        range: Range::new(position, position),
                        context: CodeActionContext::default(),
                        work_done_progress_params: wd(),
                        partial_result_params: pr(),
                    },
                )
            })?,
        )?;

        let r = name("completion");
        let completions = ask(&r, || {
            cap::completions::completions(
                &db,
                CompletionParams {
                    text_document_position: at(),
                    context: None,
                    work_done_progress_params: wd(),
                    partial_result_params: pr(),
                },
            )
        })?;
        fits(&docs, doc, &r, &completions)?;
        if let Ok(Some(response)) = completions {
            doc.completion_edits(&r, position, response)?;
        }

        let r = name("rename");
        let rename = ask(&r, || {
            cap::rename::rename(
                &db,
                RenameParams {
                    text_document_position: at(),
                    new_name: "renamed".to_string(),
                    work_done_progress_params: wd(),
                },
            )
        })?;
        fits(&docs, doc, &r, &rename)?;
        if let Ok(Some(edit)) = rename {
            docs.rename_edits(&r, edit)?;
        }

        let r = name("prepareCallHierarchy");
        let items = ask(&r, || {
            cap::call_hierarchy::prepare_call_hierarchy(
                &db,
                CallHierarchyPrepareParams {
                    text_document_position_params: at(),
                    work_done_progress_params: wd(),
                },
            )
        })?;
        fits(&docs, doc, &r, &items)?;
        for item in items.ok().flatten().unwrap_or_default() {
            let r = name("callHierarchy/incomingCalls");
            let incoming = item.clone();
            fits(
                &docs,
                doc,
                &r,
                &ask(&r, || {
                    cap::call_hierarchy::incoming_calls(
                        &db,
                        CallHierarchyIncomingCallsParams {
                            item: incoming,
                            work_done_progress_params: wd(),
                            partial_result_params: pr(),
                        },
                    )
                })?,
            )?;
            let r = name("callHierarchy/outgoingCalls");
            // Its `fromRanges` are in the file of the item asked about.
            let caller = docs.at(&r, item.uri.as_str())?;
            fits(
                &docs,
                caller,
                &r,
                &ask(&r, || {
                    cap::call_hierarchy::outgoing_calls(
                        &db,
                        CallHierarchyOutgoingCallsParams {
                            item,
                            work_done_progress_params: wd(),
                            partial_result_params: pr(),
                        },
                    )
                })?,
            )?;
        }
    }
    Ok(())
}

/// Send one request. A panic is a finding that names the request and
/// where it was made. A stack overflow cannot be caught: with
/// `RK_FUZZ_TRACE=1`, each request is printed before it is sent, so the
/// last line names the one that killed the process.
fn ask<T>(request: &str, send: impl FnOnce() -> T) -> Result<T, Finding> {
    if std::env::var_os("RK_FUZZ_TRACE").is_some() {
        eprintln!("request: {request}");
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(send)).map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        Finding::new("ide-panic", format!("{request} panicked: {message}"))
    })
}

/// Check a response, if there is one; an error answer has no ranges.
fn fits<T: serde::Serialize, E>(
    docs: &Docs,
    doc: &Doc,
    request: &str,
    response: &Result<T, E>,
) -> Result<(), Finding> {
    match response {
        Ok(value) => {
            let json = serde_json::to_value(value).unwrap_or(Value::Null);
            docs.walk(doc, request, &json)
        }
        Err(_) => Ok(()),
    }
}

/// The documents the server holds: the input, and the library it is read
/// beside. An answer may point into either, and into nothing else.
struct Docs<'a> {
    input: Doc<'a>,
    library: Doc<'a>,
}

impl<'a> Docs<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            input: Doc::new(source),
            library: Doc::new(session::LIBRARY),
        }
    }

    /// The document a uri names. One the server has not open is a
    /// location the editor cannot show.
    fn at(&self, request: &str, uri: &str) -> Result<&Doc<'a>, Finding> {
        match uri {
            session::URL => Ok(&self.input),
            session::LIBRARY_URL => Ok(&self.library),
            _ => Err(Finding::new(
                "ide-range",
                format!("{request}: a location in {uri}, which is not open"),
            )),
        }
    }

    /// Every position and range in a response, wherever it sits, against
    /// the document it is in: `doc`, until an object names another file. A
    /// `Location`, a workspace symbol or a call hierarchy item names its
    /// file as `uri`; a `LocationLink` its target's as `targetUri`, while
    /// its origin stays in `doc`; an incoming call's `fromRanges` are in
    /// its caller's file, `from`; and a `WorkspaceEdit` keys its edits by
    /// uri. An outgoing call's `fromRanges` are in the file of the item
    /// asked about, which the walk is started from.
    fn walk(&self, doc: &Doc<'a>, request: &str, v: &Value) -> Result<(), Finding> {
        let bad = |why: String| Finding::new("ide-range", format!("{request}: {why}"));
        match v {
            Value::Array(items) => items.iter().try_for_each(|i| self.walk(doc, request, i)),
            Value::Object(o) => {
                let doc = match o.get("uri").and_then(Value::as_str) {
                    Some(uri) => self.at(request, uri)?,
                    None => doc,
                };
                if let Some(p) = position(v) {
                    if !doc.contains(p) {
                        return Err(bad(format!(
                            "position {}:{} is outside the document",
                            p.line, p.character
                        )));
                    }
                } else if let Some(r) = range(v)
                    && !le(r.start, r.end)
                {
                    return Err(bad(format!("range {r:?} ends before it starts")));
                }
                // LSP: a symbol's or a call hierarchy item's selection
                // range "must be contained by the range".
                if let (Some(outer), Some(inner)) = (
                    o.get("range").and_then(range),
                    o.get("selectionRange").and_then(range),
                ) && !(le(outer.start, inner.start) && le(inner.end, outer.end))
                {
                    return Err(bad(format!(
                        "selectionRange {inner:?} is not inside range {outer:?}"
                    )));
                }
                if let (Some(start), Some(end)) = (
                    o.get("startLine").and_then(Value::as_u64),
                    o.get("endLine").and_then(Value::as_u64),
                ) && (start > end || end as usize >= doc.lines.len())
                {
                    return Err(bad(format!(
                        "folding lines {start}..{end} in a {}-line document",
                        doc.lines.len()
                    )));
                }
                let target = match o.get("targetUri").and_then(Value::as_str) {
                    Some(uri) => Some(self.at(request, uri)?),
                    None => None,
                };
                let caller = match o
                    .get("from")
                    .and_then(|from| from.get("uri"))
                    .and_then(Value::as_str)
                {
                    Some(uri) => Some(self.at(request, uri)?),
                    None => None,
                };
                o.iter().try_for_each(|(key, x)| {
                    let doc = match (key.as_str(), target, caller) {
                        (key, Some(target), _) if key.starts_with("target") => target,
                        ("fromRanges", _, Some(caller)) => caller,
                        (key, _, _) if key.starts_with("file:") => self.at(request, key)?,
                        _ => doc,
                    };
                    self.walk(doc, request, x)
                })
            }
            _ => Ok(()),
        }
    }

    /// A rename replaces a name, and only it: its edits do not overlap,
    /// each covers one identifier, and it is the same one in every file,
    /// case aside as ST has it.
    fn rename_edits(&self, request: &str, edit: WorkspaceEdit) -> Result<(), Finding> {
        let mut names: Vec<String> = Vec::new();
        for (uri, edits) in edit.changes.into_iter().flatten() {
            let doc = self.at(request, uri.as_str())?;
            doc.apart(request, &edits)?;
            for e in &edits {
                let text = doc.slice(e.range).unwrap_or_default();
                let is_name =
                    !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_');
                if !is_name {
                    return Err(Finding::new(
                        "ide-edits",
                        format!("{request}: an edit replaces {text:?}, which is not a name"),
                    ));
                }
                names.push(text.to_ascii_lowercase());
            }
        }
        names.sort();
        names.dedup();
        match names.len() {
            0 | 1 => Ok(()),
            _ => Err(Finding::new(
                "ide-edits",
                format!("{request}: the edits replace different names: {names:?}"),
            )),
        }
    }
}

/// The text as LSP positions see it: its lines, each by the bytes it
/// holds, line terminator excluded. A line ends at `\n`, at `\r\n`, and at
/// a `\r` no `\n` follows: what an editor counts. Counting `\n` alone, this
/// harness read a correct answer on the wrong line past a bare `\r`.
struct Doc<'a> {
    text: &'a str,
    lines: Vec<std::ops::Range<usize>>,
}

impl<'a> Doc<'a> {
    fn new(text: &'a str) -> Self {
        let bytes = text.as_bytes();
        let mut lines = Vec::new();
        let (mut start, mut at) = (0, 0);
        while at < bytes.len() {
            let terminator = match bytes[at] {
                b'\r' if bytes.get(at + 1) == Some(&b'\n') => 2,
                b'\r' | b'\n' => 1,
                _ => 0,
            };
            if terminator > 0 {
                lines.push(start..at);
                start = at + terminator;
            }
            at += terminator.max(1);
        }
        lines.push(start..bytes.len());
        Self { text, lines }
    }

    /// The length of a line in UTF-16 code units.
    fn width(&self, line: usize) -> Option<u32> {
        let line = self.text.get(self.lines.get(line)?.clone())?;
        Some(line.encode_utf16().count() as u32)
    }

    fn end(&self) -> Position {
        let last = self.lines.len() - 1;
        Position::new(last as u32, self.width(last).unwrap_or(0))
    }

    /// Inside the text, or the start of the line after it: how an edit
    /// says "to the end of the document", which clients accept.
    fn contains(&self, p: Position) -> bool {
        match self.width(p.line as usize) {
            Some(len) => p.character <= len,
            None => p.line as usize == self.lines.len() && p.character == 0,
        }
    }

    fn first_word(&self) -> &str {
        self.text
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .find(|w| !w.is_empty())
            .unwrap_or("")
    }

    /// Where requests are sent: word starts and the positions after `.`,
    /// `:` and `(`, spread evenly, with the first and the last position.
    fn probes(&self) -> Vec<Position> {
        let mut all = vec![Position::new(0, 0)];
        let mut prev = ' ';
        for (line, bytes) in self.lines.iter().enumerate() {
            let mut col = 0u32;
            for c in self.text[bytes.clone()].chars() {
                let word_start =
                    (c.is_alphanumeric() || c == '_') && !(prev.is_alphanumeric() || prev == '_');
                if word_start || matches!(prev, '.' | ':' | '(') {
                    all.push(Position::new(line as u32, col));
                }
                col += c.len_utf16() as u32;
                prev = c;
            }
            // The terminator between two lines separates two words.
            prev = '\n';
        }
        all.push(self.end());
        let step = all.len().div_ceil(POSITIONS).max(1);
        let mut picked: Vec<Position> = all.iter().copied().step_by(step).collect();
        picked.push(self.end());
        picked.dedup();
        picked
    }

    /// Semantic tokens: relative to each other, sorted by construction,
    /// they must not overlap and must end within their line.
    fn tokens(
        &self,
        request: &str,
        data: &[auto_lsp::lsp_types::SemanticToken],
    ) -> Result<(), Finding> {
        let (mut line, mut start, mut end_of_prev) = (0u32, 0u32, 0u32);
        for (k, t) in data.iter().enumerate() {
            if t.delta_line > 0 {
                line += t.delta_line;
                start = t.delta_start;
                end_of_prev = 0;
            } else {
                start += t.delta_start;
            }
            let bad = |why: String| {
                Finding::new(
                    "ide-tokens",
                    format!("{request}: token {k} at {line}:{start}: {why}"),
                )
            };
            if start < end_of_prev {
                return Err(bad("overlaps the token before it".to_string()));
            }
            match self.width(line as usize) {
                None => return Err(bad("is past the last line".to_string())),
                Some(len) if start + t.length > len => {
                    return Err(bad(format!(
                        "ends at {}, past the line's {len}",
                        start + t.length
                    )));
                }
                Some(_) => {}
            }
            end_of_prev = start + t.length;
        }
        Ok(())
    }

    /// LSP: a completion item's edit "must be a single line range and it
    /// must contain the position at which completion has been requested".
    fn completion_edits(
        &self,
        request: &str,
        at: Position,
        response: CompletionResponse,
    ) -> Result<(), Finding> {
        let items = match response {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        };
        for item in items {
            let ranges = match item.text_edit {
                None => continue,
                Some(CompletionTextEdit::Edit(e)) => vec![e.range],
                Some(CompletionTextEdit::InsertAndReplace(e)) => vec![e.insert, e.replace],
            };
            for r in ranges {
                if r.start.line != r.end.line || !(le(r.start, at) && le(at, r.end)) {
                    return Err(Finding::new(
                        "ide-completion",
                        format!(
                            "{request}: `{}` edits {r:?}, which is not one line around the cursor",
                            item.label
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    /// The text a range covers.
    fn slice(&self, r: Range) -> Option<&str> {
        let (start, end) = (self.offset(r.start)?, self.offset(r.end)?);
        self.text.get(start..end)
    }

    /// The byte offset of an LSP position, in UTF-16 code units.
    fn offset(&self, p: Position) -> Option<usize> {
        // The start of the line after the last: the end of the document.
        if p.line as usize == self.lines.len() && p.character == 0 {
            return Some(self.text.len());
        }
        let line = self.lines.get(p.line as usize)?.clone();
        let mut units = 0;
        for (i, c) in self.text[line.clone()].char_indices() {
            if units >= p.character {
                return (units == p.character).then_some(line.start + i);
            }
            units += c.len_utf16() as u32;
        }
        (units == p.character).then_some(line.end)
    }

    /// Edits to one document must not overlap, or the client refuses them
    /// all.
    fn apart(&self, request: &str, edits: &[TextEdit]) -> Result<(), Finding> {
        let mut edits = edits.to_vec();
        edits.sort_by_key(|e| (e.range.start.line, e.range.start.character));
        for pair in edits.windows(2) {
            if !le(pair[0].range.end, pair[1].range.start) {
                return Err(Finding::new(
                    "ide-edits",
                    format!(
                        "{request}: edits {:?} and {:?} overlap",
                        pair[0].range, pair[1].range
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn le(a: Position, b: Position) -> bool {
    (a.line, a.character) <= (b.line, b.character)
}

/// A JSON object that is exactly an LSP `Position`.
fn position(v: &Value) -> Option<Position> {
    let o = v.as_object()?;
    (o.len() == 2).then_some(())?;
    Some(Position::new(
        o.get("line")?.as_u64()? as u32,
        o.get("character")?.as_u64()? as u32,
    ))
}

/// A JSON object that is exactly an LSP `Range`.
fn range(v: &Value) -> Option<Range> {
    let o = v.as_object()?;
    (o.len() == 2).then_some(())?;
    Some(Range::new(
        position(o.get("start")?)?,
        position(o.get("end")?)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn span(line: u32) -> Value {
        json!({"start": {"line": 0, "character": 0}, "end": {"line": line, "character": 0}})
    }

    /// An answer may point into the library the input is read beside, at
    /// a position the library has, and into no other file.
    #[test]
    fn a_location_is_checked_against_the_file_it_names() {
        let docs = Docs::new("x\n");
        let walk = |v: Value| docs.walk(&docs.input, "r", &v).map_err(|f| f.detail);
        let location = |uri: &str, line: u32| json!({"uri": uri, "range": span(line)});
        assert_eq!(walk(location(session::LIBRARY_URL, 20)), Ok(()));
        assert_eq!(
            walk(location(session::URL, 20)),
            Err("r: position 20:0 is outside the document".to_string())
        );
        assert_eq!(
            walk(location(session::LIBRARY_URL, 99)),
            Err("r: position 99:0 is outside the document".to_string())
        );
        assert_eq!(
            walk(location("file:///elsewhere.st", 0)),
            Err("r: a location in file:///elsewhere.st, which is not open".to_string())
        );
    }

    /// A link's target ranges are in the file it targets and its origin in
    /// the input; a workspace edit's are in the file each is keyed by.
    #[test]
    fn a_link_and_an_edit_name_their_files_differently() {
        let docs = Docs::new("x\n");
        let walk = |v: Value| docs.walk(&docs.input, "r", &v).map_err(|f| f.detail);
        let link = |origin: u32| {
            json!({
                "originSelectionRange": span(origin),
                "targetUri": session::LIBRARY_URL,
                "targetRange": span(20),
                "targetSelectionRange": span(20),
            })
        };
        assert_eq!(walk(link(1)), Ok(()));
        assert_eq!(
            walk(link(20)),
            Err("r: position 20:0 is outside the document".to_string())
        );
        let edit = |uri: &str| json!({"changes": {uri: [{"range": span(20), "newText": "x"}]}});
        assert_eq!(walk(edit(session::LIBRARY_URL)), Ok(()));
        assert_eq!(
            walk(edit(session::URL)),
            Err("r: position 20:0 is outside the document".to_string())
        );
    }

    /// An incoming call's `fromRanges` are in its caller's file; an
    /// outgoing call's in the file of the item asked about.
    #[test]
    fn a_calls_ranges_are_in_the_callers_file() {
        let docs = Docs::new("x\n");
        let walk = |doc: &Doc, v: Value| docs.walk(doc, "r", &v).map_err(|f| f.detail);
        let outside = Err("r: position 20:0 is outside the document".to_string());
        let item = |uri: &str| json!({"name": "f", "kind": 12, "uri": uri, "range": span(0), "selectionRange": span(0)});
        let incoming = |uri: &str| json!({"from": item(uri), "fromRanges": [span(20)]});
        assert_eq!(walk(&docs.input, incoming(session::LIBRARY_URL)), Ok(()));
        assert_eq!(walk(&docs.input, incoming(session::URL)), outside);
        let outgoing = json!({"to": item(session::URL), "fromRanges": [span(20)]});
        assert_eq!(walk(&docs.library, outgoing.clone()), Ok(()));
        assert_eq!(walk(&docs.input, outgoing), outside);
    }
}
