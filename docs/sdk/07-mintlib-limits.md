# 07 — What the ported mintlib does not provide

Derived from `sdk/libc/build-libc.sh` (what is compiled at all),
`sdk/libc/expected-failures.txt` (what is known not to compile), the shim
headers in `sdk/libc/newtos-arch/`, and an `llvm-nm` pass over the `libc.a`
built on 2026-09-17 (613 objects compiled, 11 failed, 6 excluded —
`sdk/libc/build/{compiled,failed,excluded}.txt`).

## 1. mintlib files that do not compile (recorded, not repaired)

`sdk/libc/expected-failures.txt` is the authoritative list; the build stops
on any failure not in it (`sdk/lib-stale.sh:229-283`). What each costs:

| File | Reason (from the record) | What a program loses |
|------|--------------------------|----------------------|
| `mintlib/linea.c` | m68k inline asm — ST Line-A | Line-A (NewTOS has none) |
| `mintlib/nf_ops.c` | m68k inline asm — ARAnyM NatFeats | `nf_init`/`nf_get_id` (an app that needs the symbols supplies its own, as the TeraDesk overlay does) |
| `mintlib/crtinit.c` | m68k asm + two LP64 basepage casts | **`argv`/`argc` parsing, `environ` set-up, `_base`** — see §5 |
| `mintlib/spawnve.c`, `spawnvp.c` | `va_list` arithmetic that assumes m68k `char *` | `spawn*` family (`system()` links, but its `spawnvp` is dangling) |
| `stdio/gets.c` | `.stabs` directive rejected by the integrated assembler | `gets` |
| `mintlib/getwd.c` | same | `getwd` (`getcwd` is available) |
| `signal/siggetmask.c` | same | `siggetmask` |
| `mintlib/syscall.c` | needs the generated `syscall-list.h` | the generic `syscall()` |
| `gmp/support.c` | `mathimpl.h` not on the include path | part of the GMP float-printing helpers (the rest of `gmp/` compiles) |
| `tz/dumpstate.c` | struct members missing | `tz` debug dump only |

## 2. mintlib files deliberately replaced by lean shims

`EXCLUDE` in `build-libc.sh:111-135`; the replacement lives in
`sdk/libc/newtos-arch/`. "Lean" means: the real TOS call is made, the Unix
tail that has no TOS analogue is dropped.

| mintlib file | Shim | What is different |
|--------------|------|-------------------|
| `mintlib/do_fstat.c` | `do_fstat.c` | fills `st_mode` (`S_IFCHR\|0600` for fd 0–2, `S_IFREG\|0644` otherwise), an honest `st_size` via `Fseek`, `st_blksize = BUFSIZ`, `st_blocks`, `st_nlink = 1`; **times and owner stay zero** (`:1-30`) |
| `mintlib/do_stat.c` | `do_stat.c` | `__sys_stat` = the `Fstat64` trap; `__do_stat` = `__quickstat` (mode/size/uid/gid honest, **no timestamps**) (`:1-27`, `:42-56`) |
| `mintlib/isatty.c` | `isatty.c` | `isatty(fd)` is `fd >= 0 && fd <= 2`; also carries `__open_stat[]` (`:1-16`) |
| `unix/ioctl.c` | `ioctl.c` | forwards to `Fcntl(fd, arg, request)`; the m68k tty line-discipline path is gone (`:1-16`) |
| `unix/access.c` | `access.c` | checks the owner `rwx` bits from `__quickstat` only; no group/passwd database (`:1-19`) |
| `mintlib/thread.c` | — (dropped) | MiNT `tfork` threads: incompatible with the 256-byte basepage and the single-tasking model (`build-libc.sh:130-134`) |

## 3. mintlib assembly that is never assembled

`build-libc.sh` compiles `.c` files only (`:149-169`). The 24 `.S` files in
the core directories are therefore absent unless a shim replaces them:

