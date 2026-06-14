use yansi::Paint;

use crate::compiler::build_core;
use crate::workspace::init_db;

pub fn run_test(
    workspace: &std::path::Path,
    no_stdlib: bool,
    filter: Option<&str>,
    _opt_level: Option<&str>,
    verbose: bool,
) {
    let Some(db) = init_db(workspace, verbose, !no_stdlib) else {
        std::process::exit(1);
    };

    // Test profile: core module → component (no optimization)
    let Some((core_bytes, mir_module)) = build_core(&db, workspace, verbose) else {
        std::process::exit(1);
    };
    let component_bytes = wasm_codegen::component::wrap_in_component(&db, &core_bytes, &mir_module)
        .unwrap_or_else(|e| {
            eprintln!("{}{}", "component error: ".bold().red(), e);
            std::process::exit(1);
        });

    // Write component to rk_build/test/
    let build_dir = workspace.join("rk_build").join("test");
    std::fs::create_dir_all(&build_dir).ok();
    let wasm_path = build_dir.join("output.wasm");
    std::fs::write(&wasm_path, &component_bytes).unwrap_or_else(|e| {
        eprintln!(
            "{}failed to write test binary: {}",
            "error: ".bold().red(),
            e
        );
        std::process::exit(1);
    });

    // The test manifest is embedded as a custom section inside the component
    // itself (see `wrap_in_component`), so there is no sidecar file to write.
    let failures = crate::test_runner::run_tests(&wasm_path, filter);
    if failures > 0 {
        std::process::exit(1);
    }
}
