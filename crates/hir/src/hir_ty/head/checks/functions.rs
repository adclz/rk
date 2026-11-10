// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::{
    CallSite, HasPragmas, Visibility,
    check::errors::{ToIdeDiagnostic, e10_visibility::VisibilityError},
    hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope},
    hir_ty::{head::init_inference::InitInference, infer::Infer},
};

impl<'db> InitInference<'db> {
    /// E1006: a FUNCTION header may carry PRIVATE (or PUBLIC, the default
    /// made explicit); PROTECTED and INTERNAL mean nothing there.
    pub(crate) fn check_function_specifier(&mut self, db: &'db dyn WorkspaceDataBase) {
        let ScopeKind::Pou(Pou::Function(func)) = get_scope(db, self.scope).kind else {
            return;
        };
        let visibility = func.visibility(db);
        let Some(spec_id) = func.spec_id(db) else {
            return;
        };
        self.errors.push(
            VisibilityError::SpecifierNotOnFunction {
                site: CallSite::new(func.scope_id(db), spec_id),
                keyword: if visibility.contains(Visibility::PROTECTED) {
                    "PROTECTED"
                } else if visibility.contains(Visibility::INTERNAL) {
                    "INTERNAL"
                } else {
                    return;
                },
            }
            .to_diagnostic(db, self.scope.file(db)),
        );
    }

    /// The import an `{extern}` FUNCTION is, as a component names and types
    /// it: its module a WIT name or an interface, its name a WIT name
    /// (E1512), and for a module other than WASI's, a WIT name for each
    /// parameter (E1512) and 16 values at most (E1502). WASI's functions
    /// keep the types WASI's WIT gives them.
    pub(crate) fn check_extern_import(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::check::errors::e15_pragma::{ExternForbiddenKind, PragmaError};
        use crate::hir_def::expressions::spec::ElementarySpec;
        use crate::hir_def::pous::pragma::{import_module, is_wit_name, wit_parameter_name};
        use crate::hir_ty::ty::Type;
        let ScopeKind::Pou(Pou::Function(func)) = get_scope(db, self.scope).kind else {
            return;
        };
        let Some((_, pragma)) = func.extern_pragma(db) else {
            return;
        };
        let file = self.scope.file(db);
        if let Err(why) = import_module(&pragma.module) {
            self.errors.push(
                PragmaError::ExternModuleName {
                    module: pragma.module.clone(),
                    span: pragma.module_span,
                    why,
                }
                .to_diagnostic(db, file),
            );
        }
        if !is_wit_name(&pragma.name) {
            self.errors.push(
                PragmaError::ExternFunctionName {
                    name: pragma.name.clone(),
                    span: pragma.name_span,
                }
                .to_diagnostic(db, file),
            );
        }
        if pragma.is_wasi() {
            return;
        }
        // By their WIT names without `-`: what the component model compares.
        let mut named = rustc_hash::FxHashMap::default();
        let mut values = 0;
        for var in func.variables(db).iter().filter(|var| var.is_input(db)) {
            match wit_parameter_name(var.name(db).text(db)) {
                None => self
                    .errors
                    .push(PragmaError::ExternParameterName { var: *var }.to_diagnostic(db, file)),
                Some(wit) => match named.entry(wit.replace('-', "")) {
                    std::collections::hash_map::Entry::Occupied(first) => self.errors.push(
                        PragmaError::ExternParameterNamesCollide {
                            var: *var,
                            first: *first.get(),
                        }
                        .to_diagnostic(db, file),
                    ),
                    std::collections::hash_map::Entry::Vacant(slot) => {
                        slot.insert(*var);
                    }
                },
            }
            // What the declaration refuses already is no value of the import.
            let taken = match var.spec(db).infer(db).normalize(db) {
                Type::Elementary(ElementarySpec::String) => 2,
                Type::Elementary(_) | Type::Enum(_) | Type::SubRange(_) => 1,
                _ => 0,
            };
            if taken > 0 && values <= 16 && values + taken > 16 {
                self.errors.push(
                    PragmaError::ExternForbiddenSection {
                        var: *var,
                        kind: ExternForbiddenKind::TooManyValues,
                    }
                    .to_diagnostic(db, file),
                );
            }
            values += taken;
        }
    }

