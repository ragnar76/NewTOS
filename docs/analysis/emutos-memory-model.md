# EmuTOS/TOS Memory Model — Extraction, NewTOS-Current, and the EL0/EL1 Reconciliation

**Status:** analysis + design recommendation (no product code).
**Author:** Researcher (Track A extraction per `docs/SOURCE-ANALYSIS-PROTOCOL.md`).
**Primary source:** EmuTOS `references/emutos/` (authority). All EmuTOS claims cite `file:line`.
**Secondary, sanctioned source:** FreeMiNT `references/freemint/` — per `CLAUDE.md`, the
source for MiNT extensions EmuTOS lacks; here specifically for **MMU memory protection**,
which EmuTOS (single-tasking, unprotected) never implements. FreeMiNT is the one TOS-family
OS that ran unmodified TOS/GEM binaries **under real MMU protection** (68030/040/060) — i.e.
it already solved "an app calls `Malloc` and dereferences the raw pointer; the MMU protects
it from everyone else," which is precisely NewTOS's EL0 problem. All FreeMiNT claims also
cite `file:line`.
**Purpose:** inform the next kernel task — an EL0 app's GEMDOS `Malloc`/`Mxalloc` must
return memory the app can actually touch (QED faults writing an unmapped TT-RAM pool
block; note `el0-app-memory-must-be-mapped`, ADR-020).

A **draft ADR-021** (the decision to lift into `docs/MEMORY.md`) is at the end.

---

## Part 1 — The EmuTOS memory model (faithful extraction)

### 1.1 Structures: `MD`, `MPB`, `PD` (`include/bdosdefs.h`)

**`MD` — Memory Descriptor** (`bdosdefs.h:108-115`). One node in a singly-linked list;
describes one contiguous run of memory, free *or* allocated:

```c
struct _md {
    MD    *m_link;   /* next MD, or NULL          */
    UBYTE *m_start;  /* start address of block    */
    LONG   m_length; /* number of bytes in block  */
    PD    *m_own;    /* owner's process descriptor */
};
```

**`MPB` — Memory Partition/Parameter Block** (`bdosdefs.h:120-126`). One per pool; the two
list heads:

```c
struct _mpb {
    MD *mp_mfl;    /* memory FREE list       */
    MD *mp_mal;    /* memory ALLOCATED list  */
    MD *mp_rover;  /* roving pointer - NO LONGER USED */
};
```

`mp_rover` is dead (comment `bdosdefs.h:125`); first-fit always starts at the list head.

**`PD` — Process Descriptor a.k.a. BASEPAGE** (`bdosdefs.h:66-97`). Memory-relevant fields:
`p_lowtpa` (0x00, start of TPA), `p_hitpa` (0x04, end-of-TPA+1), `p_tbase/p_tlen`,
`p_dbase/p_dlen`, `p_bbase/p_blen`, `p_env` (0x2C), `p_flags` (0x28). `p_flags` bits
(`bdosdefs.h:100-103`): `PF_FASTLOAD 0x01`, `PF_TTRAMLOAD 0x02`, `PF_TTRAMMEM 0x04`,
`PF_STANDARD = all three`.

### 1.2 The two pools and their init

Globals (`umem.c:35,37`; declared `mem.h:24,26`):
- `MPB pmd;`    — the main user **ST-RAM** pool (chip/low RAM).
- `MPB pmdalt;` — the **alternate / TT-RAM (FastRAM)** pool, only under `#if CONF_WITH_ALT_RAM`.
- `int has_alt_ram;` (`umem.c:38`) — whether any alt-RAM has been registered.

**BIOS seeds ST-RAM** in `bmem_init`/`getmpb` (`bios/biosmem.c`). `bmem_init` (`:51`)
computes `membot = end_os` (`:66-67`) and `memtop = phystop` (`:72`) — the screen buffer is
carved from the top later via `balloc_stram` (`:93`). `getmpb` (`:134`) fills the single
initial descriptor `themd` (`:144-150`):

```c
themd.m_start  = membot;
themd.m_length = memtop - membot;
themd.m_own    = NULL;
mpb->mp_mfl = &themd;   /* one big free block */
mpb->mp_mal = NULL;     /* nothing allocated  */
```

So ST-RAM starts as **one free MD spanning all available ST-RAM**, owner `NULL`.

**BDOS caches the ST-RAM extent** in `umem_init` (`umem.c:549`): it calls `Getmpb(&pmd)`
then records `start_stram = pmd.mp_mfl->m_start` and `end_stram = start + length`
(`:558-559`). These bounds are how `find_mpb` later routes an address to a pool
(§1.5). `umem_init` also sets `has_alt_ram = 0` (`:564`) and the ST-RAM alignment grain
`malloc_align_stram` (`:568-581`): **2 bytes** on TOS1-3 machines, **4 bytes** on Falcon
(`MCH_FALCON`); alt-RAM is always 4 (`MALLOC_ALIGN_ALTRAM = 3`, `mem.h:34`).

**Alt-RAM is registered later** — the pool is empty at boot and populated by `Maddalt`
(§1.5.5). `has_alt_ram` flips to 1 on the first successful `Maddalt` (`umem.c:513`).

### 1.3 The OS "meta-pool": where MDs come from (`osmem.c`)

MDs are not `malloc`'d from the user pools — they live in a **fixed internal OS pool**
`static WORD osmem[LENOSM]` (`osmem.c:64`), sized `NUM_OSM_BLOCKS = 118` blocks of 64 bytes
(`:36-39`). `xmgetmd`/`xmfremd` (`:169`,`:229`) hand out `MD`s grouped 3-per-`MDBLOCK`;
`xmgetblk`/`xmfreblk` (`:291`,`:360`) manage the 64-byte blocks, drawing from a LIFO
`root[4]` fast-list or bumping `getosm` (`:100`) through `osmem`. **Consequence:** the MD
supply is *finite and small*. When it is exhausted for a DMD/DND/OFD the system prints
`"Out of internal memory. Use FOLDR100.PRG..."` and **halts** (`osmem.c:340`); for an
`MDBLOCK` it fails softly (returns NULL, the alloc fails). This is why `ffit`'s "split"
path can fail with NULL even when memory is available (§1.4).

### 1.4 The allocation algorithm (`iumem.c`)

**`ffit(amount, mp)` — first-fit** (`iumem.c:35`). The workhorse for every pool op.

- **Max-free probe:** `amount == -1L` → walk `mp_mfl`, return the *largest* `m_length`
  cast to `(MD*)` (i.e. a size, not a real pointer) (`:66-74`).
- **Rounding:** `amount` rounded up to the pool grain — `malloc_align_stram` for `pmd`,
  `MALLOC_ALIGN_ALTRAM` (=4) otherwise (`:83-86`).
- **First fit:** walk `mp_mfl` for the first `m_length >= amount` (`:92-96`); NULL if none.
- **Exact fit:** unlink the whole MD from the free list (`:103-104`).
- **Split:** else `xmgetmd()` a new MD for the remainder; new MD gets
  `[m_start+amount, rest]` and is spliced into the free list; the original MD is trimmed to
  `amount` (`:113-127`). If `xmgetmd` returns NULL (meta-pool empty) the whole alloc fails
  (`:113-117`).
- **Publish:** prepend the allocated MD to `mp_mal`, set `m_own = run` (the current
  process) (`:132-134`), return the MD.

**`freeit(m, mp)`** (`iumem.c:144`): find `m` on `mp_mal` by `m_start` equality (`:167-169`);
unlink it (`:180-183`); insert into the **address-ordered** free list `mp_mfl` (`:191-202`);
**coalesce** with the higher neighbour (`:207-213`) and lower neighbour (`:215-221`),
returning the now-redundant MD(s) via `xmfremd`. A wild `m` (no matching `m_start`) is a
silent no-op (`:171-175`).

