# Errata to the frozen case tables

The `.tsv` case tables in this directory are bound by CRC to the dumps
recorded from them (`tosdump/reference/`, `measurement-*/`). A change to a
table — even to a `#` comment — invalidates that binding, so a table that
turns out to say something wrong in its comments is **not edited**. The
correction lives here, keyed by table and line, and a future table revision
carries it into its text.

Why here and not in `COVERAGE-AES.md`: that file is the design and
prediction record of the rounds; an erratum is a statement about a frozen
artefact, and it must be found from the artefact, not from the round that
produced it. One file for all tables keeps that lookup in one place.

---

## `aes-r2b-rlist.tsv` (CRC `B4CC4A5A`, sha256 `a81a7a6c…`) — header comment, item 1

**Where:** lines 69–72, the sentence *"g_x and g_y keep whatever the last
rc_copy left, and if the list was empty from the start they keep whatever
the caller's buffer held. The poison is what makes that visible."*

**What is wrong:** the two words at the end of the list do **not** come
from the caller's buffer. They come from EmuTOS's dispatcher: `xif` keeps
a **local, uninitialised** `WORD int_out[O_SIZE]` (`aes/gemsuper.c:432`),
`crysbind` writes `RET_CODE` into it and `wm_get` writes only `g_w`/`g_h`
for the end of the list (`aes/gemwmlib.c:1240`), and `xif` copies
`OUT_LEN` words back **unconditionally** (`gemsuper.c:444-445`). The caller's
poisoned buffer is therefore overwritten in all five words, and what
lands in `int_out[1..2]` is whatever the supervisor stack held under
`int_out` at that moment.

**Evidence (A15, 2026-09-10, run on the TT):** all 15 records of the
table; the end-of-list records return `1, 31, 63, 0, 0` (cases 127, 133)
and `1, 0, 19, 0, 0` (case 204) in the **same** run — two different
value pairs from one poison pattern, which a "kept from the caller"
reading cannot produce — and one value survives a case boundary and a
reset. Table in `TASK/A15-STATUS-UNFINISHED.MD` (rows 127/133/204).

**What follows for reading the dumps:** the poison shows **that** the
caller is overwritten, not what the caller keeps. The recorded values
`31, 63` / `0, 19` are stack contents, not an answer, and must not be
used as expectations; the reference harness states the two words itself
(`tos-api/tests/aes_reference/harness.rs`, the `WF_OWNER`/end-of-list
notes). The dumps remain valid as recorded.

**Spec:** `docs/api/aes/window-graphics.md`, "The rectangle-list
iteration" and "Edge cases discovered in source" (A18, 2026-09-19) state
the two-word rule with `gemwmlib.c:1240`.
