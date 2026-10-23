// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What an edit runs again. A query's memo is worth what it saves: an edit
//! runs again what it changed, and the other files' queries are only
//! verified. Each test lists the queries salsa executes after an edit, named
//! by the file, scope or POU they are about.

use std::sync::{Arc, Mutex};

use auto_lsp::core::document::Document;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::Url;
use auto_lsp::salsa::{self, Setter, plumbing::AsId};
use db::RootDatabase;
use hir::HasName;
use hir::HirNodeInfo;
use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;
use insta::assert_snapshot;

/// A database that records the queries it executes.
struct Recorder {
    db: RootDatabase,
    executed: Arc<Mutex<Vec<String>>>,
    files: Vec<File>,
    /// What each salsa id is about, by its debug form: a file, or a POU for
    /// its own id and its scope's.
    names: Vec<(String, String)>,
}

impl Recorder {
    fn new() -> Self {
        let executed: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = executed.clone();
        let db = RootDatabase::new(Some(Box::new(move |event: salsa::Event| {
            if let salsa::EventKind::WillExecute { database_key } = event.kind {
                sink.lock().unwrap().push(format!("{database_key:?}"));
            }
        })));
        Self {
            db,
            executed,
            files: vec![],
            names: vec![],
        }
    }

    fn add(&mut self, name: &str, text: &str) -> File {
        let url = Url::parse(&format!("file:///{name}")).unwrap();
        let file = File::from_string()
            .db(&self.db)
            .parsers(&ast::RK_PARSER)
            .url(&url)
            .source(text.to_string())
            .call()
            .unwrap();
        self.db.add_file(file).unwrap();
        self.files.push(file);
        self.names
            .push((format!("{:?}", file.as_id()), name.to_string()));
        file
    }

    fn edit(&mut self, file: File, text: &str) {
        let tree = file
            .parsers(&self.db)
            .parser
            .write()
            .parse(text.as_bytes(), None)
            .unwrap();
        let document = Document::new(text.to_string(), tree, None);
        file.set_document(&mut self.db).to(Arc::new(document));
    }

    /// Checks every file, as a workspace pull does, and names the POUs the
    /// files declare.
    fn check(&mut self) {
        let files = self.files.clone();
        self.check_in(&files);
    }

    /// Checks `files` in this order, and names the POUs they declare.
    fn check_in(&mut self, files: &[File]) {
        salsa::attach(&self.db, || {
            for file in files {
                diagnostics_for_file(&self.db, *file);
            }
        });
        for file in files {
            for pou in semantic_index(&self.db, *file).global_pous.iter() {
                let name = pou.get_name_with_case(&self.db).text(&self.db).to_string();
                let scope = pou.get_scope_id(&self.db).as_id();
                self.names.push((format!("{:?}", scope), name.clone()));
                self.names.push((format!("{:?}", pou.as_id()), name));
            }
        }
    }

    /// The queries checking every file executes again, sorted, each with
    /// what it is about; `_` for what has no name here.
    fn executed_by_check(&mut self) -> String {
        let files = self.files.clone();
        self.executed_by_check_in(&files)
    }

    /// The queries checking `files` in this order executes again, as
    /// [`Recorder::executed_by_check`] lists them.
    fn executed_by_check_in(&mut self, files: &[File]) -> String {
        self.executed.lock().unwrap().clear();
        self.check_in(files);
        let mut lines: Vec<String> = self
            .executed
            .lock()
            .unwrap()
            .iter()
            .map(|key| self.named(key))
            .collect();
        lines.sort();
        lines.join("\n")
    }

