use db::{RootDatabase, WorkspaceDataBase};
use mir::function::MirLinkage;

use crate::tests::utils::{add_sources, assert_workspace_is_clean, lower_workspace};

fn fmt_ty(t: &mir::types::MirType, db: &dyn WorkspaceDataBase) -> String {
    match t {
        mir::types::MirType::Elementary(e) => format!("{:?}", e),
        mir::types::MirType::Pointer(inner) => format!("*{}", fmt_ty(inner, db)),
        mir::types::MirType::Struct(s) => format!("struct({})", s.name.text(db)),
        mir::types::MirType::Void => "void".into(),
        other => format!("{:?}", other),
    }
}

/// Lower IEC source to MIR and format every import and function as a string:
/// `export` for what the module exports, `func` for what stays internal.
pub fn mir_exports(db: &mut RootDatabase, sources: &[&str]) -> String {
    add_sources(db, sources);

    assert_workspace_is_clean(db);

    let module = lower_workspace(db);

    let mut lines = Vec::new();

    // Imports
    for ext in &module.extern_functions {
        let params: Vec<_> = ext.params.iter().map(|p| fmt_ty(&p.ty, db)).collect();
        let ret = ext
            .return_type
            .as_ref()
            .map(|t| format!(" -> {}", fmt_ty(t, db)))
            .unwrap_or_default();
        lines.push(format!(
            "import {}.{}({}){}",
            ext.module,
            ext.import_name,
            params.join(", "),
            ret
        ));
    }

    // Functions
    for func in &module.functions {
        let linkage = match func.linkage {
            MirLinkage::Export => "export",
            MirLinkage::Internal => "func",
        };
        let export_name = func
            .export_name
            .as_ref()
            .map(|s: &compact_str::CompactString| s.to_string())
            .unwrap_or_else(|| func.name.text(db).to_string());
        let params: Vec<_> = func.params.iter().map(|p| fmt_ty(&p.ty, db)).collect();
        let ret = func
            .return_type
            .as_ref()
            .map(|t| format!(" -> {}", fmt_ty(t, db)))
            .unwrap_or_default();
        lines.push(format!(
            "{linkage} {}({}){}",
            export_name,
            params.join(", "),
            ret
        ));
    }

    lines.sort();
    lines.join("\n")
}

/// Lower IEC source to MIR and format the test manifest as a string.
pub fn mir_test_manifest(db: &mut RootDatabase, sources: &[&str]) -> String {
    add_sources(db, sources);

    assert_workspace_is_clean(db);

    let module = lower_workspace(db);

    format!("{}", module.test_manifest)
}
