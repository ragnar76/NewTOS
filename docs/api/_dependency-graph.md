# TOS API Dependency Graph

> Phase 00 deliverable D12. For every spec entry produced, what does it
> depend on? Used to plan implementation order in later phases.
>
> Append-only as new spec entries are written.

---

## Format

```
<function>:
  requires: <other function or state> ...
  used by:  <other function or state> ...
  category: leaf | branch | root
```

- **leaf** — no GEMDOS/BIOS/XBIOS dependencies (only state + hardware).
  Implement first.
- **branch** — depends on other API functions but not on heavy state.
- **root** — depends on initialised filesystem, scheduler, allocator,
  or AES/VDI subsystems. Implement last.

---

## GEMDOS — Console group

```
Cconout (0x02 / xconout):
  requires:
    - run->p_uft[1]          (state: stdout standard handle)
    - Bconout BIOS:0x03      (and by extension bconout_vec[h])
    - glbcolumn[h]           (state: per-device tab column counter)
  used by: Cconws, Cconrs, BDOS dispatcher (file-redirected fn=2 path)
  category: leaf

Cconws (0x09 / xconws):
  requires:
    - Cconout (via tabout()) — same dependencies as Cconout
    - run->p_uft[1]
  used by: programs (printf, puts), Cconrs (echo)
  category: branch

Cconrs (0x0A / xconrs):
  requires:
    - getch()      → Bconin BIOS:0x02 (via run->p_uft[0])
    - conout()     → Bconout BIOS:0x03
    - cookdout()   → conout()
    - terminate()  → Pterm(-32)  *** carries dependency on Pterm ***
    - run->p_uft[0,1]
  used by: programs (line input)
  category: branch  (depends on Cnecin core path + Pterm for Ctrl-C)

Cconin (0x01 / xconin):
  requires:
    - getch()/conbrk() → Bconin BIOS:0x02, Bconout BIOS:0x03 (for ^G)
    - run->p_uft[0]
    - terminate() → Pterm(-32)
    - typeahead buffer state (con_typeahead, KBBUFSZ=64)
  used by: programs (single-char read)
  category: branch  (Pterm dep)

Cnecin (0x08 / xnecin):
  requires:
    - same as Cconin minus echo
    - terminate() → Pterm(-32)
  used by: cgets() (Cconrs internal), programs
  category: branch  (Pterm dep)

Crawcin (0x07 / xrawcin):
  requires:
    - getch() → Bconin BIOS:0x02
    - run->p_uft[0]
  used by: programs (single-char read, no Ctrl-C/Ctrl-S handling)
  category: leaf   (no Pterm; raw means raw)

Crawio (0x06 / xrawio):
  requires:
    - constat() / getch() / Bconout — depending on parm (== 0xFF or not)
    - run->p_uft[0,1]
  used by: programs (combined raw I/O)
  category: leaf

Cauxin (0x03 / xauxin):
  requires:
    - Bconin BIOS:0x02 (with handle from run->p_uft[2])
  used by: programs (RS-232 read)
  category: leaf

Cauxout (0x04 / xauxout):
  requires:
    - Bconout BIOS:0x03 (handle from run->p_uft[2])
  used by: programs (RS-232 write)
  category: leaf

Cprnout (0x05 / xprtout):
  requires:
    - Bconout BIOS:0x03 (handle from run->p_uft[3])
  used by: programs (printer)
  category: leaf

Cconis (0x0B / xconstat):
  requires:
    - constat() → typeahead state + Bconstat BIOS:0x01
    - run->p_uft[0]
  used by: programs (poll keyboard)
  category: leaf

Cconos (0x10 / xconostat):
  requires: Bcostat BIOS:0x08
  used by: programs (poll output ready)
  category: leaf

Cprnos (0x11 / xprtostat):
  requires: Bcostat BIOS:0x08 (handle from run->p_uft[3])
  used by: programs
  category: leaf

Cauxis (0x12 / xauxistat):
  requires: constat() → Bconstat BIOS:0x01 + typeahead
  used by: programs
  category: leaf

Cauxos (0x13 / xauxostat):
  requires: Bcostat BIOS:0x08 (handle from run->p_uft[2])
  used by: programs
  category: leaf
```

