// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{ExprKind, ParamAssignKind, PathExprKind, PrimaryExpr},
            spec::{Spec, SpecKind},
        },
        interned::identifier::{Ident, SpanIdent},
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        body::ScopeInference,
        infer::Infer,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "name-case";

/// L0315: a name written in another case than its declaration.
struct NameCase;

impl ErrorCode for NameCase {
    fn code(&self) -> &'static str {
        "L0315"
    }
}

/// A declaration's name as written, and where it is.
#[derive(Clone, Copy)]
struct Declared {
    name: Ident,
    file: File,
    span: auto_lsp::tree_sitter::Range,
}

fn declared<'db>(db: &'db dyn WorkspaceDataBase, decl: impl HasName<'db>) -> Declared {
    let file = decl.get_scope_id(db).file(db);
    // `get_name_span` borrows the declaration for the database's life.
    let span = semantic_index(db, file)
        .ast
        .get(*decl.get_name_id(db))
        .map(|node| *node.get_range())
        .unwrap_or_else(|| decl.get_span(db));
    Declared {
        name: decl.get_name_with_case(db),
        file,
        span,
    }
}

/// The declaration a name of type `ty` names, when it names one.
fn declared_of_type<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Declared> {
    Some(match ty {
        Type::Function(f) => declared(db, f),
        Type::FunctionBlock(fb) => declared(db, fb),
        Type::Class(class) => declared(db, class),
        Type::Interface(interface) => declared(db, interface),
        Type::DataType(dt) => declared(db, dt),
        Type::StructElement(element) => declared(db, element),
        Type::MethodDecl(method) => declared(db, method),
        Type::Variable((var, _)) => declared(db, var),
        Type::CallableType(callable) | Type::ReturnValue((callable, _)) => match callable {
            CallableType::Function(f) => declared(db, f),
            CallableType::FunctionBlock(fb) => declared(db, fb),
            CallableType::MethodDecl(method) => declared(db, method),
        },
        Type::EnumVariant(dt, variant) => {
            let SpecKind::Enum(e) = dt.spec(db).kind(db) else {
                return None;
            };
            let found = e
                .variants(db)
                .into_iter()
                .find(|v| v.name.ident(db) == variant)?;
            Declared {
                name: found.name.with_case,
                file: found.name.get_scope_id(db).file(db),
                span: found.name.get_span(db),
            }
        }
        _ => return None,
    })
}

struct Names<'db> {
    db: &'db dyn WorkspaceDataBase,
    /// Where a name was compared: a step reached twice is reported once.
    seen: FxHashSet<(usize, usize)>,
    found: Vec<IdeDiagnostic>,
}

impl<'db> Names<'db> {
    /// `written` against the declaration it names.
    fn compare(&mut self, written: SpanIdent<'db>, declared: Option<Declared>) {
        let db = self.db;
        let Some(declared) = declared else {
            return;
        };
        let text = written.with_case.text(db);
        let expected = declared.name.text(db);
        // Two names the fold took apart are different names, not two cases.
        if text == expected || text.to_lowercase() != expected.to_lowercase() {
            return;
        }
        let span = written.get_span(db);
        if !self.seen.insert((span.start_byte, span.end_byte)) {
            return;
        }
        let mut d = diag()
            .message(format!("'{text}' is declared as '{expected}'"))
            .desc(&NameCase)
            .range(
                hir::denormalize(db, written.get_scope_id(db).file(db), &span).unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::HINT)
            .call();
        d.with_related(Related::new(
            format!("'{expected}' is declared here"),
            declared.file,
            declared.span,
        ));
        d.with_note("a name reads more easily in the case of its declaration".to_string());
        d.with_help(format!("write '{expected}'"));
        self.found.push(d);
    }

    /// The type names a specification writes, nested ones included.
    fn spec(&mut self, spec: Spec<'db>) {
        let db = self.db;
        match spec.kind(db) {
            SpecKind::Target(access) => {
                self.compare(access.path.target, declared_of_type(db, spec.infer(db)))
            }
            SpecKind::Array(array) => self.spec(array.of_type(db)),
            SpecKind::ArrayConformand(array) => {
                if let Some(of) = array.of_type(db) {
                    self.spec(of);
                }
            }
            SpecKind::Subrange(subrange) => self.spec(subrange._type(db)),
            SpecKind::Ref(target) => self.spec(*target),
            SpecKind::Struct(st) => {
                for element in st.elements(db) {
                    self.spec(element.spec(db));
                }
            }
            SpecKind::Enum(e) => {
                if let Some(base) = e.typ(db) {
                    self.spec(base);
                }
            }
            SpecKind::Simple(_) | SpecKind::SizedString(_) => {}
        }
    }

    fn variables(&mut self, variables: &[VariableDecl<'db>]) {
        for var in variables {
            self.spec(var.spec(self.db));
        }
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    body: Option<ScopeInference<'db>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut names = Names {
        db,
        seen: FxHashSet::default(),
        found: Vec::new(),
    };

    // The declarations: the types named, and the blocks a block extends
    // and implements.
    match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => {
            names.variables(f.variables(db));
            if let Some(ret) = f.return_type(db) {
                names.spec(*ret);
            }
        }
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => {
            names.variables(fb.variables(db));
            for spec in fb.extends(db).into_iter().chain(fb.implements(db)) {
                names.spec(*spec);
            }
        }
        ScopeKind::Pou(Pou::Class(class)) => {
            names.variables(class.variables(db));
            for spec in class.extends(db).into_iter().chain(class.implements(db)) {
                names.spec(*spec);
            }
        }
        ScopeKind::Pou(Pou::DataType(dt)) => names.spec(dt.spec(db)),
        ScopeKind::MethodDecl(m) => {
            names.variables(m.variables(db));
            if let Some(ret) = m.return_type(db) {
                names.spec(*ret);
            }
        }
        ScopeKind::Program(program) => names.variables(program.variables(db)),
        _ => {}
    }

    if let Some(body) = body {
        // Every name a body or an initializer writes in a path: a
        // variable, a member, a field, a POU called, a type.
        for (path, ty) in body.typed_path_exprs() {
            if !matches!(
                path.expr(db),
                PathExprKind::Field(_) | PathExprKind::VarAccess(_)
            ) || body.path_expr_is_namespace(path)
            {
                continue;
            }
            let target = match body.variable_for_path_expr(path) {
                Some(var) => Some(declared(db, var)),
                None => declared_of_type(db, ty),
            };
            names.compare(path.ident(db), target);
        }
        // `Color#Red`: the variant after the `#`.
        for (expr, ty) in body.typed_exprs() {
            if let ExprKind::PrimaryExpr(PrimaryExpr::EnumValue { variant, .. }) = expr.expr(db) {
                names.compare(*variant, declared_of_type(db, ty));
            }
        }
        // `t(IN := x, Q => y)`: the parameter a binding names.
        for call in body.calls() {
            for param in call.params(db) {
                if let ParamAssignKind::FormalInput { param: name, .. }
                | ParamAssignKind::FormalOutput { param: name, .. } = param.kind(db)
                    && let Some(var) = body.variable_for_param(*param)
                {
                    names.compare(name, Some(declared(db, var)));
                }
            }
        }
    }

    // In the order of the text: the inference's maps have none.
    names.found.sort_by_key(|d| {
        let start = d.diagnostic.range.start;
        (start.line, start.character)
    });
    diagnostics.extend(names.found);
}
