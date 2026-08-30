use std::ops::ControlFlow;

use ariadne::Cache;
use ariadne::CharSet;
use ariadne::Config;
use ariadne::FnCache;
use ariadne::Source;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter::Range;
use auto_lsp::{
    default::db::{FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use db::WorkspaceDataBase;
use hir::HasName;
use hir::HirNodeInfo;
use hir::check::diagnostics_for_file;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::namespace::NamespaceDecl;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::head::inheritance::MethodRef;
use hir::hir_ty::resolver::name::{PouResolution, pou_names_res};
use ide_diagnostic::{IdeDiagnostic, Related};
use ide_proto::handlers::references::ReferenceLocation;
use ide_proto::hir_node::HirNode;
use ide_proto::walk::WalkHir;
use rstest::fixture;

#[fixture]
pub fn with_db() -> RootDatabase {
    RootDatabase::default()
}

pub fn no_color_and_ascii() -> Config {
    Config::default()
        .with_color(false)
        // Using Ascii so that the inline snapshots display correctly
        // even with fonts where characters like '┬' take up more space.
        .with_char_set(CharSet::Ascii)
}

/// Panic unless EVERY registered file is diagnostic-free.
///
/// For the tests that lower several files at once and so cannot go through
/// `codegen`'s single-source helpers. Lowering a source the compiler rejects
/// measures code no user can run — see `compile_to_wasm`.
pub fn assert_workspace_is_clean(db: &RootDatabase) {
    use auto_lsp::default::db::BaseDatabase;
    let mut reported = Vec::new();
    for file in db.get_files().iter().map(|f| *f) {
        for diag in hir::check::diagnostics_for_file(db, file).iter() {
            let code = match &diag.diagnostic.code {
                Some(auto_lsp::lsp_types::NumberOrString::String(code)) => code.clone(),
                Some(auto_lsp::lsp_types::NumberOrString::Number(code)) => code.to_string(),
                None => "?".to_string(),
            };
            reported.push(format!("  [{code}] {}", diag.diagnostic.message));
        }
    }
    assert!(
        reported.is_empty(),
        "the workspace has {} diagnostic(s), cannot compile:\n{}",
        reported.len(),
        reported.join("\n")
    );
}

/// Single-source variant of [`add_sources`]: registers one file under a
/// RANDOM url (so repeated calls in one db never collide) and returns the
/// `File` for direct queries like `diagnostics_for_file`.
pub fn add_source(db: &mut RootDatabase, source: &str) -> File {
    let url = Url::parse(&format!("file:///test{}.st", rand::random::<u32>())).unwrap();

    let file = File::from_string()
        .db(db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();
    file
}

pub fn add_sources(db: &mut RootDatabase, sources: &[&str]) {
    for (i, source) in sources.iter().enumerate() {
        let url = Url::parse(&format!("file:///test{i}.st")).unwrap();

        let file = File::from_string()
            .db(db)
            .parsers(&ast::RK_PARSER)
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();
    }
}

pub fn sources<Id, S, I>(iter: I) -> impl Cache<Id>
where
    Id: std::fmt::Display + std::hash::Hash + PartialEq + Eq + Clone,
    I: IntoIterator<Item = (Id, S)>,
    S: AsRef<str>,
{
    FnCache::new((move |id| Err(format!("Failed to fetch source '{id}'"))) as fn(&_) -> _)
        .with_sources(
            iter.into_iter()
                .map(|(id, s)| (id, Source::from(s)))
                .collect(),
        )
}

pub fn test_diagnostics<'db>(db: &'db mut RootDatabase, source: &'db [&'db str]) -> String {
    add_sources(db, source);
    let mut cache = vec![];

    // we need to sort the files by their URL
    let mut files = db.get_files().iter().map(|file| *file).collect::<Vec<_>>();
    files.sort_by_key(|file| {
        let url_str = file.url(db).as_str();
        // Extract number from "file:///testN.st" format
        url_str
            .strip_prefix("file:///test")
            .and_then(|s| s.strip_suffix(".st"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let file_sources = files
        .iter()
        .map(|file| (file.url(db).as_str(), file.document(db).as_str()))
        .collect::<Vec<_>>();

    for file in files {
        diagnostics_for_file(db, file).iter().for_each(|d| {
            d.create_report(
                db,
                file.url(db),
                file.document(db).as_str(),
                Some(no_color_and_ascii()),
                false,
            )
            .write(sources(file_sources.clone()), &mut cache)
            .unwrap();
        });
    }

    // Strip trailing whitespace from each line for clean inline snapshots
    String::from_utf8(cache)
        .unwrap()
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn test_snapshot<'db>(
    db: &'db mut RootDatabase,
    source: &'db [&'db str],
    diag_fn: impl Fn(&'db dyn WorkspaceDataBase, File) -> Vec<IdeDiagnostic>,
) -> String {
    add_sources(db, source);
    let mut cache = vec![];

    // we need to sort the files by their URL
    let mut files = db.get_files().iter().map(|file| *file).collect::<Vec<_>>();
    files.sort_by_key(|file| {
        let url_str = file.url(db).as_str();
        // Extract number from "file:///testN.st" format
        url_str
            .strip_prefix("file:///test")
            .and_then(|s| s.strip_suffix(".st"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let file_sources = files
        .iter()
        .map(|file| (file.url(db).as_str(), file.document(db).as_str()))
        .collect::<Vec<_>>();

    for file in files {
        diag_fn(db, file).iter().for_each(|d| {
            d.create_report(
                db,
                file.url(db),
                file.document(db).as_str(),
                Some(no_color_and_ascii()),
                false,
            )
            .write(sources(file_sources.clone()), &mut cache)
            .unwrap();
        });
    }

    // Strip trailing whitespace from each line for clean inline snapshots
    String::from_utf8(cache)
        .unwrap()
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run only a single lint rule, ignoring all others.
/// This prevents unrelated lints from polluting snapshots.
pub fn test_single_lint<'db>(
    db: &'db mut RootDatabase,
    source: &'db [&'db str],
    rule_name: &str,
) -> String {
    let mut rules = std::collections::BTreeMap::new();
    for name in linter::rules::ALL_RULE_NAMES {
        rules.insert(name.to_string(), *name == rule_name);
    }
    let linter_config = db::config_file::LinterConfig {
        select: Some(db::config_file::Select::All),
        rules: Some(rules),
    };
    test_lint_diagnostics_with_config(db, source, &linter_config)
}

fn test_lint_diagnostics_with_config<'db>(
    db: &'db mut RootDatabase,
    source: &'db [&'db str],
    linter_config: &db::config_file::LinterConfig,
) -> String {
    add_sources(db, source);
    let mut cache = vec![];

    let mut files = db.get_files().iter().map(|file| *file).collect::<Vec<_>>();
    files.sort_by_key(|file| {
        let url_str = file.url(db).as_str();
        url_str
            .strip_prefix("file:///test")
            .and_then(|s| s.strip_suffix(".st"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let file_sources = files
        .iter()
        .map(|file| (file.url(db).as_str(), file.document(db).as_str()))
        .collect::<Vec<_>>();

    for file in files {
        let mut all = diagnostics_for_file(db, file).as_ref().clone();
        linter::lint_file(db, file, linter_config, &mut all);
        all.iter().for_each(|d| {
            d.create_report(
                db,
                file.url(db),
                file.document(db).as_str(),
                Some(no_color_and_ascii()),
                false,
            )
            .write(sources(file_sources.clone()), &mut cache)
            .unwrap();
        });
    }

    // Strip trailing whitespace from each line for clean inline snapshots
    String::from_utf8(cache)
        .unwrap()
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn find_pou_with_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    name: &str,
) -> Option<Pou<'db>> {
    let sema = semantic_index(db, file);

    for pou in sema.global_pous.iter().copied() {
        if pou.get_name_ident(db).text(db).as_str() == name {
            return Some(pou);
        }
    }

    for ns in sema.namespaces.iter() {
        for pou in ns.pous(db) {
            if pou.get_name_ident(db).text(db).as_str() == name {
                return Some(*pou);
            }
        }
    }

    None
}

pub fn find_namespace_with_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    name: &str,
) -> Option<NamespaceDecl<'db>> {
    let sema = semantic_index(db, file);

    for ns in sema.namespaces.iter() {
        if ns.path(db).to_string(db) == name {
            return Some(*ns);
        }
    }

    None
}

pub fn render_references(db: &RootDatabase, refs: &[ReferenceLocation], name: &str) -> String {
    if refs.is_empty() {
        return String::new();
    }

    let mut sorted: Vec<&ReferenceLocation> = refs.iter().collect();
    sorted.sort_by(|a, b| {
        a.file
            .url(db)
            .as_str()
            .cmp(b.file.url(db).as_str())
            .then(a.span.start_byte.cmp(&b.span.start_byte))
    });

    let all_files: Vec<File> = db.get_files().iter().map(|f| *f).collect();
    let file_sources: Vec<_> = all_files
        .iter()
        .map(|f| (f.url(db).as_str(), f.document(db).as_str()))
        .collect();

    let first = sorted[0];
    let mut diag = ide_diagnostic::diag()
        .range(hir::denormalize(db, first.file, &first.span).unwrap_or_default())
        .message(format!("{} reference(s) to '{name}'", sorted.len()))
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    for r in &sorted[1..] {
        diag.with_related(Related::new("reference".to_string(), r.file, r.span));
    }

    let mut output = vec![];
    diag.create_report(
        db,
        first.file.url(db),
        first.file.document(db).as_str(),
        Some(no_color_and_ascii()),
        false,
    )
    .write(sources(file_sources), &mut output)
    .unwrap();

    // Strip trailing whitespace from each line for clean inline snapshots
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convenience wrapper for tests: resolve a POU by name string within a scope.
pub fn pou_name_res_from_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: impl HirNodeInfo<'db>,
    name: &str,
) -> Option<Pou<'db>> {
    match pou_names_res(db, Ident::from_slice(db, name), scope.get_scope_id(db)) {
        PouResolution::Found(pou, _) => Some(pou),
        _ => None,
    }
}

/// Renders diagnostics for a file (when sources are already added).
pub fn render_snapshot(db: &RootDatabase, file: File, diags: Vec<IdeDiagnostic>) -> String {
    let all_files: Vec<File> = db.get_files().iter().map(|f| *f).collect();
    let file_sources: Vec<_> = all_files
        .iter()
        .map(|f| (f.url(db).as_str(), f.document(db).as_str()))
        .collect();

    let mut cache = vec![];
    for d in &diags {
        d.create_report(
            db,
            file.url(db),
            file.document(db).as_str(),
            Some(no_color_and_ascii()),
            false,
        )
        .write(sources(file_sources.clone()), &mut cache)
        .unwrap();
    }

    String::from_utf8(cache)
        .unwrap()
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns a descriptive label for a HirNode variant.
pub fn hir_node_label(node: &HirNode) -> String {
    match node {
        HirNode::Namespace(_) => "Namespace".into(),
        HirNode::Using(_) => "Using".into(),
        HirNode::PouDecl(pou) => format!(
            "PouDecl({})",
            match pou {
                Pou::Function(_) => "Function",
                Pou::FunctionBlock(_) => "FunctionBlock",
                Pou::Class(_) => "Class",
                Pou::Interface(_) => "Interface",
                Pou::DataType(_) => "DataType",
            }
        ),
        HirNode::Program(_) => "Program".into(),
        HirNode::MethodRef(m) => format!(
            "MethodRef({})",
            match m {
                MethodRef::Declared(_) => "Declared",
                MethodRef::Prototype(_) => "Prototype",
            }
        ),
        HirNode::VariableDecl(_) => "VariableDecl".into(),
        HirNode::Spec(_) => "Spec".into(),
        HirNode::StructElement(_) => "StructElement".into(),
        HirNode::VariableAccess(_) => "VariableAccess".into(),
        HirNode::Invocation(_) => "Invocation".into(),
        HirNode::Expr(_) => "Expr".into(),
        HirNode::Param(_) => "Param".into(),
        HirNode::InitExpr(_) => "InitExpr".into(),
        HirNode::PathExpr(_) => "PathExpr".into(),
        HirNode::Config(_) => "Config".into(),
        HirNode::Resource(_) => "Resource".into(),
        HirNode::Task(_) => "Task".into(),
        HirNode::ProgConfig(_) => "ProgConfig".into(),
    }
}

/// Returns the appropriate span for a HirNode in diagnostic snapshots.
/// Uses name span for declarations (compact), full span for expressions.
pub fn hir_node_span<'db>(db: &'db dyn WorkspaceDataBase, node: &HirNode<'db>) -> Range {
    match node {
        HirNode::PouDecl(pou) => pou.get_name_span(db),
        HirNode::Program(p) => p.get_name_span(db),
        HirNode::VariableDecl(var) => var.get_name_span(db),
        HirNode::MethodRef(m) => m.get_name_span(db),
        HirNode::StructElement(st) => st.get_name_span(db),
        _ => node.get_span(db),
    }
}

/// Walks the HIR for a file and returns diagnostics labeling each visited node.
pub fn walk_hir_diagnostics(db: &dyn WorkspaceDataBase, file: File) -> Vec<IdeDiagnostic> {
    let sema = semantic_index(db, file);
    let mut nodes: Vec<(String, Range, File)> = vec![];

    let _ = sema.walk_hir(db, &mut |node: HirNode<'_>| {
        let label = hir_node_label(&node);
        let span = hir_node_span(db, &node);
        let file = node.get_scope_id(db).file(db);
        nodes.push((label, span, file));
        ControlFlow::Continue(())
    });

    if nodes.is_empty() {
        return vec![];
    }

    let (first_label, first_span, first_file) = &nodes[0];
    let mut diag = ide_diagnostic::diag()
        .range(hir::denormalize(db, *first_file, first_span).unwrap_or_default())
        .message(first_label.clone())
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    for (label, span, file) in &nodes[1..] {
        diag.with_related(Related::new(label.clone(), *file, *span));
    }

    vec![diag]
}
