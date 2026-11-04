//! 名前解決・型検査・スキーマ適合（SPEC 第 4・6・7 章）。
#![forbid(unsafe_code)]

mod check;
pub mod graph;
pub mod schema;
pub mod types;

use std::collections::BTreeMap;

use cdl_diag::Diagnostic;

/// 構文解析。上のクレートは、このクレートを通して使う（DECISIONS H1）。
pub use cdl_syntax as syntax;

use schema::Schema;
use syntax::ast::Module;

/// ほかのファイルが import できる、ファイルの公開物（SPEC 第 9 章）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Interface {
    pub values: BTreeMap<String, ExportedValue>,
    pub types: BTreeMap<String, types::Ty>,
}

/// 公開された値。名前が大文字で始まる関数はコンポーネント。
#[derive(Clone, Debug, PartialEq)]
pub enum ExportedValue {
    Const(types::Ty),
    Function(Vec<types::Ty>, types::Ty),
}

/// 意味解析を通ったファイルについて、評価と import に要る情報。
#[derive(Clone, Debug, PartialEq)]
pub struct Checked {
    /// トップレベルの `const` を評価する順（依存先が先。DECISIONS C7）。
    pub const_order: Vec<String>,
    /// `export const policy` の型。
    pub policy_type: Option<types::Ty>,
    /// このファイルの公開物。
    pub interface: Interface,
}

/// 型がまだ分からない段階の公開物（名前解決に使う）。構文だけから作る。
pub fn interface_skeleton(module: &Module) -> Interface {
    use syntax::ast::ItemKind;
    let mut i = Interface::default();
    for item in module.items.iter().filter(|i| i.exported) {
        match &item.kind {
            ItemKind::Const(c) => {
                i.values
                    .insert(c.name.name.clone(), ExportedValue::Const(types::Ty::Error));
            }
            ItemKind::Function(f) => {
                let params = f.params.iter().map(|_| types::Ty::Error).collect();
                i.values.insert(
                    f.name.name.clone(),
                    ExportedValue::Function(params, types::Ty::Error),
                );
            }
            ItemKind::TypeAlias(t) => {
                i.types.insert(t.name.name.clone(), types::Ty::Error);
            }
            ItemKind::Interface(t) => {
                i.types.insert(t.name.name.clone(), types::Ty::Error);
            }
            ItemKind::Import(_) | ItemKind::ExportDefault(_) => {}
        }
    }
    i
}

/// ファイルの名前解決だけを行う（E2xxx）。`imports` は import の指定子 → その公開物。
/// `entry` はエントリファイルか（ライブラリのファイルの規則に使う）。
pub fn resolve_module(
    module: &Module,
    schema: &Schema,
    imports: &BTreeMap<String, Interface>,
    entry: bool,
) -> Vec<Diagnostic> {
    let mut diags = check::Checker::run(module, schema, imports, entry, true).diagnostics;
    cdl_diag::sort(&mut diags);
    diags
}

/// ファイルを検査する（名前解決・型検査・スキーマ適合）。誤りがあれば、位置順に並べた診断を返す。
pub fn check_module(
    module: &Module,
    schema: &Schema,
    imports: &BTreeMap<String, Interface>,
    entry: bool,
) -> Result<Checked, Vec<Diagnostic>> {
    let outcome = check::Checker::run(module, schema, imports, entry, false);
    if outcome.diagnostics.is_empty() {
        Ok(Checked {
            const_order: outcome.const_order,
            policy_type: outcome.policy_type,
            interface: outcome.interface,
        })
    } else {
        let mut diags = outcome.diagnostics;
        cdl_diag::sort(&mut diags);
        Err(diags)
    }
}

/// import のないエントリファイルを検査する。
pub fn check(module: &Module, schema: &Schema) -> Result<Checked, Vec<Diagnostic>> {
    check_module(module, schema, &BTreeMap::new(), true)
}