**`shrinkit(m, mp, newlen)`** (`iumem.c:228`): `xmgetmd` a descriptor for the freed tail
`[m_start+newlen, m_length-newlen]`, splice onto `mp_mal`, trim `m` to `newlen`, then call
`freeit` on the tail so coalescing is reused (`:235-260`). `-1` if the meta-pool is empty.

### 1.5 The public calls, mode bits, `-1` query, errors

Dispatched from `bdosmain.c`: `xmaddalt` 0x14 (`:195`), `xmxalloc` 0x44 (`:264`),
`xmalloc` 0x48 (`:268`), `xmfree` 0x49 (`:269`), `xsetblk` 0x4A (`:270`).

**Mxalloc mode bits** (`bdosdefs.h:17-22`): `MX_STRAM 0`, `MX_TTRAM 1`, `MX_PREFSTRAM 2`,
`MX_PREFTTRAM 3`, `MX_MODEMASK 0x03`.

**Error codes** (`include/gemerror.h`): `E_OK 0` (`:19`), `EINVFN -32` (`:40`),
`ENSMEM -39` (`:47`), `EIMBA -40` (`:48`), `EGSBF -67` (`:60`).

#### 1.5.1 `Malloc` (0x48) — `xmalloc` (`umem.c:116`)
Pure policy wrapper: `if (run->p_flags & PF_TTRAMMEM) xmxalloc(amount, MX_PREFTTRAM); else
xmxalloc(amount, MX_STRAM);` (`:120-126`). The preference is the **running process's own**
`p_flags`. Returns a pointer; **NULL (0) on failure** (there is no `ENSMEM` from `Malloc` —
0 means out of memory).

#### 1.5.2 `Mxalloc` (0x44) — `xmxalloc` (`umem.c:234`)
1. `mode &= MX_MODEMASK` (`:241`).
2. `amount == -1` → largest-free query (`:247-279`): `MX_STRAM` → `ffit(-1,&pmd)`;
   `MX_TTRAM` → `ffit(-1,&pmdalt)`; **`MX_PREFSTRAM`/`MX_PREFTTRAM` → the max across
   *both* pools** ("verified on TOS3", `:260-262`); unknown mode → NULL.
3. `amount <= 0` → NULL (`:284`).
4. Pool select (`:292-318`): `STRAM`→`ffit(&pmd)`; `TTRAM`→`ffit(&pmdalt)`;
   `PREFSTRAM`→`pmd`, fall back to `pmdalt`; `PREFTTRAM`→`pmdalt`, fall back to `pmd`.
5. Return `m->m_start`, or NULL (`:324-327`).

#### 1.5.3 `Mfree` (0x49) — `xmfree` (`umem.c:135`)
`find_mpb(addr)` (`:99-110`; ST-RAM range→`pmd`, else→`pmdalt`) → `EIMBA` if none
(`:142-144`); walk `mp_mal` for `m_start == addr` → `EIMBA` if not found (`:148-153`);
else `freeit` → `E_OK`.

#### 1.5.4 `Mshrink` (0x4A) — `xsetblk(n, blk, len)` (`umem.c:170`)
`n` ignored (historic CP/M field). `find_mpb`→`EIMBA` (`:177-179`); locate on `mp_mal`→
`EIMBA` (`:186-194`); round `len` to the pool grain (`:200-203`); then:
`m_length < len`→`EGSBF` (`:210-211`); `== len`→`E_OK` no-op (`:212-213`); `len == 0`→
`freeit` like Mfree (`:214-218`); else `shrinkit` → `E_OK` / `ERR` (`:223`).

#### 1.5.5 `Maddalt` (0x14) — `xmaddalt(start, size)` (`umem.c:455`)
Registers alt-RAM the BIOS did not find. `size &= -4` (`:460`); `size <= 0`→`-1` (`:463`);
reject overlap with ST-RAM: `((start<start_stram && start+size>start_stram) || start<end_stram)`
→`-1` (`:467-469`) (effectively: `start` must be ≥ `end_stram`). Try to **merge** with an
adjacent free MD on `pmdalt.mp_mfl` (extend end `:473-476` or start `:477-482`) → 0 on merge.
Else `xmgetmd`→`ENSMEM` if empty (`:495-497`); new MD `{start,size,own=NULL}` prepended to
`pmdalt.mp_mfl`; first-ever registration also sets `mp_mal=NULL`, `has_alt_ram=1` (`:499-516`).
Note the **new alt-RAM MD is `m_own = NULL`** — the pool is unowned until allocated.

#### 1.5.6 `Srealloc` (0x15) — video RAM (`umem.c:358`, `#if CONF_WITH_VIDEL`)
Falcon-only screen-RAM (re)allocation, *not* part of the normal pool — noted for
completeness; irrelevant to the EL0 problem.

### 1.6 TPA vs. free pool vs. ownership — the crux

**A program owns essentially *all* free RAM as its TPA.** In `xexec` (`proc.c:179`):
`needed = h01_tlen + h01_dlen + h01_blen + sizeof(PD)` (`:291`) is only an *availability*
threshold. `alloc_tpa(flags, needed, &max)` (`:460`) then **allocates the entire largest
free block**, not `needed`:

```c
st_ram_size = (LONG) ffit(-1L, &pmd);          /* largest free in ST-RAM */
...                                             /* (alt-RAM analogous)    */
if (alt_ram_available) { *avail = alt_ram_size; md = ffit(alt_ram_size, &pmdalt); return md->m_start; }
if (st_ram_available)  { *avail = st_ram_size;  md = ffit(st_ram_size,  &pmd);    return md->m_start; }
```
(`:466-499`). ST-vs-alt choice honours `PF_TTRAMLOAD` and a TT/ST tiebreaker using bits
31-28 of `p_flags` × `TPASIZE_QUANTUM` (128 KiB) (`:481-485`; `TPASIZE_QUANTUM` `:34`).
`init_pd_fields` then sets `p_lowtpa = p`, `p_hitpa = p + max` (`:373-374`) — **max = the
whole free block**, so the TPA spans all remaining RAM of the chosen type.

**Therefore, right after `Pexec`, the free list of that pool is empty** (the running
program holds it all). The program's C startup then calls `Mshrink(0, basepage, needed)`
to hand the unused tail **back to the free list**, and only *then* does its own
`Malloc`/`Mxalloc` have anything to carve — **from the very same region it just released**.
In single-tasking TOS the TPA and the Malloc pool are literally one managed free region in
one flat address space.

**Ownership (`m_own`) drives cleanup, not access.** `ffit` stamps `m_own = run` (`:134`);
`Pexec` re-stamps the basepage + env owner to the child for `PE_LOADGO` via `set_owner`
(`proc.c:305-307`; `set_owner` `umem.c:587`). On exit, `ixterm`→`free_all_owned(r, &pmd)`
and `&pmdalt` (`proc.c:110,136-139`) walk `mp_mal` and `freeit` every MD whose `m_own == r`
(`:92-101`). `Ptermres` instead *reserves* owned blocks — removes them from `mp_mal` so they
survive termination (`reserve_blocks` `:77-89`, `xtermres` `:624-632`).

`m_own` is a pure **pointer-equality tag** — never dereferenced for access. In flat-memory
TOS, once `Malloc` returns an address the program dereferences it directly; there is **no
per-process access control** — the sole role of `m_own` is *"whose blocks to reap at
`Pterm`"*.

