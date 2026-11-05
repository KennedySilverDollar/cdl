#!/usr/bin/env bash
# scripts/check-rules.sh — CLAUDE.md の機械的な規則を、ソースと Cargo.toml から静的に確かめる。
#
# 読み取り専用: ファイルを作成・変更・削除しない。
# 前提: crates/<name>/ の構成（cdl-diag cdl-syntax cdl-sema cdl-eval cdl-host cdl-cli）。
#       テスト用コードは、各ファイルの最初の #[cfg(test)] または #[cfg(kani)] より後ろにある。
# 終了コード: 0 = すべて OK / 1 = NG あり
set -u
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" || exit 1

CRATE_DIR=${CRATE_DIR:-crates}
CRATES="cdl-diag cdl-syntax cdl-sema cdl-eval cdl-host cdl-cli"
fail=0

ok()   { printf '  OK   %s\n' "$1"; }
ng()   { printf '  NG   %s\n' "$1"; fail=1; }
show() { printf '%s\n' "$1" | head -n 5 | sed 's/^/         /'; }

# crate の .rs ファイル一覧
rs_files() { find "$CRATE_DIR/$1/src" -name '*.rs' -type f 2>/dev/null | LC_ALL=C sort; }

# テスト用コード（最初の #[cfg(test)] / #[cfg(kani)] 以降）と、コメント行を除き、
# 「ファイル:行: 内容」の形で出す。
non_test_src() {
  awk '
    /^[[:space:]]*#\[cfg\((test|kani)\)\]/ { exit }
    /^[[:space:]]*\/\//                    { next }
    { print FILENAME ":" FNR ": " $0 }
  ' "$1"
}

# scan <crate> <ERE>: テスト以外のソースで ERE に当たる行
scan() {
  local f
  for f in $(rs_files "$1"); do
    non_test_src "$f" | grep -E -- "$2"
  done
}

# Cargo.toml の [dependencies] / [dev-dependencies] / [build-dependencies] に書かれた名前
deps_of() {
  awk '
    /^\[(dev-|build-)?dependencies\.[A-Za-z0-9_-]+\]/ {
      s = $0; sub(/^\[(dev-|build-)?dependencies\./, "", s); sub(/\].*$/, "", s); print s; sec = 0; next
    }
    /^\[/ { sec = ($0 ~ /^\[(dev-|build-)?dependencies\]/); next }
    sec && /^[A-Za-z0-9_-]+[ .=]/ { match($0, /^[A-Za-z0-9_-]+/); print substr($0, RSTART, RLENGTH) }
  ' "$1"
}

has_workspace_lints() {
  awk '/^\[lints\]/ {f=1; next} /^\[/ {f=0} f && /workspace[ ]*=[ ]*true/ {ok=1} END {exit !ok}' "$1"
}

allowed_internal() {
  case $1 in
    cdl-diag)   echo "" ;;
    cdl-syntax) echo "cdl-diag" ;;
    cdl-sema)   echo "cdl-diag cdl-syntax" ;;
    cdl-eval)   echo "cdl-diag cdl-syntax cdl-sema" ;;
    cdl-host)   echo "cdl-diag cdl-syntax cdl-sema cdl-eval" ;;
    cdl-cli)    echo "cdl-diag cdl-host" ;;
  esac
}

allowed_external() {
  case $1 in
    cdl-syntax)       echo "oxc_parser oxc_allocator oxc_ast oxc_span oxc_diagnostics proptest" ;;
    cdl-diag|cdl-cli) echo "serde serde_json proptest" ;;
    *)                echo "proptest" ;;
  esac
}

in_list() { # in_list <word> <list...>
  local w=$1; shift
  local x
  for x in "$@"; do [ "$x" = "$w" ] && return 0; done
  return 1
}

echo "[1] クレートの存在"
for c in $CRATES; do
  if [ -d "$CRATE_DIR/$c/src" ]; then ok "$c"; else ng "$c: $CRATE_DIR/$c/src がない"; fi
done

echo "[2] #![forbid(unsafe_code)]"
ws_forbid=0
grep -Eq 'unsafe_code[ ]*=[ ]*"forbid"' Cargo.toml 2>/dev/null && ws_forbid=1
for c in $CRATES; do
  [ -d "$CRATE_DIR/$c/src" ] || continue
  root=$(ls "$CRATE_DIR/$c/src/lib.rs" "$CRATE_DIR/$c/src/main.rs" 2>/dev/null | head -n 1)
  if [ -n "$root" ] && grep -q '#!\[forbid(unsafe_code)\]' "$root"; then
    ok "$c"
  elif [ "$ws_forbid" = 1 ] && has_workspace_lints "$CRATE_DIR/$c/Cargo.toml"; then
    ok "$c（workspace の lints）"
  else
    ng "$c: #![forbid(unsafe_code)] がない"
  fi
