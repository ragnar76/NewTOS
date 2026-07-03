# `Setexc` (BIOS 0x05)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/bios.c:1070` (`setexc`).

## EmuTOS source

**Primary location:** `bios/bios.c:1070` — `LONG setexc(WORD num, LONG vector)`

**Helpers called:** none (a bare read-modify-write of `*(LONG *)(4L*num)`).

**Headers / type definitions used:**
- `tosvars.ld` — the low-memory vector layout (`etv_timer=0x400`,
  `etv_critic=0x404`, `etv_term=0x408`) that specific `num` values address.

**System variables / globals read/written:**
- The 4-byte LONG at address `4 * num`. For `num` in the standard vector
  range this is a CPU exception vector (`0..0xFF` → `0x000..0x3FC`) or a TOS
  system-vector (`0x100`→`0x400` etv_timer, `0x101`→`0x404` etv_critic,
  `0x102`→`0x408` etv_term).

**Build configuration affecting this function:** none — configuration- and
version-independent.

---

## Source extraction summary

`Setexc` gets/sets a raw 32-bit exception (or system) vector. It computes the
byte address `4 * num`, reads the current LONG there into `oldvector`, and —
only if `vector != -1` — overwrites it with the new value. It always returns
the previous value. `vector == -1` is therefore the "query without changing"
idiom.

There is **no range check** and **no protection**: `num` is a `WORD`, so the
address `4*num` can land anywhere in the low 256 KB (and negative `num` wraps
via unsigned addressing). Writing `num` outside the vector page clobbers
whatever system variable lives at `4*num`. This is intentional 1980s
minimalism; the caller is trusted.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x05
- **Stack frame on entry (m68k):**
  ```
  +4: vector (LONG)
  +2: num    (word)
   0: function_id (word) = 0x0005
  ```
- **Total parameter bytes:** 8
- **Native (AArch64) ABI:** ID `0x05` in `x8`; `num` in `x0`, `vector` in
  `x1`; previous vector value returned in `x0`.

  | m68k stack | AArch64 reg | Name     | Type / size |
  |------------|-------------|----------|-------------|
  | +2         | `x0`        | `num`    | `WORD` (16) |
  | +4         | `x1`        | `vector` | `LONG` (32) |

---

## Inputs

| Position | Name     | Type   | Size | Source location    | Notes |
|----------|----------|--------|------|--------------------|-------|
| arg0     | `num`    | `WORD` | 2    | `bios/bios.c:1073` | vector number; address is `4*num`. Not range-checked. |
| arg1     | `vector` | `LONG` | 4    | `bios/bios.c:1076` | new handler address, or `-1L` for query-only |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** the previous 32-bit value at `4*num`, returned always
  (whether or not a write happened).
- **Source location:** `bios/bios.c:1074,1079`.

### Side effects on global state

- Writes `*(LONG *)(4*num) = vector` when `vector != -1`
  (`bios/bios.c:1076-1078`). No other state.

### Side effects on hardware

None directly — but installing a CPU-exception vector changes which handler
the CPU dispatches to on the corresponding fault/interrupt.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (none — the function cannot fail) | `bios/bios.c:1070` | n/a |

An out-of-range `num` silently writes outside the vector page and returns the
prior contents of that address. There is no error signalling.

**Error paths not in Compendium:** none; Compendium likewise documents no
error.

---

## Verification criteria

- **Nominal set + return old:** `Setexc(0x102, newaddr)` returns the previous
  `etv_term` value (at `0x408`) and stores `newaddr` there.
- **Query only:** `Setexc(0x102, -1)` returns the current `etv_term` value and
  leaves it unchanged.
- **`-1` is never installed:** even if a caller passes `vector == -1`, no
  write occurs; the vector cannot be *set* to `-1` via this call.

---

## Edge cases discovered in source

1. **`vector == -1` is a magic "don't write" sentinel**, not a real address.
   The install idiom is: `old = Setexc(num, -1)` to read, then
   `Setexc(num, myhandler)` to install and chain via `old`.

2. **No range check on `num`.** `num > 0xFF` addresses TOS system-vectors
   (`0x100`→etv_timer, `0x101`→etv_critic, `0x102`→etv_term); larger values
   clobber arbitrary low memory. `num` is a signed `WORD` but is used as
   `4L*num`, so negative values compute negative addresses.

3. **Read-modify-write is a single LONG store.** On m68k the store is atomic
   against equal-or-lower-priority interrupts; there is no explicit locking.

4. **The standard TOS system-vector numbers** (via `4*num`): `0x100`=etv_timer
   (`0x400`), `0x101`=etv_critic (`0x404`), `0x102`=etv_term (`0x408`). AES
   installs its own via these. (The prior spec listed these offsets
   inconsistently; corrected here from `tosvars.ld:33-35`.)

---

## Dependencies

### Requires (must be initialized / functional)

