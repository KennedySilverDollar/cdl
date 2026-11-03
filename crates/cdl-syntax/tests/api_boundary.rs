//! `oxc_*` の型を公開 API に出さない（CLAUDE.md「プロトタイプの段階」）。

// 統合テストの補助関数は clippy.toml の「テストでは許す」の対象外なので、ここで許す（DECISIONS F3）。
#![allow(clippy::unwrap_used, clippy::panic)]

use std::fs;
use std::path::Path;

/// `pub`（`pub(crate)` などを除く）の行と、その宣言が続く行に `oxc` が現れないこと。
#[test]
fn public_items_do_not_mention_oxc() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let mut in_pub_signature = false;
        for (n, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("pub ") {
                in_pub_signature = true;
            }
            if in_pub_signature && line.contains("oxc") {
                offenders.push(format!("{}:{}: {line}", path.display(), n + 1));
            }
            if in_pub_signature && (t.ends_with('{') || t.ends_with(';') || t.ends_with(',')) {
                in_pub_signature = false;
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}