done

echo "[3] パニックしうる呼び出し（テスト以外）"
panic_pat='\.unwrap\(\)|\.expect\(|panic!\(|unreachable!\(|todo!\(|unimplemented!\('
for c in $CRATES; do
  [ -d "$CRATE_DIR/$c/src" ] || continue
  hits=$(scan "$c" "$panic_pat")
  if [ -z "$hits" ]; then ok "$c"; else ng "$c: unwrap / expect / panic 系がある"; show "$hits"; fi
done

echo "[4] as によるキャスト（テスト以外）"
for c in $CRATES; do
  [ -d "$CRATE_DIR/$c/src" ] || continue
  hits=$(scan "$c" '[^A-Za-z0-9_]as [A-Za-z_][A-Za-z0-9_:]*' \
    | grep -Ev '^[^ ]+ +(pub(\([a-z]+\))? +)?use |extern crate')
  if [ -z "$hits" ]; then ok "$c"; else ng "$c: as が使われている（clippy の as_conversions と同じ基準）"; show "$hits"; fi
done

echo "[5] oxc_* が cdl-syntax の外に出ていないか"
for c in $CRATES; do
  [ "$c" = "cdl-syntax" ] && continue
  [ -d "$CRATE_DIR/$c" ] || continue
  # cdl-cli/tests/architecture.rs は、この規則そのものを検査するテストで、oxc の名前を文字列として持つ。
  hits=$(grep -rn 'oxc' "$CRATE_DIR/$c" --include=Cargo.toml --include='*.rs' 2>/dev/null \
    | grep -v "^$CRATE_DIR/cdl-cli/tests/architecture\.rs:")
  if [ -z "$hits" ]; then ok "$c"; else ng "$c: oxc への参照がある"; show "$hits"; fi
done

echo "[6] 依存の向きと、許可された依存だけか"
for c in $CRATES; do
  toml="$CRATE_DIR/$c/Cargo.toml"
  [ -f "$toml" ] || continue
  bad=0
  internal=$(allowed_internal "$c")
  external=$(allowed_external "$c")
  for dep in $(deps_of "$toml" | LC_ALL=C sort -u); do
    case $dep in
      cdl-*)
        # shellcheck disable=SC2086
        if ! in_list "$dep" $internal; then ng "$c -> $dep は、許されていない向き・飛び越し"; bad=1; fi ;;
      oxc_*)
        if [ "$c" != "cdl-syntax" ]; then ng "$c -> $dep（oxc は cdl-syntax だけ）"; bad=1
        # shellcheck disable=SC2086
        elif ! in_list "$dep" $external; then ng "$c -> $dep は、許可済みの oxc 関連に入っていない（CLAUDE.md を確認）"; bad=1; fi ;;
      *)
        # shellcheck disable=SC2086
        if ! in_list "$dep" $external; then ng "$c -> $dep は、許可済みの依存にない（足す前に確認）"; bad=1; fi ;;
    esac
  done
  [ "$bad" = 0 ] && ok "$c"
done

echo "[7] syntax / sema / eval が fs・env・time に触れていないか"
io_pat='std::(fs|env|time|process|net)|SystemTime|Instant::now|thread::sleep'
for c in cdl-syntax cdl-sema cdl-eval; do
  [ -d "$CRATE_DIR/$c/src" ] || continue
  hits=$(scan "$c" "$io_pat")
  if [ -z "$hits" ]; then ok "$c"; else ng "$c: 外界に触れている"; show "$hits"; fi
done

echo "[8] HashMap / HashSet を使っていないか（決定性のため BTreeMap）"
for c in cdl-sema cdl-eval cdl-host; do
  [ -d "$CRATE_DIR/$c/src" ] || continue
  hits=$(scan "$c" 'HashMap|HashSet')
  if [ -z "$hits" ]; then ok "$c"; else ng "$c: HashMap / HashSet がある"; show "$hits"; fi
done

echo
if [ "$fail" = 0 ]; then echo "check-rules: すべて OK"; else echo "check-rules: NG あり"; fi
exit "$fail"
