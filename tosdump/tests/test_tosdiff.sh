#!/bin/sh
#
# test_tosdiff.sh -- drives the comparator against synthetic dumps (spec T1).
#
# usage: test_tosdiff.sh <build-dir>
set -u

BUILD="${1:?usage: test_tosdiff.sh <build-dir>}"
HERE=$(cd "$(dirname "$0")" && pwd)
TMP="$BUILD/t"
TOSDIFF="$BUILD/tosdiff"
MKDUMP="$BUILD/mkdump"
CASES="$HERE/fixture-vdi.tsv"
CASES2="$HERE/fixture-vdi-other.tsv"

rm -rf "$TMP"
mkdir -p "$TMP"

fails=0
check() {   # check <description> <expected-status> <actual-status>
	if [ "$2" != "$3" ]; then
		echo "  FAIL $1: expected exit $2, got $3"
		fails=$((fails + 1))
	fi
}
grep_out() {   # grep_out <description> <file> <pattern>
	if ! grep -q -- "$3" "$2"; then
		echo "  FAIL $1: output does not mention '$3'"
		echo "  --- output ---"
		sed 's/^/  | /' "$2"
		echo "  --------------"
		fails=$((fails + 1))
	fi
}

echo "tosdiff against synthetic dumps"

# --- reference and an identical copy -----------------------------------
"$MKDUMP" "$TMP/a.bin" "$CASES"                     || { echo "  FAIL mkdump a"; fails=$((fails+1)); }
"$MKDUMP" "$TMP/b.bin" "$CASES"                     || { echo "  FAIL mkdump b"; fails=$((fails+1)); }

# --- validation ---------------------------------------------------------
"$TOSDIFF" --validate "$TMP/a.bin" >"$TMP/val.out" 2>&1
check "validate accepts a well-formed dump" 0 $?
grep_out "validate names the area" "$TMP/val.out" "VDI"

"$TOSDIFF" --validate "$TMP/a.bin" "$CASES" >"$TMP/val2.out" 2>&1
check "validate accepts dump + matching case table" 0 $?

"$TOSDIFF" --validate "$TMP/a.bin" "$CASES2" >"$TMP/val3.out" 2>&1
check "validate rejects a mismatched case table" 2 $?
grep_out "validate explains the CRC mismatch" "$TMP/val3.out" "CRC"

head -c 100 "$TMP/a.bin" > "$TMP/trunc.bin"
"$TOSDIFF" --validate "$TMP/trunc.bin" >"$TMP/val4.out" 2>&1
check "validate rejects a truncated dump" 2 $?

# --- identical dumps compare clean --------------------------------------
"$TOSDIFF" "$CASES" "$TMP/a.bin" "$TMP/b.bin" >"$TMP/same.out" 2>&1
check "identical dumps report no differences" 0 $?
grep_out "clean run says so" "$TMP/same.out" "0 of 3"

# --- one poisoned field differs, and tosdiff names it -------------------
# case index 1 keeps intout[3] poisoned in b, holds 0x0555 in a.
"$MKDUMP" "$TMP/c.bin" "$CASES" intout 1 3 -8531   # 0xDEAD as a signed int16
"$TOSDIFF" "$CASES" "$TMP/a.bin" "$TMP/c.bin" >"$TMP/diff.out" 2>&1
check "a single differing field is reported" 1 $?
grep_out "the field is named, not an offset" "$TMP/diff.out" "intout\[3\]"
grep_out "the case is named by id"           "$TMP/diff.out" "case 4"
grep_out "the case comment is shown"         "$TMP/diff.out" "v_line placeholder"
grep_out "both values are shown"             "$TMP/diff.out" "DEAD"
grep_out "summary counts the cases"          "$TMP/diff.out" "1 of 3"
if grep -q "intout\[0\]" "$TMP/diff.out"; then
	echo "  FAIL equal elements must not be reported"
	fails=$((fails + 1))
fi

# --- an input array that the call must not touch ------------------------
"$MKDUMP" "$TMP/d.bin" "$CASES" ptsin 0 0 999
"$TOSDIFF" "$CASES" "$TMP/a.bin" "$TMP/d.bin" >"$TMP/diff2.out" 2>&1
check "an input-array difference is reported" 1 $?
grep_out "ptsin is named" "$TMP/diff2.out" "ptsin\[0\]"

# --- refusals (spec section 3) ------------------------------------------
"$MKDUMP" "$TMP/crc.bin" "$CASES" --crc 12345678
"$TOSDIFF" "$CASES" "$TMP/a.bin" "$TMP/crc.bin" >"$TMP/ref1.out" 2>&1
check "differing case-table CRC is refused" 2 $?
grep_out "refusal mentions the CRC" "$TMP/ref1.out" "CRC"

"$MKDUMP" "$TMP/area.bin" "$CASES" --area 5
"$TOSDIFF" "$CASES" "$TMP/a.bin" "$TMP/area.bin" >"$TMP/ref2.out" 2>&1
check "differing area is refused" 2 $?
grep_out "refusal mentions the area" "$TMP/ref2.out" "area"

# --- an AES dump: the comment column is column SEVEN, not column five ----
# The stub AES dump and the table it came from are written by test_runner_aes
# earlier in the suite.  A single patched output word is all that is needed to
# make tosdiff print a case header, which is the thing under test here: with
# the five column VDI rule the "comment" would start at the n_addrout column
# and swallow the int_in values.
AESBIN="$BUILD/stub-aes.bin"
AESTSV="$BUILD/stub-aes.tsv"
if [ -f "$AESBIN" ] && [ -f "$AESTSV" ]; then
	cp "$AESBIN" "$TMP/aes-b.bin"
	# record 0, int_out[3]: 32 byte header + offset 76 + 3*2 = 114
	printf '\377\377' | dd of="$TMP/aes-b.bin" bs=1 seek=114 count=2 \
	    conv=notrunc >/dev/null 2>&1

	"$TOSDIFF" "$AESTSV" "$AESBIN" "$TMP/aes-b.bin" >"$TMP/aes.out" 2>&1
	check "an AES difference is reported" 1 $?
	grep_out "the AES field is named"  "$TMP/aes.out" "int_out\[3\]"
	grep_out "the AES case is named by id" "$TMP/aes.out" "case 2"
	grep_out "the AES comment is the comment" "$TMP/aes.out" \
	    "wind_calc WC_WORK"
	if grep -q "0x0FEF" "$TMP/aes.out"; then
		echo "  FAIL the AES case header holds the int_in column, so the"
		echo "       comment was taken with the five column VDI rule"
		fails=$((fails + 1))
	fi

	"$TOSDIFF" --validate "$AESBIN" "$AESTSV" >"$TMP/aesval.out" 2>&1
	check "validate accepts the AES dump against its table" 0 $?
	grep_out "validate names the AES area" "$TMP/aesval.out" "AES"
else
	echo "  FAIL stub AES dump missing: $AESBIN / $AESTSV"
	fails=$((fails + 1))
fi

if [ "$fails" -eq 0 ]; then
	echo "  ok"
	exit 0
fi
echo "  $fails failure(s)"
exit 1
