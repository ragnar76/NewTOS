# `p_parent` and the environment of every process (TASK/PPARENT)

> Spec extraction, cross-cutting `Pexec` / basepage / AES. Source: EmuTOS @
> `1a7484ab` (`references/emutos`). User decision 2026-10-06: **Way A** —
> two stand-in PDs (`TASK/ARCH03-POSTE9-STATUS-UNFINISHED.MD` §1; Way B, the
> desktop/AES as a real EL0 process, is `TASK/PROC03.md`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** the value of `p_parent` (0x24) and the content of the block
> `p_env` (0x2C) names for every way a process is started; that the PD
> `p_parent` names is a readable basepage. mintlib's start-up code tells a
> desk accessory by `p_parent == 0` (`startup/crt0.S:52-56`), `getppid`
> falls back to `p_parent` (`unix/getppid.c:22`).
> **Reference:** where the stand-in PDs live, how they are published, which
> internal pointer the kernel keeps as "`run` while kernel code is on the
> stack".

## 1. EmuTOS: where `p_parent` comes from

`p_parent` is written in exactly one place, `proc_go` (`bdos/proc.c:541`):

```c
p->p_parent = run;
```

`init_pd_fields` zeroes the whole PD first (`bdos/proc.c:370`,
`bzero(p, sizeof(PD))`), so a PD that never passes `proc_go` keeps
`p_parent == 0`. `proc_go` runs for `PE_LOADGO`, `PE_GO` and `PE_GOTHENFREE`
(`bdos/proc.c:238-248,358-359`); `PE_LOAD`, `PE_BASEPAGE` and
`PE_BASEPAGEFLAGS` never call it.

`run` at each start:

| Start | `run` when it is started → `p_parent` | Source |
|---|---|---|
| GEMDOS initialisation | `run = &initial_basepage` (a static `PD`, `bdos/bdosmain.c:78`) | `bdos/bdosmain.c:337-342` |
| AUTO program | `Pexec(PE_LOADGO, "\AUTO\…", "", NULL)` from the BIOS: `&initial_basepage` | `bios/bios.c:685-695` |
| The AES (`exec_os`, the default ROM shell) | `Pexec(PE_BASEPAGEFLAGS, PF_STANDARD, "", default_env)` + `Pexec(PE_GO)` from the BIOS: `&initial_basepage` | `bios/bios.c:855-866` |
| The ROM desktop (EmuDesk), EmuCON | `aes_run_rom_program`: `Pexec(PE_BASEPAGEFLAGS, PF_STANDARD, "", NULL)` + `dos_exec(PE_GOTHENFREE)` from the AES process: **the AES PD** | `aes/gemshlib.c:569-580` |
| A program the desktop asked for / the `#Z` autorun program | the desktop has terminated (`shel_write` + exit), `sh_ldapp` → `dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn)` from the AES process: **the AES PD** | `aes/gemshlib.c:624-645` |
| Desk accessory | `pgmld` → `Pexec(PE_LOAD, pname, "", NULL)`, then `pstart` — never `proc_go`: **0** | `util/gemdos.c:29-47`, `aes/geminit.c:236-257`, `bdos/proc.c:370` |
| Child of a program (`Pexec` 0/4/6 from a running program) | the caller's PD | `bdos/proc.c:541` |

`dos_exec` is a plain `trap #1` `Pexec` (`aes/gemstart.S:169-226`); the AES
does not switch GEMDOS's `run`, so while the AES (or a program launched by it)
calls GEMDOS, `run` is the AES PD or that program.

## 2. EmuTOS: what the two PDs contain

### `initial_basepage` (`bdos/bdosmain.c:78`, static, zero-initialised)

| Field | Value | Source |
|---|---|---|
| everything | 0 (BSS) | `bdos/bdosmain.c:78` |
| `p_flags` | `PF_STANDARD` (7) | `bdos/bdosmain.c:340` |
| `p_env` | `&double_nul` = `{0, 0}` (an empty environment) | `bdos/bdosmain.c:81,341` |
| `p_uft[0..5]` | `default_handle` = `H_Console, H_Console, H_Aux, H_Print, H_Console, H_Console` = `-1, -1, -2, -3, -1, -1` | `bdos/console.c:60-68,107-112`, `bdos/console.h:23-25` (`stdhdl_init`, `bdos/bdosmain.c:347`) |
| `p_curdrv` | `bootdev` (`Dsetdrv(bootdev)` while `run` is this PD) | `bios/bios.c:827` |
| `p_parent` | 0 (it is the root) | BSS |

Later writes by the BIOS that are not boot-time constants (the `Fsetdta(&dta)`
of `autoexec`, the `dirtbl` index `ckdrv` enters into `p_curdir[]`) are
dynamic state and not part of the stand-in contract.

### The AES PD (`bios/bios.c:862-865`)

Created by `Pexec(PE_BASEPAGEFLAGS, (char *)PF_STANDARD, "", default_env)`
(`xexec` mode 7, `bdos/proc.c:212-236`), then `p_tbase = exec_os`,
`p_tlen = p_dlen = p_blen = 0`, then `Pexec(PE_GO)` (`proc_go`).

| Field | Value | Source |
|---|---|---|
| `p_lowtpa` | the PD itself | `init_pd_fields`, `bdos/proc.c:373` |
| `p_hitpa` | `p + max` — `max` the largest free block `alloc_tpa` took | `bdos/proc.c:220,374` |
| `p_tbase` | `exec_os` (the AES entry in the ROM) | `bios/bios.c:863` |
| `p_tlen`, `p_dlen`, `p_blen` | 0 | `bios/bios.c:864` |
| `p_dbase`, `p_bbase` | 0 (`bzero`) | `bdos/proc.c:370` |
| `p_xdta` | `&p->p_cmdlin` | `bdos/proc.c:375` |
| `p_parent` | `run` = `&initial_basepage` | `bdos/proc.c:541` |
| `p_flags` | `PF_STANDARD` (7) | `bdos/proc.c:234`, `bios/bios.c:862` |
| `p_env` | a copy of `default_env` (`alloc_env(PF_STANDARD, default_env)`) | `bdos/proc.c:215,376,413-432` |
| `p_uft` | inherited from `run`: a handle `> 0` is `ixforce`d, every other copied — here the six default handles | `init_pd_files`, `bdos/proc.c:388-399` |
| `p_curdir[]`, `p_curdrv` | inherited from `run` (`p_curdrv` = `bootdev`) | `bdos/proc.c:401-409` |
| `p_cmdlin` | `""` (one NUL) | `bdos/proc.c:378-384` |
| `p_lddrv`, fills | 0 | `bdos/proc.c:370` |

`gemstart.S` then `Mshrink`s its own TPA (`aes/gemstart.S:60-69`, which does
not touch the PD) and stores `p_env` as `ad_envrn` (`aes/gemstart.S:78-96`):
**the AES environment `ad_envrn` *is* the AES PD's `p_env`.**

### `default_env` (`bios/bios.c:77-80,626-636`)

```c
#define ENV_SIZE 12        /* sufficient for standard PATH=^X:\^^ (^=nul byte) */
#define DEF_PATH "A:\\"
strcpy(default_env, PATH_ENV);                 /* "PATH=" (bdosdefs.h:128) */
p = default_env + sizeof(PATH_ENV);            /* past the NUL of "PATH=" */
strcpy(p, DEF_PATH); *p += bootdev;            /* "X:\" */
p += sizeof(DEF_PATH); *p = '\0';              /* the double NUL */
```

Bytes (bootdev = 2): `50 41 54 48 3D 00 43 3A 5C 00 00 00` —
`"PATH=" NUL "C:\" NUL NUL` plus the twelfth (static, zero) byte: the Atari TOS
`PATH=` quirk with the value as a separate string after the `=`'s NUL
(`sh_find` skips that NUL, `aes/gemshlib.c:448-455`). `envsize` counts 11
bytes (`bdos/proc.c:150-159`), `alloc_env` copies `(11 + 1) & ~1 = 12`
(`bdos/proc.c:421-429`).

## 3. EmuTOS: the environment every process gets

`alloc_env(flags, env)` (`bdos/proc.c:413-432`) runs for every load and
basepage mode: `env == NULL` → a copy of `run->p_env`, otherwise a copy of
`env`, size `(envsize + 1) & ~1`, in ST-RAM or (with `PF_TTRAMLOAD`) TT-RAM
preferred. So:

| Start | `env` argument | environment content |
|---|---|---|
| AUTO program | `NULL`, `run` = `initial_basepage` | `{0, 0}` (2 bytes) |
| The AES | `default_env` | `default_env` (12 bytes) |
| ROM desktop, EmuCON | `NULL`, `run` = AES PD | copy of `ad_envrn` |
| Desktop / `#Z` program | `ad_envrn` | copy of `ad_envrn` (12 bytes) |
| Desk accessory | `NULL`, `run` = AES PD | copy of `ad_envrn` (12 bytes) |
| Child of a program | `NULL` → the caller's `p_env`, else the caller's block | as given |

## 4. NewTOS mapping (Way A)

NewTOS has no AES process and no `initial_basepage`: the desktop (`desk/`) and
the AES run in the kernel and `RUN` is `null` while kernel code is on the
stack. Way A gives the observable contract of §1–§3 with **two stand-in PDs**
(`tos-api/src/bdos/process/standin.rs`):

- **Where.** One page-aligned kernel page holding, in this order, the
  `initial_basepage` stand-in (+0x000), the AES-PD stand-in (+0x100),
  `double_nul` (+0x200) and the AES environment (+0x210, `ENV_SIZE` = 12
  bytes). It is published to every process at VA = PA, read-only
  (`usermem::expose`, `PROT_PR`, ADR-023), so its address is a 32-bit user
  address that every process can read under `memprot=on`, `off` and
  `strict` — the same way the AES environment page was published before.
  A kernel page and not a pool block: it is no process's memory, so no `Pterm`
  and no `free_all_owned` can ever reach it, and the pools keep their size.
- **Content.** As §2, with the values NewTOS has no counterpart for:
  `initial_basepage.p_curdrv` and the AES PD's = `bootdev`
  (`bdos::fs::drive::boot_drive`, the same derivation as the `$446` system
  variable); the AES PD's `p_hitpa = p_lowtpa + 0x100` (there is no AES TPA:
  its "memory" is its basepage) and `p_tbase = 0` (there is no AES code in
  the user address space). These two are deviations by absence, not choices.
