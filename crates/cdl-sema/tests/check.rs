//! 意味解析の結合テスト（SPEC 第 4・6・7 章）。スキーマはテストの中でビルダーで組み立てる。

// 統合テストの補助関数は clippy.toml の「テストでは許す」の対象外なので、ここで許す（DECISIONS F3）。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use cdl_diag::Code;
use cdl_sema::schema::{ComponentDef, Schema};
use cdl_sema::syntax::{Limits, parse};
use cdl_sema::types::{Field, Ty};
use cdl_sema::{Checked, check};

fn schema() -> Schema {
    let window = Ty::object([("appId", Field::required(Ty::String))]);
    let placement = Ty::object([("floating", Field::required(Ty::Boolean))]);
    let m = "host:t";
    Schema::builder()
        .root(m, "Root")
        .component(
            m,
            ComponentDef::new("Root")
                .children(&["Item", "Panel"])
                .unique_child("Panel"),
        )
        .component(
            m,
            ComponentDef::new("Panel")
                .prop("gap", Ty::Length)
                .children(&["Item"]),
        )
        .component(
            m,
            ComponentDef::new("Item")
                .prop("name", Ty::String)
                .optional_prop("weight", Ty::Number),
        )
        .type_def(
            m,
            "Policy",
            Ty::object([(
                "open",
                Field::optional(Ty::Function(vec![window], Box::new(placement))),
            )]),
        )
        .build()
        .unwrap()
}

const HEAD: &str = "\"use cdl 1\";\nimport { Root, Panel, Item } from \"host:t\";\nimport type { Policy } from \"host:t\";\n";

fn run(body: &str) -> Result<Checked, Vec<(Code, String)>> {
    let src = format!("{HEAD}{body}");
    let module =
        parse("main.cdl", &src, &Limits::default()).unwrap_or_else(|d| panic!("{src}\n{d:#?}"));
    check(&module, &schema()).map_err(|ds| {
        ds.into_iter()
            .map(|d| {
                let start = usize::try_from(d.span.start).unwrap();
                let end = usize::try_from(d.span.end).unwrap();
                (d.code, src[start..end].to_owned())
            })
            .collect()
    })
}

fn ok(body: &str) -> Checked {
    run(body).unwrap_or_else(|e| panic!("{body}\n{e:#?}"))
}

fn errs(body: &str) -> Vec<(Code, String)> {
    run(body).err().unwrap_or_default()
}

const ROOT: &str = "export default (<Root />);\n";

#[test]
fn const_order_puts_dependencies_first_and_keeps_source_order_otherwise() {
    let c = ok(&format!(
        "const c = b + a;\nconst a = 1;\nconst b = a * 2;\nconst d = 0;\n{ROOT}"
    ));
    assert_eq!(c.const_order, ["a", "b", "c", "d"]);
}

#[test]
fn several_errors_are_reported_and_do_not_cascade() {
    let got = errs(&format!(
        "const a: number = \"x\";\nconst b = a + 1;\nconst c: string = 1;\nconst d = px(1) * px(1);\nconst e = d + px(1);\n{ROOT}"
    ));
    // `b` と `e` は、前の誤りに引きずられて報告しない。
    assert_eq!(
        got,
        [
            (Code::TypeMismatch, "\"x\"".to_owned()),
            (Code::TypeMismatch, "1".to_owned()),
            (Code::BadOperand, "px(1) * px(1)".to_owned()),
        ]
    );
}

#[test]
fn name_errors_stop_before_type_checking() {
    let got = errs(&format!(
        "const a = missing;\nconst b: number = \"x\";\n{ROOT}"
    ));
    assert_eq!(got, [(Code::UndefinedName, "missing".to_owned())]);
}

#[test]
fn policy_hooks_must_match_exactly() {
    ok(&format!(
        "export const policy = {{\n  open: (w: {{ appId: string }}): {{ floating: boolean }} => ({{ floating: w.appId === \"x\" }}),\n}} satisfies Policy;\n{ROOT}"
    ));
    // 文脈から引数と戻り値の型が決まる。
    ok(&format!(
        "const p = {{ open: (w) => ({{ floating: true }}) }} satisfies Policy;\n{ROOT}"
    ));
    let got = errs(&format!(
        "const p = {{ open: (w: {{ appId: \"a\" }}): {{ floating: boolean }} => ({{ floating: true }}) }} satisfies Policy;\n{ROOT}"
    ));
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, Code::TypeMismatch);
    let got = errs(&format!(
        "export const policy = {{ close: 1 }} satisfies Policy;\n{ROOT}"
    ));
    assert_eq!(got, [(Code::UnknownHook, "close".to_owned())]);
}

#[test]
fn tuples_records_and_optional_chaining() {
    ok(&format!(
        "const t: [number, string] = [1, \"a\"];\nconst n: number = t[0];\nconst s: string = t[1];\n\
         const r: Record<string, number> = {{ a: 1 }};\nconst m: number | undefined = r[\"a\"];\n\
         interface W {{ name: string }}\nconst w: W | undefined = undefined;\nconst k: string | undefined = w?.name;\n{ROOT}"
    ));
    let got = errs(&format!(
        "const r: Record<string, number> = {{ a: 1 }};\nconst m = r.a;\n{ROOT}"
    ));
    assert_eq!(got, [(Code::UnknownProperty, "a".to_owned())]);
}

