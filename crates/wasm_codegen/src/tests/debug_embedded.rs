//! Tests for embedded DWARF debug information in WASM binaries.

use crate::tests::{add_source, compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[cfg(feature = "debug_info")]
#[rstest]
fn test_debug_sections_embedded(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION TestFunc : INT
        VAR
            x : INT := 10;
        END_VAR
            TestFunc := x + 5;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Parse the WASM module to check for custom sections
    let parser = wasmparser::Parser::new(0);
    let mut found_debug_sections = Vec::new();

    for payload in parser.parse_all(&wasm_bytes) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload {
            let name = reader.name();
            if name.starts_with(".debug_") {
                found_debug_sections.push(name.to_string());
            }
        }
    }

    // Phase 4: We should have DWARF sections embedded with function metadata
    println!("Found debug sections: {:?}", found_debug_sections);

    // The module should be valid even with debug sections
    assert!(
        !found_debug_sections.is_empty(),
        "Expected debug sections to be present"
    );
}

#[cfg(not(feature = "debug_info"))]
#[rstest]
fn test_no_debug_sections_without_feature(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION TestFunc : INT
        VAR
            x : INT := 10;
        END_VAR
            TestFunc := x + 5;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Parse the WASM module to check for custom sections
    let parser = wasmparser::Parser::new(0);
    let mut found_debug_sections = Vec::new();

    for payload in parser.parse_all(&wasm_bytes) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload {
            let name = reader.name();
            if name.starts_with(".debug_") {
                found_debug_sections.push(name.to_string());
            }
        }
    }

    // Without the debug_info feature, no debug sections should be present
    assert!(
        found_debug_sections.is_empty(),
        "Found debug sections when feature is disabled: {:?}",
        found_debug_sections
    );
}

#[cfg(feature = "debug_info")]
#[rstest]
fn test_function_debug_info_collected(mut with_db: db::RootDatabase) {
    use hir::hir_def::semantic_index::semantic_index;

    let source = r#"
        FUNCTION Add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            Add := a + b;
        END_FUNCTION

        FUNCTION Multiply : INT
        VAR_INPUT
            x : INT;
            y : INT;
        END_VAR
            Multiply := x * y;
        END_FUNCTION
    "#;

    let file = add_source(&mut with_db, source);
    let sem_idx = semantic_index(&with_db, file);

    let mut codegen = crate::ModuleCodeGen::new(&with_db);

    // Generate code for all functions
    for pou in &sem_idx.global_pous {
        if let hir::hir_def::pous::pou::Pou::Function(func) = pou {
            codegen.generate_function(*func);
        }
    }

    // Check that debug info was collected
    let compilation_units = codegen.debug_collector.compilation_units();

    // Should have 1 compilation unit (the source file)
    assert_eq!(compilation_units.len(), 1, "Expected 1 compilation unit");

    let cu = &compilation_units[0];

    // Should have 2 functions (Add and Multiply)
    assert_eq!(cu.functions.len(), 2, "Expected 2 functions in debug info");

    // Verify function names
    let func_names: Vec<&str> = cu.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(func_names.contains(&"Add"), "Expected 'Add' function");
    assert!(func_names.contains(&"Multiply"), "Expected 'Multiply' function");

    // Verify WASM indices are set
    for func_info in &cu.functions {
        assert!(func_info.wasm_index >= 0, "WASM index should be set");
    }

    println!("✓ Function debug info collected successfully!");
    println!("  - Compilation units: {}", compilation_units.len());
    println!("  - Functions: {}", cu.functions.len());
    for func_info in &cu.functions {
        println!("    - {} (WASM index: {})", func_info.name, func_info.wasm_index);
    }
}

