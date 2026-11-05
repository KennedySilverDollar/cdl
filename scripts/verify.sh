#!/usr/bin/env bash
# scripts/verify.sh — 手動確認用。リポジトリのルートで実行する。
#
#   scripts/verify.sh          test / fmt / clippy と、目視用の grep を実行する
#   NOLIMIT=1 scripts/verify.sh  メモリ上限（6G）をかけずに実行する
#   MEM=8G scripts/verify.sh     メモリ上限を変える
#
# ファイルを削除・上書きしない（生成物は target/ に出るだけ）。
# 結果は OK / NG / 未実行 で表示する。未実行は成功ではない。
# 目視用の grep は、見つかっても NG にしない。結果を見て、自分で判断する。
# 終了コード: 0 = NG なし / 1 = NG あり
set -u
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" || exit 1

MEM=${MEM:-6G}
names=(); results=()

# メモリ上限つきで実行する（PC 全体が固まるのを避ける）。使えなければ、そのまま実行する。
limited() {
  if [ "${NOLIMIT:-0}" != 1 ] && command -v systemd-run >/dev/null 2>&1; then
    systemd-run --user --scope --quiet -p MemoryMax="$MEM" -p MemorySwapMax=0 "$@"
  else
    "$@"
  fi
}

step() { # step <表示名> <コマンド...>
  local name=$1; shift
  printf '\n=== %s ===\n' "$name"
  "$@"
  local rc=$?
  names+=("$name")
  if [ "$rc" = 0 ]; then results+=("OK"); else results+=("NG(exit=$rc)"); fi
}

step_if() { # step_if <表示名> <サブコマンド> <コマンド...>: cargo のサブコマンドが無ければ未実行
  local name=$1 sub=$2; shift 2
  if ! cargo "$sub" --version >/dev/null 2>&1; then
    printf '\n=== %s ===\ncargo %s が使えません\n' "$name" "$sub"
    names+=("$name"); results+=("未実行")
    return
  fi
  step "$name" "$@"
}

step "test" limited cargo test --workspace
step_if "fmt" fmt cargo fmt --check
step_if "clippy" clippy limited cargo clippy --workspace --all-targets -- -D warnings

echo
echo "=== 目視用（見つかっても NG にしない）==="

echo "--- unwrap / expect / panic 系（テストの中は問題なし。それ以外を確認）"
grep -rnE '\.unwrap\(\)|\.expect\(|panic!\(|unreachable!\(|todo!\(|unimplemented!\(' crates/*/src 2>/dev/null | head -n 30
echo "--- oxc への参照（cdl-syntax の中だけにあれば問題なし）"
grep -rn 'oxc' crates --include=Cargo.toml --include='*.rs' 2>/dev/null | grep -v '^crates/cdl-syntax/' | head -n 20
echo "--- HashMap / HashSet（決定性のため、sema / eval / host では避ける）"
grep -rnE 'HashMap|HashSet' crates/cdl-sema/src crates/cdl-eval/src crates/cdl-host/src 2>/dev/null | head -n 20
echo "--- SPEC の例に edition 宣言があるか（第 8 章のポリシー例など）"
if [ -f docs/SPEC.md ]; then
  grep -n -B1 'import type { Policy' docs/SPEC.md | head -n 6
else
  echo "docs/SPEC.md がない"
fi

echo
echo "================ 結果 ================"
ng=0
for i in "${!names[@]}"; do
  printf '  %-10s %s\n' "${names[$i]}" "${results[$i]}"
  case ${results[$i]} in NG*) ng=1 ;; esac
done
echo "======================================"
for r in "${results[@]}"; do
  [ "$r" = "未実行" ] && { echo "未実行があります。未実行は成功ではないので、報告には「未実行」と書く。"; break; }
done
exit "$ng"