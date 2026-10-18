# `Crawio` (GEMDOS 0x06)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/console.c:441` (`xrawio`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the `0xFF` read-vs-write sentinel, the **non-blocking**
> read (0 if nothing available), the write returning `0`, and the fact that
> `Crawio` is **never** redirected to a file. Reference: `constat`/`getch`.

## EmuTOS source

**Primary location:** `bdos/console.c:441` — `long xrawio(int parm)`

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x06` =
`xrawio`, **`stdio_typ = 0`**, `wparms = 1` (`bdos/bdosmain.c:161`). Because
`stdio_typ` is `0`, the dispatcher's standard-handle interception is **skipped
for `Crawio`** — the gate is `if (typ && fn && …)` and `typ == 0`
(`bdos/bdosmain.c:498`). `Crawio` is therefore never redirected **to a file**;
it talks to the BIOS device `HXFORM(run->p_uft[0|1])` = `3 + h`
(`bdos/console.c:447,451`). With the default handles that is the console
(`h = -1` → device 2). **Corrected 2026-10-18 (PROC04 point 1):** this line used
to say "always the console"; when stdin/stdout are forced to a file handle
`h > 0`, EmuTOS addresses BIOS device `3 + h` (no console, no file) — NewTOS
does the same; with no such device the write goes nowhere and the read answers
0.
The `case 6:` arm inside that interception switch (`bdos/bdosmain.c:505-508`) is
**unreachable** dead code for the normal dispatch of function 6.

**Helpers called:**
- `bdos/console.c:143` — `constat(h)` — non-blocking availability probe
  (typeahead or `Bconstat`), for handles 1/2 only.
- `bdos/console.c:351` — `getch(h)` — dequeue typeahead, else `Bconin(h)`.
- BIOS `Bconout(2,parm)` (0x03) — write side.

**Headers / type definitions used:**
- `bdos/console.h:23,36` — `H_Console`, `HXFORM(h) = 3+h`.

**System variables / globals read:**
- `run->p_uft[0]` (`bdos/console.c:447`) — stdin device, for the read side.
- `run->p_uft[1]` (`bdos/console.c:451`) — stdout device, for the write side.
- `con_typeahead` (via `constat`/`getch`).

**System variables / globals written:**
- `con_typeahead.remove` (via `getch`), `glbcolumn` **not** touched (raw write
  goes straight to `Bconout`, bypassing `conout`).

**Build configuration affecting this function:** none directly.

---

## Source extraction summary

`Crawio` is raw, un-cooked console I/O selected by the argument `parm`:

- **`parm == 0xFF` → input.** `xrawio` probes the stdin device with `constat`;
  if a character is available it returns `getch` (typeahead or `Bconin`),
  otherwise it returns `0L` **immediately**. This is **non-blocking** — a very
  common documentation trap (see D-002). No Ctrl-C/Ctrl-S/Ctrl-X handling and
  no echo: even an ETX (0x03) is returned to the caller verbatim.
- **`parm != 0xFF` → output.** `xrawio` writes `parm` to the stdout device with
  `Bconout(HXFORM(p_uft[1]), parm)` directly — no tab expansion, no `conbrk`
  flow control, no `glbcolumn` update — and returns `0` (the source comment
  calls it "dummy").

The comparison is against the full WORD value `0x00FF`: `parm == 0xFF`. A value
like `0x80FF` is **not** read mode; it is treated as a write of a byte.

Unlike almost every other console function, `Crawio` has `stdio_typ = 0`, so the
GEMDOS dispatcher never reroutes it to a file — it is genuinely raw and always
hits the console BIOS.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x06
- **Stack frame on entry (m68k):**
  ```
  +2: parm (word)
   0: function_id (word) = 0x0006
  ```
- **Total parameter bytes:** 4 → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x06` in `x8`; `parm` in `x0`; LONG result
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `parm` | `WORD` (16) — `0x00FF` = read, else write |

---

## Inputs

| Position | Name   | Type | Size | Source location | Notes |
|----------|--------|------|------|-----------------|-------|
| arg0     | `parm` | `WORD` | 2  | `bdos/console.c:441` | `0x00FF` ⇒ read; any other value ⇒ write that byte |