#[test]
fn narrowing_through_logical_operators_and_member_paths() {
    ok(&format!(
        "interface O {{ inner: {{ v: number | undefined }} }}\n\
         function f(o: O): number {{\n  return o.inner.v !== undefined && o.inner.v > 1 ? o.inner.v : 0;\n}}\n\
         function g(x: number | undefined): boolean {{\n  return x === undefined || x > 0;\n}}\n{ROOT}"
    ));
}

#[test]
fn array_methods_are_typed() {
    ok(&format!(
        "const xs = [1, 2, 3];\nconst total: number = xs.reduce((acc, x) => acc + x, 0);\n\
         const names: string[] = xs.map((x) => `${{x}}`);\nconst big: number[] = xs.filter((x) => x > 1);\n\
         const has: boolean = xs.includes(2);\nconst all: number[] = xs.concat([4]);\nconst n: number = xs.length;\n{ROOT}"
    ));
    let got = errs(&format!(
        "const xs = [1];\nconst a = xs.filter((x) => x);\nconst b = xs.push(1);\n{ROOT}"
    ));
    let codes: Vec<Code> = got.iter().map(|(c, _)| *c).collect();
    // 配列にないメソッドは、未知のプロパティ（DECISIONS B15）。
    assert_eq!(
        codes,
        [Code::TypeMismatch, Code::UnknownProperty],
        "{got:?}"
    );
}

#[test]
fn equality_rules() {
    let got = errs(&format!(
        "const o = {{ a: 1 }};\nconst a = o === o;\nconst b = 1 === \"1\";\nconst c = [1] === [1];\n{ROOT}"
    ));
    let codes: Vec<Code> = got.iter().map(|(c, _)| *c).collect();
    assert_eq!(codes, [Code::BadOperand; 3], "{got:?}");
}

#[test]
fn schema_children_unique_and_root() {
    ok(
        "export default (<Root><Panel gap={px(1)}><Item name=\"a\" /></Panel>{[1, 2].map((n) => <Item name={`${n}`} weight={n} />)}</Root>);\n",
    );
    let got =
        errs("export default (<Root><Panel gap={px(1)} /><Panel gap={px(2)} /><Root /></Root>);\n");
    let codes: Vec<Code> = got.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        codes,
        [Code::DuplicateUniqueChild, Code::HostChildType],
        "{got:?}"
    );
    let got = errs("export default (<Panel gap={px(1)} />);\n");
    assert_eq!(
        got,
        [(Code::RootElement, "<Panel gap={px(1)} />".to_owned())]
    );
    let got = errs(&format!("{ROOT}{ROOT}"));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, Code::RootElement);
}

#[test]
fn user_components_with_children() {
    ok(
        "function Group(props: { children: Element<Item>[] }): Element<Panel> {\n  return <Panel gap={px(1)}>{props.children}</Panel>;\n}\n\
         export default (<Root><Group><Item name=\"a\" /></Group></Root>);\n",
    );
    let got = errs(
        "function Group(props: { children: Element<Item>[] }): Element<Panel> {\n  return <Panel gap={px(1)}>{props.children}</Panel>;\n}\n\
         export default (<Root><Group children={[]}><Item name=\"a\" /></Group></Root>);\n",
    );
    assert!(
        got.iter().any(|(c, _)| *c == Code::ChildrenTwice),
        "{got:?}"
    );
    let got = errs(&format!(
        "function Bad(props: {{}}): number {{\n  return 1;\n}}\nconst X = 1;\nconst e = <X />;\n{ROOT}"
    ));
    let codes: Vec<Code> = got.iter().map(|(c, _)| *c).collect();
    assert_eq!(codes, [Code::TypeMismatch, Code::NotAComponent], "{got:?}");
}

#[test]
fn checking_is_deterministic() {
    let body = format!(
        "const a: number = \"x\";\nconst b = <Item name={{1}} />;\nconst c = [1, \"a\"];\n{ROOT}"
    );
    assert_eq!(run(&body), run(&body));
}

/// 構文の入れ子の上限ちょうどの式も、型検査でスタックを使い切らない（テストのスレッドは 2 MiB）。
#[test]
fn deep_expressions_at_the_nesting_limit_do_not_overflow() {
    let n = 120;
    let bodies = [
        format!("const a = {}1{};", "[".repeat(n), "]".repeat(n)),
        format!("const a = {}true;", "!".repeat(n)),
        format!("const a = 1{};", " + 1".repeat(n / 2)),
        format!("const a = {}0;", "true ? 1 : ".repeat(n / 2)),
        format!(
            "const a = {}1{};",
            "{ k: ".repeat(n / 2),
            " }".repeat(n / 2)
        ),
        format!(
            "const a = <Root>{}{}</Root>;",
            "<>".repeat(n / 2),
            "</>".repeat(n / 2)
        ),
    ];
    for body in bodies {
        let src = format!("{HEAD}{body}\n{ROOT}");
        if let Ok(module) = parse("main.cdl", &src, &Limits::default()) {
            let _ = check(&module, &schema());
        }
    }
}