| Replaced by a shim | Missing — **undefined symbol if used** |
|--------------------|----------------------------------------|
| `string/bcopy.S`, `memset.S`, `bzero.S` → `memops.c`; `mintlib/libc_exit.S` → `proc.S`; `mintlib/getcookie.S` → `getcookie.c`; `mintlib/getsysvar.S` → `getsysvar.c`; `startup/crt0.S` → `sdk/libc/crt0.S` | **`mintlib/setjmp.S` — no `setjmp`/`longjmp`/`_setjmp`** (measured: not defined in `libc.a`); `frexp.S`, `ldexp.S`, `modf.S`; `stdlib/alloca.S` (clang's `__builtin_alloca` covers `alloca()` when `<alloca.h>` maps to it — **unverified**); `unix/vfork.S`, `getcontext.S`, `makecontext.S`, `setcontext.S`, `swapcontext.S`; `mintlib/setstack.S`, `stkover.S`, `checkcpu.S`, `_mon.S`, `elfattr.S`, `_infinitydf.S`, `_normdf.S` |

ADR-019 lists "setjmp (aarch64, when pulled)" as part of the port surface
(`docs/MEMORY.md:1139`); it has not been pulled yet. An application using
`setjmp` (e.g. for error recovery) needs an AArch64 implementation added to
`newtos-arch/` — a toolchain task, not an app edit.

## 4. mintlib directories not compiled at all

`DIRS` (`build-libc.sh:109`) covers `stdio stdlib string mintlib unix syscall
time misc multibyte signal termios locale dirent gmp posix tz`. Not compiled:
`argp`, `conf`, `crypt`, `login`, `pwdgrp`, `shadow`, `socket`, `startup`,
`stdiio`, `sunrpc`, `sysvipc`. Consequences: no sockets (`socket`,
`gethostbyname` are dangling references), no `crypt`, no passwd/group
database (`getpwuid`, `getgrent`, … dangling), no SysV IPC, no `argp`, no
integer-only `stdiio` (the full `printf` is linked instead,
`build-qed.sh:30-31`).

**There is no libm.** mintlib never contained one (on m68k it came from the
compiler's PML/fdlibm); nothing in the SDK supplies `sin`, `sqrt`, `pow`,
`floor`, `fmod`, … — measured: none of them is defined in `libc.a`. Floating
point *arithmetic* is native AArch64 and works; *functions* do not exist.

## 5. Runtime facts that differ from a MiNT/TOS libc

| Area | On NewTOS | Source |
|------|-----------|--------|
| `argc`/`argv` | always `1` / `{"", NULL}` | `sdk/libc/crt0.S:40-59` |
| `environ`, `getenv` | `environ` is a zero global; `getenv` finds nothing | `mintlib/globals.c`, crtinit excluded |
| `_base` (`BASEPAGE *`) | **stays NULL** (crtinit would set it) — dereferencing it works only because the basepage *is* at VA 0 | `mintlib/globals.c:33`, `loader.rs:170-177`; TeraDesk's `environm.c` relies on this |
| `malloc`/`free`/`realloc` | mintlib's allocator over the in-TPA arena `[_end, p_hitpa − 64 KiB)`; `ENOMEM` when exhausted, **no** `Malloc` fall-back | `newtos-arch/heap_init.c`, `unix/sbrk.c:54-71` |
| `exit`/`_exit` | `Pterm(rc)` | `newtos-arch/proc.S` |
| `clock()` | reads `_hz_200` at `$4BA` through `get_sysvar` — a live 200 Hz tick (verified by `sdk/libc/tests/clocktest.c`) | `newtos-arch/getsysvar.c`, `time/clock.c` |
| `time()`/`gettimeofday()` | GEMDOS clock via mintlib's own path (`Tgettimeofday` probe, then `Tgetdate`/`Tgettime`) — **unverified** here beyond linking | `unix/gettimeofday.c` |
| `stat`/`fstat` times, uid/gid | zero / from attributes only | §2 |
| `printf("%Lf")` | handled as `double` (`-D__NO_LONG_DOUBLE_MATH`) | `build-libc.sh:19-25` |
| `Getcookie` | over `Ssystem(S_GETCOOKIE)`; `C_FOUND`/`C_NOTFOUND` contract kept | `newtos-arch/getcookie.c` |
| `Super`/`Supexec` | bindings without privilege — see [05](05-abi-pitfalls.md#b4) | `overlay/include/mint/osbind.h` |
| `signal`/`raise`/`kill` | compile and link; whether NewTOS delivers signals to a process is **unverified** here (the kernel has an EL0 upcall mechanism, `loader.rs:735-765`) | — |
| `fork`/`vfork`/`exec*` | `fork` is a weak symbol that compiles; `vfork` is dangling; `execve` links — behaviour **unverified**; NewTOS is single-tasking | — |

## 6. References `libc.a` leaves unresolved (measured)

`llvm-nm` over the archive: 33 symbols are referenced by some member but
defined by none. A link only fails if the member that references them is
pulled in — `build/why.txt` (written by every link, `--why-extract`) shows
the chain.

```
__divtf3 __eqtf2 __getf2 __lttf2 __multf3 __trunctfdf2      quad-float compiler-rt (long double)
__getpwnam_r __getpwuid_r getpwuid getgrent setgrent endgrent pututline   passwd/group/utmp DB
__libc_argc __libc_argv                                     crtinit (excluded)
__sys_fstat __waittime __waitval                            other excluded/unbuilt members
getcontext makecontext swapcontext vfork                    m68k .S never assembled
nf_init                                                     nf_ops.c (does not compile)
spawnlp spawnvp                                             spawnvp.c (does not compile)
socket gethostbyname                                        socket/ not compiled
bios_table gemdos_table xbios_table yyin yyparse            syscall/ generator tools
_end                                                        provided by sdk/newtos.ld at link
```

QED (37 files, cflib+gemlib+libc) and TeraDesk (48 files, gemlib+libc) link
with **0** undefined symbols against this archive (`build-qed.sh:167`,
`build-teradesk.sh:187`; `TASK/T01-STATUS-DONE.MD`), so none of the above
is on their paths.

## 7. What the test programs prove

| Program | Proves | Source |
|---------|--------|--------|
| `sdk/libc/tests/traplink.c` | the retargeted `osbind.h` macros compile and link without `features.h`/`ostruct.h`; `Cconws`, `Fcreate`/`Fwrite`/`Fclose`, `Fopen`/`Fread` round-trip | `:1-17` |
| `sdk/libc/tests/libtest.c` | `printf("%d %ld")`, `malloc`+`free`, `fopen("wb")`/`fwrite`/`fclose` then `fopen("rb")`/`fread` round-trip on `C:` — verified on QEMU (ADR-019, `docs/MEMORY.md:1160-1166`: `-42 305419896`, `heap: heap string`, `roundtrip OK (37 bytes)`) | `:1-14` |
| `sdk/libc/tests/clocktest.c` | mintlib's unmodified `clock()` counts 200 Hz against `Tgettime` edges | `:1-28` |
| `docs/sdk/examples/hello-libc.c` | `printf`, `malloc`/`free`, `Sversion`, `Tgettime` — run 2026-09-17, see [03](03-first-programs.md) | — |
