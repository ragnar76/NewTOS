# TOS Reference Dumper — Task Specification

Build a set of programs that capture the **observed** behaviour of Atari TOS 3.06
on real hardware and compare it against NewTOS, so that NewTOS conformance is
verified against measured reality instead of against a reading of the EmuTOS
source.

This document is the single source of truth for these tasks. Do not infer
requirements from other documents unless this file points at them.

---

## 1. Why this exists

Today NewTOS is validated by reading the EmuTOS source. That means the
Researcher, the Developer and the Auditor can all read the same passage the same
wrong way: three roles, one failure mode. A dump taken from a real TT breaks that
loop because it is measured behaviour, not interpreted intent.

It also catches a class of bug that cannot be read out of any source at all:
a routine writing an output field it is not allowed to touch. That is an absence,
and absences are only visible if you poison the arrays first.

**Non-goals.** This is not a test suite for drawing correctness, not a
performance benchmark, and not a GEM application. It captures call-level
input/output behaviour and nothing else.

---

## 2. Hard constraint: two builds, one source

NewTOS is native AArch64 and dispatches its API through `svc`. TOS 3.06 on the TT
is m68k and dispatches through traps. The same source compiles for both, but the
resulting binaries are different objects.

Therefore:

- one shared source tree
- one shared case table format
- one shared record format
- two thin ABI backends, and nothing else may differ

If a difference in results can be explained by the backends rather than by the
API implementations, the whole exercise is worthless. Keep the backends as small
as physically possible: marshal registers, execute, return. No logic.

```
tosdump/
  cases/           # case tables (data, shared, never compiled in)
    vdi.tsv
    aes.tsv
    bios.tsv
    gemdos.tsv
  core/            # portable C: runner, poison, record writer
  abi/
    m68k_trap.c    # TOS 3.06 target
    newtos_svc.c   # NewTOS target
  host/            # comparator, runs on the development machine
  tests/           # host-side tests (see rules in section 8)
```

### 2.1 Endianness and type widths

Because two different builds are involved, the risk is no longer "can it be
built" but "do the two builds mean the same thing". Two hazards, both of which
produce differences that look like real bugs:

**Endianness.** m68k is big-endian; AArch64 normally runs little-endian. The
record format is big-endian. The AArch64 build must therefore byte-swap on write
and the m68k build must not. This is the **only** permitted asymmetry between the
two builds. Implement it as a single `store_be16` / `store_be32` pair in `core/`,
selected at compile time, and route every field through it — including reserved
fields and the header. A swap bug here silently fabricates differences or hides
them, and it will be mistaken for an API bug.

*Test for this before trusting any dump:* a host-side test that feeds a known
pattern through the record writer for both configurations and asserts the two
outputs are byte-identical.

**Type widths.** VDI and AES arrays are 16-bit words; return values are 32-bit.
Use `int16_t` and `int32_t` throughout. Never `int`, `short` or `long` — their
widths are not guaranteed to agree between the two toolchains. Do not rely on
struct layout for the record: write fields individually at explicit offsets, so
that padding and alignment rules cannot differ between targets.

---

## 3. Record format

Binary, big-endian, fixed-size records. **No formatting happens on the target.**
Every byte of interpretation happens in the host comparator, where there are
tools and a debugger. The target program must be as stupid as possible; every
line of logic there is a bug you cannot chase.

### File header — 32 bytes

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `TDMP` |
| 4 | 2 | format version (currently 1) |
| 6 | 2 | area: 1=BIOS, 2=XBIOS, 3=GEMDOS, 4=VDI, 5=AES |
| 8 | 2 | record size in bytes |
| 10 | 2 | record count |
| 12 | 4 | build id (git short hash as ASCII, zero padded) |
| 16 | 4 | case table CRC32 |
| 20 | 12 | reserved, must be zero |

The comparator **must refuse** to compare two files whose area, record size,
format version or case table CRC differ. Comparing dumps produced from different
case tables silently produces nonsense.

