//! 名前解決・型検査・スキーマ適合（SPEC 第 4・6・7 章）。
#![forbid(unsafe_code)]

mod check;
mod graph;
pub mod schema;
pub mod types;

use cdl_diag::Diagnostic;

/// 構文解析。上のクレートは、このクレートを通して使う（DECISIONS H1）。
pub use cdl_syntax as syntax;

use schema::Schema;
use syntax::ast::Module;

/// 意味解析を通ったファイルについて、評価に要る情報。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// トップレベルの `const` を評価する順（依存先が先。DECISIONS C7）。
    pub const_order: Vec<String>,
}

/// エントリファイルを検査する。誤りがあれば、位置順に並べた診断を返す。
pub fn check(module: &Module, schema: &Schema) -> Result<Checked, Vec<Diagnostic>> {
    let outcome = check::Checker::run(module, schema);
    if outcome.diagnostics.is_empty() {
        Ok(Checked {
            const_order: outcome.const_order,
        })
    } else {
        let mut diags = outcome.diagnostics;
        cdl_diag::sort(&mut diags);
        Err(diags)
    }
}
