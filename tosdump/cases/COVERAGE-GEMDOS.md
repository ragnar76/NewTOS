# GEMDOS trace ledger

What a GEMDOS trap trace (`tos-api/src/gemdos/trace.rs`, feature
`gemdos-trace`, decoded with `tools/gdtrace.py`) has established so far, and
what the **next** run is predicted to show. The counterpart of `COVERAGE.md`
(VDI) and `COVERAGE-AES.md` (AES), except that this instrument records a live
application rather than a scripted case table — so the ledger is per *run*,
not per case id.

Predictions are written **before** the run they are about. A prediction that
turns out wrong is the useful kind; it is left standing next to what actually
happened.

---

## Run 1 — `TERADESK-GDTRACE-r1.BIN`, 2026-09-01

Pi 4, real card. TeraDesk opened `C:`, navigated into a subdirectory and
closed the window. 313 records, complete, no drops, no suppressed hook
entries. Timer 54 MHz. Tracer overhead 4808 ticks over 311 hooks (mean 15
ticks ≈ 0.28 µs, worst 66 ticks ≈ 1.2 µs), so nothing below is the
instrument.

### What it established

**1. Duration is quantised, and the quantum is one physical block read.**
The whole run's per-call durations:

```
   70 calls x     0 us
   54 calls x   512 us
  160 calls x   513 us
   16 calls x   514 us
    6 calls x 515-518 us
    4 calls x 606-610 us
    1 call  x  1157 us      Fsfirst "C:\APPS\*.*"
    1 call  x  1209 us      Fsnext -> ENMFIL
    1 call  x 16822 us      Fread 13269 bytes (26 sectors)
```

The 70 zero-cost calls are exactly `Fsetdta`, `Fgetdta`, `Dgetdrv`,
`Dsetdrv`, `Fseek`, `Fclose`, `Malloc`, `Mxalloc`, `Psigreturn` and the
unimplemented `0x20` — every call that never reaches the block device. The
~512 µs band is every call that resolved a drive. `Fread` divides out at
~647 µs per sector, the same order.

**Cause, measured (not inferred):** `Volume::mount` called `bpb::geometry`,
which read the partition's boot sector with `blk.read_blocks` **outside** the
sector cache, once per call. A host measurement of a 56-entry root listing
(`Fsfirst` + 55 `Fsnext`) before the fix: 57 physical reads, **all 57 of them
the boot sector** — the directory sectors were cache hits throughout. After
the fix: 0 physical reads for the same listing. Fixed in `bf49a24`
(`Volume::mount` is EmuTOS `ckdrv`, `bdos/fsdrive.c:77-105`, and now logs a
drive in once).

The 229 `Fsnext` calls of this run therefore spent 117 ms re-reading one
sector.

**2. `Fsfirst` on a path with no filename component answered the wrong code.**

```
 33  3522.899  606us  4e Fsfirst  "C:\" attr=0x37 -> -34 EPTHNF
174 22119.775  608us  4e Fsfirst  "C:\" attr=0x37 -> -34 EPTHNF
245 32144.329  607us  4e Fsfirst  "C:\" attr=0x37 -> -34 EPTHNF
```

Each is preceded by `Fgetdta` + `Fsetdta(0x3ff7f4)` and followed by
`Fsetdta(0x80)` — TeraDesk's `x_attr()` on a FAT volume
(`sdk/apps/teradesk/xfilesys.c:1164-1176`). EmuTOS answers `EFILNF`: `findit`
**succeeds** for `"C:\"` (root DND, `*sp == ""`, `bdos/fsdir.c:1717-1730`,
`:1782-1793`, `:1430,1495`) and it is `scan` that fails, because `builds("")`
produces eleven blanks (`bdos/fsdir.c:737-795`). Fixed in `e5344ef`.

TeraDesk passes the code through unchanged (`library/utility/xerror.c:26-32`),
where `-33` is its `ENOENT` and `-34` its `ENOTDIR`, and it branches on that
difference in `copy.c:531,582`, `dir.c:803` and `error.c:77,82`.

### What it ruled out

* `Super` (0x20) → `EINVFN` matches EmuTOS (`bdos/bdosmain.c:221` is `ni`,
  `:307-309` returns `EINVFN`). Four calls, all from the C runtime's startup.
* `Fopen("C:\teradesk.inf") -> EFILNF` is normal — TeraDesk writes that file
  only on an explicit save.
* `Dfree` (0x36) **does not appear in this trace at all.** The claim that
  TeraDesk calls it per directory window is not supported by run 1; it may be
  issued only when a window shows the free-space line, or not at all in this
  build. Its cost (a full FAT walk) is therefore unmeasured on this card.

### Still open after run 1

* Which TeraDesk call site issues `x_attr()` on the window's own root path was
  not identified — only that it is `x_attr` on a path ending in a separator
  (`xfilesys.c:1127`, reached from one of `dir.c`, `icon.c`, `window.c`,
  `copy.c`). The user's report that window handling "is still not quite
  right" is not explained by the error code alone: both `-33` and `-34` are
  negative, and every caller on the window-open path tests `>= 0` or `== 0`.

---

## Predictions for run 2

Same gesture: boot, open `C:`, navigate into a subdirectory, come back, close
the window. Kernel built with `--features gemdos-trace` from a tree containing
`e5344ef` and `bf49a24`.