#[cfg(feature = "debug_info")]
#[rstest]
fn test_fb_method_debug_info_collected(mut with_db: db::RootDatabase) {
    use hir::hir_def::semantic_index::semantic_index;

    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            value : INT := 0;
        END_VAR

        METHOD Increment : INT
            THIS.value := THIS.value + 1;
            Increment := THIS.value;
        END_METHOD

        METHOD Decrement : INT
            THIS.value := THIS.value - 1;
            Decrement := THIS.value;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;

    let file = add_source(&mut with_db, source);
    let sem_idx = semantic_index(&with_db, file);

    let mut codegen = crate::ModuleCodeGen::new(&with_db);

    // Generate code for the function block
    for pou in &sem_idx.global_pous {
        if let hir::hir_def::pous::pou::Pou::FunctionBlock(fb) = pou {
            codegen.generate_function_block(*fb);
        }
    }

    // Check that debug info was collected
    let compilation_units = codegen.debug_collector.compilation_units();

    // Should have 1 compilation unit (the source file)
    assert_eq!(compilation_units.len(), 1, "Expected 1 compilation unit");

    let cu = &compilation_units[0];

    // Should have 2 methods (Increment and Decrement)
    assert_eq!(cu.functions.len(), 2, "Expected 2 methods in debug info");

    // Verify method names are qualified (Counter$Increment, Counter$Decrement)
    let func_names: Vec<&str> = cu.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        func_names.contains(&"Counter$Increment"),
        "Expected 'Counter$Increment' method"
    );
    assert!(
        func_names.contains(&"Counter$Decrement"),
        "Expected 'Counter$Decrement' method"
    );

    // Verify WASM indices are set
    for func_info in &cu.functions {
        assert!(func_info.wasm_index >= 0, "WASM index should be set");
    }

    println!("✓ FB method debug info collected successfully!");
    println!("  - Compilation units: {}", compilation_units.len());
    println!("  - Methods: {}", cu.functions.len());
    for func_info in &cu.functions {
        println!("    - {} (WASM index: {})", func_info.name, func_info.wasm_index);
    }
}

#[cfg(feature = "debug_info")]
#[rstest]
fn test_variable_debug_info_collected(mut with_db: db::RootDatabase) {
    use hir::hir_def::semantic_index::semantic_index;

    let source = r#"
        FUNCTION Calculate : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        VAR
            temp : INT;
            result : INT;
        END_VAR
            temp := a + b;
            result := temp * 2;
            Calculate := result;
        END_FUNCTION
    "#;

    let file = add_source(&mut with_db, source);
    let sem_idx = semantic_index(&with_db, file);

    let mut codegen = crate::ModuleCodeGen::new(&with_db);

    // Generate code for the function
    for pou in &sem_idx.global_pous {
        if let hir::hir_def::pous::pou::Pou::Function(func) = pou {
            codegen.generate_function(*func);
        }
    }

    // Check that debug info was collected
    let compilation_units = codegen.debug_collector.compilation_units();
    assert_eq!(compilation_units.len(), 1, "Expected 1 compilation unit");

    let cu = &compilation_units[0];
    assert_eq!(cu.functions.len(), 1, "Expected 1 function");

    let func_info = &cu.functions[0];

    // Should have 2 parameters (a, b)
    assert_eq!(func_info.parameters.len(), 2, "Expected 2 parameters");

    // Verify parameter names
    let param_names: Vec<&str> = func_info.parameters.iter().map(|p| p.name.as_str()).collect();
    assert!(param_names.contains(&"a"), "Expected parameter 'a'");
    assert!(param_names.contains(&"b"), "Expected parameter 'b'");

    // Should have 2 local variables (temp, result)
    assert_eq!(func_info.locals.len(), 2, "Expected 2 local variables");

    // Verify local variable names
    let local_names: Vec<&str> = func_info.locals.iter().map(|l| l.name.as_str()).collect();
    assert!(local_names.contains(&"temp"), "Expected local 'temp'");
    assert!(local_names.contains(&"result"), "Expected local 'result'");

    // Verify return type is INT
    assert!(func_info.return_type.is_some(), "Expected return type");
    if let Some(ref return_type) = func_info.return_type {
        match return_type {
            crate::debug_info::collector::TypeDebugInfo::Elementary { name, byte_size, .. } => {
                assert_eq!(name, "INT", "Expected INT return type");
                assert_eq!(*byte_size, 2, "INT should be 2 bytes");
            }
            _ => panic!("Expected Elementary type for return type"),
        }
    }

    println!("✓ Variable debug info collected successfully!");
    println!("  - Parameters: {}", func_info.parameters.len());
    for param in &func_info.parameters {
        println!("    - {} ({:?})", param.name, param.var_type);
    }
    println!("  - Locals: {}", func_info.locals.len());
    for local in &func_info.locals {
        println!("    - {} ({:?})", local.name, local.var_type);
    }
    println!("  - Return type: {:?}", func_info.return_type);
}

