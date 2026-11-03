//! 名前解決・型検査・スキーマ適合。
#![forbid(unsafe_code)]

/// 構文解析。上のクレートは、このクレートを通して使う（DECISIONS H1）。
pub use cdl_syntax as syntax;