    /// The export an `{export}` FUNCTION is, as a component names and types
    /// it: a WIT name of its own that no other export has (E1513), a WIT
    /// name for each parameter (E1513), scalar and STRING inputs worth 16
    /// values at most, no output and no in-out, and a scalar or a STRING
    /// returned (E1509). What has no single export to give is refused
    /// where the pragma's place is checked (`check_export`).
    pub(crate) fn check_export_interface(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::check::errors::e15_pragma::{ExportForbiddenSection, PragmaError};
        use crate::hir_def::expressions::spec::ElementarySpec;
        use crate::hir_def::pous::pragma::wit_parameter_name;
        use crate::hir_def::pous::variable::VariableKind;
        use crate::hir_ty::exports::{ExportNameError, export_wit_name, wit_key};
        use crate::hir_ty::ty::Type;
        use crate::{HasName, HirNodeInfo};
        let ScopeKind::Pou(Pou::Function(func)) = get_scope(db, self.scope).kind else {
            return;
        };
        if !func.is_export(db) || func.extern_pragma(db).is_some() || func.is_test(db) {
            return;
        }
        let file = self.scope.file(db);
        match export_wit_name(db, func) {
            Err(ExportNameError::Given(name, span)) => self
                .errors
                .push(PragmaError::ExportNameNotWit { name, span }.to_diagnostic(db, file)),
            Err(ExportNameError::Derived(word)) => self
                .errors
                .push(PragmaError::ExportWithoutWitName { func, word }.to_diagnostic(db, file)),
            Ok(name) => {
                // Reported at each of them, with the smallest of the others
                // by file and place: the same on every run.
                let other = crate::hir_ty::index_graphs::exports_named(db, &wit_key(&name))
                    .into_iter()
                    .filter(|other| *other != func)
                    .min_by_key(|other| {
                        (
                            other.get_scope_id(db).file(db).url(db).to_string(),
                            other.get_name_span(db).start_byte,
                        )
                    });
                if let Some(other) = other {
                    self.errors.push(
                        PragmaError::ExportNamesCollide { func, other, name }
                            .to_diagnostic(db, file),
                    );
                }
            }
        }

        // By their WIT names without `-`: what the component model compares.
        let mut named = rustc_hash::FxHashMap::default();
        let mut values = 0;
        for var in func.variables(db) {
            let refuse = |kind| {
                PragmaError::ExportForbiddenSection { var: *var, kind }.to_diagnostic(db, file)
            };
            match var.kind(db) {
                VariableKind::Output => self.errors.push(refuse(ExportForbiddenSection::Output)),
                // An interface is refused with the pragma: one copy of the
                // FUNCTION per implementation.
                VariableKind::InOut
                    if matches!(var.spec(db).infer(db).normalize(db), Type::Interface(_)) => {}
                VariableKind::InOut => self.errors.push(refuse(ExportForbiddenSection::InOut)),
                VariableKind::Input => {
                    // What has no single export is refused with the pragma.
                    if var.variadic(db) || var.conformand(db).is_some() {
                        continue;
                    }
                    let taken = match var.spec(db).infer(db).normalize(db) {
                        Type::Elementary(ElementarySpec::String) => 2,
                        Type::Elementary(_) | Type::Enum(_) | Type::SubRange(_) => 1,
                        Type::Interface(_) | Type::Never => continue,
                        Type::RefTo(_) => {
                            self.errors.push(refuse(ExportForbiddenSection::Reference));
                            continue;
                        }
                        _ => {
                            self.errors
                                .push(refuse(ExportForbiddenSection::AggregateInput));
                            continue;
                        }
                    };
                    match wit_parameter_name(var.name(db).text(db)) {
                        None => self.errors.push(
                            PragmaError::ExportParameterName { var: *var }.to_diagnostic(db, file),
                        ),
                        Some(wit) => match named.entry(wit_key(&wit)) {
                            std::collections::hash_map::Entry::Occupied(first) => self.errors.push(
                                PragmaError::ExportParameterNamesCollide {
                                    var: *var,
                                    first: *first.get(),
                                }
                                .to_diagnostic(db, file),
                            ),
                            std::collections::hash_map::Entry::Vacant(slot) => {
                                slot.insert(*var);
                            }
                        },
                    }
                    if values <= 16 && values + taken > 16 {
                        self.errors
                            .push(refuse(ExportForbiddenSection::TooManyValues));
                    }
                    values += taken;
                }
                _ => {}
            }
        }

        if let Some(ret) = func.return_type(db)
            && !matches!(
                ret.infer(db).normalize(db),
                Type::Elementary(_) | Type::Enum(_) | Type::SubRange(_) | Type::Never
            )
        {
            self.errors
                .push(PragmaError::ExportReturn { func, ret: *ret }.to_diagnostic(db, file));
        }
    }

    /// What a FUNCTION or METHOD returns is made afresh by each call, so it
    /// cannot hold a variable declared `AT %I*`, whose pointer nothing would
    /// bind (E1425).
    pub(crate) fn check_return_type(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::check::errors::e14_config::{ConfigError, PartlyUnlocated};
        let callable = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(Pou::Function(_)) => "FUNCTION",
            ScopeKind::MethodDecl(_) | ScopeKind::MethodProt(_) => "METHOD",
            _ => return,
        };
        let Some(ret) = self.scope.return_type(db) else {
            return;
        };
        let returned = ret.infer(db);
        let element = returned.element_under_arrays(db);
        let held = match crate::hir_ty::head::instances::pou_of_type(db, element) {
            Some(pou) => crate::hir_ty::head::instances::partly_located_members(db, pou)
                .first()
                .and_then(|path| {
                    Some((
                        path.iter()
                            .map(|m| m.name_with_case(db))
                            .collect::<Vec<_>>(),
                        *path.last()?,
                    ))
                }),
            None => crate::hir_ty::head::instances::partly_located_in_struct(
                db,
                element,
                &mut Vec::new(),
            ),
        };
        let Some((names, member)) = held else {
            return;
        };
        self.errors.push(
            ConfigError::PartlyLocatedUnlocated(PartlyUnlocated::Returned {
                ret: *ret,
                callable,
                ty: returned,
                member: names,
                address: compact_str::CompactString::from(
                    member
                        .location(db)
                        .map(|dv| dv.to_address(db))
                        .unwrap_or_default(),
                ),
            })
            .to_diagnostic(db, self.scope.file(db)),
        );
    }
}
