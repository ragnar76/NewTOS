# `Getbpb` (BIOS 0x07)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/bios.c:1120` (`getbpb`) and the installed handler
> `bios/blkdev.c:569` (`blkdev_getbpb`).

## EmuTOS source

**Primary location:** `bios/bios.c:1120` — `LONG getbpb(WORD drive)`
(body: `return protect_w(hdv_bpb, drive);`).

**Handler behind the vector:** `bios/blkdev.c:569` — `blkdev_getbpb()`,
installed as `hdv_bpb` by `blkdev_init()` (`bios/blkdev.c:101`).

**Helpers called:**
- `util/miscasm.S:225` — `protect_w(func, drive)` — d2/a2-preserving thunk.
- `bios/blkdev.c:625` — `blkdev_rwabs(RW_READ|RW_NOMEDIACH|RW_NOTRANSLATE,…)` —
  physical read of the boot sector into `dskbufp`.
- `bios/disk.c` — `disk_mediach()` (removable non-floppy re-scan).
- `bios/blkdev.c` — `getiword()` (big-endian field decode), `flop_checksum()`.

**Headers / type definitions used:**
- `include/biosdefs.h:67` — `BPB` struct (9 UWORDs = 18 bytes)
- `include/biosdefs.h:84-85` — `B_16`, `B_1FAT` flags
- `bios/blkdev.h:25-26` — `MAX_FAT12_CLUSTERS` (4084), `MAX_FAT16_CLUSTERS`
  (65524)
- `bios/blkdev.h` — `struct bs`, `struct fat16_bs` (boot-sector layout)

**System variables / globals read/written:**
- `hdv_bpb` (TOS var **`0x472`**) — the get-BPB vector (`bios/blkdev.c:101`)
- `blkdev[drive].bpb` — the returned struct (filled in place)
- `blkdev[drive].mediachange = MEDIANOCHANGE`, `.forcechange = FALSE` (reset,
  `bios/blkdev.c:589-590`)
- `dskbufp` — the shared boot-sector read buffer

**Build configuration affecting this function:**
- `CONF_WITH_1FAT_SUPPORT` — enables the single-FAT (`B_1FAT`) path and its
  `fatrec` computation (`bios/blkdev.c:711,753-756`).

---

## Source extraction summary

`Getbpb` returns a pointer to the BIOS Parameter Block for a logical drive,
re-reading and re-parsing the boot sector every call (no caching). The
trap-level `getbpb` forwards to `hdv_bpb`; on EmuTOS that is `blkdev_getbpb`.

`blkdev_getbpb` validates the drive (`0..BLKDEVNUM-1`, `DEVICE_VALID`),
**resets** the drive's `mediachange`/`forcechange` state, primes `bpb.recsiz`
(real size for floppies, 0 = "invalid" for hard units until proven valid),
optionally re-scans a removable unit's partitions, then reads the boot sector
via physical `blkdev_rwabs`. It decodes the (big-endian) boot-sector fields
with `getiword`, validates sector size against `pun_info.max_sect_siz`,
cluster size against `MAX_CLUSTER_SIZE`, and the FAT count, then computes the
9 BPB fields (`recsiz`, `clsiz`, `clsizb`, `rdlen`, `fsiz`, `fatrec`,
`datrec`, `numcl`, `b_flags`). It classifies FAT12 vs FAT16 purely by cluster
count (`numcl > MAX_FAT12_CLUSTERS` ⇒ `B_16`), and rejects FAT32
(`numcl > MAX_FAT16_CLUSTERS` **or** `fsiz == 0`). On success it returns
`(LONG)&blkdev[drive].bpb`; on **any** failure it returns `0L` (NULL).

There is no separate error code — the only failure signal is the NULL return.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x07
- **Stack frame on entry (m68k):**
  ```
  +2: drive (word)
   0: function_id (word) = 0x0007
  ```
