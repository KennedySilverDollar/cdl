//! 診断コード表。`docs/DIAGNOSTICS.md` と一致させる（DECISIONS F1）。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! codes {
    ($($name:ident => $code:literal, $summary:literal;)*) => {
        /// 安定した診断コード。意味を変えるときは新しいコードを振る。
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Code {
            $($name,)*
        }

        impl Code {
            /// 表に載っているすべてのコード（表の順）。
            pub const ALL: &'static [Code] = &[$(Code::$name,)*];

            /// `E0001` の形の文字列。
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Code::$name => $code,)*
                }
            }

            /// コード表に載せる、コードの意味の短い説明。
            pub const fn summary(self) -> &'static str {
                match self {
                    $(Code::$name => $summary,)*
                }
            }

            pub fn parse(s: &str) -> Option<Code> {
                match s {
                    $($code => Some(Code::$name),)*
                    _ => None,
                }
            }
        }
    };
}

codes! {
    UnterminatedString => "E0001", "閉じていない文字列";
    InvalidEscape => "E0002", "不正なエスケープ";
    UnterminatedTemplate => "E0003", "閉じていないテンプレート文字列";
    UnterminatedComment => "E0004", "閉じていないブロックコメント";
    UnsupportedLiteral => "E0005", "使えない形のリテラル（`0o`・先頭 0 の数値・BigInt・正規表現）";
    NonFiniteNumber => "E0006", "有限にならない数値リテラル";
    ForbiddenCharacter => "E0007", "使えない文字（BOM・単独の CR・U+2028・U+2029・文字列の外の `\\`）";
    JsxAttrCharRef => "E0008", "JSX の属性文字列の `&`（文字参照の扱いが未決）";
    SyntaxError => "E1001", "構文エラー";
    MissingSemicolon => "E1002", "文末の `;` がない";
    InvalidEdition => "E1003", "edition 宣言がない、または形が不正";
    UnsupportedEdition => "E1004", "実装が対応していない edition";
    UnsupportedSyntax => "E1005", "CDL にない構文";
    ReservedWord => "E1006", "予約語の使用";
    ReactFeature => "E1007", "CDL にない React の機能（Hooks・イベント属性・`ref`・`key`）";
    NestingTooDeep => "E1008", "構文の入れ子が深すぎる";
    JsxTextChild => "E1009", "JSX のテキスト子";
    MissingElse => "E1010", "`else` のない `if`";
    LowercaseJsxTag => "E1011", "小文字で始まる JSX タグ";
    ModuleNotFound => "E2001", "モジュールを解決できない";
    SourceTooLarge => "E5001", "ソースサイズの上限超過";
}

impl Serialize for Code {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Code {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Code::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("未知の診断コード: {s}")))
    }
}

#[cfg(test)]
mod tests {
    use super::Code;

    const TABLE: &str = include_str!("../../../docs/DIAGNOSTICS.md");

    /// `docs/DIAGNOSTICS.md` の表の行を (コード, 意味) として取り出す。
    fn documented() -> Vec<(String, String)> {
        TABLE
            .lines()
            .filter(|line| line.starts_with("| E"))
            .map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                (cells[1].to_owned(), cells[2].to_owned())
            })
            .collect()
    }

    #[test]
    fn table_matches_docs() {
        let defined: Vec<(String, String)> = Code::ALL
            .iter()
            .map(|c| (c.as_str().to_owned(), c.summary().to_owned()))
            .collect();
        assert_eq!(defined, documented());
    }

    #[test]
    fn codes_are_well_formed_sorted_and_unique() {
        let strs: Vec<&str> = Code::ALL.iter().map(|c| c.as_str()).collect();
        for s in &strs {
            let bytes = s.as_bytes();
            assert_eq!(bytes.len(), 5, "{s}");
            assert_eq!(bytes[0], b'E', "{s}");
            assert!((b'0'..=b'5').contains(&bytes[1]), "帯の外: {s}");
            assert!(bytes[1..].iter().all(u8::is_ascii_digit), "{s}");
        }
        assert!(
            strs.windows(2).all(|w| w[0] < w[1]),
            "昇順で重複なし: {strs:?}"
        );
    }

    #[test]
    fn parse_round_trips() {
        for &code in Code::ALL {
            assert_eq!(Code::parse(code.as_str()), Some(code));
        }
        assert_eq!(Code::parse("E9999"), None);
    }
}
