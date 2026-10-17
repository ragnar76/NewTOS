# `Dsetdrv` (GEMDOS 0x0E)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsmain.c:420` (`xsetdrv`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the 0-based drive index, the "silently ignore
> out-of-range" rule, the LONG drive-bitmap return. Reference: the per-process
> `PD.p_curdrv` field and the `Drvmap()` BIOS call it forwards to.

## EmuTOS source

**Primary location:** `bdos/fsmain.c:420` — `long xsetdrv(int drv)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x0E` =
`xsetdrv`, `wparms = 1` (`bdos/bdosmain.c:179`).

**Helpers called:**
- `include/biosbind.h:29` — `Drvmap()` = `bios_l_v(0xa)` → BIOS 0x0A
  `blkdev_drvmap()` (`bios/blkdev.c:833`) — returns `drvbits`.

**Headers / type definitions used:**
- `include/sysconf.h:43` — `BLKDEVNUM = 26` (drives A:..Z:).
- `include/bdosdefs.h:86` — `PD.p_curdrv` (`UBYTE`, basepage offset **0x37**).

**System variables / globals read:** `run` (current PD, `bdos/proc.c:54`);
`drvbits` (via `Drvmap()`, in the BIOS layer, `bios/blkdev.c`).
**System variables / globals written:** `run->p_curdrv` (only when `drv` is in
range).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Dsetdrv` sets the calling process's default (current) drive and returns the
system drive bitmap. `xsetdrv` accepts a 0-based drive index (`0 = A:`,
`25 = Z:`). If `drv` is in the range `0 <= drv < BLKDEVNUM` (i.e. `0..25`) it
stores it into `run->p_curdrv`; otherwise it leaves the current drive
**unchanged**. In **all** cases it then returns `Drvmap()` — the BIOS bitmap of
all present block devices — so the return value does not tell the caller whether
the requested change actually took effect.

Two things are deliberately *not* done. First, `xsetdrv` does **not** verify
that the requested drive is actually mounted/present: a program may select a
drive whose bit is clear in `Drvmap()`; the failure only surfaces on the next
filesystem operation (as `EDRIVE`/`EPTHNF` from `ckdrv`). The source comment
(`bdos/fsmain.c:414-418`) records that this permissiveness is intentional —
historically only mounted drives were selectable, but BetaDOS expects
`Dsetdrv(15)` (P:) to succeed even when P: is offline, so EmuTOS follows TOS 4.04
in only bounds-checking against `BLKDEVNUM`. Second, the current drive is
**per-process**: `p_curdrv` lives in the caller's PD, so different processes can
have different current drives.

Note the argument is 0-based (`0 = A:`), unlike `Dfree`/`Dgetpath`/`Dgetdir`
which use the `+1` convention (`0 = current`, `1 = A:`).

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x0E
- **Stack frame on entry (m68k):** `Dsetdrv(drv)` = `trap1(0x0e, drv)`:
  ```
  +2: drv (word)
   0: function_id (word) = 0x000E
  ```
- **Total parameter bytes:** 2 (WORD) → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x0E` in `x8`; arg in `x0`; LONG drive bitmap
  returned in `x0` (a 32-bit value zero-extended into the 64-bit register — bits
  0..25 only can be set, so bit 31 is never set and sign-extension is moot).

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `drv` | `WORD` (16) — 0-based drive index |

---

## Inputs

| Position | Name  | Type  | Size | Source location      | Notes |
|----------|-------|-------|------|----------------------|-------|
| arg0     | `drv` | `int` | 2    | `bdos/fsmain.c:422`  | 0-based drive index (`0 = A:`, `25 = Z:`); out-of-range values are ignored |

The C prototype is `int drv`, so the WORD is treated as a **signed** 16-bit
value: a negative `drv` fails the `drv >= 0` guard and is ignored.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (drive bitmap)
- **Encoding:** bit `n` set ⇒ drive `n` (`A:` = bit 0 … `Z:` = bit 25) is
  present. Returned **regardless** of whether `p_curdrv` was updated.
- **Source location:** `bdos/fsmain.c:425` (`return Drvmap()`).

### Side effects on global state

