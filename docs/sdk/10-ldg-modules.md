# 10 — LDG modules (`.NLD`): building and loading native LDG libraries

Stand 2026-10-06, TASK/LDG-M1. Design and evidence trail:
`docs/analysis/ldg-after-arch03-2026-10-06.md` (the mechanism, file:line for
every step), `measurements/measurement-2026-10-06-ldg-m1/RUNINFO.TXT` (the
QEMU runs this page's "verified" statements rest on).

LDG ("GEM Dynamical Libraries", Landemarre/Bereziat/Bercegeay, LGPL 2.1) is
the Atari way of loading a library at run time: the library is an ordinary
program that a client loads with `Pexec(3)` ("load, don't go"), starts once
with `Pexec(4)` so that its `main()` can hand back a descriptor, and whose
functions the client then calls directly through pointers. zView's codecs are
LDG modules; that is why NewTOS has it.

On NewTOS this runs **through LDG's own client code, unchanged in logic** —
there is no NewTOS loader for LDG. The kernel's `Pexec` loads and relocates
the module, makes its code visible to instruction fetch, leaves the module
block owned by the client, and runs `Pexec(4)` as a thread in the client's
address space (`docs/api/gemdos/process/4b-pexec.md`, `PE_GO`).

## 1. The pieces

| What | Where | Built by |
|---|---|---|
| LDG source, pinned | `sdk/ldg/ldg-svn-r133.tar.xz` (SVN r133, sha256 in `sdk/ldg/ldg-sources.sha256`, per-file manifest `ldg-svn-r133.files.sha256`) | — |
| `libldg.a` (client + module side: `ldg_open/find/close/error`, `ldg_init`, `ldg_getpath`, `ldg_Malloc` & Co.) | `sdk/ldg/libldg.a` | `sdk/ldg/build-ldg.sh` (also via `lib_ensure ldg`, `sdk/lib-stale.sh`) |
| Headers | `sdk/ldg/ldg-r133/include/ldg.h` (extracted tree) | `build-ldg.sh` |
| Overlays | `sdk/ldg/overlay/src/devel/{ldgload.c,ldgutil.c}` | — |
| Demo module + acceptance client | `sdk/ldg/build/EX_LIB.NLD`, `sdk/ldg/build/LDGTEST.NCP` | `sdk/ldg/build-ldg-demo.sh` |
| LDG's own WinDom demo client + its second module (TASK/LDG-M3) | `sdk/ldg/build/client/dist/LDGDEMO/` (`CLIENT/LDGDEMO.NAP`, `CLIENT/CLIENT.RSC`, `LIBS/EX_LIB.NLD`, `LIBS/POPUP.NLD`) | `sdk/ldg/build-ldg-client.sh` ([12](12-windom.md)) |

The two overlays below are the only source differences of `libldg.a` (each
file's header says why, with file:line); `demo/client/client.c` has a third,
for the demo client only (module suffix `.ldg` → `.nld`, TASK/LDG-M3):

* `ldgload.c` — `BASEPAGE.p_env` is a fixed 32-bit field in the SDK
  (`sdk/libc/overlay/include/mint/basepage.h`), so the one assignment to a
  `char *` gets a `(char *)(uintptr_t)` cast.
* `ldgutil.c` — `ldg_cookie` reads the jar through `Getcookie()`
  (`Ssystem(S_GETCOOKIE)`) instead of `*(long *)0x5a0` (8 bytes on LP64,
  the jar holds 32-bit words); `ldg_cpush` is empty because the kernel
  already synchronised the caches inside `Pexec(3)`
  (`tos-api/src/bdos/process/loader.rs:599`).

`ldgapp.c` and `ldglib.c` compile unmodified.

## 2. Building a module

An LDG module is built **exactly like a program** — PIE, `sdk/libc/crt0.S`,
`sdk/newtos.ld` — and only the name differs: **`.NLD`** (ADR-018 amendment
2026-10-06 in `docs/MEMORY.md`; `.LDG` stays reserved for m68k modules and the
later emulator). Its `main()` calls `ldg_init(&descriptor)` and returns.

The link line `build-ldg-demo.sh` uses for `ex_lib`:

```sh
rust-lld -flavor gnu -T sdk/newtos.ld -pie --no-dynamic-linker -z text \
    -z max-page-size=0x1000 --build-id=none crt0.o ex_lib.o \
    --start-group sdk/ldg/libldg.a sdk/gemlib/gemlib.a sdk/libm/libm.a \
                  sdk/libc/libc.a --end-group -o EX_LIB.NLD
```

compiled with the application flag set of `sdk/apps/build-qed.sh` and the
include path `-I sdk/ldg/ldg-r133/include` before gemlib's and mintlib's. A
client links the same archives (it needs `libldg.a` for `ldg_open` & Co.).

* `PROC` tables with function pointers in `.data` are fine: they become
  `R_AARCH64_RELATIVE` entries, which the kernel applies when it loads the
  module. `-z text` keeps relocations out of `.text` (any other relocation
  type makes `Pexec` fail with `EPLFMT`, `tos-api/src/bdos/process/reloc.rs`).
* `ldg_init` finds the client through `getenv("OFFSETLDG")`. That works since
  TASK/PORT09 (the libc start-up builds `environ` from `p_env`); before it,
  every module failed here.
* A module needs no special `PRGFLAGS`: the default `PF_STANDARD` gives a
  `PROT_P` block whose owner is the **client** that loaded it.

## 3. Loading one (client side)

```c
LDG *ldg = ldg_open("C:\\LDG\\EX_LIB.NLD", aes_global);   /* or a bare name after chdir */
double (*sinus)(Real *) = ldg_find("sinus", ldg);
...
ldg_close(ldg, aes_global);
```

`ldg_open` reads only `gl[2]` (the AES id, stored in `ldg->id`), so a client
that runs before the AES (an `\AUTO\` `.NCP`) may pass a zeroed array of its
own — `sdk/examples/ldgtest.c` does.

## 4. Limits (what is not there, and what that means)

| Limit | Consequence |
|---|---|
| **No LDG TSR (`ldg.prg`), no `LDGM` cookie** | Every `ldg_open` loads the module again (no sharing, no reference count, no `LDG_NOT_SHARED`/`LDG_RESIDENT`/`LDG_LOCKED` handling — those are TSR features, `ldgcook.c`). `ldg_error()` always returns `LDG_NO_TSR` (-11, `ldgapp.c:103`): detect failure by `ldg_open` returning `NULL`. `ldg_libpath` returns `LDG_NO_TSR` |
| **No library search path** | Without the TSR `ldg_open` passes the name straight to `Pexec(3)` (`ldgapp.c:37-38`): give an absolute path, or `chdir` to the module's directory and pass the bare name (as zView does) |
| **No `ldg_callback`** | `stdcall.s` is m68k assembly and is not built; the symbol does not exist in `libldg.a`, so a program that calls it fails to **link** (lead decision 2026-10-06; no stub) |
| **No libshare** | Not needed by plain modules (`ex_lib`); zView's codecs link it (`docs/analysis/libshare-2026-10-06.md`) — later milestone |
| **One client per module** | The module block is `PROT_P`, owned by the client that loaded it; another process cannot call into it under `memprot=on/strict` |
| **Environment copy ≤ 512 bytes** | `ldg_load` passes `OFFSETLDG=<addr>` followed by a copy of the client's environment to `Pexec(3)`; the kernel reads at most `ENV_BUF_LEN = 512` bytes of it and truncates the rest silently (`kernel/src/arch/aarch64/exceptions.rs:1781`). `OFFSETLDG` comes first, so loading is unaffected; the module just sees a shortened environment |
| **64 KiB stack kept per loaded module** | mintlib's `_crtinit` keeps `_stksize` (default `MINKEEP` = 64 KiB, `sdk/libc/newtos-arch/heap_init.c:63`) in the module block; module functions run on the **client's** stack, so it is unused. Original mintlib behaviour, kept on purpose (lead decision Q6) — a module may set its own `_stksize` |
| **Module `Malloc`s during `main()` are freed at its `Pterm`** | As on TOS: memory the module allocates in `main()` belongs to the module and is released when `Pexec(4)` returns. Memory allocated later, inside a function the client calls, belongs to the client |
| **Open defect (TASK/LDG-M3, B2): a module's `malloc` after loading reads freed memory** | The row above has a consequence the libc hides: the module's `exit()` closes stdio (`__fcloseall` → `fclose` → `init_stream` `malloc` + `free`), so the module's own static free list (`_mchunk_free_list`) ends `main()` pointing into a hunk its `Pterm` frees. The next `malloc` inside the module (called by the client) faults under `memprot=on` (POPUP.NLD `__get_bkgr`, `measurements/measurement-2026-10-06-ldg-m3/`) and corrupts/hangs with `memprot=off`. `EX_LIB.NLD` does not `malloc` and is unaffected. Fix location (libc, libldg or kernel) not decided |
| `Pwait()` | LDG calls MiNT's `Pwait` (0x109) after `Pexec(4)` and ignores the result; NewTOS answers `EINVFN` — harmless |

## 5. Testing

`tools/make-sd-image.sh` installs `C:\LDG\EX_LIB.NLD` and `C:\LDGTEST.NCP`
when `sdk/ldg/build-ldg-demo.sh` has been run (QEMU image only), and
`SD_LDGTEST_AUTO=1` additionally puts the client into `\AUTO\`. Its serial
verdict line is `ldgtest: ALL PASS (N checks)` or `ldgtest: FAIL (k of N
checks failed)`; the checks are listed at the top of `sdk/examples/ldgtest.c`.

**Only real hardware can confirm** that the code of a freshly loaded — and
especially a **re-loaded** — module is fetched correctly: QEMU has no caches
(`loader.rs:594-597`), so the I-cache maintenance in `Pexec(3)` is exercised
only on a Pi. `LDGTEST`'s reload step (close, open again, often at the same
address) is the case to watch there.
