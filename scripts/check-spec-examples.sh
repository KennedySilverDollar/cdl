#!/usr/bin/env bash
# scripts/check-spec-examples.sh — docs/SPEC.md の中の CDL のコード例が、実装で通ることを確かめる。
#
# 対象: ```tsx / ```ts のコードブロックのうち、`export default` か `export const policy` を含み、
#       `export declare`（ホストの型宣言）を含まないもの。
# 確かめること:
#   - 先頭の文が "use cdl 1"; であること（SPEC 第 13 章: edition 宣言は必須）
#   - cdl check が終了コード 0 で通ること
# 書き込むのは target/spec-examples/ だけ（毎回同じ名前で作り直す生成物。削除はしない）。
#
# 環境変数:
#   SPEC            既定 docs/SPEC.md
#   CDL_BIN         既定 target/debug/cdl
#   CDL_CHECK_FLAGS 既定 "--stage syntax"（構文だけ）。
#                   ホストのスキーマも含めて通すには "--preset contrast"
# 終了コード: 0 = OK / 1 = NG あり / 2 = 未実行
set -u
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" || exit 1

SPEC=${SPEC:-docs/SPEC.md}
CDL_BIN=${CDL_BIN:-target/debug/cdl}
FLAGS=${CDL_CHECK_FLAGS:---stage syntax}
OUT=${OUT_DIR:-target/spec-examples}
fail=0

ok() { printf '  OK   %s\n' "$1"; }
ng() { printf '  NG   %s\n' "$1"; fail=1; }

[ -f "$SPEC" ] || { echo "未実行: $SPEC がない"; exit 2; }

if [ ! -x "$CDL_BIN" ]; then
  echo "$CDL_BIN がないので、cargo build -p cdl-cli を実行します"
  cargo build -q -p cdl-cli || { echo "未実行: cdl-cli がビルドできない"; exit 2; }
fi
[ -x "$CDL_BIN" ] || { echo "未実行: $CDL_BIN が見つからない"; exit 2; }

mkdir -p "$OUT" || exit 2

count=$(awk -v out="$OUT" '
  /^```(tsx|ts)[ \t]*$/ { inb = 1; buf = ""; next }
  /^```[ \t]*$/ {
    if (inb) {
      if (buf ~ /export default|export const policy/ && buf !~ /export declare/) {
        n++
        f = sprintf("%s/%02d.cdl", out, n)
        printf "%s", buf > f
        close(f)
      }
      inb = 0
    }
    next
  }
  inb { buf = buf $0 "\n" }
  END { print n + 0 }
' "$SPEC")

if [ "$count" -eq 0 ]; then
  echo "NG: $SPEC に、確かめられる CDL の例が見つからない"
  exit 1
fi
echo "SPEC の例: $count 件（$OUT/ に書き出し）"

for i in $(seq 1 "$count"); do
  f=$(printf '%s/%02d.cdl' "$OUT" "$i")
  first=$(head -n 1 "$f")
  if [ "$first" != '"use cdl 1";' ]; then
    ng "$f: 先頭が \"use cdl 1\"; ではない（SPEC の例に edition 宣言がない）"
    continue
  fi
  # shellcheck disable=SC2086
  out=$("$CDL_BIN" check $FLAGS "$f" 2>&1)
  rc=$?
  if [ "$rc" = 0 ]; then
    ok "$f"
  else
    ng "$f: cdl check が失敗（exit=$rc）"
    printf '%s\n' "$out" | head -n 8 | sed 's/^/         /'
  fi
done

echo
if [ "$fail" = 0 ]; then echo "check-spec-examples: すべて OK"; else echo "check-spec-examples: NG あり"; fi
exit "$fail"
