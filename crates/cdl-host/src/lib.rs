//! ホストに組み込むための公開 API。互換性を約束するのはこのクレートだけ。
//!
//! エントリファイルを読み、構文解析・意味解析・評価を行って [`Program`] を返す。
#![forbid(unsafe_code)]

use cdl_diag::{Code, Diagnostic, Span};
use cdl_eval::sema;
use cdl_eval::sema::syntax;
use cdl_eval::{Evaluated, sema::Checked, sema::syntax::ast::Module};

pub use cdl_diag as diag;
pub use cdl_eval::{Element, Value};
pub use sema::schema::{ComponentDef, Schema, SchemaBuilder};
pub use sema::types::{Field, Literal, Ty};

/// リゾルバが返すソース。`id` は正準な名前で、同じファイルなら同じ `id` を返す（DECISIONS E6）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    pub text: String,
}

/// ソースを読む唯一の経路（SPEC 第 9 章）。ファイルシステムなどの実装はホストが用意する。
pub trait ModuleResolver {
    /// `importer` は読み込み元の `id`。エントリを解決するときは `None`。
    /// 見つからなければ、利用者向けの理由を返す。
    fn resolve(&self, importer: Option<&str>, specifier: &str) -> Result<Source, String>;
}

/// リソース上限（SPEC 第 7 章）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// 1 ファイルのソースサイズ（バイト）。
    pub max_source_bytes: u32,
    /// 構文の入れ子の深さ。
    pub max_nesting: u32,
    /// Document 全体の評価ステップ。
    pub max_steps: u64,
    /// 関数・コンポーネント・コールバックの呼び出しの深さ。
    pub max_call_depth: u32,
    /// 1 つの文字列の長さ（UTF-8 のバイト数）。
    pub max_string_bytes: u32,
    /// 1 つの配列・マップの要素数。
    pub max_collection_len: u32,
    /// 評価結果の値木のノード数。
    pub max_nodes: u64,
    /// フックの呼び出し 1 回あたりの評価ステップ。
    pub max_hook_steps: u64,
}

impl Default for Limits {
    fn default() -> Self {
        let syntax = syntax::Limits::default();
        let eval = cdl_eval::Limits::default();
        Limits {
            max_source_bytes: syntax.max_source_bytes,
            max_nesting: syntax.max_nesting,
            max_steps: eval.max_steps,
            max_call_depth: eval.max_call_depth,
            max_string_bytes: eval.max_string_bytes,
            max_collection_len: eval.max_collection_len,
            max_nodes: eval.max_nodes,
            max_hook_steps: eval.max_hook_steps,
        }
    }
}

impl Limits {
    fn eval(&self) -> cdl_eval::Limits {
        cdl_eval::Limits {
            max_steps: self.max_steps,
            max_call_depth: self.max_call_depth,
            max_string_bytes: self.max_string_bytes,
            max_collection_len: self.max_collection_len,
            max_nodes: self.max_nodes,
            max_hook_steps: self.max_hook_steps,
        }
    }
}

/// 評価を終えたプログラム。要素ツリーとポリシーを 1 つの不変な値として持つ（DECISIONS E5）。
#[derive(Debug)]
pub struct Program {
    module: Module,
    checked: Checked,
    evaluated: Evaluated,
    schema: Schema,
    limits: cdl_eval::Limits,
}

impl Program {
    /// 要素ツリー（ルート要素）。
    pub fn document(&self) -> &Element {
        &self.evaluated.document
    }

    /// ポリシーのフックを呼ぶ（SPEC 第 8 章）。フックがなければ `Ok(None)`。
    /// 失敗したら診断を返す。ホストはそのフックの既定の動作に戻す。
    pub fn call_hook(&self, hook: &str, arg: Value) -> Result<Option<Value>, Diagnostic> {
        cdl_eval::call_hook(
            &self.module,
            &self.checked,
            &self.schema,
            &self.evaluated,
            &self.limits,
            hook,
            arg,
        )
    }
}

/// CDL の処理系。
#[derive(Clone, Debug, Default)]
pub struct Engine {
    limits: Limits,
    schema: Schema,
}

impl Engine {
    pub fn new() -> Self {
        Engine::default()
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// ホストのスキーマ（組み込みコンポーネントとルート要素）。
    pub fn with_schema(mut self, schema: Schema) -> Self {
        self.schema = schema;
        self
    }

    /// エントリファイルを検査し、評価する。誤りがあれば、位置順に並べた診断を返す。
    pub fn check(&self, resolver: &dyn ModuleResolver, entry: &str) -> Result<(), Vec<Diagnostic>> {
        self.load(resolver, entry).map(|_| ())
    }

    /// エントリファイルを読み、評価したプログラムを返す。
    pub fn load(
        &self,
        resolver: &dyn ModuleResolver,
        entry: &str,
    ) -> Result<Program, Vec<Diagnostic>> {
        let source = resolver.resolve(None, entry).map_err(|reason| {
            let span = Span {
                file: entry.to_owned(),
                start: 0,
                end: 0,
            };
            vec![Diagnostic::error(
                Code::ModuleNotFound,
                span,
                format!("`{entry}` を読めない: {reason}"),
            )]
        })?;
        let limits = syntax::Limits {
            max_source_bytes: self.limits.max_source_bytes,
            max_nesting: self.limits.max_nesting,
        };
        let module = syntax::parse(&source.id, &source.text, &limits)?;
        let checked = sema::check(&module, &self.schema)?;
        let eval_limits = self.limits.eval();
        let evaluated = cdl_eval::evaluate(&module, &checked, &self.schema, &eval_limits)
            .map_err(|d| vec![d])?;
        Ok(Program {
            module,
            checked,
            evaluated,
            schema: self.schema.clone(),
            limits: eval_limits,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    struct Memory(BTreeMap<&'static str, &'static str>);

    impl ModuleResolver for Memory {
        fn resolve(&self, _importer: Option<&str>, specifier: &str) -> Result<Source, String> {
            self.0
                .get(specifier)
                .map(|text| Source {
                    id: specifier.to_owned(),
                    text: (*text).to_owned(),
                })
                .ok_or_else(|| "ない".to_owned())
        }
    }

    #[test]
    fn checks_the_entry_through_the_resolver() {
        let files = Memory(
            [(
                "main.cdl",
                "\"use cdl 1\";\nimport { Root } from \"host:t\";\nexport default (<Root />);\n",
            )]
            .into_iter()
            .collect(),
        );
        let schema = Schema::builder()
            .root("host:t", "Root")
            .component("host:t", ComponentDef::new("Root"))
            .build()
            .unwrap();
        assert_eq!(
            Engine::new().with_schema(schema).check(&files, "main.cdl"),
            Ok(())
        );
    }

    #[test]
    fn unresolvable_entry_is_a_diagnostic() {
        let files = Memory(BTreeMap::new());
        let err = Engine::new().check(&files, "main.cdl").unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].code, Code::ModuleNotFound);
        assert_eq!(err[0].span.file, "main.cdl");
    }

    #[test]
    fn limits_are_passed_to_the_parser() {
        let files = Memory(
            [("main.cdl", "\"use cdl 1\";\nconst a = 1;\n")]
                .into_iter()
                .collect(),
        );
        let engine = Engine::new().with_limits(Limits {
            max_source_bytes: 4,
            ..Limits::default()
        });
        let err = engine.check(&files, "main.cdl").unwrap_err();
        assert_eq!(err[0].code, Code::SourceTooLarge);
    }
}