- **The kernel's `run`.** "`run` while kernel code is on the stack" is the
  `initial_basepage` stand-in until the AES starts — EmuTOS `exec_os`, which
  follows `autoexec` (`bios/bios.c:840-866`); in NewTOS right after
  `launch::run_auto_folder` — and the AES-PD stand-in from then on
  (`standin::kernel_run`). `RUN` itself stays `null`: the stand-ins are never
  the running process, own no memory and are never an owner tag; `*os_run`,
  `parent_slot` and the return path of `Pterm` are unchanged.
- **`p_parent`.** `pexec::set_parent` writes the caller's PD, or, when the
  caller is no process (`current()` is `null`), `standin::kernel_run()`. The
  kernel launcher (`kernel/src/launch.rs`, AUTO / desktop / `#Z`) writes it
  the same way before the program's first instruction. Accessories are
  started by the scheduler and keep the loader's 0.
- **Environment.** `EnvSpec::Inherit` copies `run->p_env`; with no running
  process that is the kernel `run` stand-in's environment (`double_nul`
  before the AES, the AES environment after it). The kernel launcher and
  the accessory loader inherit (EmuTOS: `NULL` from AUTO and `pgmld`,
  `ad_envrn` — the AES PD's `p_env` — from `sh_ldapp`).
- **`shel_envrn`.** `ad_envrn` is the AES-PD stand-in's `p_env`: `shel_envrn`
  searches that block and hands an EL0 program its published user address
  (the in-kernel desktop: the kernel address). For `"PATH="` the answer is
  the NUL after the `=` (`gemshlib.c:338-347`); gemlib's `mt_shel_envrn`
  steps over it (`a_shel_envrn.c:40-50`), so a gemlib program sees `"X:\"`.
  The built-in `"PATH=C:\;"` default it searched before had no EmuTOS
  source and is gone.

### What follows for programs (observable)

| Start | `p_parent` | `p_env` content |
|---|---|---|
| AUTO program | `initial_basepage` stand-in | `00 00` |
| Desktop launch, `#Z` program (TeraDesk, QED, APITEST, …) | AES-PD stand-in | `PATH=\0X:\\\0\0` (12 bytes, X = boot drive) |
| Desk accessory | 0 | `PATH=\0X:\\\0\0` |
| `Pexec` child of a program | the caller's PD | the caller's `p_env` copy, or the given block |
| `Pexec` from EmuCON (in-kernel, EL1) | AES-PD stand-in (deviation: EmuTOS has an EmuCON PD, NewTOS's EmuCON is no process — `TASK/PROC03.md`) | the AES environment (as EmuTOS: EmuCON's own environment is a copy of `ad_envrn`) |

The stand-in chain is readable to the end: the AES-PD stand-in's `p_parent`
is the `initial_basepage` stand-in, whose `p_parent` is 0.

### The SDK start-up code

mintlib's accessory test (`startup/crt0.S:46-62`): `a0 != 0` (the AES hands an
accessory its basepage in `a0`; for a program `a0` is 0) **and**
`a0->p_parent == 0`. NewTOS passes the basepage in `x0` on every path
(`sdk/libc/crt0.S`), so the first half always holds and the test is
`p_parent == 0`. `sdk/libc/newtos-arch/heap_init.c` (`__libc_crtinit`) and
`sdk/crt0.S` use exactly that test; the `EGSBF` heuristic (a failed `Mshrink`
meant "accessory") is gone, and the `Mshrink` result is ignored as mintlib
ignores it (`crtinit.c:227-247`).

## Edge cases

1. A program reads its own `p_parent` chain: AES stand-in → initial stand-in →
   0, no fault in any protection mode (the page is published read-only).
2. A program writes into a stand-in: the page is read-only for EL0, the write
   faults (EmuTOS has no protection; a write into the AES PD would corrupt
   EmuTOS). Not observed in any ported application.
3. `PE_LOAD` + `PE_GO` from a kernel caller: `p_parent` is set at `PE_GO`
   (`proc_go`), not at load — unchanged.
4. A `Pexec` child of an accessory: in NewTOS the accessory is a running
   process, so its PD becomes the child's `p_parent`; in EmuTOS an accessory
   is no GEMDOS process and `run` is still the AES PD. A pre-existing
   deviation of the process model, not touched by Way A (`TASK/PROC03.md`).

## Related

- [`4b-pexec.md`](./4b-pexec.md) — NewTOS notes, "Kernel-launched programs".
- [`../../structures/basepage.md`](../../structures/basepage.md) — `p_parent`,
  `p_env`.
- [`../../aes/scrap-shell.md`](../../aes/scrap-shell.md) — `shel_envrn`.
- [`../../aes/accessories.md`](../../aes/accessories.md) — accessory start.