| # | Prediction | Where to look |
|---|---|---|
| P1 | `Fsfirst("C:\", attr=0x37)` answers **-33 EFILNF**, at the same three points in the sequence, still once per directory-window operation | `--errors` |
| P2 | `Dgetpath` mean drops from 512.5 µs to **under 10 µs** — it touches no block device at all once `C:` is logged in | `--summary` |
| P3 | `Fsnext` mean drops from 516.6 µs to **under 100 µs**. Most calls become 0 µs; the exceptions are the calls whose scan crosses into a directory sector that is not in the 16-slot cache, which stay ~512 µs each | `--summary`, `--op Fsnext` |
| P4 | The 229-`Fsnext` total drops from 118 ms to **under 20 ms** | `--summary` |
| P5 | Exactly **one** ~512 µs call per drive for the whole session is a boot-sector read (the log-in). After that, no call's duration is explained by a boot-sector read | duration histogram |
| P6 | `Fread` of `icons.rsc` is **unchanged** (~16.8 ms for 13269 bytes): `Fread` never calls `Volume::mount`, so neither fix touches it. If this moves, the model above is wrong | seq 11 equivalent |
| P7 | The zero-cost set is unchanged: `Fsetdta`, `Fgetdta`, `Dgetdrv`, `Dsetdrv`, `Fseek`, `Fclose`, `Malloc`, `Mxalloc` still mean 0 | `--summary` |
| P8 | No new failing call. The `Fsnext -> ENMFIL` at the end of each listing, the `Fopen("C:\teradesk.inf") -> EFILNF` and the four `0x20 -> EINVFN` all stay | `--errors` |
| P9 | The directory listings are byte-identical in content (same entries, same order, same sizes) — the fixes change *when* a sector is read, not *what* is read | `--files`, DTA lines |
| P10 | **No visible change in TeraDesk's window behaviour** from P1 alone. Both codes are negative and the window-open path tests only the sign. If window handling does change, the cause is elsewhere and P1 was not it | screendump / operator |

A prediction not made, deliberately: nothing here says the total
window-opening wall time drops by a specific amount. The trace's `t_ms` gaps
are dominated by the operator (3.4 s, 7 s, 5.5 s of idle between actions), so
the saving is visible in `--summary` totals, not in the run length.

---

## Open defect — not fixed, diagnosis only

**A failed sector write-back is invisible, and loses the file's directory
entry.** Reported after run 1: one trace file in three came back with a
directory entry carrying **neither a size nor a first cluster**, while the
volume's free space had dropped by the right amount and the data was
recoverable by following the orphaned FAT chain. It does not reproduce under
QEMU.

That state was reproduced exactly, on the host, by refusing exactly one
sector write:

1. `Fcreate` writes the directory entry with cluster 0 and size 0, then
   flushes — so the entry is on disk in that shape from the start.
2. `Fwrite` allocates clusters, writes the FAT and the data, and flushes each
   time — so free space and the chain are correct on disk.
3. `Fclose` → `flush_ofd` patches cluster and size into the entry and calls
   `cache::flush()`. **This is the only write that ever carries the size.**

Refuse that one write and the on-disk result is
`name present, cluster=0, size=0, free space correct, chain intact` — the
reported signature — and `Fclose` still returns `E_OK`. Two sub-cases:

* the refusal is transient and a later `cache::flush()` retries the slot →
  self-heals (measured: `cluster=5 size=4096` afterwards);
* the refusal lasts until the dirty slot is **evicted** → permanent, because
  `cache::evict` (`tos-api/src/bdos/fs/cache.rs:63-72`) clears `dirty` even
  when its `write_blocks` returned `false`, and discards the result.

EmuTOS does not do this. `flush()` (`bdos/fsbuf.c:85-102`) reaches
`b->b_dirty = 0` only if `longjmp_rwabs` (`bdos/fs.h:60-73`) did not jump; on
an `Rwabs` error it sets `errdrv`/`errcode` and longjmps, and `osif`'s
`setjmp(errbuf)` returns that code as the **GEMDOS call's result**
(`bdos/bdosmain.c:448-451,489-491`). Our `cache::flush()` returns a `bool`
that every caller discards.

What is **not** established: that a write actually failed on the card in that
run. `write_blocks` returns `false` when `SR_DAT_INHIBIT` does not clear
within `SPIN_LIMIT` = 1 000 000 polls, or an SDHCI error bit is set, or
`INT_DATA_DONE` times out (`kernel/src/drivers/bcm2711/emmc.rs:91,150-176,313-330`).
Whether that happened cannot be known from the run, because the result is
thrown away. **The cheapest next step is an error counter on that path** —
then a repeat of the failure is self-identifying instead of needing to be
reconstructed from a damaged card.

Ruled out as explanations of the same signature:

* *`Fclose` never ran.* The tracer calls `close(handle)` unconditionally,
  outside the `ok` chain (`tos-api/src/aes/trace.rs:1042-1130`), and writes a
  companion `.TXT` afterwards.
* *`ofd.dirty` was not set.* `Fwrite` sets it whenever any byte was written,
  and `resolve_cluster` sets it as soon as the first cluster is allocated
  (`tos-api/src/bdos/fs/file.rs:430-436,444-451`).
* *An overwrite defect.* Run 3 overwrote an existing multi-cluster file
  correctly, and the host reproduction shows the create-then-overwrite path
  producing a correct entry.
* *A wrong slot.* `dir_start`/`dir_slot` are fixed at `Fcreate` and
  `slot_sector` is a pure function of them, so `flush_ofd` addresses the same
  sector `Fcreate` wrote.
* *Cache aliasing.* `cache::read`/`cache::write` both search for an existing
  slot with the same LBA first, so two slots cannot hold the same sector.