- **Total parameter bytes:** 4
- **Native (AArch64) ABI:** ID `0x07` in `x8`; `drive` in `x0`; **the full
  64-bit pointer result (or 0) in `x0`** — this is a `BPB *`, not a
  sign-extended WORD.

  | m68k stack | AArch64 reg | Name    | Type / size |
  |------------|-------------|---------|-------------|
  | +2         | `x0`        | `drive` | `WORD` (16) |

---

## Inputs

| Position | Name    | Type   | Size | Source location     | Notes |
|----------|---------|--------|------|---------------------|-------|
| arg0     | `drive` | `WORD` | 2    | `bios/blkdev.c:569` | logical drive 0=A:..15=P:; validated against `BLKDEVNUM` |

---

## Outputs

### Return value

- **Register:** D0 / `x0` (full pointer width)
- **Type:** `BPB *` (LONG on m68k)
- **Encoding:** non-NULL pointer to the drive's 18-byte BPB on success; `0L`
  (NULL) on any failure. The pointer is into a BIOS-owned per-drive struct —
  the caller must not free or write it.
- **Source location:** `bios/blkdev.c:776` (success) / `:584,615,634,646,655,
  669,741` (each NULL return).

### Side effects on global state

- `blkdev[drive].bpb` fully rewritten (`bios/blkdev.c:675-762`).
- `blkdev[drive].mediachange = MEDIANOCHANGE`, `.forcechange = FALSE`
  (`bios/blkdev.c:589-590`) — clears any pending media-change.
- `blkdev[drive].geometry`, `.serial`, `.serial2` updated.
- `dskbufp` overwritten with the boot sector.
- For a removable non-floppy unit, `disk_mediach(unit)` may re-scan partitions
  (`bios/blkdev.c:608-609`).

### Side effects on hardware

- Reads the physical boot sector from the drive.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `drive` invalid / not `DEVICE_VALID` | `bios/blkdev.c:581` | `0L` (NULL) |
| Device does not support GetBPB (`!GETBPB_ALLOWED`) | `bios/blkdev.c:612` | `0L` |
| Boot sector unreadable (I/O error) | `bios/blkdev.c:631` | `0L` |
| Logical sector size > `pun_info.max_sect_siz` | `bios/blkdev.c:642` | `0L` |
| Cluster size 0 or > `MAX_CLUSTER_SIZE` | `bios/blkdev.c:651` | `0L` |
| FAT count < `MIN_FATS` or > `MAX_FATS` | `bios/blkdev.c:666` | `0L` |
| FAT32 (`numcl > MAX_FAT16_CLUSTERS` or `fsiz == 0`) | `bios/blkdev.c:736` | `0L` (also sets `recsiz=0`) |

**No error code is ever returned — only `0L`.** To distinguish "drive absent"
from "unreadable/unsupported filesystem" the caller must consult `Mediach`
and/or prior state; `Getbpb` collapses all of it to NULL.

**Error paths not in Compendium:** the `!GETBPB_ALLOWED` and FAT32 branches
are EmuTOS-specific NULL returns.

---

## Verification criteria

- **Nominal (FAT16):** `Getbpb(2)` on a valid FAT16 C: returns a non-NULL
  `BPB *` whose `recsiz == 512`, `b_flags & B_16 != 0`, and `clsizb ==
  recsiz*clsiz`.
- **Nominal (FAT12):** on a valid FAT12 floppy A:, `Getbpb(0)` returns
  non-NULL with `b_flags & B_16 == 0` (`numcl <= 4084`).
- **FAT32/unsupported:** `Getbpb` on a FAT32 partition returns `0L`.
- **Absent drive:** `Getbpb(15)` with no P: returns `0L`.
- **State reset:** after `Getbpb(d)`, `Mediach(d)` no longer reports the
  media change that `Getbpb` consumed (mediachange reset to `MEDIANOCHANGE`).

---

## Edge cases discovered in source

