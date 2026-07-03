# `Pexec` (GEMDOS 0x4B)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/proc.c:179` (`xexec`) and its helpers in
> `bdos/proc.c` / `bdos/kpgmld.c`.

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** trap ID/ABI, the seven mode constants (0/3/4/5/6/7 + the
> `EINVFN` default) and their observable semantics, the returned value shape
> per mode (basepage pointer vs. child exit code), the `PD`/basepage byte
> layout the program receives, the program-header (`PGMHDR01`) + `PRGFLAGS`
> layout, the command-tail 125-byte + length-byte format, the environment
> double-NUL format + inheritance rule, and every `E*` error.
> **Reference (NewTOS reimplements freely):** the loader/relocation mechanics
> (`kpgmld`/`pgmld01`/`pgfix01`), `alloc_tpa`'s tie-break policy, the m68k
> `gouser`/`proc_go` register set-up, `supstk`, the `setjmp`/`bakbuf` longjmp
> plumbing, `set_owner`/`m_own` — these are how EmuTOS does it, not the
> contract. NewTOS runs the loaded program **at EL0 with a per-process TTBR0**,
> not m68k, so the launch mechanics are entirely NewTOS's own.

## EmuTOS source

**Primary location:** `bdos/proc.c:179` —
`long xexec(WORD flag, char *path, char *tail, char *env)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x4B` =
`xexec`, `wparms = 7` (`bdos/bdosmain.c:271`; dispatcher table in
`docs/api/gemdos/_dispatcher.md §6`). Note the bind is **not** the generic
`trap1()` — `Pexec` has its own reentrancy-safe wrapper `trap1_pexec`
(`include/bdosbind.h:20,50`).

**Helpers called:**
- `bdos/kpgmld.c:40` — `kpgmhdrld()` — read + validate the 2-byte magic and
  the 26-byte `PGMHDR01` header.
- `bdos/kpgmld.c:79` — `kpgmld()` → `bdos/kpgmld.c:107` `pgmld01()` — load
  text+data, apply relocations (`pgfix01`, `bdos/kpgmld.c:243`), clear BSS/heap,
  close the file.
- `bdos/proc.c:413` — `alloc_env()` — size + allocate + copy the environment.
- `bdos/proc.c:460` — `alloc_tpa()` — pick the pool and carve the TPA.
- `bdos/proc.c:364` — `init_pd_fields()` — zero the PD, set TPA bounds, default
  DTA, env ptr, copy the command tail.
- `bdos/proc.c:387` — `init_pd_files()` — inherit std handles, current dirs,
  current drive from `run`.
- `bdos/proc.c:536` — `proc_go()` — build the launch stack and jump into the
  child (`gouser`, `bdos/rwa.S`).
- `bdos/umem.c:587` — `set_owner()` — tag the basepage/env MD with an owner PD.
- `bdos/umem.c` — `xmxalloc()`/`xmfree()`; `bdos/fsopnclo.c` — `xopen()`/
  `xclose()`; `bdos/fsdir.c` — `ixsfirst()` (existence probe).
- `bdos/proc.c:601` — `xterm()` — the child's eventual termination path that
  delivers the exit code back to this call (mode 0/4/6).

**Headers / type definitions used:**
- `include/bdosdefs.h:25-32` — `PE_LOADGO=0`, `PE_LOAD=3`, `PE_GO=4`,
  `PE_BASEPAGE=5`, `PE_GOTHENFREE=6`, `PE_BASEPAGEFLAGS=7`, `PE_RELOCATE=50`.
- `include/bdosdefs.h:65-97` — `PD` (basepage) layout; `PDCLSIZE = 0x80`.
- `include/bdosdefs.h:100-103` — `PF_FASTLOAD=1`, `PF_TTRAMLOAD=2`,
  `PF_TTRAMMEM=4`, `PF_STANDARD=7` (the `PRGFLAGS` bits).
- `bdos/pghdr.h:14-24` — `PGMHDR01` (the on-disk program header after magic).
- `include/bdosdefs.h:18-22` — `MX_STRAM`/`MX_PREFTTRAM` (pool modes used by
  `alloc_env`/`alloc_tpa`).