### 1.7 Single-tasking (EmuTOS/TOS) vs. MiNT

EmuTOS is single-tasking: one `run` at a time; `Malloc` preference = `run->p_flags`;
cleanup = `free_all_owned(run)`. **MiNT** adds per-process memory contexts and honours
`m_own` for real per-process accounting/protection, plus `Mxalloc` "global/supervisor/
readable" protection bits in the high nibble of `mode` (which EmuTOS masks off with
`MX_MODEMASK`, `umem.c:241`). NewTOS today follows the EmuTOS single-tasking model with a
16-slot process table; the MiNT protection bits are a documented future concern, not part of
this design.

---

## Part 1B — FreeMiNT: the MMU-protected TOS (sanctioned secondary source)

FreeMiNT adds real memory protection (68030/040/060 MMU) to the same GEMDOS memory API
EmuTOS implements flat. It is gated behind `WITH_MMU_SUPPORT` (`sys/arch/mprot.h:14-21`) and
is a **complete no-op on 68000** — `sys/arch/mprot000.c` (a "dummy memprot module...compiled
for 68000-only kernels", `:26-29`) makes `mark_region`/`mark_proc_region` no-ops (`:50-60`)
and `get_prot_mode` always return `PROT_G` (`:44-48`) — i.e. on a 68000, FreeMiNT is exactly
as flat/unprotected as EmuTOS. Protection is a **machine-capability-gated feature**, same
category as EmuTOS's `CONF_WITH_ALT_RAM` (memory note `check-conf-gating-emutos-extraction`
applies to FreeMiNT too — the general engineering habit is a project sanctioned pattern, not
one lifted from FreeMiNT itself).

### 1B.1 Per-process memory + protection model — one global, identity-mapped space, per-process protection tables

FreeMiNT does **not** give each process a distinct virtual-to-physical mapping the way
NewTOS's per-process `TTBR0_EL1` does. The 68030 MMU code's own design comment is explicit
(`sys/arch/mprot030.c:17-19`):

> "Since, initially, the logical address space is the physical address space, we only need
> to worry about 26MB plus 32K for I/O space."

I.e. **one flat, identity-mapped logical=physical address space, shared by every process**
(`mprot030.c:22-38` lays out the fixed memory map: `0..membot` supervisor, `membot..memtop`
"protected TPA", etc.). What *is* per-process is the **page table** describing that same
physical layout: `struct memspace.page_table` (`sys/mint/proc.h:67-79`, field at `:74`) is a
private copy per process. `init_page_table` (`mprot030.c:824-825`) builds one process's
4-level table (A/B/C/D, 8 KiB pages, `mprot030.c:93-107`) covering the **entire** ST+TT-RAM
range, and sets that process's **root pointer** (`proc->ctxt[0].crp`, `mprot030.c:1025-1028`).
Each table entry's protection bits (not its target address — the address is always the same
physical page) differ per process depending on whether *that* process owns the region.

**Context switch = swap the MMU root pointer, not the mapping.** `change_context`
(`sys/arch/context.S:445-446`) saves the outgoing process's CRP (`pmove %crp,C_CRP(a0)`,
`:89,234`) and restores the incoming one (`pmove C_CRP(a0),%crp`, `:485`) on every switch.
So "the current process's protection context" is realised by loading a different **table**,
not a different **address range** — the CPU's MMU walks the same physical descriptors either
way; only the access bits it finds differ.

**Answering the framing question directly:** this is model **2** from the task — "one global
address space with per-page protection tagged by owner + mode" — realised via **per-process
tables that redundantly describe the same global layout**, because 68030-class MMUs have no
cheaper way to give each process a different *view* of the same address than to give it its
own full table. This is a materially different mechanism from NewTOS's per-process `TTBR0`
(which maps the *same low VA* to *different physical pages* per process — a true separate
address space per process, the modern/ARM-native idiom). See the reconciliation, §4.4a.

### 1B.2 The `Mxalloc` protection modes — `PROT_*` values, `F_PROTMODE` encoding, defaults

