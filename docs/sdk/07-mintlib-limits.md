# 07 — What the ported mintlib does not provide

Derived from `sdk/libc/build-libc.sh` (what is compiled at all),
`sdk/libc/expected-failures.txt` (what is known not to compile), the shim
headers in `sdk/libc/newtos-arch/`, and an `llvm-nm` pass over the `libc.a`
built on 2026-09-17 (613 objects compiled, 11 failed, 6 excluded —
`sdk/libc/build/{compiled,failed,excluded}.txt`); re-measured 2026-09-21
after TASK/T07 added `setjmp` and the libm (§3, §4, §6 updated; the numbers
that did not change are left as measured), and again 2026-09-25 after
TASK/PORT01 added `newtos-arch/cmdargs.c` (§1, §5, §6). TASK/PORT10
(2026-10-02) updated §1, §5, §6 and added §8: `spawnve.c`/`spawnvp.c` compile
(9 failures now, not 11), `environ` is the empty environment, and the GMP limb
width behind `printf("%f")` is fixed.

## 1. mintlib files that do not compile (recorded, not repaired)

`sdk/libc/expected-failures.txt` is the authoritative list; the build stops
on any failure not in it (`sdk/lib-stale.sh:229-283`). What each costs:

| File | Reason (from the record) | What a program loses |
|------|--------------------------|----------------------|
| `mintlib/linea.c` | m68k inline asm — ST Line-A | Line-A (NewTOS has none) |
| `mintlib/nf_ops.c` | m68k inline asm — ARAnyM NatFeats | `nf_init`/`nf_get_id` (an app that needs the symbols supplies its own, as the TeraDesk overlay does) |
| `mintlib/crtinit.c` | m68k asm + two LP64 basepage casts | `environ` set-up and the `ARGV=` extension — see §5. **`argv`/`argc` and `_base` are covered**: `newtos-arch/cmdargs.c` reimplements `parseargs()`'s argument half (PORT01) and `newtos-arch/heap_init.c` sets `_base` (T05) |
| ~~`mintlib/spawnve.c`, `spawnvp.c`~~ | compile since TASK/PORT10 (`sdk/libc/overlay/mintlib/`, `va_arg` instead of `va_list` arithmetic) | — the `spawn*` family works (synchronous `Pexec(0)`, QEMU-verified); **`system()` still does not link** — §8 |
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
| `string/bcopy.S`, `memset.S`, `bzero.S` → `memops.c`; `mintlib/libc_exit.S` → `proc.S`; `mintlib/getcookie.S` → `getcookie.c`; `mintlib/getsysvar.S` → `getsysvar.c`; `startup/crt0.S` → `sdk/libc/crt0.S`; **`mintlib/setjmp.S` → `setjmp.S`** (since 2026-09-21, TASK/T07 — see below) | `frexp.S`, `ldexp.S`, `modf.S` (the libm supplies `frexp`/`ldexp`/`modf` since T07, §4); `stdlib/alloca.S` (clang's `__builtin_alloca` covers `alloca()` when `<alloca.h>` maps to it — **unverified**); `unix/vfork.S`, `getcontext.S`, `makecontext.S`, `setcontext.S`, `swapcontext.S`; `mintlib/setstack.S`, `stkover.S`, `checkcpu.S`, `_mon.S`, `elfattr.S`, `_infinitydf.S`, `_normdf.S` |

**`setjmp`/`longjmp` (T07, 2026-09-21).** ADR-019 listed "setjmp (aarch64,
when pulled)" (`docs/MEMORY.md:1139`); it is pulled: `sdk/libc/newtos-arch/
setjmp.S` is the AArch64 retarget of mintlib's m68k `setjmp.S`, exporting the
same four symbols `sigsetjmp`, `setjmp`, `siglongjmp`, `longjmp` (mintlib's
`<setjmp.h>` builds `_setjmp`/`_longjmp` and the `setjmp()` macro on those;
under `_GNU_SOURCE` `setjmp(jb)` is `sigsetjmp(jb, 1)`). The control flow is
the m68k source's, instruction for instruction: the mask slot holds
`savemask`, then — when non-zero — `_sigmask | 1` (mintlib's TOS-emulation
mask, `signal/initsig.c`), or `Psigblock(0) | 1` when `__mint` is non-zero;
`longjmp` calls `Psigreturn()` when `__mint` is non-zero, restores `_sigmask`
(bit 0 cleared, plus `Psigsetmask` under MiNT) when the slot is non-zero, and
returns `val` or 1. On NewTOS `__mint` is 0 (only `crtinit.c` writes it, and
it does not compile), so the GEMDOS calls are never issued. The buffer is
`typedef long jmp_buf[22]` on AArch64 (overlay `include/setjmp.h`; m68k keeps
`[14]`): x19–x28, x29, x30, sp, d8–d15, sigmask — 176 bytes, layout table in
`setjmp.S`. Verified twice: on the host (`sdk/libc/tests/setjmp-host.sh`,
13 checks, the register check proven by sabotage — removing the `d8–d15` or
`x19/x20` restore fails exactly it) and on NewTOS (`sdk/examples/setjmptst.c`
→ `C:\AUTO\SETJMPT.NCP`, QEMU raspi4b serial: `setjmptst: ALL PASS (13
checks)`, `measurements/measurement-2026-09-21-t07/`).

## 4. mintlib directories not compiled at all

`DIRS` (`build-libc.sh:109`) covers `stdio stdlib string mintlib unix syscall
time misc multibyte signal termios locale dirent gmp posix tz`. Not compiled:
`argp`, `conf`, `crypt`, `login`, `pwdgrp`, `shadow`, `socket`, `startup`,
`stdiio`, `sunrpc`, `sysvipc`. Consequences: no sockets (`socket`,
`gethostbyname` are dangling references), no `crypt`, no passwd/group
database (`getpwuid`, `getgrent`, … dangling), no SysV IPC, no `argp`, no
integer-only `stdiio` (the full `printf` is linked instead,
`build-qed.sh:30-31`).

**libm: OpenLibm (T07, 2026-09-21).** mintlib never contained a libm nor a
`<math.h>` (on m68k both came from the compiler's PML/fdlibm; its
`include/tcmath.h` is a placeholder that `#error`s), and `libc.a` defines none
of `sin`, `sqrt`, `pow`, `floor`, `fmod`, … (measured, unchanged). The SDK's
libm is **OpenLibm 0.8.8** (lead decision 2026-09-21; the FreeBSD msun /
OpenBSD libm / fdlibm-5.3-derived standalone libm the Julia project maintains,
MIT + ISC + 2-clause BSD + the SunPro fdlibm notice — `sdk/libm/
LICENSE.openlibm.md`, provenance and SHA-256 in `sdk/libm/openlibm-sources.sha256`),
built exactly like the other three libraries: pinned tarball + `overlay/`
(empty so far) + `sdk/libm/build-libm.sh` → `sdk/libm/libm.a`, with the
`expected-failures.txt` gate (empty) and a place in `lib-stale.sh`'s chain
(`libc → libm`, a second branch off libc; gemlib/cflib do not depend on it).