1. **No caching — always re-reads the boot sector.** Every call re-does the
   physical read and rebuild (`bios/blkdev.c:624-776`), so a media change is
   always reflected. Programs that only want to *test* for change must use
   `Mediach`, not `Getbpb`.

2. **`Getbpb` resets the media-change state.** It sets `mediachange =
   MEDIANOCHANGE` and `forcechange = FALSE` up front (`:589-590`) — this is the
   documented "clears the change flag" behaviour, **not** a GEMDOS FAT-cache
   flush. (The prior spec's "flushes any GEMDOS in-memory FAT cache" claim is
   not in the source and has been removed.)

3. **FAT type from cluster count only.** `B_16` is set iff `numcl >
   MAX_FAT12_CLUSTERS` (4084) — the Microsoft rule, cluster-count based, not
   media-size based (`bios/blkdev.c:750-752`).

4. **FAT32 fails softly.** `numcl > MAX_FAT16_CLUSTERS` (65524) or `fsiz == 0`
   ⇒ mark `recsiz = 0` (XHDI "invalid BPB") and return NULL
   (`bios/blkdev.c:736-742`). A subsequent logical `Rwabs` on that drive then
   returns `ESECNF` (see `04-rwabs.md`).

5. **Zero-FAT boot sector accepted as 2 FATs.** For TOS compatibility, `fat ==
   0` is treated as 2 (`bios/blkdev.c:662-665`).

6. **Per-drive pointer, stable across drives.** `Getbpb(0)` and `Getbpb(1)`
   return distinct pointers (each into its own `blkdev[].bpb`) that both stay
   valid; a re-call for the *same* drive rewrites that drive's struct in place.

7. **`datrec` computed for all cases as `fatrec + fsiz + rdlen`.** For the
   2-FAT case `fatrec` already includes `+fsiz` (second FAT); for 1-FAT it does
   not (`bios/blkdev.c:705-713`).

---

## Dependencies

### Requires (must be initialized / functional)

- `hdv_bpb` populated (`blkdev_init`).
- `hdv_rw`/`hdv_mediach` functional (used internally).
- `dskbufp` allocated; a live disk/floppy driver.

### Used by

- GEMDOS drive-login / mount logic; drive-enumeration utilities; anything
  needing sector/cluster geometry.

---

## Related data structures

- `docs/api/structures/bpb.md` — the returned 18-byte BPB
- `docs/api/bios/_state.md` — `hdv_bpb` vector + address

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically documents
  NULL-on-failure and pointer-into-BIOS-buffer.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `BLKDEVNUM = 16`. `MAX_FAT12_CLUSTERS = 4084`, `MAX_FAT16_CLUSTERS = 65524`.
- `CONF_WITH_1FAT_SUPPORT` toggles the single-FAT path.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04: NewTOS returns a `BPB *` only for FAT12/FAT16 logical drives,
matching EmuTOS; FAT32/exFAT partitions return NULL from `Getbpb` and are
reached through a NewTOS-native path instead. Note `Getbpb`'s result is a full
64-bit pointer in `x0` for native callers.

---

## Pi / BCM2711 binding

> Re-points the hardware reference from the Atari FDC / ACSI boot sector to the
> **BCM2711 EMMC2 SD card**. The contract above (NULL-on-failure, the 18-byte
> BPB, media-state reset, FAT type from cluster count) is unchanged; only the
> backing hardware and the concrete NewTOS fields are pinned here. See
> `docs/HARDWARE-PROFILE.md` §3.4.

### Where the BPB comes from

The BPB is **synthesized from the FAT16 boot sector of the mounted partition**,
not from Atari media geometry. `Getbpb` (`tos-api/src/bios/bpb.rs:160-167`):

1. resolves `drive`→partition via the MBR-built drive table
   (`drive::resolve`, see `0a-drvmap.md`);
2. reads **partition-relative sector 0** (the FAT boot sector) from the EMMC SD
   at `part.start_lba` via `BlockDev::read_blocks` (`bpb.rs:139-148`) — which
   forwards to `emmc.rs` (see `04-rwabs.md`);