### Console-group implementation order (suggested, leaf-first)

1. **Bconout** (BIOS 0x03) — leaf into hardware, mandatory dependency.
2. **Bconin** (BIOS 0x02), **Bconstat** (0x01), **Bcostat** (0x08).
3. `Cconout` (0x02), `Cauxout` (0x04), `Cprnout` (0x05) —
   1-line wrappers over Bconout.
4. `Cauxin` (0x03), `Crawcin` (0x07) — wrappers over Bconin.
5. `Cconis` (0x0B), `Cconos` (0x10), `Cprnos` (0x11),
   `Cauxis` (0x12), `Cauxos` (0x13) — status wrappers.
6. `Cconws` (0x09), `Crawio` (0x06) — small composites of (3).
7. `Cconin` (0x01), `Cnecin` (0x08), `Cconrs` (0x0A) — require
   `Pterm(-32)` for Ctrl-C handling. **Block on Pterm**.

This means the console group can be implemented **except for Ctrl-C**
without first implementing process management. Ctrl-C handling is
the single dependency that bridges to the process group.

**Update (D6 + process group spec'd):** `Pterm` spec is now in
`docs/api/gemdos/process/01-pterm.md`; the implementation dependency
for Ctrl-C is unblocked. Implementation order for Ctrl-C-aware
console reads:

```
Pterm  (process)              ── needs ── PD, MD, sft[], dirtbl[],
                                          Setexc(0x102, ...)
  └── Cconin / Cnecin / Cconrs (console with break)

Pexec  (process)              ── needs ── alloc_env, alloc_tpa,
                                          kpgmhdrld, kpgmld,
                                          init_pd_*, proc_go,
                                          gouser/termuser
  └── shell / AES / source-port loader

Ptermres  (process)           ── needs ── xsetblk, reserve_blocks,
                                          and Pterm transitively
  └── TSR-style installers
```

`Pexec` is the heavyweight one — it transitively pulls in the
filesystem (via `xopen` / `kpgmld`), so its full implementation must
wait for PHASE-05. The basepage-only modes (`PE_BASEPAGE = 5` and
`PE_BASEPAGEFLAGS = 7`) and `PE_GO`/`PE_GOTHENFREE` are
filesystem-free and can land in PHASE-06.

---

## BIOS — character device path (transitive deps used above)

```
Bconstat (BIOS 0x01):
  requires:
    - boot_status & CHARDEV_AVAILABLE
    - bconstat_vec[h]  (init by chardev_init)
    - per-handle driver: serport (h=1), IKBD (h=2), MIDI (h=3)
  used by: Cconis, Cauxis, constat()
  category: leaf  (modulo driver readiness)

Bconin (BIOS 0x02):
  requires:
    - boot_status & CHARDEV_AVAILABLE
    - bconin_vec[h]
    - per-handle driver: parport (h=0, dummy in practice),
      serport (h=1), IKBD queue (h=2), MIDI (h=3)
  used by: Cconin, Cnecin, Crawcin, Cauxin, Crawio (read mode)
  category: leaf  (modulo driver)

Bconout (BIOS 0x03):
  requires:
    - boot_status & CHARDEV_AVAILABLE
    - bconout_vec[h]  (passes (handle, what) to handler)
    - per-handle driver: parport (h=0), serport (h=1),
      cputc/VT52 (h=2), MIDI (h=3), IKBD-cmd (h=4),
      ascii_out raw (h=5)
  used by: every GEMDOS console output function, kprintf
  category: leaf  (modulo driver)

Bcostat (BIOS 0x08):
  requires:
    - boot_status & CHARDEV_AVAILABLE
    - bcostat_vec[h]  (note: MIDI/IKBD slots swapped vs. bcon{in,out}!)
  used by: Cconos, Cprnos, Cauxos
  category: leaf
```

---

## Future entries

Process / memory / time / drive / file / directory groups will be
appended here as their spec entries are produced.
