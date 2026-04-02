use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rk::watcher::{ChangeEvent, Watcher};

/// Create a watcher on a directory, returning (watcher, collected_events).
fn setup(dir: &Path) -> (Watcher, Arc<Mutex<Vec<ChangeEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();
    let watcher = Watcher::new(dir, move |batch| {
        events_clone.lock().unwrap().extend(batch);
    });
    // Give the OS watcher time to register
    std::thread::sleep(Duration::from_millis(50));
    (watcher, events)
}

/// Flush and wait for the debouncer to deliver events.
fn collect(watcher: &Watcher, events: &Arc<Mutex<Vec<ChangeEvent>>>) -> Vec<String> {
    watcher.flush();
    std::thread::sleep(Duration::from_millis(200));
    events
        .lock()
        .unwrap()
        .drain(..)
        .map(|e| format!("{e:?}"))
        .collect()
}

#[test]
fn new_st_file() {
    let dir = tempfile::tempdir().unwrap();
    let (watcher, events) = setup(dir.path());

    std::fs::write(dir.path().join("test.st"), "PROGRAM p END_PROGRAM").unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected
            .iter()
            .any(|e| e.contains("Created") && e.contains("test.st")),
        "expected Created event for test.st, got: {collected:?}"
    );
}

#[test]
fn changed_st_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.st");
    std::fs::write(&file, "PROGRAM p END_PROGRAM").unwrap();

    let (watcher, events) = setup(dir.path());

    std::fs::write(&file, "PROGRAM p VAR x : INT; END_VAR END_PROGRAM").unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected
            .iter()
            .any(|e| e.contains("Changed") && e.contains("main.st")),
        "expected Changed event for main.st, got: {collected:?}"
    );
}

#[test]
fn deleted_st_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("old.st");
    std::fs::write(&file, "PROGRAM p END_PROGRAM").unwrap();

    let (watcher, events) = setup(dir.path());

    std::fs::remove_file(&file).unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected
            .iter()
            .any(|e| e.contains("Deleted") && e.contains("old.st")),
        "expected Deleted event for old.st, got: {collected:?}"
    );
}

#[test]
fn renamed_st_file() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("before.st");
    std::fs::write(&old, "PROGRAM p END_PROGRAM").unwrap();

    let (watcher, events) = setup(dir.path());

    let new = dir.path().join("after.st");
    std::fs::rename(&old, &new).unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected
            .iter()
            .any(|e| e.contains("Deleted") && e.contains("before.st")),
        "expected Deleted event for before.st, got: {collected:?}"
    );
    assert!(
        collected
            .iter()
            .any(|e| e.contains("Created") && e.contains("after.st")),
        "expected Created event for after.st, got: {collected:?}"
    );
}

#[test]
fn config_toml_changed() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "[project]\nname = \"test\"\nversion = \"1\"").unwrap();

    let (watcher, events) = setup(dir.path());

    std::fs::write(&config, "[project]\nname = \"test2\"\nversion = \"1\"").unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected.iter().any(|e| e.contains("config.toml")),
        "expected event for config.toml, got: {collected:?}"
    );
}

#[test]
fn non_st_file_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (watcher, events) = setup(dir.path());

    std::fs::write(dir.path().join("readme.md"), "# hello").unwrap();
    std::fs::write(dir.path().join("data.json"), "{}").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "stuff").unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected.is_empty(),
        "expected no events for non-.st files, got: {collected:?}"
    );
}

#[test]
fn nested_st_file() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("src").join("plc");
    std::fs::create_dir_all(&sub).unwrap();

    let (watcher, events) = setup(dir.path());

    std::fs::write(
        sub.join("motor.st"),
        "FUNCTION_BLOCK Motor END_FUNCTION_BLOCK",
    )
    .unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected
            .iter()
            .any(|e| e.contains("Created") && e.contains("motor.st")),
        "expected Created event for nested motor.st, got: {collected:?}"
    );
}

#[test]
fn multiple_changes_batched() {
    let dir = tempfile::tempdir().unwrap();
    let (watcher, events) = setup(dir.path());

    for i in 0..5 {
        std::fs::write(dir.path().join(format!("f{i}.st")), "PROGRAM p END_PROGRAM").unwrap();
    }

    let collected = collect(&watcher, &events);
    let created_count = collected.iter().filter(|e| e.contains("Created")).count();
    assert!(
        created_count >= 5,
        "expected at least 5 Created events, got {created_count}: {collected:?}"
    );
}

#[test]
fn directory_created_with_st_files() {
    let dir = tempfile::tempdir().unwrap();
    let (watcher, events) = setup(dir.path());

    // Simulate creating a directory tree with .st files
    let sub = dir.path().join("new_module");
    std::fs::create_dir_all(&sub).unwrap();
    // Small delay to let the watcher register the new directory
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(sub.join("a.st"), "PROGRAM a END_PROGRAM").unwrap();
    std::fs::write(sub.join("b.st"), "PROGRAM b END_PROGRAM").unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected.iter().any(|e| e.contains("a.st")),
        "expected event for a.st, got: {collected:?}"
    );
    assert!(
        collected.iter().any(|e| e.contains("b.st")),
        "expected event for b.st, got: {collected:?}"
    );
}

#[test]
fn rapid_create_then_delete() {
    let dir = tempfile::tempdir().unwrap();
    let (watcher, events) = setup(dir.path());

    let file = dir.path().join("ephemeral.st");
    std::fs::write(&file, "PROGRAM p END_PROGRAM").unwrap();
    std::fs::remove_file(&file).unwrap();

    // Should not hang or crash, events may or may not be delivered
    let _collected = collect(&watcher, &events);
}

#[test]
fn rk_build_output_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let build_dir = dir.path().join("rk_build").join("release");
    std::fs::create_dir_all(&build_dir).unwrap();

    let (watcher, events) = setup(dir.path());

    // Simulate compile output — should NOT trigger events
    std::fs::write(build_dir.join("output.wasm"), &[0u8; 100]).unwrap();
    // Also test .st files inside rk_build (e.g. intermediate artifacts)
    std::fs::write(build_dir.join("gen.st"), "PROGRAM p END_PROGRAM").unwrap();

    let collected = collect(&watcher, &events);
    assert!(
        collected.is_empty(),
        "expected no events for rk_build/ contents, got: {collected:?}"
    );
}

#[test]
fn shutdown_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    let (mut watcher, _events) = setup(dir.path());

    watcher.stop();
}