### VDI record — 1592 bytes

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | case id (line number in `cases/vdi.tsv`) |
| 2 | 2 | opcode |
| 4 | 4 | reserved, zero |
| 8 | 24 | `contrl[12]` after the call |
| 32 | 256 | `intin[128]` after the call |
| 288 | 512 | `ptsin[256]` after the call |
| 800 | 256 | `intout[128]` after the call |
| 1056 | 512 | `ptsout[256]` after the call |
| 1568 | 24 | `contrl[12]` **before** the call |

Input arrays are dumped **after** the call on purpose: a VDI routine is not
allowed to modify its inputs, and dumping them afterwards is what makes a
violation visible.

### AES record — 120 bytes

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | case id |
| 2 | 2 | opcode |
| 4 | 10 | `contrl[5]` after |
| 14 | 30 | `global[15]` after |
| 44 | 32 | `int_in[16]` after |
| 76 | 14 | `int_out[7]` after |
| 90 | 12 | `addr_in[3]` after |
| 102 | 4 | `addr_out[1]` after |
| 106 | 14 | reserved, zero |

### BIOS / XBIOS / GEMDOS record — 320 bytes

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | case id |
| 2 | 2 | opcode |
| 4 | 32 | input arguments, 8 longs as passed |
| 36 | 4 | return value (d0) |
| 40 | 256 | capture buffer (see case table `capture` column) |
| 296 | 24 | reserved, zero |

---

## 4. Poisoning

Before every case, fill **all** arrays and the capture buffer with the 16-bit
pattern `0xDEAD`, repeated.

This is what makes the method work. If TOS leaves a field poisoned and NewTOS
writes it, a plain byte comparison of the two dumps shows the difference with no
extra logic. Spec violations of the "wrote a field it must not touch" kind fall
out for free.

Do not skip poisoning for fields you believe are always written. That belief is
exactly what is being tested.

---

## 5. State reset

Before every case, reset the state the call could depend on. For VDI that means
at minimum `v_clrwk` plus re-applying the default attributes.

Without this, a result depends on the preceding cases and the whole dump becomes
invalid the moment somebody inserts a case in the middle. The reset sequence
lives in `core/` and is identical on both targets.

**Workstation handling for VDI:** open with `v_opnvwk` on the handle returned by
`graf_handle`. This is what real applications do, and matching application
behaviour is the point. Record the returned handle in the dump but exclude it
from comparison — handles legitimately differ between machines.

---

## 6. Case tables

Tab-separated, one case per line, `#` starts a comment. Loaded at runtime from
the same directory as the executable. **Never compiled in** — the case table must
be provably identical for both runs, and recompiling between runs makes that
unprovable.

### `cases/vdi.tsv` and `cases/aes.tsv`

```
opcode <TAB> sub <TAB> intin values (comma separated) <TAB> ptsin values <TAB> comment
```

Comparison is exact for every byte of these two areas. No exception mechanism —
if you find yourself wanting one, the case is wrong, not the comparator.

### `cases/bios.tsv` and `cases/gemdos.tsv`

```
opcode <TAB> args <TAB> retclass <TAB> capture <TAB> comment
```

`retclass` is one of:

- `EXACT` — return value must match byte for byte
- `IGNORE` — not compared (times, tick counts)
- `NONZERO` — must be non-zero on both
- `HANDLE` — must be >= 0 on both, value not compared
- `PTR_RAM` — must be 4-byte aligned and inside RAM on both, value not compared

`capture` is either `-` or `<argindex>:<length>`, meaning: after the call, copy
`length` bytes from the buffer pointed to by argument `argindex` into the record.
This is how `Fread`, `Fsfirst` and `Getcookie` results get compared even though
the pointer itself cannot be.

The lower two areas need this classification because BIOS and GEMDOS return
pointers, handles and timestamps that differ legitimately between a TT and a
Pi 4. VDI and AES do not — they are number arrays at fixed positions, which is
why they get the strict treatment.

---

## 7. Programs to build

