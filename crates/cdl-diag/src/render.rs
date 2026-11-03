//! 人間向けのレンダリング（ソース抜粋と下線）。

use std::fmt::Write as _;

use crate::{Diagnostic, Span};

/// ファイル名からソース本文を引く。
pub trait SourceMap {
    fn source(&self, file: &str) -> Option<&str>;
}

/// span が指す 1 行ぶんの抜粋。列と下線は文字（Unicode スカラー値）単位で数える。
struct Snippet<'a> {
    line: usize,
    col: usize,
    text: &'a str,
    underline_len: usize,
}

/// span の位置を求める。範囲外・文字境界でない span には `None` を返す。
fn locate<'a>(src: &'a str, span: &Span) -> Option<Snippet<'a>> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    if end < start {
        return None;
    }
    let before = src.get(..start)?;
    src.get(start..end)?;
    let line_start = before.rfind('\n').map_or(0, |i| i.saturating_add(1));
    let rest = src.get(line_start..)?;
    let raw = rest.get(..rest.find('\n').unwrap_or(rest.len()))?;
    let text = raw.strip_suffix('\r').unwrap_or(raw);
    let line_end = line_start.saturating_add(text.len());
    let col0 = src.get(line_start..start)?.chars().count();
    let covered = src.get(start..end.min(line_end).max(start))?;
    Some(Snippet {
        line: before.matches('\n').count().saturating_add(1),
        col: col0.saturating_add(1),
        text,
        underline_len: covered.chars().count().max(1),
    })
}

fn write_location(out: &mut String, pad: &str, span: &Span, snippet: Option<&Snippet<'_>>) {
    let _ = match snippet {
        Some(s) => writeln!(out, "{pad}--> {}:{}:{}", span.file, s.line, s.col),
        None => writeln!(
            out,
            "{pad}--> {}（バイト {}..{}）",
            span.file, span.start, span.end
        ),
    };
}

fn write_snippet(out: &mut String, pad: &str, s: &Snippet<'_>, mark: char, note: Option<&str>) {
    let width = pad.len();
    // 下線の前は、行の中のタブをそのまま写して位置を合わせる。
    let lead: String = s
        .text
        .chars()
        .take(s.col.saturating_sub(1))
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let marks: String = std::iter::repeat_n(mark, s.underline_len).collect();
    let _ = writeln!(out, "{pad} |");
    let _ = writeln!(out, "{:>width$} | {}", s.line, s.text);
    let _ = match note {
        Some(note) => writeln!(out, "{pad} | {lead}{marks} {note}"),
        None => writeln!(out, "{pad} | {lead}{marks}"),
    };
}

