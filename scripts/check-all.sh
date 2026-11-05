#!/usr/bin/env bash
# scripts/check-all.sh — 変更を出す前のチェックを、まとめて実行する。
#
# 実行するもの:
#   rules          scripts/check-rules.sh
#   fmt            cargo fmt --check
#   clippy         cargo clippy --workspace --all-targets -- -D warnings
#   test           cargo test --workspace
#   conformance    scripts/check-conformance.sh
#   spec-examples  scripts/check-spec-examples.sh
#   kani           cargo kani（時間がかかるので WITH_KANI=1 のときだけ）
#
# 結果は OK / NG / 未実行 の 3 種類に分けて表にする。ツールが入っていないものは「未実行」であって、
# 成功ではない。未実行が 1 つでも残る間は、「完成」と言えない。
# どのコマンドも、ファイルを削除・上書きしない（生成物は target/ に出るだけ）。
#
# 終了コード: 0 = すべて OK / 1 = NG あり / 2 = NG はないが未実行あり
set -u
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" || exit 1

names=()
results=()
notes=()

record() { names+=("$1"); results+=("$2"); notes+=("${3:-}"); }

# run_step <表示名> <コマンド...>: 終了コード 0 = OK、2 = 未実行（スクリプトの約束）、それ以外 = NG
run_step() {
  local name=$1; shift
  printf '\n=== %s ===\n' "$name"
  "$@"
  local rc=$?
  case $rc in
    0) record "$name" "OK" ;;
    2) record "$name" "未実行" "スクリプトが前提不足と判断" ;;
    *) record "$name" "NG" "exit=$rc" ;;
  esac
}

# run_cargo <表示名> <cargo のサブコマンド> <コマンド...>: 入っていなければ未実行
run_cargo() {
  local name=$1 sub=$2; shift 2
  printf '\n=== %s ===\n' "$name"
  if ! cargo "$sub" --version >/dev/null 2>&1; then
    echo "cargo $sub が使えません（コンポーネントが入っていない）"
    record "$name" "未実行" "cargo $sub が入っていない"
    return
  fi
  "$@"
  local rc=$?
  if [ "$rc" = 0 ]; then record "$name" "OK"; else record "$name" "NG" "exit=$rc"; fi
}

run_step "rules" bash scripts/check-rules.sh
run_cargo "fmt" fmt cargo fmt --check
run_cargo "clippy" clippy cargo clippy --workspace --all-targets -- -D warnings
run_step "test" cargo test --workspace
run_step "conformance" bash scripts/check-conformance.sh
run_step "spec-examples" bash scripts/check-spec-examples.sh

if [ "${WITH_KANI:-0}" = 1 ]; then
  run_cargo "kani" kani cargo kani
else
  printf '\n=== kani ===\nスキップ（実行するには WITH_KANI=1）\n'
  record "kani" "未実行" "WITH_KANI=1 で実行"
fi

echo
echo "================ 結果 ================"
ng=0; skipped=0
for i in "${!names[@]}"; do
  printf '  %-14s %-6s %s\n' "${names[$i]}" "${results[$i]}" "${notes[$i]}"
  [ "${results[$i]}" = "NG" ] && ng=1
  [ "${results[$i]}" = "未実行" ] && skipped=1
done
echo "======================================"

if [ "$ng" = 1 ]; then
  echo "NG があります。"
  exit 1
fi
if [ "$skipped" = 1 ]; then
  echo "NG はありませんが、未実行があります。未実行が残る間は、完成とは言えません。"
  exit 2
fi
echo "すべて OK です。"
exit 0