    /// The codes of `file`'s diagnostics, in the order of the file.
    fn codes(&self, file: File) -> Vec<String> {
        diagnostics_for_file(&self.db, file)
            .iter()
            .map(|d| match &d.diagnostic.code {
                Some(auto_lsp::lsp_types::NumberOrString::String(code)) => code.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    fn named(&self, key: &str) -> String {
        let Some((query, rest)) = key.split_once('(') else {
            return key.to_string();
        };
        let id = rest.strip_suffix(')').unwrap_or(rest);
        let name = self
            .names
            .iter()
            .find(|(known, _)| known == id)
            .map_or("_", |(_, name)| name.as_str());
        format!("{query}({name})")
    }
}

const A: &str =
    "FUNCTION FA : INT\nVAR x : INT; END_VAR\n    x := 1;\n    FA := x;\nEND_FUNCTION\n";
const B: &str = "FUNCTION FB : INT\n    FB := FA() + 1;\nEND_FUNCTION\n";

/// A statement of FA changes and its signature does not: b.st, which only
/// calls FA, has nothing to run again.
#[test]
fn a_body_edit_runs_again_its_own_file_only() {
    let mut recorder = Recorder::new();
    let a = recorder.add("a.st", A);
    recorder.add("b.st", B);
    recorder.check();
    recorder.edit(a, &A.replace("x := 1;", "x := 2;"));
    assert_snapshot!(recorder.executed_by_check(), @r"
    Integer::as_i16_(_)
    diagnostics_for_file(a.st)
    enclosing_namespaces(FA)
    file_global_pous(a.st)
    file_namespaces(a.st)
    file_pou_map(a.st)
    file_program_map(a.st)
    get_scope(FA)
    get_scope(_)
    infer_body(FA)
    infer_initialization(FA)
    library_roots(_)
    parse(a.st)
    semantic_index(a.st)
    unbuilt_nodes(a.st)
    unbuilt_nodes(b.st)
    ");
}

/// FA's return type changes: FB, which uses its result, is inferred again.
#[test]
fn a_signature_edit_runs_again_its_callers() {
    let mut recorder = Recorder::new();
    let a = recorder.add("a.st", A);
    recorder.add("b.st", B);
    recorder.check();
    recorder.edit(a, &A.replace("FA : INT", "FA : DINT"));
    assert_snapshot!(recorder.executed_by_check(), @r"
    Integer::as_i32_(_)
    diagnostics_for_file(a.st)
    diagnostics_for_file(b.st)
    enclosing_namespaces(FA)
    file_global_pous(a.st)
    file_namespaces(a.st)
    file_pou_map(a.st)
    file_program_map(a.st)
    get_scope(FA)
    get_scope(_)
    infer_body(FA)
    infer_body(FB)
    infer_initialization(FA)
    infer_signature(FA)
    library_roots(_)
    parse(a.st)
    semantic_index(a.st)
    unbuilt_nodes(a.st)
    unbuilt_nodes(b.st)
    ");
}

/// A file added after a check answers, in the files checked before it, the
/// names it declares; removed, it answers them no more. The set of files is
/// a salsa input: adding or removing a file is a change to what read it.
#[test]
fn a_file_added_or_removed_changes_what_the_others_resolve() {
    let mut recorder = Recorder::new();
    let b = recorder.add("b.st", B);
    recorder.check();
    let unresolved = recorder.codes(b);
    assert!(!unresolved.is_empty(), "FA is declared nowhere yet");

    recorder.add("a.st", A);
    recorder.check();
    assert_eq!(recorder.codes(b), [] as [String; 0]);

    recorder
        .db
        .remove_file(&Url::parse("file:///a.st").unwrap())
        .unwrap();
    assert_eq!(recorder.codes(b), unresolved);
}

/// FB calls a FUNCTION declared nowhere, so its E0201 searches every file's
/// symbols for names to suggest. A body edit in a.st leaves those symbols as
/// they were, and FB is not inferred again.
#[test]
fn a_body_edit_keeps_the_inference_of_an_unresolved_name_elsewhere() {
    let mut recorder = Recorder::new();
    let a = recorder.add("a.st", A);
    recorder.add("b.st", &B.replace("FA()", "FX()"));
    recorder.check();
    recorder.edit(a, &A.replace("x := 1;", "x := 2;"));
    assert_snapshot!(recorder.executed_by_check(), @r"
    Integer::as_i16_(_)
    diagnostics_for_file(a.st)
    enclosing_namespaces(FA)
    file_configs(a.st)
    file_global_pous(a.st)
    file_namespaces(a.st)
    file_pou_map(a.st)
    file_program_map(a.st)
    file_symbol_index(a.st)
    get_scope(FA)
    get_scope(_)
    infer_body(FA)
    infer_initialization(FA)
    library_roots(_)
    parse(a.st)
    semantic_index(a.st)
    unbuilt_nodes(a.st)
    unbuilt_nodes(b.st)
    ");
}

/// a.st is edited to write a number no file wrote before. b.st's names are
/// the oldest, and a.st is indexed again before anything reads them: the
/// number took the slot of a name only b.st holds, and b.st was indexed and
/// inferred again. Salsa recycles an interned value the revision after
/// nothing used it, unless told to keep it longer. A new value takes a slot
/// of its own shard of the interner, so b.st has names in every shard.
#[test]
fn a_new_name_leaves_the_names_of_the_other_files_alone() {
    let mut recorder = Recorder::new();
    let a = recorder.add("a.st", A);
    let variables: String = (0..400).map(|i| format!("    v{i} : INT;\n")).collect();
    let b = recorder.add(
        "b.st",
        &B.replace(
            "FB : INT\n",
            &format!("FB : INT\nVAR\n{variables}END_VAR\n"),
        ),
    );
    recorder.check_in(&[b, a]);
    recorder.edit(a, &A.replace("x := 1;", "x := 7;"));
    assert_snapshot!(recorder.executed_by_check_in(&[a, b]), @r"
    Integer::as_i16_(_)
    diagnostics_for_file(a.st)
    enclosing_namespaces(FA)
    file_global_pous(a.st)
    file_namespaces(a.st)
    file_pou_map(a.st)
    file_program_map(a.st)
    get_scope(FA)
    get_scope(_)
    infer_body(FA)
    infer_initialization(FA)
    library_roots(_)
    parse(a.st)
    semantic_index(a.st)
    unbuilt_nodes(a.st)
    unbuilt_nodes(b.st)
    ");
}