**System variables / globals read:**
- `run` (`bdos/proc.c:52`) — the calling process, source of the inherited
  environment (mode with `env==NULL`), std handles, current dirs/drive, and the
  new child's `p_parent`.
- `errbuf` (dispatcher longjmp buffer) — saved into `bakbuf` and re-installed.

**System variables / globals written:**
- `run` — set to the child by `proc_go` when a program is actually launched
  (modes 0/4/6), restored to the parent by the child's `xterm`.
- The selected memory pool (`pmd`/`pmdalt`) — env + TPA allocations.
- `sft[]` reference counts + child `p_uft[]` (inherited redirected handles) and
  `dirtbl[]` use counts (inherited current dirs) via `init_pd_files`.
- `bakbuf`/`errbuf`, `cur_p` (anti-longjmp-clobber static).

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — enables the alt-/TT-RAM arm of `alloc_tpa`/`alloc_env`
  (the `PF_TTRAMLOAD` pool selection + the ST-vs-TT tie-break). Without it,
  every TPA and env comes from ST-RAM (`pmd`).
- `DETECT_NATIVE_FEATURES` — compiles in `PE_RELOCATE` (50), used only by the
  BIOS bootstrap; not reachable by an ordinary program.

---

## Source extraction summary

`Pexec` is the process launcher: it loads an executable, builds its basepage
(`PD`, a.k.a. the TPA/basepage), and optionally transfers control to it. The
`flag` word selects one of several modes. EmuTOS splits them into two groups: a
first `switch` (`bdos/proc.c:192`) handles the modes that do **not** read a file
— `PE_BASEPAGE (5)` / `PE_BASEPAGEFLAGS (7)` build a bare basepage and return
its pointer; `PE_GO (4)` / `PE_GOTHENFREE (6)` launch an already-built basepage
passed in `tail`; anything not `PE_LOADGO`/`PE_LOAD` falls through to `EINVFN`.
The load path (`PE_LOADGO (0)`, `PE_LOAD (3)`) then probes the file with
`ixsfirst` (→ `EFILNF` if missing), opens it, reads and validates the header via
`kpgmhdrld`, allocates the environment and the TPA, initialises the PD, loads
and relocates the image via `kpgmld`, and — for `PE_LOADGO` only — launches it.

The **basepage** carved for the child is the `PD` struct at the start of the
allocated TPA (offsets 0x00-0xFF). `init_pd_fields` zeroes it, then sets
`p_lowtpa = p` (the basepage's own address), `p_hitpa = p + max` (the *whole*
free block, not just `needed` — the Atari "give the program everything
available" policy), the default DTA `p_xdta = p_cmdlin` (offset 0x80), the env
pointer, and copies the **command tail** into `p_cmdlin`. `p_flags` is then set
from the header's `PRGFLAGS` (or, for mode 5/7, from `path` reinterpreted as a
ULONG). `kpgmld`/`pgmld01` fills `p_tbase`/`p_tlen`/`p_dbase`/`p_dlen`/
`p_bbase`/`p_blen` (six longs written together at offset 0x08) and lays the text
segment immediately after the PD, data after text, BSS after data.

**Memory ownership** decides who frees the TPA + env at termination. For
`PE_LOADGO` the *child* owns them (`owner = p`), so its own `Pterm` reclaims
them; for `PE_LOAD` the *parent* owns them (`owner = run`) — the parent gets the
basepage pointer back and must arrange the eventual launch. `PE_GOTHENFREE (6)`
re-owns an already-built basepage + its env to itself *before* launching, which
is how a `PE_LOAD`-then-`PE_GO` sequence hands ownership to the child at launch
time. The **environment** is a block of NUL-terminated strings ended by a double
NUL; `alloc_env` copies the caller's block verbatim, or (when `env==NULL`)
duplicates the parent's `run->p_env`, rounding the size up to even and choosing
the TT-RAM pool when `PF_TTRAMLOAD` is set.