- `run->p_curdrv = drv` **iff** `0 <= drv < 26` (`bdos/fsmain.c:422-423`).
  Nothing else changes; no drive is logged in and no media check is performed
  here (that happens lazily on the first file operation via `ckdrv`).

### Side effects on hardware

- None directly. `Drvmap()` reads the cached `drvbits`; it does not probe
  hardware.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error path) | — | always returns the drive bitmap |

`xsetdrv` has **no error return**. An out-of-range `drv` (< 0 or >= 26) is
silently ignored — the current drive is left unchanged and the bitmap is still
returned, so the caller cannot detect the failure from the return value (it must
call `Dgetdrv` to confirm). This matches historical Atari TOS behaviour.

**Error paths not in Compendium:** none; the silent-ignore behaviour is a
non-error path Compendium does not spell out.

---

## Verification criteria

- **Nominal:** on a system with C: present, `Dsetdrv(2)` sets the current drive
  to C: (a subsequent `Dgetdrv` returns 2) and returns a bitmap with at least
  bit 2 set.
- **Bitmap independent of change:** `Dsetdrv(drv)` returns the same value as a
  standalone `Drvmap()` call, for any `drv`.
- **Out-of-range ignored:** after `Dsetdrv(2)`, calling `Dsetdrv(30)` leaves the
  current drive at 2 (`Dgetdrv` still returns 2) and still returns the bitmap.
- **Offline drive selectable:** `Dsetdrv(15)` (P:) succeeds and updates the
  current drive even when bit 15 is **clear** in the returned bitmap; the failure
  only appears on the next file operation as `EDRIVE`/`EPTHNF`.

---

## Edge cases discovered in source

1. **Range is A:..Z: (26), not A:..P: (16).** `BLKDEVNUM = 26`
   (`include/sysconf.h:43`); the accepted range is `0..25`. (Earlier drafts of
   this spec used 16 — wrong; see Discrepancy log D-025.)

2. **Out-of-range is silently ignored.** `drv < 0` or `drv >= 26` leaves
   `p_curdrv` untouched but still returns the bitmap (`bdos/fsmain.c:422-425`).

3. **No mount/presence check.** The selected drive need not exist in the bitmap;
   selection always "succeeds". Intentional (BetaDOS compatibility,
   `bdos/fsmain.c:414-418`).

4. **The bitmap reflects all present drives**, not a per-process view. EmuTOS
   has no per-process drive masking or chroot.

5. **0-based here, `+1` elsewhere.** `Dsetdrv`/`Dgetdrv` are 0-based;
   `Dfree`/`Dgetpath` use `0 = current, 1 = A:`. A frequent source of off-by-one
   bugs in TOS software.

---

## Dependencies

### Requires (must be initialized / functional)

- `run` (current PD pointer, `bdos/proc.c:54`).
- BIOS `Drvmap` (0x0A) and the populated `drvbits`.

### Used by

- Shell `cd X:` / drive-change commands, AES file selector, any chdir-equivalent
  that switches drive before `Dsetpath`.

---

## Related data structures

- `docs/api/gemdos/_state.md` §4 — current-drive model (`PD.p_curdrv`) and the
  drive bitmap (`drvbits`/`Drvmap`).
- `docs/api/structures/basepage.md` — `p_curdrv` at basepage offset 0x37.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically documents the
  0-based index and the drive-bitmap return; the silent-ignore of out-of-range
  is EmuTOS/TOS behaviour.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `BLKDEVNUM = 26` in this build (A:..Z:). The comment references TOS 4.04's
  A:..P: restriction, but this EmuTOS build allows A:..Z:.

---

## NewTOS implementation notes

(Empty during Phase 00.)

**2026-10-18 (TASK/PROC03/PROC04 point 2):** the PD written is `run`'s;
`p_curdrv` is inherited at PD creation (`init_pd_files`, `bdos/proc.c:409`), the
AES shell writes the accdesk PD's before each launch, the AES writes the AES
PD's on a resolution change (`aes/geminit.c:796-801`)
(`../process/_run-ownership-curdir.md` §4). NewTOS today: PD+0x37 reads 0.

PHASE-05: `p_curdrv` is a per-process field in the kernel `Process`. Preserve the
0-based index, the 26-drive range, and the silent-ignore-out-of-range behaviour
for m68k-binary compatibility.