3. parses the DOS/BPB fields and caches the result in a per-drive static
   (`TABLE`, one slot per drive, `bpb.rs:53-56,150-153`);
4. returns a native 64-bit pointer into that slot (`bpb.rs:166`), or `0`.

Every call re-reads and re-parses (no caching of the sector) — matching the
EmuTOS "always re-reads" contract.

### The exact fields NewTOS fills (`bpb.rs:60-103`)

All fields decoded **little-endian** (no byte-swap on ARM) from the boot-sector
BPB at the standard offsets:

| BPB field | Source (boot-sector offset) | Computation |
|-----------|-----------------------------|-------------|
| `recsiz` | `0x0B..0x0C` bytes/sector | direct |
| `clsiz` | `0x0D` sectors/cluster | direct |
| `clsizb` | — | `recsiz * clsiz` |
| `rdlen` | `0x11..0x12` root entries | `ceil(root_entries*32 / recsiz)` sectors |
| `fsiz` | `0x16..0x17` sectors/FAT (16-bit) | direct |
| `fatrec` | `0x0E..0x0F` reserved, `0x10` num_fats | `reserved + (num_fats-1)*fsiz` (first sector of the last FAT) |
| `datrec` | reserved, num_fats, rdlen | `reserved + num_fats*fsiz + rdlen` |
| `numcl` | total sectors, `datrec`, `clsiz` | `(total - datrec) / clsiz` |
| `b_flags` | `numcl`, num_fats | `B_16` if `numcl >= 4085`; `B_1FAT` if `num_fats == 1` |

- **Total sectors:** `tot16` (`0x13..0x14`) if non-zero, else `tot32`
  (`0x20..0x23`), else the partition length `part.count` from the MBR
  (`bpb.rs:75-81`).
- **FAT type = cluster count** (`numcl >= 4085` ⇒ `B_16`), the Microsoft rule,
  matching EmuTOS (`bpb.rs:95-97`).

### FAT16-only; FAT32 / exFAT return NULL

`parse` returns `None` (→ `Getbpb` returns `0`) when
`recsiz==0 || clsiz==0 || num_fats==0 || fsiz16==0` (`bpb.rs:71-73`). A FAT32
boot record has its 16-bit sectors-per-FAT field (`0x16`) zeroed, so `fsiz16==0`
⇒ NULL — the same "FAT32 fails softly" outcome as EmuTOS. exFAT (no classic BPB)
likewise fails. FAT16 hard-disk is the scope; FAT12/floppy is out of scope and
exFAT is a Phase-09 goal (`docs/HARDWARE-PROFILE.md` §3.4).

### Gaps vs. the contract (Step-2 candidates)

- **Media-state reset not implemented.** EmuTOS resets
  `mediachange = MEDIANOCHANGE`, `forcechange = FALSE` at the top of `Getbpb`
  (`bios/blkdev.c:589-590`). NewTOS keeps **no per-drive media/force flags** at
  all (`Mediach`≡0, no `forcechange`), so there is nothing to reset. Consistent
  with the non-removable model, but the reset step is absent (B-series).
- **No sector-size / cluster-size / FAT-count validation branches** beyond the
  degenerate-zero check — EmuTOS's `max_sect_siz`, `MAX_CLUSTER_SIZE`,
  `MIN_FATS`/`MAX_FATS` guards (each a NULL return) are not reproduced; a
  malformed-but-nonzero BPB is parsed as-is.
- **Zero-FAT→2 quirk not applied.** EmuTOS treats `num_fats==0` as 2; NewTOS
  rejects `num_fats==0` outright (`bpb.rs:71`). Harmless for real FAT16 media
  (always ≥1 FAT).
- **`Geometry` extension:** NewTOS additionally exposes `reserved`+`num_fats`
  raw (`bpb.rs:105-134`) for the FAT write path — a NewTOS-native superset, not
  part of the Atari ABI BPB.
