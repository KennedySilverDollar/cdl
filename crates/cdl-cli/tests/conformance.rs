//! 適合テストスイート（`conformance/`）のランナー。
//!
//! いまはテストの組（`.cdl` と期待結果）と期待結果の形を検査する。
//! 実行は、構文解析を実装する段階で足す。

// 統合テストの補助関数は clippy.toml の「テストでは許す」の対象外なので、ここで許す（DECISIONS F3）。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// 1 件のテスト。
struct Case {
    name: String,
    entry: PathBuf,
    expected: PathBuf,
}

fn suite_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

/// `<章>-<名前>` の形か。
fn valid_name(name: &str) -> bool {
    let Some((chapter, rest)) = name.split_once('-') else {
        return false;
    };
    let chapter_ok =
        !chapter.is_empty() && chapter.len() <= 2 && chapter.bytes().all(|b| b.is_ascii_digit());
    let rest_ok = !rest.is_empty()
        && !rest.starts_with('-')
        && !rest.ends_with('-')
        && rest
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    chapter_ok && rest_ok
}

/// スイートを走査する。組になっていないファイルは誤りとして集める。
fn discover(root: &Path) -> (Vec<Case>, Vec<String>) {
    let mut cases = Vec::new();
    let mut errors = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in &entries {
        let file_name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if path.is_dir() {
            let entry = path.join("main.cdl");
            let expected = path.join("expected.json");
            if !entry.is_file() || !expected.is_file() {
                errors.push(format!("{file_name}/: main.cdl と expected.json が要る"));
            }
            cases.push(Case {
                name: file_name,
                entry,
                expected,
            });
        } else if let Some(stem) = file_name.strip_suffix(".cdl") {
            let expected = path.with_file_name(format!("{stem}.expected.json"));
            if !expected.is_file() {
                errors.push(format!("{file_name}: {stem}.expected.json がない"));
            }
            cases.push(Case {
                name: stem.to_owned(),
                entry: path.clone(),
                expected,
            });
        } else if let Some(stem) = file_name.strip_suffix(".expected.json") {
            if !path.with_file_name(format!("{stem}.cdl")).is_file() {
                errors.push(format!("{file_name}: {stem}.cdl がない"));
            }
        } else if file_name != "README.md" {
            errors.push(format!("{file_name}: スイートに置けないファイル"));
        }
    }
    for case in &cases {
        if !valid_name(&case.name) {
            errors.push(format!("{}: 名前は <章>-<名前> の形にする", case.name));
        }
    }
    (cases, errors)
}

/// 期待結果が `{"value": …}` か `{"diagnostics": […]}` のどちらかであることを検査する。
fn check_expected(case: &Case) -> Result<(), String> {
    let text = fs::read_to_string(&case.expected).map_err(|e| format!("{}: {e}", case.name))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", case.name))?;
    let object = value.as_object().filter(|o| o.len() == 1);
    match object.and_then(|o| o.iter().next()) {
        Some((key, _)) if key == "value" => Ok(()),
        Some((key, diags)) if key == "diagnostics" => cdl_diag::from_json(&diags.to_string())
            .map(|_| ())
            .map_err(|e| format!("{}: 診断の形が不正: {e}", case.name)),
        _ => Err(format!(
            "{}: キーは value か diagnostics の 1 つだけ",
            case.name
        )),
    }
}

#[test]
fn suite_is_well_formed() {
    let (cases, mut errors) = discover(&suite_root());
    errors.extend(
        cases
            .iter()
            .filter(|c| c.entry.is_file())
            .filter_map(|c| check_expected(c).err()),
    );
    assert!(
        errors.is_empty(),
        "適合テストスイートの誤り:\n{}",
        errors.join("\n")
    );
}

#[test]
fn names_follow_the_convention() {
    assert!(valid_name("5-switch-exhaustive"));
    assert!(valid_name("12-determinism"));
    assert!(!valid_name("switch"));
    assert!(!valid_name("5-"));
    assert!(!valid_name("5-Switch"));
    assert!(!valid_name("5--a"));
    assert!(!valid_name("123-a"));
}