**The raw protection levels** (`sys/mint/mem.h:130-137`, "actual values...used as args to
alloc_region"):
```c
# define PROT_P        0   /* private: owner only */
# define PROT_G        1   /* global: any access OK, for anyone */
# define PROT_S        2   /* any *supervisor* access OK */
# define PROT_PR       3   /* any read OK, no write */
# define PROT_I        4   /* invalid page */
# define PROT_MAX_MODE 4
```
`mprot030.c:400-402` gives the per-mode table applied to **non-owner** processes:
```c
static const ushort other_dt[] = { 0, 1, 1, 1, 0 };  /* descriptor type: 0=invalid,1=valid */
static const ushort other_s[]  = { 0, 0, 1, 0, 0 };  /* supervisor-only bit */
static const ushort other_wp[] = { 0, 0, 0, 1, 0 };  /* write-protect bit */
```
So for a non-owner: `PROT_P`→invalid (no access at all); `PROT_G`→valid, user-mode R/W (true
shared memory); `PROT_S`→valid but supervisor-mode only; `PROT_PR`→valid, user-mode
**read-only**; `PROT_I`→invalid. **The owner** (or a process flagged `F_OS_SPECIAL`,
`mprot030.c:660-664,716-720`) always gets full valid R/W regardless of mode
(`mprot030.c:669-673`) — ownership trumps the mode entirely.

**`F_PROTMODE` in the `Mxalloc`/PRGFLAGS word** (`sys/mint/mem.h:120-127`):
```c
# define F_PROTMODE  0xf0   /* protection-mode bits, bits 4-7 */
# define F_PROT_P    0x00
# define F_PROT_G    0x10
# define F_PROT_S    0x20
# define F_PROT_PR   0x30
# define F_PROT_I    0x40
# define F_PROTSHIFT 4
```
These sit **above** the low ST/TT-preference bits: `F_XALLOCMODE = 0x03` (bits 0-1: `F_STONLY
0`, `F_ALTONLY 1`, `F_STPREF 2`, `F_ALTPREF 3` — `mem.h:112-116`, **numerically identical** to
EmuTOS's `MX_STRAM/MX_TTRAM/MX_PREFSTRAM/MX_PREFTTRAM`, confirming the ST/TT low-bit encoding
is universal across EmuTOS and FreeMiNT). Bit 14 is `F_KEEP` (`mem.h:105`, "don't auto-free" —
survives `Ptermres`-style reservation, per-allocation rather than whole-process).

**The encoding is genuinely nontrivial** (`sys_m_xalloc`, `dosmem.c:169-222`, esp.
`:205-222`): `protmode_raw = (mode & F_PROTMODE) >> F_PROTSHIFT`. If `protmode_raw == 0`
(caller passed no explicit `F_PROT_*` bits), the **process's own PRGFLAGS protmode is
inherited** (`dosmem.c:211-214`) — i.e. **the default protection for `Malloc`'d memory is
whatever the running program's own header says**, and for an ordinary unmodified GEMDOS
binary (header protmode field `= 0 = F_PROT_P`) that default is **`PROT_P` (private,
owner-only)**. If `protmode_raw != 0`, the final mode is `protmode_raw - 1`
(`dosmem.c:216`) — so to explicitly request e.g. `PROT_G`, the caller passes `F_PROT_G +
0x10 = 0x20` in the mode word (`mem.h:61-62` documents this exact "+0x10" offset: *"if it's
zero, you get the F_PROTMODE value from curproc's prgflags. Else you get (F_PROTMODE >>
F_PROTSHIFT)-1"*). **The TPA gets its protection the same way**, but via the **program
header's** flag word directly (`create_base`, `memory.c:1511,1513`: `protmode = (flags &
F_PROTMODE) >> F_PROTSHIFT` — no `-1`, since header flags are pre-shifted PROT_* values, a
distinct convention from the Mxalloc *argument* encoding). Default header protmode is `0`
(`F_PROT_P`) for an ordinary compiled binary, so **the TPA defaults to `PROT_P` (private)
too**.

**How an app requests shared/global memory:** pass `Mxalloc(size, mode | 0x20)` (i.e.
`F_PROT_G + 0x10`, decoding to `PROT_G = 1`). The block is then valid, user-mode R/W in
**every** process's page table (`other_dt[PROT_G]=1, other_s[PROT_G]=0, other_wp[PROT_G]=0`,
§1B.1's `mark_region`, `mprot030.c:616-690`) at the **same numeric address** it has in the
allocating process — because the address space is identity-mapped and global, a second
process reads/writes the exact pointer value the first process got back from `Mxalloc`. This
is FreeMiNT's entire shared-memory mechanism: no separate "shared memory" primitive, just
`PROT_G` on an ordinary `Mxalloc` block.

### 1B.3 How a process's memory becomes accessible — global pools, protect-on-allocate

**Allocation is from the same two global pools EmuTOS has**, not per-process arenas:
`MMAP core = &_core_regions; MMAP alt = &_alt_regions;` (`memory.c:210-211`), populated once
at boot by `add_region` during `init_core`/`init_mem` (`memory.c:222-223` ff.) — directly
analogous to EmuTOS's `pmd`/`pmdalt`. `get_region`/`_get_region` (`memory.c:667,686`) do a
`MEMREGION`-list first-fit exactly like EmuTOS's `ffit` (`MEMREGION` is FreeMiNT's `MD`:
`loc`/`len`/`links`/`mflags`/`next`, `sys/mint/mem.h:24-34`; `MMAP` is `MEMREGION**`,
`mem.h:147`). `alloc_region` (`memory.c:1241-1242`) then **attaches** the region to the
calling process (`attach_region`, `memory.c:511-551`) and calls `mark_region` with the
requested `PROT_*` mode (`memory.c:498, alloc_region.c:1256`, `mark_region` itself in
`mprot030.c:616-690`) — **allocate from the shared pool, then protect**, exactly the "protect
on allocate" step NewTOS's Option (a) needs to add (as a *map*, not a protection-bit flip).

**Ownership is tracked by the process, not by the block** (the reverse of EmuTOS's `m_own`):
`struct memspace.mem[]`/`addr[]`/`num_reg` (`sys/mint/proc.h:67-79`) is *the process's own
array* of region pointers/addresses it holds; `MEMREGION.links` (`mem.h:28`) is a reference
count (supports one region attached to several processes — `fork`, shared `PROT_G` blocks).
`addr2mem`/`valid_address` (`memory.c:1877-1918`) resolve "is this address mine" by scanning
the **calling process's own** `mem[]`/`addr[]` arrays, not a global sweep.

**Reclamation.** `Mfree` (`sys_m_free`, `dosmem.c:330-405`) refuses to free the caller's own
basepage (`:350-355`), otherwise `detach_region_by_addr` (`memory.c:613-652`) removes the
region from the caller's `mem[]`, decrements `links`, and `free_region`s it at `links == 0`
(the same TSR-reservation idea as EmuTOS's `Ptermres`/`reserve_blocks` shows up too: an
`M_KEEP`-flagged, links-exhausted block found in `rootproc`'s list can still be freed by
address, `dosmem.c:364-402`). `Mshrink` (`sys_m_shrink`, `dosmem.c:407` ff.) resolves the
block the same way, then calls `shrink_region` (`memory.c:1006-1007`). On **`Pterm`**,
`terminate()` (`k_exit.c:70-135`) calls `free_mem(pcurproc)` for a real exit (`:132-133`,
`ZOMBIE_Q`), which walks the **process's own** `p_mem->mem[i]` array (`proc_help.c:217-283`,
esp. `:245-263`), decrements `links`, `free_region`s at zero, and re-attaches `M_KEEP` blocks
to `rootproc` instead of freeing them (`:254-257`) — functionally the same "walk what I own,
release it" outcome as EmuTOS's `free_all_owned`, achieved via a *process-side* list rather
than a *block-side* owner pointer. `Ptermres` (`k_exit.c:307-340`) instead **flips**
still-privately-owned (`links==1`, `PROT_P`) TSR blocks to `PROT_G` (`:328-336`) so a later
process can see them, plus `shrink_region`s the basepage (`:320-322`) — the FreeMiNT analogue
of EmuTOS's `reserve_blocks`.

### 1B.4 Running unmodified apps — the compatibility trick

An unmodified TOS/GEM binary that just `Malloc`s and dereferences a raw pointer, expecting a
flat space, works unchanged under FreeMiNT because of **two facts holding simultaneously**:
1. **Identity mapping**: the pointer `Malloc` returns *is* the physical address, exactly as
   under flat TOS — no VA-vs-PA distinction the app could observe.
2. **Default protection is `PROT_P` (owner-only, full R/W)**: the *first* time any process
   touches its own `Malloc`'d/TPA memory, the CPU's MMU walk (through *that process's own*
   page table) finds a valid, unrestricted page — indistinguishable from no protection at
   all. Protection is invisible until a **different** process tries the same address (getting
   `PROT_P`'s "invalid" descriptor and a bus fault) — a case an unmodified single-process app
   never triggers against its own memory.

Classic GEMDOS `.PRG` binaries are **load-time relocatable** (a fix-up table lets the loader
place text/data anywhere the pool first-fit landed them, then patch absolute references) —
this is *why* identity-map-plus-protection works at all: the same binary loads at a
*different* physical/logical address on every run (wherever `alloc_region` found space), and
the relocation pass makes that address whatever it turns out to be. FreeMiNT does not need a
fixed load VA the way a non-relocating loader would.

### 1B.5 Summary for the reconciliation

FreeMiNT confirms: (i) **allocate from a single global ST/TT pool** (not per-process arenas)
— matches Part 4's rejection of option (b); (ii) the **"allocate, then make (in)accessible"**
two-step pattern is exactly right — NewTOS's equivalent of FreeMiNT's protection-bit flip is
a page-table **map/unmap**, because AArch64 processes don't share one identity-mapped layout
the way 68k FreeMiNT processes do; (iii) the sane **default is exclusive-owner access** —
FreeMiNT's `PROT_P` default corresponds to NewTOS mapping a fresh block into **only** the
requesting process's table; (iv) FreeMiNT's `PROT_G`/shared-memory mechanism is something
NewTOS's current design (§4.5) has **no equivalent for** — a fresh block is mapped into
exactly one process's window — which is a real, if currently out-of-scope, gap (§4.4a).

---

## Part 2 — How NewTOS does it now

NewTOS ports the EmuTOS allocator **faithfully at the data-structure level** but seeds the
pools with **physical addresses** and layers a separate EL0 mapping scheme on top.

### 2.1 The pool allocator is a faithful port
`tos-api/src/bdos/mem/pool.rs` is a line-by-line port of `iumem.c`: `ffit` (`:59`),
`largest_free` (a clean split-out of the `ffit(-1)` overload, `:150`), `freeit` (`:187`),
`shrinkit` (`:294`). `traps.rs` ports the `umem.c` entries: `maddalt` (`:157`),
`mxalloc` (`:262`), `malloc` (`:371`), `mfree` (`:458`), `mshrink` (`:516`), `find_mpb`
(`:394`), `set_owner` (`:417`). Mode bits, the `-1` "max across both pools" query
(`:281-292`), `EIMBA`/`EGSBF`/`ENSMEM` are all preserved. **Divergence noted in-code:**
the alignment grain is **16 bytes for both pools** (`PoolConfig::AARCH64`, `pool.rs:34`)
vs. EmuTOS 2/4 — an AArch64 SIMD baseline, documented in `4a-mshrink.md`.

### 2.2 Pools hold physical addresses; `Malloc` returns a PA
`init.rs::init` (`:143`) seeds `pmd`/`pmdalt` from kernel-discovered `(start, len)` tuples,
storing them as `MD.m_start` **physical addresses** narrowed to `Long`=`i32` via
`seed_md` (`:195`) — hence the `< 2 GiB` gate (`InitError::AddressOutOfRange`, `:199-205`).
`Mxalloc`/`Malloc` therefore return a **pool physical address** (`m_start`). The kernel can
touch it because all DRAM is linearly mapped in the high half at `phys + VA_OFFSET`
(`0xFFFF_0000_0000_0000`, e.g. `arena.rs:59`, `loader.rs:61`). `malloc` currently forwards
unconditionally as `MX_PREFTTRAM` (kernel sentinel PD treated as `PF_TTRAMMEM`,
`traps.rs:371-374`).

### 2.3 Per-process fixed 4 MiB TPA, mapped into TTBR0 at VA 0
The loader does **not** give a process "all free RAM". `arena.rs` allocates a **fixed 4 MiB
TPA** (`TPA_SIZE`, `:52`) via `Mxalloc(TPA_SIZE + page, MX_PREFTTRAM)` (`:104-121`),
page-aligns it, and the loader (`loader.rs`) maps every 4 KiB page of it into the process's
`TTBR0_EL1` at low-half VA `[USER_VA_BASE=0, USER_TPA_TOP=4 MiB)` (`:481-494`) as
`UserRwExec`. The basepage's `p_lowtpa=0`, `p_hitpa=4 MiB` are therefore **user VAs**, not
PAs (`:590-591`). Per-slot page tables are **static** (`mmu.rs`): a fixed L0/L1/L2 chain
plus `USER_L3_COUNT = 3` L3 leaves (`:63`), giving `USER_TPA_MAX_VA = 6 MiB` of mappable
low-half VA (`:70`) — 2 L3s for the TPA, 1 spare. `TCR_EL1.T0SZ = 16` (`boot.rs:176`), so
the low-half VA space itself is the full 48 bits; **the 6 MiB cap is purely the static
table count, not the TCR.**

### 2.4 The RSC spare-window (task #99) — the only existing "map a pool block into EL0"
`aes/resource.rs` already solves a narrow instance of the exact problem: an EL0
`rsrc_load` puts the `.RSC` in a BDOS-pool block and **maps it into the process's spare L3
window** above the TPA (`RSC_WINDOW_BASE = TPA_SIZE`, `:481`; `RSC_WINDOW_SIZE = 2 MiB`,
`:484`), then `rsrc_gaddr` returns the **user VA**. It maps pages via
`mmu::map_page(slot, app_va+off, base_pa+off, UserRwNx)` and `publish_user_mappings`
(`:694-712`), and tracks a **single** loaded RSC per process in one `AtomicI32` (`USER_RSC_RAW`,
`:495`), advancing a cursor to a *fresh* VA on reload so it never has to TLBI before the
return-to-EL0 `eret` (the QUIRK-001 QEMU wedge, `mmu.rs:326` docs). This is the mechanism the
memory note says to generalise.

### 2.5 Owner model + Pterm cleanup
`m_own` is a pointer-equality cookie = the child's basepage kernel VA (`current_owner`
`traps.rs:92`, `KERNEL_PD_SENTINEL` for kernel allocs `:65`). The loader re-tags the TPA and
env blocks to the child (`loader.rs:382,709`), and `Pterm`'s `free_all_owned` reaps them —
a faithful port of `proc.c` ownership. `current()` = `RUN` basepage; `slot_for(bp)` maps a
basepage back to a slot (`process.rs:426,552`); `MAX_PROCESSES = 16` (`:62`).

---

## Part 3 — Divergences (EmuTOS ⇄ NewTOS)

| Aspect | EmuTOS / classic TOS | NewTOS current | Contract or internal? |
|---|---|---|---|
| Address space | one flat physical space; any PA dereferenceable | EL0 app sees only its `TTBR0` pages | **Enabling constraint** (NewTOS runs at EL0) |
| `MD.m_start` / `Malloc` return | a real address the program uses directly | a **pool PA**, kernel-high-half reachable, **not** EL0-mapped | Divergence at the heart of the bug |
| TPA size | **all free RAM** of the chosen type; program `Mshrink`s the rest back | **fixed 4 MiB**, pool block mapped at VA 0 | Internal (observable size differs — see below) |
| TPA ⇄ Malloc pool | same region; Malloc carves from Mshrink'd tail | TPA is one pool block; Malloc hits the *global* pool at scattered PAs | Divergence |
| Basepage `p_lowtpa/p_hitpa` | real addresses | **user VAs** (0, 4 MiB) | Internal, but breaks `Mshrink(basepage)` (below) |
| Pool grain | 2 B (TOS1-3) / 4 B (Falcon) ST, 4 B alt | 16 B both | Internal (documented) |
| Ownership | `m_own` = reap tag only | same | Faithful |
| `Malloc(-1)` | largest free block in the real pool(s) | largest free in the real pool(s) — a **PA-space** size, ~1 GB | Divergence (unbounded vs. app's mappable space) |

**The load-bearing divergence and the bug.** An EL0 app calls `Mxalloc` → gets a **pool
PA** (e.g. QED's `0x0160a000`, a TT-RAM block). That PA is mapped in the *kernel* high half,
but **not** in the app's `TTBR0_EL1` (which only covers `[0, 6 MiB)` — TPA + RSC window). The
app dereferences the returned pointer → **EL0 translation/permission fault**. This is not a
QED quirk: every TT-RAM-aware program requests TT-RAM heaps and dereferences them (note
`el0-app-memory-must-be-mapped`).

**A second, latent instance of the same gap:** `Mshrink(basepage, len)` from an EL0 app
passes the basepage **user VA 0**. `find_mpb(0)` (`traps.rs:394`) tests the ST-RAM PA range,
misses, routes to `pmdalt`, walks `mp_mal` for `m_start == 0`, finds nothing → **`EIMBA`**.
The classic crt0 `Mshrink` therefore cannot work today either, because the address the app
holds (a user VA) is not the address the pool is keyed on (a PA). Any faithful fix must make
the app-visible address and the pool key reconcilable **for the TPA too**, not just for
`Malloc`.

**Where FreeMiNT fits in the table.** FreeMiNT is architecturally a *third* point, not a
mid-point between the other two: like EmuTOS it uses **one global, identity-mapped address
space** (§1B.1) — closer to EmuTOS's row 1 than to NewTOS's per-`TTBR0` model — but like the
NewTOS *requirement* it enforces **per-process access control** over that space, via
per-process page tables carrying only *protection bits* (not different target addresses).
NewTOS cannot reuse identity-mapping directly (its loader is non-relocating, §1B.4 explains
why FreeMiNT could and NewTOS currently cannot); see §4.4a for what NewTOS *does* take from
this.

---

## Part 4 — The reconciliation (design)

### 4.1 What must stay faithful (the contract, per `docs/COMPATIBILITY.md`)
- MD/MPB first-fit semantics, split/coalesce, `Mshrink`/`Mfree` behaviour, `EIMBA`/`EGSBF`.
- ST-RAM vs TT-RAM **preference honoured with the real RAM type** — an app that asks for
  (fast) TT-RAM must get TT-RAM, just at an address it can touch.
- `Malloc(-1)`/`Mxalloc(-1,...)` "largest free" query.
- `m_own` reap-on-`Pterm`.
- Returned pointers must be valid **ILP32** values (ADR-020: the app ABI is 32-bit pointers)
  — i.e. representable in 32 bits and, ideally, positive `Long`.

### 4.2 What is free to differ (internal, per `COMPATIBILITY.md`)
- The **numeric value** of the address (an EL0 app cannot observe the physical address and
  must not — it has no DMA). So *how* a pool PA is presented to EL0 (identity VA, offset VA)
  is a NewTOS implementation detail.
- The exact TPA size and whether the Malloc pool is physically the same region as the TPA.

### 4.3 The three options

**(a) Per-process EL0 window on demand — generalise the #99 RSC window.**
Keep the global EmuTOS pool allocator unchanged. On an **EL0** `Malloc`/`Mxalloc`, allocate
the block from the real pool (honouring ST/TT), **map its pages into a per-process EL0 heap
window**, return the **user VA**, and record `(user_va → pool m_start, size, pages)` in a
per-process table. `Mfree`/`Mshrink`/`Pterm` translate user_va→m_start via that table and
unmap. EL1/kernel callers keep the raw PA.

**(b) One large per-process mapped region carved by the MD allocator inside EL0.**
Give each process a big pre-mapped arena (an enlarged TPA); run a **second, per-process** MD
allocator whose pool *is* that arena. `Mxalloc` carves from the pre-mapped arena, so every
returned VA is already mapped. Closest to the classic "program owns its TPA and
sub-allocates".

**(c) Hybrid / identity-map.** Variants combining the above, or identity-mapping pool PAs
into EL0 (user VA = PA) so returned pointers numerically match classic TOS.

### 4.4 Evaluation

**(b) breaks ST/TT faithfulness and wastes RAM.** A single per-process arena has one RAM
type; it cannot serve an `MX_STRAM` request from a TT-RAM arena or vice-versa. Restoring
faithfulness needs *two* arenas per process (ST + TT), each a fixed pool block sized at
`Pexec` — which (i) re-introduces the "megabytes of resident BSS for idle slots" problem the
current `arena.rs` doc explicitly moved *away* from (`arena.rs:14-24`), (ii) caps each
process's total memory at arena-decision time, and (iii) hides the app's sub-allocations from
the global pool, so `Malloc(-1)` reports the arena's free space, not the system's. It also
duplicates the allocator. Rejected as the primary model.

**(c) identity-map is the most faithful *addressing*, but cannot be uniform.** With
`T0SZ = 16` the low-half VA already spans 48 bits, so mapping a pool PA at `user VA = PA` is
architecturally allowed and needs **no TCR change**; the returned pointer would even match
classic TOS (QED's `0x0160a000` would be a real EL0 VA). *But* the TPA **image** (text/data)
must live at its **ELF-linked VA** (the loader maps segments at their fixed `vaddr`, base
`0x100`; it does **not** relocate — `loader.rs:441`, `validate_segment` `:754`). So the image
is inherently offset-mapped at low VA and *cannot* be identity-mapped to a pool PA. Identity
mapping would thus apply only to heap blocks, giving a **split model** (image offset-mapped,
heap identity-mapped) and *still* leaving `Mshrink(basepage@VA 0)` needing a translation.
Identity-map also requires **sparse/dynamic page tables** to cover scattered PAs across up to
2 GiB (the static 3-L3 scheme cannot), a larger MMU change. Deferred (see 4.6).

**(a) is faithful, uniform, and minimal.** The global pool allocator — the extracted spec —
stays **byte-for-byte**: every `Malloc`/`Mxalloc`/`Mfree`/`Mshrink` still hits the real
`ffit`/`freeit`/`shrinkit` on the real ST/TT pools, so ST/TT preference, split/coalesce,
`m_own`, `EIMBA`/`EGSBF` are unchanged. The only addition is a thin **address-translation +
EL0-mapping shim** at the EL0 trap boundary — exactly "generalise the RSC window" from the
memory note. It also **fixes `Mshrink(basepage)`** for free: register the TPA block as the
first entry in the same per-process table (its user VA 0 ↔ its pool `m_start = raw_start`), so
`Mshrink`/`Mfree` of the TPA translate through the same path as heap blocks.

### 4.4a Does FreeMiNT confirm option (a), or suggest something cleaner?

**FreeMiNT confirms the pattern, not the mechanism.** Stripped of 68k-specific detail,
FreeMiNT's design is: *one global ST/TT pool feeds every process; on allocation, make the
block accessible to its owner and (by default) inaccessible to everyone else; on free/Pterm,
reverse that.* That is exactly option (a)'s shape — global pool untouched, an
allocate-then-grant / free-then-revoke step wrapped around it. FreeMiNT does **not** suggest
option (b) (per-process arenas) — its pools are as global as EmuTOS's, which independently
corroborates §4.4's rejection of (b) from a second, MMU-protected TOS-family source.

**Why FreeMiNT's *mechanism* (identity-map + per-process protection-only tables) does not
transfer, and why that is not a shortcoming of the NewTOS design:** FreeMiNT's mechanism is
only cheap on a 68030-class MMU because (i) the physical address space is small (≤ a few
hundred MB) so replicating a *full* page table per process is affordable, and (ii)
identity-mapping is only viable because classic GEMDOS binaries are **load-time relocatable**
(§1B.4) — the same binary tolerates loading at a different address on every run, which is
precisely what a shared, first-fit, identity-mapped pool requires. NewTOS's loader
(ADR-018/ADR-020) is a **non-relocating ELF loader**: every process's image is linked at, and
loaded at, the fixed VA `0x100` (`loader.rs:441` region), which *requires* per-process VA
remapping regardless of what the allocator does — precisely the per-`TTBR0` model NewTOS
already has, and precisely the modern/ARM-native idiom (cheap arbitrary VA→PA remapping is
what `TTBR0` is *for*; a 68030 MMU has no equivalent primitive at that granularity/cost).
**NewTOS's per-process address-space model is therefore not a weaker imitation of FreeMiNT's
— it is the ISA-appropriate generalisation of the same "isolate the owner" goal**, achieved
by remapping instead of by table-replication-with-differing-permission-bits. Option (a)'s
window-mapping is the correct AArch64 realisation of FreeMiNT's "allocate then grant access"
step.

**Where FreeMiNT *does* sharpen the recommendation:**
- It independently confirms **default = owner-exclusive** (FreeMiNT's `PROT_P` default,
  §1B.2) is the right default for NewTOS's mapped blocks too: map a fresh allocation into
  **only** the requesting process's window, matching §4.5 step 3 as already designed — no
  change needed, just added confidence.
- It **exposes a real, currently-unaddressed gap**: FreeMiNT's `PROT_G` gives a documented,
  supported way for one process's `Mxalloc` block to become visible to *every* process at the
  same address (§1B.2) — genuine inter-process shared memory. NewTOS's §4.5 design maps a
  block into exactly **one** process's window; there is no path today for a second process to
  see it. This is not a defect in the recommendation (NewTOS is currently single-app-at-a-time,
  §1.7/§2.5), but it is a **documented forward gap**: a future multitasking NewTOS wanting
  FreeMiNT-style shared memory needs the window manager extended to map one physical block
  into *several* processes' page tables. Flagged for later, not designed now (see the `PROT_*`
  deferral below).
- It reinforces **why identity-map is the correct v2 target** (§4.6): FreeMiNT shows that
  identity-map-plus-protection is the *natural* end-state once binaries no longer need a fixed
  load VA — exactly the condition the LP64/PIC-relocatable path (#91) would create. FreeMiNT
  is independent, working proof that this end-state is sound, not merely "TCR permits it."

**Should NewTOS adopt FreeMiNT's `PROT_*` mode bits on `Mxalloc` now?** **Recommend: defer,
mask them exactly as EmuTOS does.** Rationale:
1. **EmuTOS itself — the primary source — ignores these bits.** `xmxalloc` does
   `mode &= MX_MODEMASK` (`umem.c:241`) before ever looking at `mode`; the `F_PROTMODE` nibble
   (bits 4-7) is a **MiNT-only extension**, not part of the base GEMDOS contract EmuTOS/TOS
   implements. Masking it (as NewTOS already does) is fully spec-faithful — not a stub, not a
   missing feature relative to the contract NewTOS targets today.
2. **No current requirement exists.** NewTOS is single-app-at-a-time (§2.5); there is no
   second process to grant/deny access to, so `PROT_G`/`PROT_S`/`PROT_PR`/`PROT_I` have no
   observable effect to implement yet.
3. **ABI implication of adopting now vs. later:** adopting the bits means (a) committing to
   *an* encoding — FreeMiNT's own "`+0x10`, inherit-from-prgflags-if-zero" convention
   (§1B.2) is a legacy quirk worth reconsidering rather than copying blindly if/when this is
   revisited; (b) extending the window manager (§4.5) to support one block mapped into
   *multiple* processes' tables (only `PROT_G` needs this; `PROT_P`/`PROT_S`/`PROT_PR`/`PROT_I`
   are expressible with the single-owner window NewTOS already plans) — real, non-trivial
   scope, not a flag to thread through today. Deferring costs nothing now (`MX_MODEMASK`
   masking is forward-compatible: unmasking later does not break any program that never set
   the bits) and avoids building unused plumbing.
4. Per the project's own sourcing rule (`CLAUDE.md`: "FreeMiNT is the source for MiNT
   extensions EmuTOS lacks") and the memory note `freemint-extension-direction`, adopting
   `PROT_*` belongs with **real multitasking**, when NewTOS actually needs FreeMiNT-compatible
   process isolation/sharing semantics — not before.

This is a scoping recommendation, not a technical uncertainty; flagged here for the lead's
confirmation since it is a forward-ABI decision, not because the source analysis is unclear.

### 4.5 Recommendation — Option (a): per-process EL0 heap window + block table

Realise a **per-process EL0 user-memory manager** that wraps the unchanged global pool:

1. **Per-process VA layout (all low-half, ILP32-safe):**
   `[0, USER_TPA_TOP)` = TPA image + stack (as today);
   `[USER_TPA_TOP, USER_TPA_TOP + HEAP_WINDOW)` = the **EL0 heap window** into which
   `Malloc`/`Mxalloc` blocks (and the RSC blob — fold #99 into this) are mapped. Keep the
   window top well under 2 GiB so every returned pointer is a positive `Long`.

2. **Per-process block table** (replaces the single `USER_RSC_RAW` static): entries
   `{ user_va, m_start (pool PA / Long), size, page_count, kind }`. Seed **entry 0 = the TPA**
   at load time (`user_va = 0`, `m_start = raw_start`). Sized for the max simultaneous
   GEMDOS allocations an app holds (mintlib `malloc` grabs *large* chunks and sub-allocates in
   user space, so GEMDOS `Malloc` is called rarely with big sizes — a few dozen entries
   suffice; make it a documented cap).

3. **EL0 `Mxalloc(amount, mode)`:** call the unchanged `traps::mxalloc` → pool PA `p`
   (real ST/TT block). Page-round, map `ceil(size/4K)` pages of `[p, p+size)` at the next
   window VA (bump-cursor, fresh VA per alloc as the RSC path does — dodges the
   TLBI-before-`eret` QUIRK-001), `publish_user_mappings`, record the entry, **return the
   user VA**. ST/TT preference is intact because `p` came from the requested pool.

4. **EL0 `Malloc(amount)`:** as EmuTOS — `PF_TTRAMMEM ? MX_PREFTTRAM : MX_STRAM` on the
   caller's `p_flags`, then the step-3 path.

5. **EL0 `Mfree(user_va)` / `Mshrink(user_va, len)`:** look up `user_va` in the table →
   `m_start`; call the unchanged `traps::mfree`/`mshrink` on `m_start`; on `Mfree` (and
   `Mshrink`-to-0) drop the entry and leave the window pages unmapped-and-not-reused (fresh
   cursor) or unmap in EL1 context. `Mshrink` shrinks the pool block *and* the recorded
   size; the freed tail's window pages become reusable. `EIMBA` if `user_va` is not a table
   entry — matching EmuTOS's "not on `mp_mal`" `EIMBA`.

6. **EL0 `Malloc(-1)`/`Mxalloc(-1,mode)`:** the honest answer is bounded by what the app can
   *map*, not the ~1 GB physical pool. Report `min(pool largest-free for mode, remaining
   HEAP_WINDOW bytes)`. This stays contract-faithful ("the largest block you can actually
   allocate") because an EL0 app can never use more than it can map. Document this as the one
   deliberate contract adaptation. **EL1/kernel** `Malloc(-1)` reports the true pool size.

7. **EL0 vs EL1 discrimination:** the shim runs **only for SVC-originated (EL0) callers**.
   Kernel/desktop code calls `bdos::mem::traps::*` directly and keeps raw PAs. `TrapArgs`
   (`trap.rs:57`) does not currently carry the source EL; thread an explicit
   "on-behalf-of-EL0-process(slot)" flag from the exception vector into the GEMDOS memory
   dispatch (`gemdos/memory.rs`), or route EL0 memory traps through the new manager while
   EL1 uses `traps::*`. **This is the one new plumbing decision the implementer must make;
   flag for the lead.**

**Mapping choice — offset-map, not identity-map, for now:** user VA ≠ PA. The heap window is
a compact low-VA range → tiny page tables (bump `USER_L3_COUNT`), 32-bit-safe pointers, and a
*uniform* model with the offset-mapped TPA image. The translation table is the price; it is
also what fixes `Mshrink(basepage)`. Alignment: pool blocks are 16-B aligned but pages are
4 KiB; **map at page granularity and do not pack two pool allocations into one mapped page**
(a shared page would be wrongly unmapped on the first `Mfree`). Simplest: over-map — round
each allocation's window VA to a page and map `ceil(size/4K)` pages (≤ ~4 KiB VA slack per
allocation; negligible for the few large GEMDOS allocations real apps make). Return
`window_va` (page-aligned) — the block's usable start.

### 4.6 Faithfulness under future paths
- **ST vs TT survives mapping:** the block still comes from `pmd`/`pmdalt` per `mode`; the
  app gets genuine fast TT-RAM, merely at a mapped VA. Preference logic is untouched.
- **`Malloc(-1)`:** window-bounded for EL0, true-pool for EL1 (4.5.6).
- **LP64-native (#91, task deferred):** an ELF64/PIC-relocatable native app could be loaded
  at an arbitrary base, which *unlocks the identity-map end-state* (4.3c): map pool PAs at
  `user VA = PA` with dynamic per-process page tables, drop the translation table, and return
  pointers that equal `m_start` — the maximally faithful model, viable precisely because a
  relocatable image no longer forces the low-VA offset map. Recommend identity-map as the
  **v2 target** once dynamic tables + a relocating/PIC loader exist; the block-table API from
  v1 degenerates to identity there.
- **Multitasking:** the global pool stays the single source of truth for physical RAM;
  per-process windows are just views, and `m_own` already scopes reclamation. No per-process
  physical arenas to fragment (the (b) failure mode).
- **Inter-process shared memory (deferred, per §4.4a):** FreeMiNT's `PROT_G` shows the target
  shape — one physical block, mapped into *several* processes' windows at (not necessarily
  identical) VAs, all pointing at the same PA. NewTOS's block table (§4.5.2) already records
  `m_start`↔`user_va`, so extending one entry to a *list* of `(slot, user_va)` pairs is the
  natural growth path when multitasking + `Mxalloc` `PROT_*` bits are adopted — not designed
  further here.

### 4.7 Concrete change surface (for the implementation task — no code here)

- **New:** `tos-api/src/bdos/process/` (e.g. `usermem.rs`) — the per-process EL0 block table
  + window cursor + `map/unmap/translate(user_va)↔(m_start)`; generalises
  `aes/resource.rs:461-745`. Register the TPA as entry 0 at load
  (`loader.rs::populate_and_map`, near `:481-506`).
- **Edit `tos-api/src/gemdos/memory.rs`** (`mxalloc`/`malloc`/`mfree`/`mshrink`, `:38-86`):
  for EL0 callers, wrap `bdos::mem::traps::*` with the manager (allocate→map→return user VA;
  translate on free/shrink). `bdos/mem/traps.rs` and `bdos/mem/pool.rs` stay **unchanged**
  (the faithful core).
- **Edit `tos-api/src/bdos/process/mmu.rs`:** bump `USER_L3_COUNT` (`:63`) to size
  `HEAP_WINDOW`; keep the static-table scheme (or introduce on-demand L3 allocation if the
  window is large). `map_page`/`publish_user_mappings` are reused as-is.
- **Edit `tos-api/src/aes/resource.rs`:** fold `USER_RSC_RAW`/window (`:461-745`) into the
  new per-process table (one owner of the EL0 window; the RSC becomes just another mapped
  block). Backward-compatible refactor.
- **Plumb the EL0-caller signal** from the kernel SVC exception path into
  `gemdos/memory.rs` (4.5.7) — the single design decision to confirm with the lead.
- **Update specs:** add a "NewTOS EL0 realization" section to
  `docs/api/gemdos/memory/{44-mxalloc,48-malloc,49-mfree,4a-mshrink}.md` and
  `docs/api/structures/md.md` describing the user-VA return + window + the `Malloc(-1)`
  window bound. `docs/STATUS.md`: record the EL0-memory work item.

**No `.rs` product code is written by this analysis** (Researcher scope). The above is the
spec+design the Developer implements against.

---

## Draft ADR-021 (to lift into `docs/MEMORY.md`)

### ADR-021: EL0 apps get GEMDOS memory via a per-process mapped window; the pool allocator stays EmuTOS-faithful

**Date:** 2026-07-20 · **Status:** Accepted — promoted to `docs/MEMORY.md#adr-021` (2026-07-20) · **Phase:** cross-cutting (BDOS mem + process MMU)

