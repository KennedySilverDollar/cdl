//! 診断の型（コード・重大度・span・メッセージ）とレンダリング。
#![forbid(unsafe_code)]

mod code;
mod json;
mod render;

pub use code::Code;
pub use json::{from_json, to_json};
pub use render::{SourceMap, render_human};

use serde::{Deserialize, Serialize};

/// 診断の重大度。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Hint => "hint",
        }
    }
}

/// ファイル内の位置。`start` と `end` は元のソースのバイトオフセット。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub start: u32,
    pub end: u32,
}

/// 診断に添える、関連する別の位置と短い注記。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

/// 1 件の診断。JSON 形式は互換性の対象（SPEC 第 10 章）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: Code,
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: Code, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            severity: Severity::Error,
            span,
            message: message.into(),
            labels: Vec::new(),
            help: None,
        }
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// 診断を (ファイル, 開始, 終了, コード) の順に安定ソートする（DECISIONS D5）。
pub fn sort(diags: &mut [Diagnostic]) {
    diags.sort_by(|a, b| {
        let key = |d: &Diagnostic| {
            (
                d.span.file.clone(),
                d.span.start,
                d.span.end,
                d.code.as_str(),
            )
        };
        key(a).cmp(&key(b))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(file: &str, start: u32, end: u32) -> Span {
        Span {
            file: file.to_owned(),
            start,
            end,
        }
    }

    #[test]
    fn sort_orders_by_file_position_then_code() {
        let mut diags = vec![
            Diagnostic::error(Code::InvalidEscape, span("b.cdl", 0, 1), "x"),
            Diagnostic::error(Code::InvalidEscape, span("a.cdl", 5, 6), "x"),
            Diagnostic::error(Code::InvalidEscape, span("a.cdl", 1, 2), "x"),
            Diagnostic::error(Code::UnterminatedString, span("a.cdl", 1, 2), "x"),
        ];
        sort(&mut diags);
        let got: Vec<_> = diags
            .iter()
            .map(|d| (d.span.file.as_str(), d.span.start, d.code.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("a.cdl", 1, "E0001"),
                ("a.cdl", 1, "E0002"),
                ("a.cdl", 5, "E0002"),
                ("b.cdl", 0, "E0002"),
            ]
        );
    }
}
