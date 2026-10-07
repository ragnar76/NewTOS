# conservative.py <tree> -- scratch-only: make every rights change mark the
# stage-3b set (non-owner pattern + every live slot + private copies) and
# disable batching, i.e. the work the shortcut avoids. Correctness stays (the
# verification mode still checks every sync); only the work bounds of
# tos-api/tests/memprot_fast.rs must turn red.
import sys, pathlib
p = pathlib.Path(sys.argv[1]) / "tos-api/src/bdos/process/memprot.rs"
s = p.read_text()
old = "    if !enabled() || hi <= lo || (slots == 0 && !shared && !privates) {\n        return;\n    }\n"
new = "    if !enabled() || hi <= lo {\n        return;\n    }\n    let (slots, shared, privates) = (ALL_SLOTS, true, true); // RED-CONTROL\n    let _ = (slots, shared, privates);\n"
assert s.count(old) == 1; s = s.replace(old, new)
old = "    DEFER.fetch_add(1, Ordering::Relaxed);\n    Batch { active: true }"
new = "    Batch { active: false } // RED-CONTROL: no batching"
assert s.count(old) == 1; s = s.replace(old, new)
p.write_text(s)
print("conservative marking applied to", p)
s = p.read_text()
for old, new in [
    ("        let shared = other_flags(old) != other_flags(mode);\n", "        let shared = true; // RED-CONTROL\n"),
    ("            if slots != 0 {\n                if let Some((s, e)) = block_at(loc) {", "            if true { // RED-CONTROL\n                if let Some((s, e)) = block_at(loc) {"),
    ("        if hb != ha {\n", "        if true { // RED-CONTROL\n"),
]:
    assert s.count(old) == 1, old
    s = s.replace(old, new)
p.write_text(s)
print("skip shortcuts removed")
