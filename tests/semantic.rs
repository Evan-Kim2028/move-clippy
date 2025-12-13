#![cfg(feature = "full")]

use move_clippy::lint::LintSettings;
use std::fs;
use std::path::Path;

#[test]
fn semantic_lints_fire_on_fixture_package() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic_pkg");
    let tmp = tempfile::tempdir().expect("temp dir should create");

    fs::copy(fixture.join("Move.toml"), tmp.path().join("Move.toml"))
        .expect("Move.toml copy should succeed");

    let tmp_sources = tmp.path().join("sources");
    fs::create_dir_all(&tmp_sources).expect("sources dir should create");
    fs::copy(
        fixture.join("sources").join("semantic.move"),
        tmp_sources.join("semantic.move"),
    )
    .expect("source copy should succeed");

    let diags = move_clippy::semantic::lint_package(tmp.path(), &LintSettings::default())
        .expect("semantic linting should succeed");

    let mut names: Vec<&str> = diags.iter().map(|d| d.lint.name).collect();
    names.sort();
    names.dedup();

    assert!(names.contains(&"capability_naming"));
    assert!(names.contains(&"event_naming"));
    assert!(names.contains(&"getter_naming"));
}
