//! Contrast（Wayland コンポジタ）が CDL を組み込む例。
//!
//! 1. 組み込みコンポーネント（Contrast / Appearance / Animation / Keybind）とポリシーの型を Rust で登録する。
//! 2. ファイルシステムのリゾルバを用意する（ソースを読むのはホストの仕事。SPEC 第 9 章）。
//! 3. 設定を読み、値木を Rust の型に変換する。変換の失敗は診断として返る。
//! 4. ポリシーのフックを呼ぶ。失敗したらホストの既定の動作に戻す。
//!
//! 実行: `cargo run -p cdl-host --example contrast`（または、引数に .cdl のパス）

use std::path::Path;
use std::process::ExitCode;

use cdl_host::{
    ComponentDef, Diagnostic, Engine, Field, Millis, ModuleResolver, Node, Program, Px, Rgba,
    Schema, Source, Ty, Value,
};

/// Contrast のスキーマ。
fn schema() -> Result<Schema, Vec<String>> {
    let m = "host:contrast";
    let window_info = Ty::object([
        ("appId", Field::required(Ty::String)),
        ("workspace", Field::required(Ty::Number)),
    ]);
    let placement = Ty::object([
        ("workspace", Field::required(Ty::Number)),
        ("floating", Field::required(Ty::Boolean)),
    ]);
    Schema::builder()
        .root(m, "Contrast")
        .component(
            m,
            ComponentDef::new("Contrast")
                .children(&["Appearance", "Animation", "Keybind"])
                .unique_child("Appearance")
                .unique_child("Animation"),
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
        .type_def(m, "WindowInfo", window_info.clone())
        .type_def(m, "Placement", placement.clone())
        .type_def(
            m,
            "Policy",
            Ty::object([(
                "windowOpen",
                Field::optional(Ty::Function(vec![window_info], Box::new(placement))),
            )]),
        )
        .build()
}

/// ファイルシステムのリゾルバ。`id` は正規化した絶対パス。
struct Files;

impl ModuleResolver for Files {
    fn resolve(&self, importer: Option<&str>, specifier: &str) -> Result<Source, String> {
        let path = match importer {
            None => Path::new(specifier).to_path_buf(),
            Some(from) => Path::new(from)
                .parent()
                .unwrap_or(Path::new(""))
                .join(specifier),
        };
        // 同じファイルが同じ id になるよう、正規化した絶対パスを id にする。
        let path = path
            .canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(Source {
            id: path.to_string_lossy().into_owned(),
            text,
        })
    }
}

/// Contrast 側の設定の型。
#[derive(Debug)]
struct Config {
    gap: Px,
    radius: Px,
    focus_color: Option<Rgba>,
    animation: Millis,
    animation_enabled: bool,
    keybinds: Vec<(String, String)>,
}

/// 値木を Contrast の型に変換する。スキーマが保証しない部分（必須の子など）もここで確かめる。
fn config(root: Node<'_>) -> Result<Config, Diagnostic> {
    root.expect("Contrast")?;
    let mut appearance = None;
    let mut animation = None;
    let mut keybinds = Vec::new();
    for child in root.children() {
        match child.name() {
            "Appearance" => appearance = Some(child),
            "Animation" => animation = Some(child),
            "Keybind" => keybinds.push((child.prop("keys")?, child.prop("action")?)),
            other => return Err(child.error(format!("未知の要素 `<{other}>`"))),
        }
    }
    let appearance = appearance.ok_or_else(|| root.error("`<Appearance>` が要る"))?;
    let animation = animation.ok_or_else(|| root.error("`<Animation>` が要る"))?;
    Ok(Config {
        gap: appearance.prop("gap")?,
        radius: appearance.prop("radius")?,
        focus_color: appearance.optional_prop("focusColor")?,
        animation: animation.prop("duration")?,
        animation_enabled: animation.optional_prop("enabled")?.unwrap_or(true),
        keybinds,
    })
}

/// ポリシーで新しいウィンドウの置き場所を決める。失敗したら既定の動作（いまのワークスペース）。
fn place(program: &Program, app_id: &str, workspace: f64) -> (f64, bool) {
    let arg = Value::object([
        ("appId", Value::string(app_id)),
        ("workspace", Value::Number(workspace)),
    ]);
    match program.call_hook("windowOpen", arg) {
        Ok(Some(Value::Object(p))) => match (p.get("workspace"), p.get("floating")) {
            (Some(Value::Number(w)), Some(Value::Bool(f))) => (*w, *f),
            _ => (workspace, false),
        },
        Ok(_) => (workspace, false),
        Err(d) => {
            eprintln!("ポリシーの誤り（既定の動作に戻す）: {}", d.message);
            (workspace, false)
        }
    }
}

fn report(diags: &[Diagnostic]) -> ExitCode {
    for d in diags {
        eprintln!(
            "{}[{}] {}:{}..{}: {}",
            d.severity.as_str(),
            d.code.as_str(),
            d.span.file,
            d.span.start,
            d.span.end,
            d.message
        );
    }
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let entry = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/contrast/main.cdl"
        )
        .to_owned()
    });
    let schema = match schema() {
        Ok(s) => s,
        Err(errors) => {
            for e in errors {
                eprintln!("スキーマの誤り: {e}");
            }
            return ExitCode::FAILURE;
        }
    };
    let program = match Engine::new().with_schema(schema).load(&Files, &entry) {
        Ok(p) => p,
        Err(diags) => return report(&diags),
    };
    let config = match config(program.root()) {
        Ok(c) => c,
        Err(d) => return report(&[d]),
    };
    println!(
        "読み込んだファイル: {}",
        program.files().collect::<Vec<_>>().join(", ")
    );
    println!("gap = {}px, radius = {}px", config.gap.0, config.radius.0);
    if let Some(Rgba([r, g, b, a])) = config.focus_color {
        println!("focusColor = rgba({r}, {g}, {b}, {a})");
    }
    println!(
        "animation = {}ms（{}）",
        config.animation.0,
        if config.animation_enabled {
            "有効"
        } else {
            "無効"
        }
    );
    for (keys, action) in &config.keybinds {
        println!("keybind: {keys} → {action}");
    }
    for app in ["firefox", "pavucontrol", "kitty"] {
        let (workspace, floating) = place(&program, app, 3.0);
        println!(
            "windowOpen({app}) → workspace {workspace}{}",
            if floating {
                "（フローティング）"
            } else {
                ""
            }
        );
    }
    ExitCode::SUCCESS
}
