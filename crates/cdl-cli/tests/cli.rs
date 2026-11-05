//! `cdl` コマンドの結合テスト。リポジトリのルートで実行する。

// 統合テストの補助関数は clippy.toml の「テストでは許す」の対象外なので、ここで許す（DECISIONS F3）。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn cdl(args: &[&str]) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Command::new(env!("CARGO_BIN_EXE_cdl"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// 数を f64 にそろえる。
fn normalize(v: Value) -> Value {
    match v {
        Value::Number(n) => {
            serde_json::Number::from_f64(n.as_f64().unwrap()).map_or(Value::Null, Value::Number)
        }
        Value::Array(xs) => Value::Array(xs.into_iter().map(normalize).collect()),
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, v)| (k, normalize(v))).collect()),
        other => other,
    }
}

#[test]
fn check_accepts_the_appendix_a_example() {
    let out = cdl(&["check", "conformance/14/appendix-a.cdl"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}

#[test]
fn eval_prints_the_value_tree_of_the_appendix_a_example() {
    let out = cdl(&["eval", "conformance/14/appendix-a.cdl"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let got: Value = serde_json::from_slice(&out.stdout).unwrap();
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/14/appendix-a.expected.json");
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
    assert_eq!(normalize(got), normalize(expected["value"].clone()));
}

#[test]
fn eval_runs_the_example_with_imports() {
    let out = cdl(&["eval", "examples/contrast/main.cdl"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let got: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(got["element"], "Contrast");
    assert_eq!(got["children"].as_array().unwrap().len(), 7);
}

#[test]
fn diagnostics_are_rendered_for_humans() {
    let out = cdl(&["check", "conformance/6/missing-host-prop.cdl"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let want = "error[E4002]: `<Keybind>` に必須の props `action` がない
 --> conformance/6/missing-host-prop.cdl:7:6
  |
7 |     <Keybind keys=\"a\" />
  |      ^^^^^^^

";
    assert_eq!(text(&out.stderr), want);
}

#[test]
fn diagnostics_are_rendered_as_json() {
    let out = cdl(&["check", "conformance/4/type-mismatch.cdl", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let diags: Value = serde_json::from_slice(&out.stdout).unwrap();
    let codes: Vec<&str> = diags
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["E3001"; 4]);
    assert_eq!(diags[0]["span"]["file"], "conformance/4/type-mismatch.cdl");
    // eval でも、誤りがあれば同じ診断を出し、値木は出さない。
    let out = cdl(&["eval", "conformance/4/type-mismatch.cdl", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(serde_json::from_slice::<Value>(&out.stdout).unwrap(), diags);
}

#[test]
fn import_errors_name_the_imported_file() {
    let out = cdl(&["check", "conformance/9/import-cycle/main.cdl", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let diags: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(diags[0]["code"], "E2010");
    assert_eq!(diags[0]["span"]["file"], "conformance/9/import-cycle/a.cdl");
}

#[test]
fn missing_files_and_bad_usage() {
    let out = cdl(&["check", "no/such/file.cdl"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("E2001"), "{}", text(&out.stderr));
    for args in [
        &[][..],
        &["check"][..],
        &["run", "x.cdl"][..],
        &["check", "x.cdl", "--yaml"][..],
    ] {
        let out = cdl(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(text(&out.stderr).contains("使い方"));
    }
}

#[test]
fn stage_stops_after_the_given_stage() {
    // 型の誤り（E3xxx）は、syntax では見つからず、sema 以降で見つかる。
    let file = "conformance/4/type-mismatch.cdl";
    let out = cdl(&["check", "--stage", "syntax", file]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    for stage in ["sema", "eval"] {
        let out = cdl(&["check", file, "--stage", stage, "--json"]);
        assert_eq!(out.status.code(), Some(1), "{stage}");
        let diags: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(diags[0]["code"].as_str().unwrap().starts_with("E3"));
    }
    // 評価の誤り（E5xxx）は、sema では見つからず、eval で見つかる。
    let file = "conformance/7/divide-by-zero.cdl";
    let out = cdl(&["check", "--stage", "sema", file]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = cdl(&["check", "--stage", "eval", file, "--json"]);
    assert_eq!(out.status.code(), Some(1));
    // 構文の誤りは、どの段階でも見つかる。
    let out = cdl(&[
        "check",
        "--stage",
        "syntax",
        "conformance/forbidden/let.cdl",
    ]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn preset_accepts_only_contrast() {
    let file = "conformance/14/appendix-a.cdl";
    let out = cdl(&["check", "--preset", "contrast", file]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        cdl(&["check", "--preset", "other", file]).status.code(),
        Some(2)
    );
    assert_eq!(cdl(&["check", "--preset", file]).status.code(), Some(2));
}

#[test]
fn bad_usage_exits_with_2() {
    let file = "conformance/14/appendix-a.cdl";
    assert_eq!(
        cdl(&["check", "--stage", "nope", file]).status.code(),
        Some(2)
    );
    assert_eq!(cdl(&["check", "--stage"]).status.code(), Some(2));
    // --stage は check だけ。
    assert_eq!(
        cdl(&["eval", "--stage", "sema", file]).status.code(),
        Some(2)
    );
    assert_eq!(cdl(&["check", "--unknown", file]).status.code(), Some(2));
}
