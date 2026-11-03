//! リポジトリ構成の規則（CLAUDE.md・DECISIONS H1）を、`cargo metadata` の出力で検査する。

// 統合テストの補助関数は clippy.toml の「テストでは許す」の対象外なので、ここで許す（DECISIONS F3）。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// 各クレートが依存してよいワークスペース内のクレート。
fn allowed_internal(krate: &str) -> &'static [&'static str] {
    match krate {
        "cdl-diag" => &[],
        "cdl-syntax" => &["cdl-diag"],
        "cdl-sema" => &["cdl-diag", "cdl-syntax"],
        "cdl-eval" => &["cdl-diag", "cdl-sema"],
        "cdl-host" => &["cdl-diag", "cdl-eval"],
        "cdl-cli" => &["cdl-diag", "cdl-host"],
        other => panic!("想定していないクレート: {other}"),
    }
}

/// 外部クレートを使ってよいかどうか（CLAUDE.md の許可リスト）。
fn external_allowed(krate: &str, dep: &str, dev: bool) -> bool {
    match dep {
        "serde" | "serde_json" => matches!(krate, "cdl-diag" | "cdl-cli"),
        "proptest" => dev,
        _ if dep.starts_with("oxc") => krate == "cdl-syntax",
        _ => false,
    }
}

/// クレート名 → [(依存先, dev 依存か)]。
fn workspace_dependencies() -> BTreeMap<String, Vec<(String, bool)>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| {
            let deps = package["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| {
                    (
                        d["name"].as_str().unwrap().to_owned(),
                        d["kind"].as_str() == Some("dev"),
                    )
                })
                .collect();
            (package["name"].as_str().unwrap().to_owned(), deps)
        })
        .collect()
}

#[test]
fn workspace_has_exactly_the_six_crates() {
    let names: Vec<String> = workspace_dependencies().into_keys().collect();
    assert_eq!(
        names,
        [
            "cdl-cli",
            "cdl-diag",
            "cdl-eval",
            "cdl-host",
            "cdl-sema",
            "cdl-syntax"
        ]
    );
}

#[test]
fn internal_dependencies_follow_the_layering() {
    for (krate, deps) in workspace_dependencies() {
        for (dep, _) in deps.iter().filter(|(d, _)| d.starts_with("cdl-")) {
            assert!(
                allowed_internal(&krate).contains(&dep.as_str()),
                "{krate} → {dep} は依存の向きに反する"
            );
        }
    }
}

#[test]
fn external_dependencies_are_on_the_allow_list() {
    for (krate, deps) in workspace_dependencies() {
        for (dep, dev) in deps.iter().filter(|(d, _)| !d.starts_with("cdl-")) {
            assert!(
                external_allowed(&krate, dep, *dev),
                "{krate} → {dep} は許可されていない依存"
            );
        }
    }
}