**Context.** Classic TOS is flat/unprotected: a program dereferences any PA `Malloc`/
`Mxalloc` returns. NewTOS runs apps at EL0 behind `TTBR0_EL1`; the pool returns physical
addresses that are mapped only in the kernel high half, so an EL0 app faults touching them
(QED `Mxalloc`→`0x0160a000` write fault; note `el0-app-memory-must-be-mapped`). The TPA
image is loaded at a fixed low VA (non-relocating loader), and `Mshrink(basepage@VA 0)`
already fails `EIMBA` for the same address-mismatch reason. **FreeMiNT** (`references/freemint/`,
the sanctioned MiNT-extension source) is the one TOS-family OS that solved memory protection
for unmodified TOS/GEM apps: one global, identity-mapped ST/TT pool (`sys/memory.c:210-211`),
per-process page tables holding only *protection bits* over that same layout
(`sys/arch/mprot030.c:17-19,824-825`), default owner-exclusive access (`PROT_P`,
`sys/mint/mem.h:130`), and an explicit `PROT_G` mode for true shared memory
(`sys/mint/mem.h:122`). Its identity-map mechanism relies on load-time-relocatable binaries
(§1B.4) and does not transfer to NewTOS's non-relocating, fixed-load-VA ELF loader — but its
*pattern* ("global pool; allocate then grant the owner exclusive access; reverse on
free/`Pterm`") independently confirms the design below.

**Decision.** Keep the ported EmuTOS pool allocator (`bdos/mem/{pool,traps}.rs`) **unchanged
and authoritative**. Add a thin **per-process EL0 memory manager** that, for SVC-originated
callers, allocates from the real ST/TT pool, **maps the block into a per-process EL0 heap
window**, returns the **user VA**, and tracks `user_va↔m_start` for `Mfree`/`Mshrink`/`Pterm`.
This generalises the task-#99 RSC window and also registers the TPA as its first entry
(fixing `Mshrink(basepage)`). EL1/kernel callers keep raw PAs. `Malloc(-1)` for EL0 reports
`min(pool-free, window-free)`; ST/TT preference is preserved because the block still comes
from the requested pool. Offset-map now (compact window, 32-bit-safe, tiny tables);
identity-map with dynamic tables is the v2 target once the LP64/PIC-relocatable path (#91)
lands — the FreeMiNT precedent (§1B.4/§4.4a) is independent evidence that identity-map is the
right end-state once binaries no longer need a fixed load VA. **`Mxalloc`'s `F_PROTMODE`
bits (FreeMiNT's `PROT_P`/`G`/`S`/`PR`/`I`) stay deferred**: continue masking them exactly as
EmuTOS does (`mode &= MX_MODEMASK`) until real multitasking needs FreeMiNT-compatible
process isolation/sharing (§4.4a) — EmuTOS, the primary source, ignores these bits too, so
deferring is spec-faithful, not a gap.

**Rationale.** Faithful (allocator untouched; contract preserved), uniform (image + heap +
RSC all offset-mapped through one table), and minimal (a boundary shim). Option (b)
(per-process arenas) breaks ST/TT faithfulness, caps memory at `Pexec`, and wastes resident
RAM — independently corroborated by FreeMiNT also using global, not per-process, pools. Pure
identity-map cannot be uniform while the loader places images at fixed VAs — FreeMiNT shows
*why* this is a loader property (relocatable binaries), not a limitation of the recommended
design. NewTOS's per-process `TTBR0` (map same VA to different PA per process) is the
ISA-appropriate generalisation of FreeMiNT's per-process-table-over-one-global-layout
mechanism, not a weaker substitute for it (§4.4a).

**Consequences.** + EL0 apps get dereferenceable Malloc/Mxalloc memory of the right RAM type;
`Mshrink(basepage)` works; RSC window subsumed; default owner-exclusive access matches both
EmuTOS's a-process-owns-its-blocks model and FreeMiNT's `PROT_P` default. − A per-process
block table + window cap; an EL0 `Malloc(-1)` bounded by mappable space (documented
adaptation); one new EL0-caller signal to plumb from the SVC path; **no inter-process shared
memory** yet (FreeMiNT's `PROT_G` has no NewTOS equivalent — documented forward gap, §4.4a/
§4.6). ± Returned pointers no longer equal PAs (fine — EL0 apps do no DMA).

**Related.** ADR-020 (ILP32 pointers bound the window), ADR-010/QUIRK-001 (no TLBI before
`eret`; fresh-VA cursor), task #99 (RSC window — subsumed), task #91 (LP64/identity-map v2),
note `el0-app-memory-must-be-mapped`, note `freemint-extension-direction`, ADR (future):
`Mxalloc` `PROT_*` bits + multi-process window sharing, once real multitasking lands.