The **return value shape differs by mode.** Modes that only build a basepage
(3/5/7) return the basepage **pointer** cast to LONG. `PE_LOADGO (0)` launches
and does not return to the caller until the child terminates — at which point
the child's `xterm` delivers the child's exit code (a UWORD, zero-extended to
LONG) as *this* call's return value via the parent's saved trap frame. `PE_GO
(4)`/`PE_GOTHENFREE (6)` likewise launch and return the child's exit code. Any
failure before launch returns a negative GEMDOS error.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x4B
- **Stack frame on entry (m68k):** `Pexec(mode, name, cmdline, env)` =
  `trap1_pexec(mode, path, tail, env)` (`include/bdosbind.h:20,50`):
  ```
  +12: env   (LONG, pointer)
  +8:  tail  (LONG, pointer)
  +4:  path  (LONG, pointer)
  +2:  flag  (WORD)
   0:  function_id (word) = 0x004B
  ```
- **Total parameter bytes:** 14 (WORD + 3× LONG) → `wparms = 7`.
- **Native (AArch64) ABI:** ID `0x4B` in `x8`; args in `x0..x3`. Result in
  `x0`, LONG. **Mixed result shapes:** for modes 3/5/7 the result is a
  **basepage pointer** (full 64-bit `x0`, not sign-extended); for modes 0/4/6
  it is the child's **exit code** (a WORD **sign-extended** into `x0`); any
  pre-launch failure is a negative LONG error, sign-extended.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `flag` | `WORD` (16) — mode selector |
  | +4         | `x1`        | `path` | `char *` (mode 0/3) / `ULONG` flags (mode 5/7) / unused |
  | +8         | `x2`        | `tail` | `char *` command tail (mode 0/3) / `PD *` (mode 4/6) |
  | +12        | `x3`        | `env`  | `char *` env block, or NULL → inherit from `run` |

  The three pointer arguments are **overloaded by mode** (see Inputs).

---

## Inputs

| Position | Name   | Type   | Size | Source location    | Notes |
|----------|--------|--------|------|--------------------|-------|
| arg0     | `flag` | `WORD` | 2    | `bdos/proc.c:179`  | mode selector; unknown mode → `EINVFN` |
| arg1     | `path` | `char *` / `ULONG` | 4 | `bdos/proc.c:210-214,258` | program path (mode 0/3); reinterpreted as the `PRGFLAGS` ULONG (mode 7); ignored/zeroed (mode 5) |
| arg2     | `tail` | `char *` / `PD *` | 4 | `bdos/proc.c:240,245,380` | command tail (mode 0/3/5/7); the target basepage `PD *` (mode 4/6) |
| arg3     | `env`  | `char *` | 4 | `bdos/proc.c:419` | environment block; `NULL` → duplicate `run->p_env` |

### Mode constants (`include/bdosdefs.h:25-32`) — CONTRACT

| `flag` | Constant           | Behaviour |
|-------:|--------------------|-----------|
| 0  | `PE_LOADGO`        | Load the file, allocate env+basepage **owned by the child**, launch it; the call returns the child's exit code when it terminates. |
| 3  | `PE_LOAD`          | Load the file, allocate env+basepage **owned by the parent (`run`)**, return the basepage pointer; do **not** launch. |
| 4  | `PE_GO`            | Launch an already-built basepage passed in `tail`; ownership unchanged. Returns the child's exit code. |
| 5  | `PE_BASEPAGE`      | Allocate a basepage only (no file load), `p_flags = 0`; return the basepage pointer. |
| 6  | `PE_GOTHENFREE`    | Re-own the `tail` basepage + its env to the child, then launch it (as `PE_GO`). |
| 7  | `PE_BASEPAGEFLAGS` | Allocate a basepage only, taking `p_flags` from `(ULONG)path`; return the pointer. |
| 50 | `PE_RELOCATE`      | **Internal (NatFeats bootstrap) only**, `DETECT_NATIVE_FEATURES`. Relocate an already-loaded image; not reachable by an ordinary program. |
| other | —               | `EINVFN` (-32). |

### Command tail format (`p_cmdlin`, `bdos/proc.c:379-384`) — CONTRACT

`init_pd_fields` copies the caller's `tail` string into `p_cmdlin` byte-for-byte
up to `PDCLSIZE (0x80)` bytes or the first NUL, then appends a terminating NUL.
The classic TOS command-tail convention is a **1-byte length** followed by up to
**125 bytes** of text (127-byte usable region + the trailing NUL fits the
128-byte `p_cmdlin`). EmuTOS's copy is a raw byte copy of whatever the caller
passed; it does not itself synthesise the length byte — the caller supplies the
full length-prefixed image. (See Edge case 5.)

