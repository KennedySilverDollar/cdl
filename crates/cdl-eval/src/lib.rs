//! 評価器とリソース上限（SPEC 第 7・8 章）。
#![forbid(unsafe_code)]

mod budget;
mod eval;
pub mod num;
pub mod value;

use std::collections::BTreeMap;
use std::rc::Rc;

use cdl_diag::{Code, Diagnostic};
use cdl_sema::Checked;
use cdl_sema::schema::Schema;
use cdl_sema::syntax::ast::{ItemKind, Module, Span};
use cdl_sema::types::Ty;

/// 意味解析。上のクレートは、このクレートを通して使う（DECISIONS H1）。
pub use cdl_sema as sema;

pub use budget::{Budget, Exhausted};
pub use value::{Element, Function, Value, conforms};

/// 評価の上限（SPEC 第 7・8 章。既定値は暫定）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Document 全体の評価ステップ。
    pub max_steps: u64,
    /// 関数・コンポーネント・コールバックの呼び出しの深さ。
    pub max_call_depth: u32,
    /// 1 つの文字列の長さ（UTF-8 のバイト数）。
    pub max_string_bytes: u32,
    /// 1 つの配列・マップの要素数。
    pub max_collection_len: u32,
    /// 評価結果の値木のノード数。
    pub max_nodes: u64,
    /// フックの呼び出し 1 回あたりの評価ステップ。
    pub max_hook_steps: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_steps: 1_000_000,
            max_call_depth: 64,
            max_string_bytes: 64 * 1024,
            max_collection_len: 65_536,
            max_nodes: 1_000_000,
            max_hook_steps: 10_000,
        }
    }
}

/// 評価の結果。`document` は不変の要素ツリー。
#[derive(Clone, Debug)]
pub struct Evaluated {
    pub document: Rc<Element>,
    globals: BTreeMap<String, Value>,
}

fn diagnostic(module: &Module, e: eval::EvalError) -> Diagnostic {
    let span = e.span.unwrap_or_default();
    Diagnostic::error(
        e.code,
        cdl_diag::Span {
            file: module.file.clone(),
            start: span.start,
            end: span.end,
        },
        e.message,
    )
}

/// 意味解析を通ったファイルを評価する。誤りは最初の 1 つで打ち切る（DECISIONS D9）。
pub fn evaluate(
    module: &Module,
    checked: &Checked,
    schema: &Schema,
    limits: &Limits,
) -> Result<Evaluated, Diagnostic> {
    let mut ev = eval::Evaluator::new(module, schema, *limits, limits.max_steps);
    ev.globals(&checked.const_order)
        .map_err(|e| diagnostic(module, e))?;
    let document = ev.document().map_err(|e| diagnostic(module, e))?;
    Ok(Evaluated {
        document,
        globals: ev.globals,
    })
}

/// `export const policy` の名前の位置。
fn policy_span(module: &Module) -> Span {
    module
        .items
        .iter()
        .find_map(|i| match &i.kind {
            ItemKind::Const(c) if i.exported && c.name.name == "policy" => Some(c.name.span),
            _ => None,
        })
        .unwrap_or_default()
}

/// フックを呼ぶ（SPEC 第 8 章）。フックが定義されていなければ `Ok(None)`（ホストは既定の動作をとる）。
/// 呼び出しごとに専用のステップ上限を使う。
pub fn call_hook(
    module: &Module,
    checked: &Checked,
    schema: &Schema,
    evaluated: &Evaluated,
    limits: &Limits,
    hook: &str,
    arg: Value,
) -> Result<Option<Value>, Diagnostic> {
    let span = policy_span(module);
    let err = |code: Code, message: String| {
        diagnostic(
            module,
            eval::EvalError {
                code,
                span: Some(span),
                message,
            },
        )
    };
    let Some(Value::Object(policy)) = evaluated.globals.get("policy") else {
        return Ok(None);
    };
    let Some(Value::Function(f)) = policy.get(hook) else {
        return Ok(None);
    };
    let param = match &checked.policy_type {
        Some(Ty::Object(fields)) => match fields.get(hook).map(|f| &f.ty) {
            Some(Ty::Function(params, _)) => params.first().cloned(),
            _ => None,
        },
        _ => None,
    };
    if let Some(param) = &param
        && !conforms(&arg, param)
    {
        return Err(err(
            Code::HookArgument,
            format!("フック `{hook}` の引数が `{param}` に合わない"),
        ));
    }
    let mut ev = eval::Evaluator::new(module, schema, *limits, limits.max_hook_steps);
    ev.globals = evaluated.globals.clone();
    ev.reset_budget(limits.max_hook_steps);
    ev.call(f, vec![arg], span)
        .map(Some)
        .map_err(|e| diagnostic(module, eval::fill(e, span)))
}