The dispatcher passes `pw[1]` as a WORD; `xrawio` compares it to `0xFF`
(i.e. `0x00FF`), so only the exact value `0x00FF` selects read.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:**
  - Read: the packed console LONG (scancode in bits 16-23, ASCII in bits 0-7),
    **or** `0L` if no character was available.
  - Write: always `0` (`bdos/console.c:452`).
- **Source location:** `bdos/console.c:448` (read), `:452` (write).

### Side effects on global state

- Read: consumes one typeahead/BIOS character.
- Write: none in the BDOS layer (`glbcolumn` untouched).

### Side effects on hardware

- Read: reads keyboard/ACIA input. Write: writes one byte to the console.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Read, no character available | `bdos/console.c:448` | `0L` |
| Write | `bdos/console.c:452` | `0` |

**Error paths not in Compendium:** the ambiguous `0L` on read (means both "no
character" and "read a NUL byte").

---

## Verification criteria

- **Read, character available:** with 'A' pending, `Crawio(0xFF)` returns a LONG
  with low byte `0x41`.
- **Read, nothing available:** `Crawio(0xFF)` returns `0L` immediately
  (non-blocking) — the key difference from `Crawcin` (0x07), which blocks.
- **Raw read passes Ctrl-C:** a pending Ctrl-C is returned as `0x03`; the
  process is **not** terminated.
- **Write:** `Crawio('A')` writes `0x41` to the console and returns `0`.
- **Write is raw:** `Crawio('\t')` writes a literal 0x09 (no space expansion,
  no column tracking).
- **Never redirected:** with stdin/stdout forced to files, `Crawio` still reads
  from / writes to the console BIOS — the dispatcher does not intercept it
  (`stdio_typ = 0`).

---

## Edge cases discovered in source

1. **Read is non-blocking.** `constat(i) ? getch(i) : 0L` — no character means
   an immediate `0L` (D-002). Distinct from `Crawcin`'s blocking read.

2. **`Crawio` is never redirected to a file.** `funcs[6].stdio_typ == 0`, so the
   dispatcher's interception (`if (typ && fn && …)`) is skipped, and the
   `case 6:` arm inside it is unreachable for function 6
   (`bdos/bdosmain.c:498-508`). (Correction: an earlier draft claimed the
   dispatcher intercepts `Crawio` and does file read/write with `EOF_INDICATOR`
   — it does not; see D-028.)

3. **`0xFF` test is on the full WORD.** `parm == 0xFF` means `0x00FF`; `0x80FF`
   is a write of byte `0xFF`, not a read. EmuTOS narrows the WORD; it does not
   sign-extend the classic Atari-TOS way.

4. **Fully raw.** Read side: no Ctrl-C/Ctrl-S/echo. Write side: no tab
   expansion, no `conbrk` pause, no `glbcolumn` update — `Bconout` is called
   directly, not through `conout`.

5. **Read shares the typeahead buffer.** `getch` prefers `con_typeahead`, so
   bytes an earlier `Cconin`/`Cnecin` `conbrk` queued will be returned here.

6. **Read/write use different handles.** Read uses `p_uft[0]` (stdin); write
   uses `p_uft[1]` (stdout).

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS console I/O `Bconstat(h)`, `Bconin(h)`, `Bconout(h,…)`.
- `stdhdl_init()` (sets `p_uft[0]`/`p_uft[1]` to `H_Console`).
- `con_typeahead` (read path).

### Used by

- Programs implementing their own line discipline (shells, terminal emulators)
  that need un-cooked, non-blocking I/O.

---

## Related data structures

- `docs/api/bios/01-bconstat.md`, `02-bconin.md`, `03-bconout.md`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents read mode as
  "next character, or 0 if none" — which is actually the EmuTOS non-blocking
  behaviour, though often misread as blocking; D-002.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred. (Describes non-blocking
  read — consistent.)

---

## Configuration / version notes

- Behaviour is configuration-independent. Read non-blocking, write returns `0`,
  never redirected — all EmuTOS `1a7484ab`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-02: `Crawio` write returns `0` (D-011); an early drift to returning the
byte was corrected.