### Environment format (`alloc_env`, `bdos/proc.c:413-431`) — CONTRACT

A sequence of NUL-terminated `"NAME=value"` strings, ended by an extra NUL (so
the block terminates in a **double NUL**). `envsize` (`bdos/proc.c:150`) counts
up to and including that double NUL; the size is rounded up to even before
allocation and the block is copied verbatim. `env==NULL` → the parent's
`run->p_env` is duplicated.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG — a pointer, an exit code, or a negative error, per mode.
- **Encoding:**

  | Mode | Return on success | Source location |
  |------|-------------------|-----------------|
  | 0 `PE_LOADGO` | child's exit code (its `Pterm` `rc`, UWORD zero-extended to LONG) delivered via the parent's saved trap frame | `bdos/proc.c:359` → `xterm`, `bdos/proc.c:616` |
  | 3 `PE_LOAD` | basepage pointer (`(long)cur_p`) | `bdos/proc.c:360` |
  | 4 `PE_GO` | child's exit code (does not return until the child terminates) | `bdos/proc.c:246-248` |
  | 5 `PE_BASEPAGE` | basepage pointer (`(long)p`) | `bdos/proc.c:237` |
  | 6 `PE_GOTHENFREE` | child's exit code (as `PE_GO`) | `bdos/proc.c:241-248` |
  | 7 `PE_BASEPAGEFLAGS` | basepage pointer (`(long)p`) | `bdos/proc.c:237` |

- **Note:** the `(long)p` return for mode 4/6 at `:248` ("should not return ?")
  is effectively dead — `proc_go` transfers control to the child and never falls
  through; the observable result is the child's exit code.

### Side effects on global state

- Allocates an environment block and a TPA/basepage from `pmd`/`pmdalt`,
  tagging each MD's owner (`set_owner`, `bdos/proc.c:229-230,306-307`): child
  for `PE_LOADGO`/`PE_GOTHENFREE`, parent (`run`) for `PE_LOAD`/`PE_BASEPAGE`/
  `PE_BASEPAGEFLAGS`.
- Populates the child `PD` (`init_pd_fields` + `kpgmld`) and inherits std
  handles (bumping `sft[]` use counts via `ixforce`), current dirs (bumping
  `dirtbl[].use`), and the current drive from `run` (`init_pd_files`,
  `bdos/proc.c:387-410`).
- On launch (modes 0/4/6): sets the child's `p_parent = run`, builds the launch
  stack, and sets `run = child` (`proc_go`, `bdos/proc.c:541,575`).
- On a load I/O error after allocation (modes 0/3): the `setjmp(errbuf)` path
  frees the env + basepage and re-`longjmp`s into the dispatcher
  (`bdos/proc.c:319-333`).

### Side effects on hardware

- Reads program-file sectors (via the file layer / `Rwabs`).
- Invalidates the **instruction cache over the text segment only** after load
  (`invalidate_instruction_cache`, `bdos/proc.c:356`) — m68k I-cache coherence;
  the NewTOS analogue is `dc cvau`+`ic ivau`+`dsb ish`+`isb` over the loaded
  text range, same scope.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Unknown `flag` | `bdos/proc.c:252-253` | `EINVFN` (-32) |
| Program file not found (`ixsfirst` fails) | `bdos/proc.c:258-260` | `EFILNF` (-33) |
| `xopen` fails (path/handle problems) | `bdos/proc.c:264-268` | pass-through of `xopen`'s negative code (e.g. `EPTHNF` -34, `ENHNDL` -35) |
| Header read short / bad magic (≠ 0x601A) / short header | `bdos/kpgmld.c:47-63` (via `kpgmhdrld`) | `EPLFMT` (-66) |
| No memory for the environment | `bdos/proc.c:216-219,284-288` | `ENSMEM` (-39) |
| No memory for the basepage/TPA | `bdos/proc.c:222-226,295-300` | `ENSMEM` (-39) |
| Image too big for the TPA (text+data > TPA, or no room for BSS) | `bdos/kpgmld.c:140-141` | `ENSMEM` (-39) |
| Relocation offset wraps/overruns BSS, or an odd/out-of-range fixup | `bdos/kpgmld.c:184-185,265-268` | `EPLFMT` (-66) |
| I/O error during `kpgmld` (sector read fails) | `bdos/kpgmld.c:151-153` | pass-through of the negative BIOS/`Rwabs` error |