One program per area, sharing `core/` and one `abi/` backend:

| Program | Area | Notes |
|---|---|---|
| `DUMPVDI` | VDI | build this first |
| `DUMPAES` | AES | after VDI is clean |
| `DUMPBIOS` | BIOS + XBIOS | needs `retclass` support |
| `DUMPGDOS` | GEMDOS | needs `retclass` and `capture` |

Each writes exactly one output file, named `<AREA>.BIN`, into the current
directory. Buffer records in RAM and write once at the end. Do not write
per record.

**Build VDI before AES.** The AES calls the VDI internally, so an AES difference
has ambiguous causes and a VDI difference does not. A clean VDI will make a
significant share of the AES differences disappear on its own.

The host comparator `host/tosdiff` takes two dump files plus the case table and
reports differences per case, per field, with the field named — not as a hex
offset.

---

## 8. Rules for these tasks

These exist because the failure mode in this project is drift, not lack of
ability. Every one of them is an acceptance condition, not a suggestion.

1. **Commit on every green state.** One task, one commit. The maximum damage from
   any failure is then one task.
2. **Tests are written first and must be seen failing.** A test that passes the
   moment you write it verifies nothing. Write it, run it, see red, commit it,
   and only then implement.
3. **The implementing task may not modify tests, case tables or the record
   format.** If a test appears wrong, stop and say so; do not adjust it. Changing
   the test to match the code is the single most common form of drift.
4. **Done means a command exits zero**, and the task states which command. Not
   "implement X correctly" but "`host/tosdiff` reports zero differences for
   opcode 6".
5. **The two ABI backends stay symmetric.** Any logic that appears in one and not
   the other is a bug, regardless of how well justified it looks.
6. **State what could not be verified.** If something was not run, say so
   explicitly rather than reporting success.

---

## 9. Task order

Each task ends with the stated command exiting zero, and with a commit.

**T1 — record format and comparator skeleton**
Implement the header and the three record layouts as host-side code, plus
`tosdiff`. Verify with synthetic dump files generated by the tests, not by any
target program. Include the byte-order test from section 2.1.
*Done:* `make -C host test` exits zero, and includes both a test where a single
poisoned field differs and `tosdiff` names that field, and a test asserting the
big-endian and little-endian host configurations emit byte-identical records.

**T2 — core runner and m68k backend, VDI only**
Case table loader, poison, reset, runner, record writer, `abi/m68k_trap.c`.
Cross-compile `DUMPVDI.TOS`.
*Done:* builds without warnings, and a host-side test drives the runner against a
stub backend and produces a well-formed dump file.

**T3 — capture the reference**
Run `DUMPVDI.TOS` on the TT, retrieve `VDI.BIN` via CF.
*Done:* `tosdiff --validate VDI.BIN` accepts the file and the record count
matches the case table.

**T4 — AArch64 backend**
`abi/newtos_svc.c`, same core, same case table.
*Done:* `DUMPVDI` runs under NewTOS and produces a well-formed `VDI.BIN`.

**T5 — first comparison**
*Done:* `tosdiff cases/vdi.tsv TOS306-VDI.BIN NEWTOS-VDI.BIN` runs and produces a
per-case report. Differences are expected here — the deliverable is the report,
not a zero.

**T6 — AES** — as T2 to T5 for `DUMPAES`.

**T7 — BIOS, XBIOS, GEMDOS** — adds `retclass` and `capture` to the comparator.

Capture the reference (T3) **before** measuring NewTOS (T4). Doing it the other
way round invites choosing cases that make NewTOS look good.

---

## 10. First case list

Do not start with a full opcode sweep. Take the VDI calls that QED actually
issues while starting up and opening a file — that is the path that already runs,
and it is where the current bugs are.

Obtain the list by logging the trap dispatcher under NewTOS, since the dispatcher
is yours there. Nothing needs to be modified on the TT to get this list.

That log is the input to `cases/vdi.tsv`. Expand from there once the first
comparison is clean.
