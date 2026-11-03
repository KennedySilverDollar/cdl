//! 機械向けの JSON 形式（DECISIONS F2）。

use crate::Diagnostic;

/// 診断の列を JSON の配列にする。
pub fn to_json(diags: &[Diagnostic]) -> Result<String, serde_json::Error> {
    serde_json::to_string(diags)
}

/// JSON の配列から診断の列を読む。
pub fn from_json(s: &str) -> Result<Vec<Diagnostic>, serde_json::Error> {
    serde_json::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Code, Span};

    #[test]
    fn field_names_and_shape_are_stable() {
        let span = |start, end| Span {
            file: "main.cdl".to_owned(),
            start,
            end,
        };
        let diag = Diagnostic::error(Code::UnterminatedString, span(3, 7), "m")
            .with_label(span(0, 1), "l")
            .with_help("h");
        assert_eq!(
            to_json(&[diag]).unwrap(),
            r#"[{"code":"E0001","severity":"error","span":{"file":"main.cdl","start":3,"end":7},"message":"m","labels":[{"span":{"file":"main.cdl","start":0,"end":1},"message":"l"}],"help":"h"}]"#
        );
    }

    #[test]
    fn help_is_omitted_when_absent() {
        let diag = Diagnostic::error(
            Code::InvalidEscape,
            Span {
                file: "a.cdl".to_owned(),
                start: 0,
                end: 0,
            },
            "m",
        );
        let json = to_json(&[diag]).unwrap();
        assert!(!json.contains("help"), "{json}");
    }

    #[test]
    fn unknown_code_is_rejected() {
        let json = r#"[{"code":"E9999","severity":"error","span":{"file":"a","start":0,"end":0},"message":"m","labels":[]}]"#;
        assert!(from_json(json).is_err());
    }
}
