//! 適合テストスイート（`conformance/`）のランナー。`cdl-host` の公開 API だけを使う。
//!
//! 比べるのは、診断の `code`・`severity`・`span` だけ（メッセージは実装ごとに違ってよい）。
//! まだ実装していない段階の診断は、期待結果から除いて比べる（[`implemented`]）。
//! 値木の比較は、評価を実装する段階で足す。いまは「誤りがない」ことだけを確かめる。

// 統合テストの補助関数は clippy.toml の「テストでは許す」の対象外なので、ここで許す（DECISIONS F3）。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use cdl_host::diag::Diagnostic;
use cdl_host::{ComponentDef, Engine, Field, ModuleResolver, Schema, Source, Ty};
use serde_json::Value;

/// 実装済みの段階が出す診断か。段階を実装したら、ここを広げる。
fn implemented(code: &str) -> bool {
    !code.starts_with("E5") || code == "E5001"
}

/// `conformance/HOST.md` の参照ホスト。
fn reference_schema() -> Schema {
    let window_info = Ty::object([
        ("appId", Field::required(Ty::String)),
        ("workspace", Field::required(Ty::Number)),
    ]);
    let placement = Ty::object([
        ("workspace", Field::required(Ty::Number)),
        ("floating", Field::required(Ty::Boolean)),
    ]);
    let policy = Ty::object([(
        "windowOpen",
        Field::optional(Ty::Function(
            vec![window_info.clone()],
            Box::new(placement.clone()),
        )),
    )]);
    let m = "host:contrast";
    Schema::builder()
        .root(m, "Contrast")
        .component(
            m,
            ComponentDef::new("Contrast")
                .children(&["Appearance", "Animation", "Keybind"])
                .unique_child("Appearance"),
        )
        .component(
            m,
            ComponentDef::new("Appearance")
                .prop("gap", Ty::Length)
                .prop("radius", Ty::Length)
                .optional_prop("focusColor", Ty::Color),
        )
        .component(
            m,
            ComponentDef::new("Animation")
                .prop("duration", Ty::Duration)
                .optional_prop("enabled", Ty::Boolean),
        )
        .component(
            m,
            ComponentDef::new("Keybind")
                .prop("keys", Ty::String)
                .prop("action", Ty::String),
        )
        .type_def(m, "WindowInfo", window_info)
        .type_def(m, "Placement", placement)
        .type_def(m, "Policy", policy)
        .build()
        .unwrap()
}

/// 1 件のテスト。
struct Case {
    name: String,
    /// モジュールの `id` → ソース。
    files: BTreeMap<String, String>,
    entry: String,
    expected: PathBuf,
}

/// 比べる部分だけを取り出した診断。
#[derive(Debug, PartialEq, Eq)]
struct Key {
    code: String,
    severity: String,
    file: String,
    start: u64,
    end: u64,
}

enum Expected {
    Value(Value),
    Diagnostics(Vec<Key>),
}

struct Memory<'a>(&'a BTreeMap<String, String>);

impl ModuleResolver for Memory<'_> {
    fn resolve(&self, _importer: Option<&str>, specifier: &str) -> Result<Source, String> {
        self.0
            .get(specifier)
            .map(|text| Source {
                id: specifier.to_owned(),
                text: text.clone(),
            })
            .ok_or_else(|| "スイートにないファイル".to_owned())
    }
}

fn suite_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

/// `<章>-<名前>` の形か。
fn valid_name(name: &str) -> bool {
    let Some((chapter, rest)) = name.split_once('-') else {
        return false;
    };
    let chapter_ok =
        !chapter.is_empty() && chapter.len() <= 2 && chapter.bytes().all(|b| b.is_ascii_digit());
    let rest_ok = !rest.is_empty()
        && !rest.starts_with('-')
        && !rest.ends_with('-')
        && !rest.contains("--")
        && rest
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    chapter_ok && rest_ok
}

/// ディレクトリの中の `.cdl` を、ディレクトリからの相対パスを `id` にして読む。
fn read_dir_files(dir: &Path, prefix: &str, out: &mut BTreeMap<String, String>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let id = format!("{prefix}{name}");
        if path.is_dir() {
            read_dir_files(&path, &format!("{id}/"), out);
        } else if name.ends_with(".cdl") {
            out.insert(id, fs::read_to_string(&path).unwrap());
        }
    }
}

