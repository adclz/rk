// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A POU nothing the application runs reaches. The application is what the
//! configuration runs, with the FUNCTIONs it exports and the tests: from
//! those, every call, the type of every variable, and what a block extends
//! and implements. A workspace with no configuration is a library, and a
//! library offers its POUs to others: nothing is reported there.

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{
    HasName, HasPragmas, HirNodeInfo,
    hir_def::{
        config::ConfigDecl,
        expressions::spec::{Spec, SpecKind},
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        index_graphs::declared_configs,
        infer::Infer,
        oop::MethodRef,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "unused-pou";

/// L0216: a POU nothing the application runs reaches.
struct UnusedPou;

impl ErrorCode for UnusedPou {
    fn code(&self) -> &'static str {
        "L0216"
    }
}

/// The POUs a file declares, its namespaces' included: FUNCTIONs,
/// FUNCTION_BLOCKs, CLASSes and PROGRAMs, with their scopes.
fn declared<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> Vec<(ScopeId<'db>, Pou<'db>)> {
    let sema = semantic_index(db, file);
    let mut out = Vec::new();
    let pous = sema.global_pous.iter().copied().chain(
        sema.namespaces
            .iter()
            .flat_map(|ns| ns.pous(db).iter().copied()),
    );
    for pou in pous {
        if matches!(
            pou,
            Pou::Function(_) | Pou::FunctionBlock(_) | Pou::Class(_)
        ) {
            out.push((pou.get_scope_id(db), pou));
        }
    }
    out
}

pub fn check(db: &dyn WorkspaceDataBase, file: File, diagnostics: &mut Vec<IdeDiagnostic>) {
    // A library's POUs are its users'.
    if db.library_of(file).is_some() {
        return;
    }
    let configs = declared_configs(db);
    if configs.is_empty() {
        return;
    }
    let reached = reached(db, &configs);
    let sema = semantic_index(db, file);
    for (scope, pou) in declared(db, file) {
        if !reached.contains(&scope) {
            report(
                db,
                file,
                pou.get_name_with_case(db).text(db),
                pou.get_name_span(db),
                diagnostics,
            );
        }
    }
    for program in sema.programs.iter() {
        if !reached.contains(&program.scope_id(db)) {
            report(
                db,
                file,
                program.get_name_with_case(db).text(db),
                program.get_name_span(db),
                diagnostics,
            );
        }
    }
}

fn report(
    db: &dyn WorkspaceDataBase,
    file: File,
    name: &str,
    span: auto_lsp::tree_sitter::Range,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut d = diag()
        .message(format!("'{name}' is never used"))
        .desc(&UnusedPou)
        .range(hir::denormalize(db, file, &span).unwrap_or_default())
        .severity(DiagnosticSeverity::INFORMATION)
        .tags(vec![DiagnosticTag::UNNECESSARY])
        .call();
    d.with_note(
        "no program, {export} FUNCTION or {test} uses it, directly or through others".to_string(),
    );
    d.with_help(format!("remove '{name}'"));
    diagnostics.push(d);
}

/// Every scope the application reaches.
fn reached<'db>(
    db: &'db dyn WorkspaceDataBase,
    configs: &[ConfigDecl<'db>],
) -> FxHashSet<ScopeId<'db>> {
    let mut queue: Vec<ScopeId<'db>> = configs.iter().map(|c| c.scope_id(db)).collect();
    for file in db.get_files().iter().map(|entry| *entry) {
        for (scope, pou) in declared(db, file) {
            if let Pou::Function(f) = pou
                && (f.is_export(db) || f.is_test(db))
            {
                queue.push(scope);
            }
        }
    }
    let mut reached = FxHashSet::default();
    while let Some(scope) = queue.pop() {
        if !reached.insert(scope) {
            continue;
        }
        // A library reaches none of the workspace's POUs.
        if db.library_of(scope.file(db)).is_some() {
            continue;
        }
        walk(db, scope, &mut queue);
    }
    reached
}

/// What `scope` reaches: the types it declares and names, the blocks it
/// extends and implements, its methods and what it calls.
fn walk<'db>(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>, queue: &mut Vec<ScopeId<'db>>) {
    let mut specs: Vec<Spec<'db>> = Vec::new();
    let variables = |list: &[VariableDecl<'db>], specs: &mut Vec<Spec<'db>>| {
        specs.extend(list.iter().map(|var| var.spec(db)));
    };
    let mut has_body = true;
    match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => {
            variables(f.variables(db), &mut specs);
            specs.extend(f.return_type(db).iter().copied());
        }
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => {
            variables(fb.variables(db), &mut specs);
            specs.extend(fb.extends(db).into_iter().copied());
            specs.extend(fb.implements(db).iter().copied());
            queue.extend(fb.methods(db).iter().map(|m| m.get_scope_id(db)));
        }
        ScopeKind::Pou(Pou::Class(class)) => {
            variables(class.variables(db), &mut specs);
            specs.extend(class.extends(db).into_iter().copied());
            specs.extend(class.implements(db).iter().copied());
            queue.extend(class.methods(db).iter().map(|m| m.get_scope_id(db)));
        }
        ScopeKind::Pou(Pou::DataType(dt)) => {
            specs.push(dt.spec(db));
            has_body = false;
        }
        ScopeKind::MethodDecl(m) => {
            variables(m.variables(db), &mut specs);
            specs.extend(m.return_type(db).iter().copied());
        }
        ScopeKind::Program(program) => variables(program.variables(db), &mut specs),
        ScopeKind::Config(config) => {
            variables(config.variables(db), &mut specs);
            for resource in config.resources(db) {
                specs.extend(resource.programs(db).iter().map(|p| p.prog_type(db)));
            }
        }
        _ => has_body = false,
    }
    while let Some(spec) = specs.pop() {
        match spec.kind(db) {
            SpecKind::Target(_) => {
                if let Some(target) = type_scope(db, spec.infer(db)) {
                    queue.push(target);
                }
            }
            SpecKind::Array(array) => specs.push(array.of_type(db)),
            SpecKind::ArrayConformand(array) => specs.extend(array.of_type(db)),
            SpecKind::Subrange(subrange) => specs.push(subrange._type(db)),
            SpecKind::Ref(target) => specs.push(*target),
            SpecKind::Struct(st) => specs.extend(st.elements(db).iter().map(|e| e.spec(db))),
            SpecKind::Enum(_) | SpecKind::Simple(_) | SpecKind::SizedString(_) => {}
        }
    }
    if has_body {
        for (_, resolved) in scope.inference(db).resolved_calls() {
            let callee = match resolved.callable {
                CallableType::Function(f) => Some(f.get_scope_id(db)),
                CallableType::FunctionBlock(fb) => Some(fb.get_scope_id(db)),
                CallableType::MethodDecl(MethodRef::Declared(m)) => Some(m.get_scope_id(db)),
                CallableType::MethodDecl(MethodRef::Prototype(_)) => None,
            };
            queue.extend(callee);
        }
    }
}

/// The scope of the declaration a type names.
fn type_scope<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<ScopeId<'db>> {
    Some(match ty {
        Type::Function(f) => f.get_scope_id(db),
        Type::FunctionBlock(fb) => fb.get_scope_id(db),
        Type::Class(class) => class.get_scope_id(db),
        Type::Interface(interface) => interface.get_scope_id(db),
        Type::DataType(dt) => dt.get_scope_id(db),
        Type::Program(program) => program.scope_id(db),
        _ => return None,
    })
}
