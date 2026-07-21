//! Spike test: verify WASM exception handling works with wasm-encoder + wasmtime.

#[test]
fn test_wasm_exceptions_throw_and_catch() {
    use wasm_encoder::*;

    // Build a module with:
    // - Tag 0: assert_failure(i32) — carries an assertion ID
    // - Function "assert_true": throws tag 0 if input == 0
    let mut module = Module::new();

    // Type 0: (i32) -> () — tag signature (assert payload)
    // Type 1: (i32) -> () — assert_true function signature
    let mut types = TypeSection::new();
    types.ty().function(vec![ValType::I32], vec![]); // type 0: tag sig
    types.ty().function(vec![ValType::I32], vec![]); // type 1: assert_true
    module.section(&types);

    // Function section: function 0 uses type 1
    let mut functions = FunctionSection::new();
    functions.function(1);
    module.section(&functions);

    // Memory (minimal)
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory);

    // Tag section: tag 0 uses type 0
    // WASM section order: Type, Import, Function, Table, Memory, Tag, Global, Export, ...
    let mut tags = TagSection::new();
    tags.tag(TagType {
        kind: TagKind::Exception,
        func_type_idx: 0,
    });
    module.section(&tags);

    // Export: "assert_true" and tag "assert_failure"
    let mut exports = ExportSection::new();
    exports.export("assert_true", ExportKind::Func, 0);
    exports.export("assert_failure", ExportKind::Tag, 0);
    exports.export("memory", ExportKind::Memory, 0);
    module.section(&exports);

    // Code for assert_true(condition: i32):
    //   if condition == 0:
    //     throw tag 0 with value 1 (assertion ID)
    let mut code = CodeSection::new();
    let mut f = Function::new(vec![]);
    // if (condition == 0)
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::If(BlockType::Empty));
    // throw tag 0 with assertion_id = 1
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::Throw(0));
    f.instruction(&Instruction::End); // end if
    f.instruction(&Instruction::End); // end function
    code.function(&f);
    module.section(&code);

    let wasm_bytes = module.finish();

    // Now instantiate with wasmtime (exceptions enabled)
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).expect("valid WASM");
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let assert_true = instance
        .get_typed_func::<i32, ()>(&mut store, "assert_true")
        .unwrap();

    // Passing true (1) should succeed
    assert!(assert_true.call(&mut store, 1).is_ok());

    // Passing false (0) should throw
    let result = assert_true.call(&mut store, 0);
    assert!(result.is_err(), "Expected assertion failure");

    // Verify the error message mentions an exception/tag
    let err = result.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("exception") || err_str.contains("wasm trap") || err_str.contains("tag"),
        "Error should mention exception handling: {}",
        err_str
    );
}
