//! ホストに組み込むための公開 API。互換性を約束するのはこのクレートだけ。
//!
//! プロトタイプの段階では、実装済みの段階（構文解析）までを行う。
#![forbid(unsafe_code)]

use cdl_diag::{Code, Diagnostic, Span};
use cdl_eval::sema::syntax;

pub use cdl_diag as diag;

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
}

impl Default for Limits {
    fn default() -> Self {
        let syntax = syntax::Limits::default();
        Limits {
            max_source_bytes: syntax.max_source_bytes,
            max_nesting: syntax.max_nesting,
        }
    }
}

/// CDL の処理系。
#[derive(Clone, Debug, Default)]
pub struct Engine {
    limits: Limits,
}

impl Engine {
    pub fn new() -> Self {
        Engine::default()
    }

    pub fn with_limits(limits: Limits) -> Self {
        Engine { limits }
    }

    /// エントリファイルを検査する。誤りがあれば、位置順に並べた診断を返す。
    pub fn check(&self, resolver: &dyn ModuleResolver, entry: &str) -> Result<(), Vec<Diagnostic>> {
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
        syntax::parse(&source.id, &source.text, &limits).map(|_| ())
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
            [("main.cdl", "\"use cdl 1\";\nconst a = 1;\n")]
                .into_iter()
                .collect(),
        );
        assert_eq!(Engine::new().check(&files, "main.cdl"), Ok(()));
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
        let engine = Engine::with_limits(Limits {
            max_source_bytes: 4,
            ..Limits::default()
        });
        let err = engine.check(&files, "main.cdl").unwrap_err();
        assert_eq!(err[0].code, Code::SourceTooLarge);
    }
}