- A populated low-memory vector page (set up by `vecs_init()`,
  `bios/bios.c:118`, and the boot ROM).

### Used by

- AES/VDI install hooks; `Pterm` cleanup via `etv_term`; custom interrupt
  handlers; debuggers; terminate-stay-resident programs.

---

## Related data structures

- `docs/api/bios/_state.md` — low-memory vector / etv_* layout
- `docs/api/structures/osheader.md` — related boot vectors

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: agrees on
  `-1` query semantics and always-return-old.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

### Atari ST Internals

- Not present in `references/`. (Would be the reference for the full vector
  number → address table.)

---

## Configuration / version notes

None. Identical across all EmuTOS builds and TOS versions.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06: NewTOS keeps a software "m68k vector shadow" at virtual `0..0x400`
for m68k binaries; native AArch64 handlers consult the shadow for
m68k-emulated hooks (etv_term etc.). For native source ports the etv_* hooks
are reached by name, not by poking `4*num`.

---

## Pi / BCM2711 binding

> Re-points the hardware reference from the **68000 low-memory vector table**
> (`4*num` in physical RAM) and the MFP/GLUE interrupt sources to the **AArch64
> exception model** (`VBAR_EL1` + GIC-400) plus a **software vector shadow**.
> The `Setexc` contract (get/set an entry, `-1` = query, always return the
> previous value) is unchanged; only the backing store is re-pointed here. See
> `docs/HARDWARE-PROFILE.md` §3.8, §3.9.

### There is no 68k vector page — `Setexc` is a software shadow

AArch64 has **no flat `4*num` vector page**. Exceptions are dispatched through
`VBAR_EL1` (a 2 KiB, 16-entry vector table set up in
`kernel/src/arch/aarch64/exceptions.rs`), and hardware interrupts come from the
GIC-400 (GICv2), not from the MFP/GLUE. NewTOS therefore keeps a **software
shadow** of the m68k vector table in `tos-api/src/bios/vectors.rs`: a
`[AtomicI64; NVEC]` array (`vectors.rs:22`) that `Setexc` reads/writes.

| Property | Value | Source |
|----------|-------|--------|
| Shadow size `NVEC` | `0x120` entries (the 256-LONG m68k vector page + `etv_*`/GEM hooks above `$0400`) | `vectors.rs:21` |
| Entry width | `i64` (native handler addresses are 64-bit) | `vectors.rs:22` |
| Out-of-range `num` (`>= NVEC`) | returns `0`, **no write** (safer than m68k, which clobbers low memory) | `vectors.rs:38-40` |
| `vector == -1` | query only — returns old, no store | `vectors.rs:42-44` |

### Canonical hook vector numbers (unchanged from the contract)

The `etv_*` system hooks keep their `4*num` addresses / `num` values as the
contract; NewTOS shadows them at these indices (`vectors.rs:27-29`):

| `Setexc` num | m68k addr | Symbol | Meaning |
|-------------:|-----------|--------|---------|
| `0x100` | `$0400` | `ETV_TIMER` | 20 ms system tick hook |
| `0x101` | `$0404` | `ETV_CRITIC` | critical-error handler |
| `0x102` | `$0408` | `ETV_TERM` | process-terminate hook |

| Atari reference | Pi-4 model today |
|-----------------|------------------|
| `4*num` LONG in physical low RAM | `VECTORS[num]` atomic entry (a kernel struct, no fixed VA) |
| CPU exception vectors `0..0xFF` | shadowed but **not dispatched** by the native EL1 handlers (kept for the future m68k userland) |
| MFP timer-C / GLUE IRQ vectors | replaced by GIC-400 + `VBAR_EL1`; the `etv_*` hooks are logical, invoked by name |
| Installing a vector changes CPU dispatch | installing a shadow entry changes nothing until a NewTOS code path calls `vectors::vector(num)` |

### Gaps vs. the contract (Step-2 candidates)

- **`etv_*` hooks are stored but never invoked.** `Setexc(0x100/0x101/0x102,
  handler)` records the handler, but no NewTOS code path calls the reader
  `vectors::vector(num)` (`vectors.rs:52-58`) — it is `#[allow(dead_code)]`.
  So a program that installs an `etv_timer` / `etv_critic` / `etv_term` hook via
  `Setexc` sees it **never fire**: there is no 20 ms timer IRQ dispatching
  `etv_timer` (§ `06-tickcal.md` binding — the clock is derived on demand, not
  tick-driven), no critical-error chain calling `etv_critic` (ties to the block
  `Rwabs` B6 gap), and `Pterm` does not run `etv_term`. The get/set/return-old
  contract holds; the **dispatch-through-the-vector** contract does not.
- **CPU-exception shadow entries are inert.** Shadowing `num` in `0..0xFF` does
  not redirect the AArch64 exception handlers; those entries exist only for the
  deferred m68k userland.