/// 1 件の診断を、ソース抜粋と下線つきの文字列にする。
/// ソースが見つからないか span が不正なときは、抜粋を省いてバイトオフセットを示す。
pub fn render_human(diag: &Diagnostic, sources: &dyn SourceMap) -> String {
    let find = |span: &Span| sources.source(&span.file).and_then(|src| locate(src, span));
    let primary = find(&diag.span);
    let labels: Vec<_> = diag.labels.iter().map(|l| (l, find(&l.span))).collect();

    let width = std::iter::once(primary.as_ref())
        .chain(labels.iter().map(|(_, s)| s.as_ref()))
        .flatten()
        .map(|s| s.line.to_string().len())
        .max()
        .unwrap_or(1);
    let pad = " ".repeat(width);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}[{}]: {}",
        diag.severity.as_str(),
        diag.code.as_str(),
        diag.message
    );
    write_location(&mut out, &pad, &diag.span, primary.as_ref());
    if let Some(s) = &primary {
        write_snippet(&mut out, &pad, s, '^', None);
    }
    for (label, snippet) in &labels {
        write_location(&mut out, &pad, &label.span, snippet.as_ref());
        match snippet {
            Some(s) => write_snippet(&mut out, &pad, s, '-', Some(&label.message)),
            None => {
                let _ = writeln!(out, "{pad} = note: {}", label.message);
            }
        }
    }
    if let Some(help) = &diag.help {
        let _ = writeln!(out, "{pad} = help: {help}");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::Code;

    struct Files(BTreeMap<&'static str, &'static str>);

    impl SourceMap for Files {
        fn source(&self, file: &str) -> Option<&str> {
            self.0.get(file).copied()
        }
    }

    fn files(entries: &[(&'static str, &'static str)]) -> Files {
        Files(entries.iter().copied().collect())
    }

    fn span(file: &str, start: u32, end: u32) -> Span {
        Span {
            file: file.to_owned(),
            start,
            end,
        }
    }

    const MAIN: &str = "\"use cdl 1\";\nconst s = \"abc;\n";

    #[test]
    fn renders_snippet_with_underline_and_help() {
        let diag = Diagnostic::error(
            Code::UnterminatedString,
            span("main.cdl", 23, 28),
            "文字列が閉じていません",
        )
        .with_help("閉じる `\"` を足してください");
        let got = render_human(&diag, &files(&[("main.cdl", MAIN)]));
        let want = concat!(
            "error[E0001]: 文字列が閉じていません\n",
            " --> main.cdl:2:11\n",
            "  |\n",
            "2 | const s = \"abc;\n",
            "  |           ^^^^^\n",
            "  = help: 閉じる `\"` を足してください\n",
        );
        assert_eq!(got, want);
    }

    #[test]
    fn renders_labels_with_their_own_snippets() {
        let src = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nkk\n";
        let diag = Diagnostic::error(Code::InvalidEscape, span("m.cdl", 20, 22), "主")
            .with_label(span("m.cdl", 0, 1), "関連");
        let got = render_human(&diag, &files(&[("m.cdl", src)]));
        let want = concat!(
            "error[E0002]: 主\n",
            "  --> m.cdl:11:1\n",
            "   |\n",
            "11 | kk\n",
            "   | ^^\n",
            "  --> m.cdl:1:1\n",
            "   |\n",
            " 1 | a\n",
            "   | - 関連\n",
        );
        assert_eq!(got, want);
    }

    #[test]
    fn counts_columns_in_characters_and_strips_cr() {
        let src = "\"あい\";\r\nx\r\n";
        // `;` は 8 バイト目（"あい" はそれぞれ 3 バイト）。
        let diag = Diagnostic::error(Code::InvalidEscape, span("m.cdl", 8, 9), "m");
        let got = render_human(&diag, &files(&[("m.cdl", src)]));
        assert!(got.contains(" --> m.cdl:1:5\n"), "{got}");
        assert!(got.contains("1 | \"あい\";\n"), "{got}");
        assert!(got.contains("  |     ^\n"), "{got}");
    }

    #[test]
    fn falls_back_to_offsets_without_source_or_on_bad_span() {
        let diag = Diagnostic::error(Code::InvalidEscape, span("none.cdl", 1, 2), "m")
            .with_label(span("m.cdl", 1, 2), "途中のバイト");
        // "あ" の 2 バイト目は文字境界ではない。
        let got = render_human(&diag, &files(&[("m.cdl", "あ")]));
        let want = concat!(
            "error[E0002]: m\n",
            " --> none.cdl（バイト 1..2）\n",
            " --> m.cdl（バイト 1..2）\n",
            "  = note: 途中のバイト\n",
        );
        assert_eq!(got, want);
    }

    #[test]
    fn empty_and_multiline_spans_underline_at_least_one_char_on_first_line() {
        let src = "ab\ncd\n";
        let empty = Diagnostic::error(Code::InvalidEscape, span("m.cdl", 1, 1), "m");
        let multi = Diagnostic::error(Code::InvalidEscape, span("m.cdl", 1, 5), "m");
        let f = files(&[("m.cdl", src)]);
        assert!(render_human(&empty, &f).contains("  |  ^\n"));
        assert!(render_human(&multi, &f).contains("  |  ^\n"));
    }
}