What is in `libm.a` (175 objects, 279 defined symbols, OpenLibm's own
`src/`, `bsdsrc/` and `aarch64/` `Make.files` manifests expanded by `make`):
the C99 `double` and `float` functions, the complex functions, `tgamma`, and
the `<fenv.h>` functions over FPCR/FPSR (`aarch64/fenv.c`). Pure C — OpenLibm
has no AArch64 assembly beyond that. What is **not** in it: the `long double`
(`…l`) functions and `ld128/` — AAPCS64 `long double` is binary128, every
operation on it is a soft-float libcall, and the SDK links no compiler-rt
([05 B12](05-abi-pitfalls.md#b12-long-double-is-128-bit-and-there-is-no-compiler-rt));
OpenLibm's Darwin/aarch64 configuration makes the same cut, so no source
needed an overlay. Five `double`/`float` members carry `long double` on a
NaN/inf path or share their object with an `l` variant and are therefore
unlinkable — `powf`, `remainder`, `remainderf`, `fdim`/`fdimf`, `scalbln`/
`scalblnf`, `nexttowardf` — and four complex ones need the compiler-rt
complex multiply (`casin`, `casinh`, `cacosh`, `catanh` and the `f` forms);
§6 lists the references. Everything else is usable: `sqrt`, `sin`, `cos`,
`tan`, `atan2`, `exp`, `log`, `log10`, `pow`, `floor`, `ceil`, `fmod`,
`frexp`, `ldexp`, `modf`, `fabs`, `hypot`, `cbrt`, `sqrtf`, `isnan`/`isinf`,
… verified on NewTOS (`sdk/examples/mathtst.c` → `C:\AUTO\MATHTST.NCP`,
QEMU raspi4b serial: `mathtst: ALL PASS (23 checks)`,
`measurements/measurement-2026-09-21-t07/`).

Headers: a program includes `<math.h>` (and `<complex.h>`, `<fenv.h>`); the
adapters in `sdk/libm/include/` map those names onto OpenLibm's
`openlibm_math.h`/`openlibm_complex.h`/`openlibm_fenv.h` and turn mintlib's
feature macros (`__USE_MISC`/`__USE_XOPEN`, set by `_GNU_SOURCE`) into the
`__BSD_VISIBLE`/`__XSI_VISIBLE` gates OpenLibm uses for `M_PI` & Co. The app
scripts add `-I sdk/libm/include -I sdk/libm/openlibm-0.8.8/include` after
mintlib's include directory and `libm.a` before `libc.a` in the link group
(`build-qed.sh`, `build-teradesk.sh`; both still link with 0 undefined —
neither uses the libm). `libm.a` needs three libc symbols, `memset`, `_ctype`
(`nan()`) and `__assert_fail` (`j0`/`j1`), hence the group.

## 5. Runtime facts that differ from a MiNT/TOS libc

| Area | On NewTOS | Source |
|------|-----------|--------|
| `argc`/`argv` | built from the BasePage command tail `p_cmdlin` with mintlib's own rules (`argv[0]` = `""`, leading blanks skipped, split on runs of blanks/tabs, MagX single-quoting); `1` / `{"", NULL}` when the tail is empty. Host-checked by `tests/cmdargs-host.sh`, proved on the machine by `C:\ARGVTEST.NAP` (PORT01, `measurements/measurement-2026-09-25-port01/`) | `newtos-arch/cmdargs.c`, `sdk/libc/crt0.S` |
| `environ`, `getenv` | `environ` is the **empty environment** (one NULL, published by `__libc_parseargs` since TASK/PORT10 — mintlib's `_spawnve` indexes it unconditionally); `getenv` finds nothing. `p_env` is **never read**: desktop, AUTO and boot launcher pass `EnvSpec::None` (`p_env` 0), and a program started by another program's `Pexec` (TeraDesk, `spawn*`) gets a physical address that is not mapped in its EL0 space (TASK/MEM03). The walk and the `ARGV=` extension are TASK/PORT09, blocked on MEM03 | `newtos-arch/cmdargs.c`, `mintlib/globals.c`, crtinit excluded; `kernel/src/launch.rs`, `loader.rs` |
| `_base` (`BASEPAGE *`) | set by `__libc_init_heap` from the basepage address crt0 hands it (TASK/T05; until then it stayed NULL and dereferencing it worked only because the basepage *is* at VA 0 — the value is still 0 today, `USER_VA_BASE`) | `newtos-arch/heap_init.c`, `sdk/libc/crt0.S`; readers: `unix/getppid.c`, `gmon.c`, TeraDesk's `environm.c` |
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

`llvm-nm` over the archive: **31** symbols are referenced by some member but
defined by none (33 before PORT01: `newtos-arch/cmdargs.c` now defines
`__libc_argc`/`__libc_argv`, which only `posix/wordexp.c` referenced). A link
only fails if the member that references them is pulled in —
`build/why.txt` (written by every link, `--why-extract`) shows the chain.
Re-measured 2026-09-21 after T07: **still 33, the same list** —
`setjmp.S`'s two references (`_sigmask`, `__mint`) resolve inside the archive
(`signal/initsig.c`, `mintlib/globals.c`), and the libm is a separate archive
(its own list follows). `setjmp`/`longjmp` were never in this list because no
mintlib member references them; they were simply undefined.

```
__divtf3 __eqtf2 __getf2 __lttf2 __multf3 __trunctfdf2      quad-float compiler-rt (long double)
__getpwnam_r __getpwuid_r getpwuid getgrent setgrent endgrent pututline   passwd/group/utmp DB
__sys_fstat __waittime __waitval                            other excluded/unbuilt members
getcontext makecontext swapcontext vfork                    m68k .S never assembled
nf_init                                                     nf_ops.c (does not compile)
(spawnlp spawnvp: defined since TASK/PORT10 -- spawnvp.c compiles)
socket gethostbyname                                        socket/ not compiled
bios_table gemdos_table xbios_table yyin yyparse            syscall/ generator tools
_end                                                        provided by sdk/newtos.ld at link
```

QED (37 files, cflib+gemlib+libc) and TeraDesk (48 files, gemlib+libc) link
with **0** undefined symbols against this archive (`build-qed.sh:167`,
`build-teradesk.sh:187`; `TASK/T01-STATUS-DONE.MD`), so none of the above
is on their paths.

**`libm.a` (T07, measured 2026-09-21, `sdk/libm/build/dangling.txt` — the
build script writes it on every run):** 16 symbols referenced, defined by no
member.

```
__assert_fail _ctype memset                                   libc.a (e_j0/e_j1, s_nan) -- resolve in the link group
__addtf3 __subtf3 __multf3 __divtf3 __gttf2 __netf2
__extendsftf2 __extenddftf2 __trunctfsf2 __trunctfdf2         binary128 compiler-rt: e_powf (nan_mix in long double),
                                                              e_remainder/e_remainderf (a long double NaN path),
                                                              s_fdim (fdiml shares the object), s_nexttowardf
__muldc3 __mulsc3                                             compiler-rt complex multiply: s_casin, s_casinh,
                                                              s_cacosh, s_catanh (+ the f forms)
scalbnl                                                       s_scalbln (scalblnl shares the object with scalbln/scalblnf)
```

Consequence: `powf`, `remainder`, `remainderf`, `fdim`, `fdimf`, `scalbln`,
`scalblnf`, `nexttowardf`, `casin`, `casinh`, `cacosh`, `catanh` (+`f`) fail
to link until the binary128 helpers / complex helpers exist — the separate
item TASK/T07 names ("Soft-Float-Helfer sind ein eigener Punkt"). The
`mathtst.NCP` link pulls 28 libm members and none of these.

## 7. What the test programs prove

| Program | Proves | Source |
|---------|--------|--------|
| `sdk/libc/tests/traplink.c` | the retargeted `osbind.h` macros compile and link without `features.h`/`ostruct.h`; `Cconws`, `Fcreate`/`Fwrite`/`Fclose`, `Fopen`/`Fread` round-trip | `:1-17` |
| `sdk/libc/tests/libtest.c` | `printf("%d %ld")`, `malloc`+`free`, `fopen("wb")`/`fwrite`/`fclose` then `fopen("rb")`/`fread` round-trip on `C:` — verified on QEMU (ADR-019, `docs/MEMORY.md:1160-1166`: `-42 305419896`, `heap: heap string`, `roundtrip OK (37 bytes)`) | `:1-14` |
| `sdk/libc/tests/clocktest.c` | mintlib's unmodified `clock()` counts 200 Hz against `Tgettime` edges | `:1-28` |
| `docs/sdk/examples/hello-libc.c` | `printf`, `malloc`/`free`, `Sversion`, `Tgettime` — run 2026-09-17, see [03](03-first-programs.md) | — |
| `sdk/libc/tests/setjmp-host.sh` (+ `setjmp-host.c`, `setjmp-cases.h`) | `newtos-arch/setjmp.S` on the macOS/arm64 host: return values (0 / val / 1 for 0), the sigmask slot exactly as the m68k code, x19–x28 and d8–d15 restored after a longjmp out of a callee that clobbered them, nested buffers, the frame after unwinding four frames — 13 checks, exit status = failures | `setjmp-host.sh:1-29`, `setjmp-cases.h:1-19` |
| `sdk/examples/setjmptst.c` | the same 13 checks on NewTOS through mintlib's `<setjmp.h>` (`_Static_assert(sizeof(jmp_buf) == 176)`), verdicts over `Cconws` — QEMU raspi4b `ALL PASS (13 checks)` | `:1-17` |
| `sdk/examples/mathtst.c` | 23 libm checks against closed-form values with tolerances (`sqrt(2)`, `sin(M_PI/6)`, `pow(2,10)`, `floor(-1.5)`, `frexp(8.0)`, `atan2(1,1)`, `exp`/`log`, `fmod`, `modf`, `hypot`, `cbrt`, `sqrtf`, NaN/-inf) via `<math.h>` — QEMU raspi4b `ALL PASS (23 checks)` | `:1-15` |

## 8. `system()` (TASK/PORT10, measured 2026-10-02)

`system()` does **not** link: `posix/system.c` carries mintlib's POSIX branch
(`vfork` + `execve` + `waitpid`, taken at run time only when `/bin/sh` is
executable or `$SHELL` is set) next to the TOS branch (`system_hack`: split the
line, `>`/`<` redirection via `Fcreate`/`Fforce`, then
`spawnvp(P_WAIT, argv[0], argv)`). The TOS branch's pieces all exist and run
(`sdk/examples/port10.c`); the link stops on the POSIX branch's
`vfork` (`unix/vfork.S`, m68k, never assembled) and `__waitval`/`__waittime`
(`unix/wait4.c` → defined in `mintlib/thread.c`, excluded). On plain TOS
mintlib's `vfork` is `tfork()` = `Pexec(PE_CBASEPAGE)` + `Pexec(PE_GO)` on a
basepage whose text is the parent's own code; NewTOS's `Pexec` answers modes
5/6/7 with `EINVFN` (`tos-api/src/bdos/process/pexec.rs`), so that path needs a
kernel feature. Decision pending (TASK/PORT10-STATUS-UNFINISHED.MD). Also
pulled by `misc/syslog.c` (`vfork`).

`printf("%f")` (`stdio/printf_fp.c`) printed wrong digits and could fault
until TASK/PORT10: `stdlib/gmp-mparam.h` declared 32-bit GMP limbs while
`mp_limb_t` (`unsigned long`) is 64 bits on LP64. The overlay
`sdk/libc/overlay/stdlib/gmp-mparam.h` derives the width from `long`; host
check `sdk/libc/tests/gmp-host.sh`.