**Error paths not in Compendium:** the header-magic and relocation-format checks
both collapse to `EPLFMT`; `EGSBF` (-67) is *documented* by Compendium as a
`Pexec` error but this EmuTOS load path does not produce it (it comes from
`xsetblk`/`Mshrink`, used by `Ptermres`, not from `Pexec`). Atari TOS's tolerant
behaviour is preserved: if reading the relocation-start longword fails, the
program is **loaded without relocation** rather than rejected
(`bdos/kpgmld.c:176-178`).

---

## Verification criteria

- **Mode 0 nominal (load & go):** `Pexec(0, "C:\\PROG.PRG", cmdtail, env)` on a
  valid `0x601A` program loads it, launches it, and returns the child's exit
  code (the `rc` from the child's `Pterm`) once it terminates; the child's
  env+TPA are freed by the child's own termination.
- **Mode 3 nominal (load, don't go):** `Pexec(3, "C:\\PROG.PRG", cmdtail, env)`
  returns a non-negative basepage pointer, does **not** run the program, and
  the basepage + env are owned by the *parent* (`run`), not the child.
- **Mode 4 nominal (just go):** `Pexec(4, 0, basepage_from_mode3, 0)` launches
  the previously loaded basepage and returns its exit code.
- **Mode 6 (go then free):** `Pexec(6, 0, basepage, 0)` re-owns the basepage +
  env to the child, launches it, and returns the exit code; the child's `Pterm`
  frees them.
- **Mode 5 (basepage only):** `Pexec(5, 0, cmdtail, env)` returns a basepage
  pointer with `p_flags == 0`, `p_lowtpa == basepage`, `p_hitpa == basepage +
  allocated_size`, `p_xdta == &p_cmdlin`, without touching any file.
- **Mode 7 (basepage + flags):** `Pexec(7, (char*)PF_TTRAMLOAD, cmdtail, env)`
  returns a basepage whose `p_flags == PF_TTRAMLOAD`.
- **Env inheritance:** `Pexec(0/3, path, tail, NULL)` gives the child a copy of
  the parent's `run->p_env` (double-NUL-terminated, even-sized).
- **Command tail:** the child's `p_cmdlin` contains the caller's `tail` bytes
  (truncated to `PDCLSIZE`) followed by a NUL; `p_xdta` defaults to `&p_cmdlin`.
- **Error — unknown mode:** `Pexec(99, ...)` returns `EINVFN` (-32).
- **Error — missing file:** `Pexec(0, "C:\\NOPE.PRG", t, e)` returns `EFILNF`
  (-33).
- **Error — bad magic:** loading a file whose first word ≠ `0x601A` returns
  `EPLFMT` (-66).
- **Error — out of memory:** a TPA request larger than the largest free block
  returns `ENSMEM` (-39), and any partial allocation is freed (no leak).

---

## Edge cases discovered in source

1. **TPA is the *whole* free block, not `needed`.** `alloc_tpa` finds the
   largest fitting free block and returns its full size in `*avail`;
   `p_hitpa - p_lowtpa` is that whole size (`bdos/proc.c:374,488-498`). The
   program receives all remaining memory of the pool it landed in, not just its
   text+data+bss+PD footprint. A program that wants to hand memory back calls
   `Mshrink`.

2. **Tolerant relocation.** If the relocation-start longword can't be read (I/O
   short), EmuTOS loads the program **without relocating** rather than failing —
   matching Atari TOS (`bdos/kpgmld.c:176-178`). Only a *present but malformed*
   fixup (odd address, or reaching into/after BSS) yields `EPLFMT`.

3. **`PF_FASTLOAD` changes BSS clearing scope.** With `PF_FASTLOAD` set only the
   declared BSS (`h01_blen`) is zeroed; without it, the **entire heap** from BSS
   end to `p_hitpa` is cleared (`bdos/kpgmld.c:212-221`). A `PF_FASTLOAD` program
   therefore sees uninitialised bytes above its BSS — it must not assume a
   zeroed heap. (Contract: the flag's observable effect on heap contents.)

4. **`PF_TTRAMLOAD` / `PF_TTRAMMEM` steer the pool, and the ST/TT tie-break.**
   `alloc_env`/`alloc_tpa` allocate from TT-RAM (`pmdalt`) when `PF_TTRAMLOAD` is
   set *and* alt-RAM exists; if both pools fit, EmuTOS prefers alt-RAM unless
   ST-RAM is larger, in which case a flags-bits-31-28 "would-like" quantum
   (`* 128 KiB`) tie-break can force ST-RAM (`bdos/proc.c:481-485`, per the TT030
   Release Notes). The *pool a program lands in* is observable; the exact
   tie-break arithmetic is reference (NewTOS may implement it straightforwardly).

5. **Command tail is truncated at `PDCLSIZE` and NUL-terminated.** The copy loop
   stops at 128 bytes or the source NUL, then writes one NUL
   (`bdos/proc.c:380-383`). The Atari convention (1 length byte + ≤125 text
   bytes) is the caller's responsibility; EmuTOS copies whatever image is passed.

6. **Default DTA overlaps the command line.** `init_pd_fields` sets
   `p_xdta = (DTA *)p_cmdlin` (offset 0x80). A child that calls `Fsfirst` before
   any `Fsetdta` overwrites its own command tail — the classic quirk, shared
   with the DTA state model (`docs/api/gemdos/_state.md §6`).

7. **`init_pd_files` inherits handles/dirs/drive.** Redirected std handles
   (`p_uft[i] > 0`) are duplicated into the child via `ixforce` (bumping the
   `sft[]` use count so both share the file); non-redirected entries are copied
   verbatim. Every non-zero `p_curdir[i]` bumps `dirtbl[dn].use`; `p_curdrv` is
   copied (`bdos/proc.c:392-409`). Parent and child then diverge.

8. **Ownership split (modes 0 vs 3) and the `PE_GOTHENFREE` hand-off.**
   `owner = (flag==PE_LOADGO) ? p : run` (`bdos/proc.c:305`). For `PE_LOAD` the
   parent owns everything, so the parent must either free it or launch with
   `PE_GOTHENFREE (6)` — which re-owns the basepage **and its env** to the child
   right before `proc_go` (`bdos/proc.c:238-242`). Plain `PE_GO (4)` leaves
   ownership with whoever held it (so a mode-3→mode-4 sequence leaves the parent
   owning the child's memory — a deliberate pattern for a parent that wants to
   reclaim it).

9. **Partial-allocation cleanup on load failure.** After the env+TPA are
   allocated, a `kpgmld` I/O error either returns directly after `xmfree`-ing
   both (`bdos/proc.c:337-343`) or, for deeper `Rwabs` longjmps, is caught by the
   re-installed `setjmp(errbuf)` which frees both and re-`longjmp`s into the
   dispatcher with the original error (`bdos/proc.c:319-333`). `cur_p` is a
   `static` purely to dodge the "clobbered by longjmp" warning — an EmuTOS
   implementation detail, not contract.

10. **`PE_RELOCATE (50)` is not a program-visible mode.** It exists only under
    `DETECT_NATIVE_FEATURES` for the NatFeats host bootstrap
    (`bdos/proc.c:194-208`); documented for completeness, off NewTOS's path.

11. **I-cache invalidation is text-only.** Only `h01_tlen` bytes from the text
    base are invalidated; programs that jump into data/BSS/heap must manage
    coherence themselves (source comment, `bdos/proc.c:352-356`).

---

## Dependencies

### Requires (must be initialized / functional)

- Memory allocator up (`pmd`/`pmdalt`, `ffit`, `xmxalloc`/`xmfree`,
  `set_owner`) — see [`../memory/44-mxalloc.md`](../memory/44-mxalloc.md).
- File layer (`ixsfirst`, `xopen`/`xclose`, `xread`/`xlseek`) and a mounted
  drive for the load modes.
- `run` set to a valid parent PD; `dirtbl[]`/`sft[]` initialised for inheritance.
- The launch mechanism (`proc_go`/`gouser` on m68k; the EL0-entry path on
  NewTOS) and, transitively, `xterm` for the exit-code return.

### Used by

- Shells / command interpreters launching programs.
- The AES (program + accessory install).
- `system()`-equivalent library calls.

---

## Related data structures

- `docs/api/structures/basepage.md` — the `PD`/basepage byte layout (mirrored
  in the table below).
- `docs/api/structures/md.md` — `MD`/`m_own`, `set_owner`, the TPA pool model.
- `docs/api/gemdos/_state.md §1` (`run`), §2 (std-handle inheritance), §5
  (current-dir inheritance), §8 (pools).

### Basepage `PD` byte layout (`include/bdosdefs.h:65-97`) — CONTRACT

| Offset | Field | Type / size | Meaning |
|-------:|-------|-------------|---------|
| 0x00 | `p_lowtpa` | ptr (4) | start of the TPA = the basepage itself |
| 0x04 | `p_hitpa`  | ptr (4) | end of the TPA + 1 (whole allocated block) |
| 0x08 | `p_tbase`  | ptr (4) | base of the text segment (= basepage + 0x100) |
| 0x0C | `p_tlen`   | LONG (4) | text length |
| 0x10 | `p_dbase`  | ptr (4) | base of the data segment |
| 0x14 | `p_dlen`   | LONG (4) | data length |
| 0x18 | `p_bbase`  | ptr (4) | base of the BSS segment |
| 0x1C | `p_blen`   | LONG (4) | BSS length |
| 0x20 | `p_xdta`   | ptr (4) | DTA pointer (defaults to `p_cmdlin` @ 0x80) |
| 0x24 | `p_parent` | ptr (4) | parent PD (set by `proc_go` at launch) |
| 0x28 | `p_flags`  | ULONG (4) | `PRGFLAGS` (from the header, or mode-5/7 `path`) |
| 0x2C | `p_env`    | ptr (4) | pointer to the environment block |
| 0x30 | `p_uft[6]` | SBYTE[6] | per-process std-handle table (0..5) |
| 0x36 | `p_lddrv`  | char (1) | load drive (bookkeeping) |
| 0x37 | `p_curdrv` | UBYTE (1) | current default drive (per-process) |
| 0x38 | `p_1fill[2]` | LONG[2] | reserved |
| 0x40 | `p_curdir[26]` | UBYTE[26] | per-drive current-dir indices into `dirtbl[]` |
| 0x5A | `p_2fill[6]` | char[32-26] | reserved (pads to 0x60) |
| 0x60 | `p_3fill[2]` | LONG[2] | reserved |
| 0x68 | `p_dreg[1]` | LONG (4) | saved D0 (child's exit code is stored here) |
| 0x6C | `p_areg[5]` | LONG[5] | saved A3..A7 (m68k launch/return frame) |
| 0x80 | `p_cmdlin[128]` | char[128] | command-tail image (also the default DTA) |

> **Native vs m68k note.** The *field order and offsets 0x00-0x80 are contract*
> (programs read them). On native AArch64 the pointer fields are 64-bit (per
> `docs/COMPATIBILITY.md` "Native vs m68k struct variants"); the m68k emulator
> sees the 32-bit layout above. `p_dreg`/`p_areg` are the m68k register save
> area and are **reference** — NewTOS's EL0 launch uses its own frame, but
> `p_dreg[0]` (the exit code slot) stays meaningful as the contract way the
> child's `Pterm` code reaches the parent.

### Program header `PGMHDR01` (`bdos/pghdr.h:14-24`) — CONTRACT (on-disk)

Preceded on disk by a 2-byte **magic = `0x601A`** (a `BRA.S` branch on m68k),
already consumed by `kpgmhdrld` before this struct is read:

| Offset (after magic) | Field | Type | Meaning |
|---------------------:|-------|------|---------|
| 0x00 | `h01_tlen`  | LONG  | text-segment length |
| 0x04 | `h01_dlen`  | LONG  | data-segment length |
| 0x08 | `h01_blen`  | LONG  | BSS-segment length |
| 0x0C | `h01_slen`  | LONG  | symbol-table length |
| 0x10 | `h01_res1`  | LONG  | reserved (always 0) |
| 0x14 | `h01_flags` | ULONG | `PRGFLAGS` (see below) |
| 0x18 | `h01_abs`   | WORD  | non-zero → no relocation table present |

Total header = 2 (magic) + 26 (`PGMHDR01`) = 28 bytes; the relocation info
follows text+data+symbols (a 4-byte first offset, then byte deltas).

### `PRGFLAGS` bits (`include/bdosdefs.h:100-103`) — CONTRACT

| Bit | Mask | Constant | Effect |
|----:|------|----------|--------|
| 0 | 0x0001 | `PF_FASTLOAD` | clear only declared BSS, not the whole heap (see Edge case 3) |
| 1 | 0x0002 | `PF_TTRAMLOAD` | the program **may be loaded** into alt-/TT-RAM |
| 2 | 0x0004 | `PF_TTRAMMEM` | the program's `Malloc` **may return** alt-/TT-RAM |
| — | 0x0007 | `PF_STANDARD` | all three above |
| 31-28 | 0xF0000000 | — | "would-like" TPA-size quantum for the ST/TT tie-break (`* 128 KiB`), `bdos/proc.c:482` |

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Historically documents all
  seven modes (0/3/4/5/6/7) and `EPLFMT`/`ENSMEM`/`EINVFN`/`EFILNF`; it also
  lists `EGSBF` as a `Pexec` error, which this load path does **not** produce
  (see Error conditions).

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

### Atari ST Internals

- Basepage / program-header layouts historically match; the authoritative
  layout for NewTOS is the EmuTOS source cited above.

---

## Configuration / version notes

- `CONF_WITH_ALT_RAM` gates the TT-RAM allocation arms; without it the
  `PF_TTRAMLOAD`/`PF_TTRAMMEM` flags have no pool to steer to and everything is
  ST-RAM.
- `DETECT_NATIVE_FEATURES` gates `PE_RELOCATE (50)` — bootstrap-only.
- `PDCLSIZE = 0x80`, `NUMSTD = 6`, `NUMCURDIR = BLKDEVNUM = 26`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06 D4/D5 status (2026-06-16):
- `PE_LOADGO` (0), `PE_LOAD` (3), `PE_GO` (4) implemented and verified in QEMU.
  `PE_BASEPAGE` (5) / `PE_GOTHENFREE` (6) / `PE_BASEPAGEFLAGS` (7) return
  `EINVFN` for now.
- **Loader/relocation is NewTOS-native**, not a port: NewTOS runs the loaded
  program at **EL0 with a per-process TTBR0** (not m68k). The `gouser`/`proc_go`
  register set-up, `supstk`, and the `setjmp`/`bakbuf` longjmp plumbing are
  EmuTOS internals — NewTOS uses the AArch64 EL0-entry / exception-return path
  and its own error unwinding. The **basepage layout, the seven Pexec modes,
  `PRGFLAGS`, the cmdtail/env formats, and the per-mode return values are hard
  contract** and are reproduced exactly.
- **PD-handle ABI deviation (interim).** EmuTOS's PD is a real shared,
  dereferenceable pointer; under NewTOS's per-process-TTBR0 model every TPA maps
  at user VA 0, so a VA-0 PD is neither unique nor parent-dereferenceable, and a
  64-bit kernel VA does not fit the `LONG` return. `PE_LOAD` therefore returns an
  **opaque slot handle** (the child's `basepage_pa`, slot-unique and
  `LONG`-representable); the parent passes it back verbatim in `PE_GO`'s `tail`
  and does **not** dereference it. The faithful dereferenceable PD requires the
  single shared address space (ADR-010), correct for real hardware but blocked
  on QEMU cortex-a72 TCG (QUIRK-001 + ADR-010 in `docs/MEMORY.md`); revisit on
  RPi4 / Phase 09.
- Ownership (`owner = (flag==PE_LOADGO) ? p : run`) is currently expressed
  through slot lifetime only — there is no BDOS-pool MD backing the static-arena
  TPA yet (Phase 09).
- I-cache: invalidate the loaded **text range** with `dc cvau`+`ic ivau`+
  `dsb ish`+`isb` (same text-only scope as EmuTOS).
