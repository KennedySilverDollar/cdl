//! P5: 診断の JSON は往復する（SPEC 第 10 章「出力形式」）。

// 統合テストの補助関数は clippy.toml の「テストでは許す」の対象外なので、ここで許す（DECISIONS F3）。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use cdl_diag::{Code, Diagnostic, Label, Severity, Span, from_json, to_json};
use proptest::prelude::*;

fn span() -> impl Strategy<Value = Span> {
    ("[a-z./]{1,12}", any::<u32>(), any::<u32>()).prop_map(|(file, start, end)| Span {
        file,
        start,
        end,
    })
}

fn diagnostic() -> impl Strategy<Value = Diagnostic> {
    (
        proptest::sample::select(Code::ALL.to_vec()),
        proptest::sample::select(vec![Severity::Error, Severity::Warning, Severity::Hint]),
        span(),
        any::<String>(),
        proptest::collection::vec((span(), any::<String>()), 0..3),
        proptest::option::of(any::<String>()),
    )
        .prop_map(|(code, severity, span, message, labels, help)| Diagnostic {
            code,
            severity,
            span,
            message,
            labels: labels
                .into_iter()
                .map(|(span, message)| Label { span, message })
                .collect(),
            help,
        })
}

proptest! {
    #[test]
    fn json_round_trips(diags in proptest::collection::vec(diagnostic(), 0..4)) {
        let json = to_json(&diags).unwrap();
        prop_assert_eq!(from_json(&json).unwrap(), diags);
    }
}