/// スイートを走査する。組になっていないファイルは誤りとして集める。
fn discover(root: &Path) -> (Vec<Case>, Vec<String>) {
    let mut cases = Vec::new();
    let mut errors = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in &entries {
        let file_name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if path.is_dir() {
            let expected = path.join("expected.json");
            let mut files = BTreeMap::new();
            read_dir_files(path, "", &mut files);
            if !files.contains_key("main.cdl") || !expected.is_file() {
                errors.push(format!("{file_name}/: main.cdl と expected.json が要る"));
                continue;
            }
            cases.push(Case {
                name: file_name,
                files,
                entry: "main.cdl".to_owned(),
                expected,
            });
        } else if let Some(stem) = file_name.strip_suffix(".cdl") {
            let expected = path.with_file_name(format!("{stem}.expected.json"));
            if !expected.is_file() {
                errors.push(format!("{file_name}: {stem}.expected.json がない"));
                continue;
            }
            let files = BTreeMap::from([(file_name.clone(), fs::read_to_string(path).unwrap())]);
            cases.push(Case {
                name: stem.to_owned(),
                files,
                entry: file_name.clone(),
                expected,
            });
        } else if let Some(stem) = file_name.strip_suffix(".expected.json") {
            if !path.with_file_name(format!("{stem}.cdl")).is_file() {
                errors.push(format!("{file_name}: {stem}.cdl がない"));
            }
        } else if !matches!(file_name.as_str(), "README.md" | "HOST.md") {
            errors.push(format!("{file_name}: スイートに置けないファイル"));
        }
    }
    for case in &cases {
        if !valid_name(&case.name) {
            errors.push(format!("{}: 名前は <章>-<名前> の形にする", case.name));
        }
    }
    (cases, errors)
}

fn key_of_expected(v: &Value) -> Option<Key> {
    let span = v.get("span")?;
    Some(Key {
        code: v.get("code")?.as_str()?.to_owned(),
        severity: v.get("severity")?.as_str()?.to_owned(),
        file: span.get("file")?.as_str()?.to_owned(),
        start: span.get("start")?.as_u64()?,
        end: span.get("end")?.as_u64()?,
    })
}

fn key_of(d: &Diagnostic) -> Key {
    Key {
        code: d.code.as_str().to_owned(),
        severity: d.severity.as_str().to_owned(),
        file: d.span.file.clone(),
        start: u64::from(d.span.start),
        end: u64::from(d.span.end),
    }
}

/// 期待結果は `{"value": …}` か `{"diagnostics": […]}` のどちらか。
fn read_expected(case: &Case) -> Result<Expected, String> {
    let text = fs::read_to_string(&case.expected).map_err(|e| format!("{}: {e}", case.name))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", case.name))?;
    let object = value.as_object().filter(|o| o.len() == 1);
    match object.and_then(|o| o.iter().next()) {
        Some((key, v)) if key == "value" => Ok(Expected::Value(v.clone())),
        Some((key, Value::Array(items))) if key == "diagnostics" => items
            .iter()
            .map(|item| {
                key_of_expected(item).ok_or_else(|| {
                    format!(
                        "{}: 診断には code・severity・span が要る: {item}",
                        case.name
                    )
                })
            })
            .collect::<Result<_, _>>()
            .map(Expected::Diagnostics),
        _ => Err(format!(
            "{}: キーは value か diagnostics（配列）の 1 つだけ",
            case.name
        )),
    }
}

fn run(case: &Case) -> Result<(), Vec<Diagnostic>> {
    Engine::new()
        .with_schema(reference_schema())
        .check(&Memory(&case.files), &case.entry)
}

fn check_case(case: &Case) -> Result<(), String> {
    let expected = read_expected(case)?;
    let first = run(case);
    // 決定性: 2 回の実行で結果が完全に一致する（SPEC 第 12 章）。
    if first != run(case) {
        return Err(format!("{}: 2 回の実行で結果が違う", case.name));
    }
    let actual: Vec<Key> = first.err().unwrap_or_default().iter().map(key_of).collect();
    let want: Vec<Key> = match expected {
        // 値木の比較は評価を実装する段階で足す。
        Expected::Value(_tree) => Vec::new(),
        Expected::Diagnostics(all) => all.into_iter().filter(|k| implemented(&k.code)).collect(),
    };
    if actual == want {
        Ok(())
    } else {
        Err(format!(
            "{}:\n  期待: {want:?}\n  実際: {actual:?}",
            case.name
        ))
    }
}

#[test]
fn conformance_suite() {
    let (cases, mut errors) = discover(&suite_root());
    errors.extend(cases.iter().filter_map(|c| check_case(c).err()));
    assert!(
        errors.is_empty(),
        "適合テストの失敗（{} 件中）:\n{}",
        cases.len(),
        errors.join("\n")
    );
}

#[test]
fn names_follow_the_convention() {
    assert!(valid_name("5-switch-exhaustive"));
    assert!(valid_name("12-determinism"));
    assert!(!valid_name("switch"));
    assert!(!valid_name("5-"));
    assert!(!valid_name("5-Switch"));
    assert!(!valid_name("5--a"));
    assert!(!valid_name("5-a--b"));
    assert!(!valid_name("123-a"));
}