#[cfg(feature = "debug_info")]
#[rstest]
fn test_comprehensive_debug_output(mut with_db: db::RootDatabase) {
    use hir::hir_def::semantic_index::semantic_index;

    let source = r#"
        FUNCTION Calculate : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        VAR
            temp : INT;
            result : INT;
        END_VAR
            temp := a + b;
            result := temp * 2;
            Calculate := result;
        END_FUNCTION

        FUNCTION_BLOCK Counter
        VAR
            value : INT := 0;
        END_VAR

        METHOD Increment : INT
            THIS.value := THIS.value + 1;
            Increment := THIS.value;
        END_METHOD

        METHOD Decrement : INT
            THIS.value := THIS.value - 1;
            Decrement := THIS.value;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;

    let file = add_source(&mut with_db, source);
    let sem_idx = semantic_index(&with_db, file);

    let mut codegen = crate::ModuleCodeGen::new(&with_db);

    // Generate code for all POUs
    for pou in &sem_idx.global_pous {
        match pou {
            hir::hir_def::pous::pou::Pou::Function(func) => {
                codegen.generate_function(*func);
            }
            hir::hir_def::pous::pou::Pou::FunctionBlock(fb) => {
                codegen.generate_function_block(*fb);
            }
            _ => {}
        }
    }

    // Get debug info before building module
    let compilation_units = codegen.debug_collector.compilation_units();

    println!("\n=== Debug Information Collected ===\n");

    for (cu_idx, cu) in compilation_units.iter().enumerate() {
        println!("Compilation Unit {}:", cu_idx);
        println!("  File: {}", cu.file_path);
        println!("  Functions: {}", cu.functions.len());

        for func_info in &cu.functions {
            println!("\n  Function: {}", func_info.name);
            println!("    WASM Index: {}", func_info.wasm_index);

            if !func_info.parameters.is_empty() {
                println!("    Parameters ({}):", func_info.parameters.len());
                for param in &func_info.parameters {
                    println!("      - {} : {:?}", param.name, param.var_type);
                }
            }

            if !func_info.locals.is_empty() {
                println!("    Locals ({}):", func_info.locals.len());
                for local in &func_info.locals {
                    println!("      - {} : {:?}", local.name, local.var_type);
                }
            }

            if let Some(ref return_type) = func_info.return_type {
                println!("    Return Type: {:?}", return_type);
            }

            if !func_info.line_mappings.is_empty() {
                println!("    Line Mappings: {} statements tracked", func_info.line_mappings.len());
            }
        }
    }

    // Build WASM and check debug sections
    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    println!("\n=== WASM Module Analysis ===\n");
    println!("Total WASM size: {} bytes", wasm_bytes.len());

    let parser = wasmparser::Parser::new(0);
    let mut debug_sections = Vec::new();

    for payload in parser.parse_all(&wasm_bytes) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload {
            let name = reader.name();
            if name.starts_with(".debug_") {
                debug_sections.push((name.to_string(), reader.data().len()));
            }
        }
    }

    println!("Debug Sections Embedded:");
    for (name, size) in &debug_sections {
        println!("  {} : {} bytes", name, size);
    }

    println!("\n✅ Debug information successfully embedded in WASM!");

    // Assertions
    assert!(!debug_sections.is_empty(), "Expected debug sections");
    assert_eq!(compilation_units.len(), 1);
    assert_eq!(compilation_units[0].functions.len(), 3); // Calculate + 2 methods

    // Write WASM to file for debugging
    std::fs::write("example_with_debug.wasm", &wasm_bytes).unwrap();
    println!("\n📁 Written to example_with_debug.wasm for VSCode debugging");
}
