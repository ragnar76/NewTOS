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
width behind `printf("%f")` is fixed. Later the same day §8: the program-name
search uses NewTOS's extensions (`ntp`, `nap`, `ncp`); and then the execute
bits of `stat()`/`access()` follow the same native extensions (§8). TASK/PORT-WAITH
(2026-10-06) added §9: the `<sys/wait.h>` status macros read the status word
correctly on little-endian AArch64 (header overlay). TASK/PORT09 (2026-10-06)
updated §1 and §5: `newtos-arch/cmdargs.c` is now all of `parseargs()` —
`environ` from `p_env`, the `ARGV=` extension with `NULL:`,
`program_invocation_name`.

## 1. mintlib files that do not compile (recorded, not repaired)

`sdk/libc/expected-failures.txt` is the authoritative list; the build stops
on any failure not in it (`sdk/lib-stale.sh:229-283`). What each costs:

| File | Reason (from the record) | What a program loses |
|------|--------------------------|----------------------|
| `mintlib/linea.c` | m68k inline asm — ST Line-A | Line-A (NewTOS has none) |
| `mintlib/nf_ops.c` | m68k inline asm — ARAnyM NatFeats | `nf_init`/`nf_get_id` (an app that needs the symbols supplies its own, as the TeraDesk overlay does) |
| `mintlib/crtinit.c` | m68k asm + two LP64 basepage casts | nothing any more: `newtos-arch/cmdargs.c` is `parseargs()` with AArch64 pointer widths — `argc`/`argv` (PORT01), `environ`, the `ARGV=` extension and `program_invocation_name` (PORT09) — and `newtos-arch/heap_init.c` is `_crtinit`/`_acc_main` (`_base`, `_stksize`, Mshrink; T05, E9) — see §5 |
| ~~`mintlib/spawnve.c`, `spawnvp.c`~~ | compile since TASK/PORT10 (`sdk/libc/overlay/mintlib/`, `va_arg` instead of `va_list` arithmetic) | — the `spawn*` family works (synchronous `Pexec(0)`, QEMU-verified); `system()` is mintlib's own since TASK/PROC01 — §8 |
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
| ~~`mintlib/thread.c`~~ | builds since TASK/PROC01 from `sdk/libc/overlay/mintlib/thread.c` (LP64 casts for the 32-bit basepage fields, plus one fix: `tfork` frees the saved `Pexec(PE_CBASEPAGE)` environment instead of `b->p_env`, §8) | `tfork`, `__waitval`, `__waittime` — `vfork`'s TOS branch, `wait4` |

## 3. mintlib assembly that is never assembled

`build-libc.sh` compiles `.c` files only (`:149-169`). The 24 `.S` files in
the core directories are therefore absent unless a shim replaces them:

| Replaced by a shim | Missing — **undefined symbol if used** |
|--------------------|----------------------------------------|
| `string/bcopy.S`, `memset.S`, `bzero.S` → `memops.c`; `mintlib/libc_exit.S` → `proc.S`; `mintlib/getcookie.S` → `getcookie.c`; `mintlib/getsysvar.S` → `getsysvar.c`; `startup/crt0.S` → `sdk/libc/crt0.S`; **`mintlib/setjmp.S` → `setjmp.S`** (since 2026-09-21, TASK/T07 — see below); **`unix/vfork.S` → `vfork.S`, `mintlib/setstack.S` → `setstack.S`** (since 2026-10-05, TASK/PROC01 — §8) | `frexp.S`, `ldexp.S`, `modf.S` (the libm supplies `frexp`/`ldexp`/`modf` since T07, §4); `stdlib/alloca.S` (clang's `__builtin_alloca` covers `alloca()` when `<alloca.h>` maps to it — **unverified**); `getcontext.S`, `makecontext.S`, `setcontext.S`, `swapcontext.S`; `stkover.S`, `checkcpu.S`, `_mon.S`, `elfattr.S`, `_infinitydf.S`, `_normdf.S` |

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
| `argc`/`argv` | mintlib's `parseargs()` rules (TASK/PORT01, TASK/PORT09). **With an `ARGV=` variable in the environment** (Atari's extended argument spec, `references/freemint/doc/programmer/atari/ARGV.txt`): `argv[0]` is the string after `ARGV=` (the launcher's path name), the arguments are the strings after it — unbounded, blanks inside an argument kept; the command tail is not read. As in mintlib the tail's length byte 127 is **not** required (`STRICTLY_COMPATIBLE_WITH_STANDARD` is off, `crtinit.c:61-70`). `ARGV=NULL:i,j,...` cuts the listed entries to `""` (`ARGV-blank.txt`). `program_invocation_name` / `_short_name` become `argv[0]` and its last `\`- or `/`-component. **Without `ARGV=`**: from the command tail `p_cmdlin` — `argv[0]` = `""`, leading blanks skipped, split on runs of blanks/tabs, MagX single-quoting; `1` / `{"", NULL}` when the tail is empty; `program_invocation_name` stays mintlib's `"unknown application"`. Who sends `ARGV=`: mintlib's own `spawn*`/`system()` always (`mintlib/spawn.c:353-420`), TeraDesk for a `PT_ARGV` program type when the command line is not empty (`applik.c:1470`, `startprg.c:543-567`). Host-checked by `tests/cmdargs-host.sh` (21 cases), proved on the machine by `C:\ARGVTEST.NAP` (14 arguments through `ARGV=`: argc 15; `measurements/measurement-2026-10-06-port09/`) | `newtos-arch/cmdargs.c`, `newtos-arch/heap_init.c`, `sdk/libc/crt0.S` |
| `environ`, `getenv` | built from the environment block `p_env` as mintlib does (TASK/PORT09): one entry per string up to the empty one, pointing **into** the block; an `ARGV=` string ends the environment and is cut **in place** (its `A` becomes NUL, so a later `Pexec(.., env = NULL)` child inherits the environment without the arguments); the desktop form `"PATH=\0C:\\\0"` — every `#Z`, desktop and accessory start gets a copy of the AES's (TASK/PPARENT, `docs/api/gemdos/process/_parent-and-environment.md`) — is joined **in place** to `"PATH=C:\\"` (`crtinit.c:332-345`). A program that reads its raw `p_env` block after start-up therefore sees mintlib's rewrite, not the bytes the kernel delivered — as on TOS with mintlib. `p_env == 0` or an empty block give the empty environment (one NULL); `environ` is never NULL (mintlib's `_spawnve` indexes it unconditionally, TASK/PORT10). An accessory gets `environ = {"", NULL}` like mintlib's `_acc_main` (no walk). The arrays (environ, the split tail, argv) lie in the TPA just above the BSS, as mintlib lays them out, and count to what the program keeps (`heap_init.c`, `crtinit.c:167-172`); an environment too large for the TPA stops the start with mintlib's "insufficient memory" | `newtos-arch/cmdargs.c`, `newtos-arch/heap_init.c`, `mintlib/globals.c`, `stdlib/getenv.c`; `kernel/src/launch.rs`, `loader.rs` |
| `_base` (`BASEPAGE *`) | set by `__libc_init_heap` from the basepage address crt0 hands it (TASK/T05; until then it stayed NULL and dereferencing it worked only because the basepage was at VA 0). Since ARCH03 stage 1 the basepage is the start of the TPA block, so `_base` is that address — never 0 | `newtos-arch/heap_init.c`, `sdk/libc/crt0.S`; readers: `unix/getppid.c`, `gmon.c`, TeraDesk's `environm.c` |
| `malloc`/`free`/`realloc` | mintlib's allocator; since ARCH03 E9 the start shrinks the TPA like `_crtinit` and, with the default `_stksize`, malloc takes its memory from GEMDOS `Malloc` (`_split_mem`) | `newtos-arch/heap_init.c`, `crtinit.c:212-247`, `unix/sbrk.c:54-71` |
| `exit`/`_exit` | `exit()` is mintlib's: atexit handlers in reverse order, then every stdio stream closed (flushed), then `_exit` = `Pterm(rc)`. A `return` from `main` goes through it: the libc `crt0` ends with `exit(main(argc, argv, environ))`, as mintlib's `__libc_main` ends (`mintlib/main.c:302`) — since post-E9 (2026-10-05); before, crt0 called `Pterm` directly and buffered output and atexit handlers were lost. The rest of `__libc_main` is not run (see `sdk/libc/crt0.S`: it breaks stdout until NewTOS `Fseek` answers 0 on a console handle as EmuTOS does) | `stdlib/exit.c:43-59`, `newtos-arch/proc.S`, `sdk/libc/crt0.S` |
| `clock()` | reads `_hz_200` at `$4BA` through `get_sysvar` — a live 200 Hz tick (verified by `sdk/libc/tests/clocktest.c`) | `newtos-arch/getsysvar.c`, `time/clock.c` |
| `time()`/`gettimeofday()` | GEMDOS clock via mintlib's own path (`Tgettimeofday` probe, then `Tgetdate`/`Tgettime`) — **unverified** here beyond linking | `unix/gettimeofday.c` |
| `stat`/`fstat` times, uid/gid | zero / from attributes only | §2 |
| `printf("%Lf")` | handled as `double` (`-D__NO_LONG_DOUBLE_MATH`) | `build-libc.sh:19-25` |
| `Getcookie` | over `Ssystem(S_GETCOOKIE)`; `C_FOUND`/`C_NOTFOUND` contract kept | `newtos-arch/getcookie.c` |
| `Super`/`Supexec` | bindings without privilege — see [05](05-abi-pitfalls.md#b4) | `overlay/include/mint/osbind.h` |
| `signal`/`raise`/`kill` | compile and link; whether NewTOS delivers signals to a process is **unverified** here (the kernel has an EL0 upcall mechanism, `loader.rs:735-765`) | — |
| `fork`/`vfork`/`exec*` | `fork` is a weak symbol that compiles; `vfork` works (TOS branch: `tfork` = `Pexec` 5 + 4, QEMU-verified, §8); `execve` is `Pexec(200)` on TOS, which NewTOS answers `EINVFN` like EmuTOS — it fails with `ENOSYS` | `newtos-arch/vfork.S`, `mintlib/thread.c` |

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
__sys_fstat                                                 other excluded/unbuilt members
getcontext makecontext swapcontext                          m68k .S never assembled
(__waitval __waittime vfork: defined since TASK/PROC01 -- thread.c, vfork.S)
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
| `sdk/libc/tests/waith-host.sh` (+ `waith-probe.c`, `waith-host.c`, `waith-host-mintbind.h`) | the `<sys/wait.h>` macros and `union wait` on known status words and through mintlib's unmodified `wait4.c`/`waitpid.c`/`wait.c` — 74 checks, exit status = failures (§9) | `waith-host.sh:1-19`, `waith-probe.c:1-20` |

## 8. `system()` and `vfork()` (TASK/PORT10, TASK/PROC01)

`system()` is **mintlib's own `posix/system.c`** again (TASK/PROC01,
2026-10-05; from 2026-10-02 to then an overlay without its POSIX branch stood
in). mintlib picks at run time:

* the **POSIX branch** when `/bin/sh` is executable or `$SHELL` is set:
  `vfork()`, the child `execve(shell, {name, "-c", line})`, the parent
  `waitpid()`. On NewTOS, as on EmuTOS, `execve` is `Pexec(200)` (mintlib's
  `_spawnve(P_OVERLAY)`), which the kernel answers with `EINVFN`, so the child
  `_exit(127)`s and `system()` returns the `waitpid` status `127 << 8`,
  i.e. `WEXITSTATUS == 127` — QEMU-verified as the status word
  (`measurements/measurement-2026-10-05-arch03-2/`, VFORKT) and, since the
  §9 header fix, through `WIFEXITED`/`WEXITSTATUS`
  (`measurements/measurement-2026-10-06-waith/`);
* the **TOS branch** (`system_hack`) otherwise — the case of every NewTOS
  program that does not set `$SHELL` (`environ` starts empty): split the line
  into words (`'`/`"`/`` ` `` group), `<`, `>`, `>>` redirect handles 0/1 via
  `Fopen`/`Fcreate` + `Fdup`/`Fforce`, then `spawnvp(P_WAIT, argv[0], argv)` =
  a synchronous `Pexec(0)`; the result is the child's `Pterm` code, `-1` with
  `errno` `ENOENT` when the program is not found, `2` when a redirection file
  cannot be opened. `system(NULL)` is 1. Note: this is the **raw** exit code
  (mintlib `system_hack` returns what `spawnvp(P_WAIT)` returns), not a wait
  status — `WEXITSTATUS()` does not apply to it; `system("X 42")` is `42`, not
  `42 << 8`. That is mintlib's TOS behaviour, unchanged.

`vfork()` is `sdk/libc/newtos-arch/vfork.S`, the AArch64 port of mintlib's
`unix/vfork.S`: with `__mint == 0` (always on NewTOS) the TOS branch,
`tfork(newprog, save)` from `mintlib/thread.c` = `Pexec(PE_CBASEPAGE)`, then
`Pexec(104)` (answered `EINVFN` = `-ENOSYS`), then `Pexec(PE_GO)` on code of
the program itself. The child returns 0 from `vfork` into the caller's frame
and runs **before** the parent continues (TOS `Pexec` is synchronous); the
parent gets the child's pid when the child has ended, and `waitpid`/`wait4`
deliver its exit status from `__waitval`. Differences of the port: the child
resumes on the caller's stack pointer (AArch64 code reaches its locals through
`sp`, m68k through `a6`), and the parent's own `tfork` call runs on a separate
16-KiB stack so the child may use the stack below; the save area and that
stack are static, so a `vfork` from inside a `vfork` child before it ends is
not supported (the m68k original has the same static save area). **One fix
to mintlib's `tfork`** (2026-10-06, `docs/analysis/vfork-malloc-ownership-2026-10-06.md`):
in mintlib 0.60.1 the thread child `Mfree`s its basepage's environment and
adopts the parent's (`b->p_env = parent->p_env`), and after the child the
parent `Mfree`d `b->p_env` — its **own** environment, and on later forks
whatever block had since been allocated at that address (a `malloc` hunk:
VFORKT crashed in `exit()` under memprot=on). The overlay's `tfork` saves the
environment pointer of the `Pexec(PE_CBASEPAGE)` basepage before starting the
child and frees that saved pointer (both the success and the `pid < 0`
branch). Evidence `measurements/measurement-2026-10-06-vforkt-fix/`. Host check
`sdk/libc/tests/system-host.sh` (TOS branch, and that the object carries the
POSIX branch); target checks `sdk/examples/port10.c` (TOS branch) and the
ARCH03 stage-2 probe VFORKT (vfork, waitpid, POSIX branch).

What a caller must know:

* **The search list is NewTOS's, not mintlib's** (user decision
  2026-10-02). A name without extension is tried as given, then with `.ntp`,
  `.nap`, `.ncp` — in that order, replacing mintlib's `.ttp`, `.prg`, `.tos`
  position by position. On NewTOS only the native AArch64 programs are
  executable; `.TTP`/`.PRG`/`.TOS` are m68k and belong to the later m68k
  emulator, which will append them **after** the native three. `.NAC`
  (accessory) is not startable and is not in the list. Overlays:
  `sdk/libc/overlay/mintlib/spawnvp.c` (what `spawnvp`/`spawnlp`/`system()`
  use), `mintlib/spawn.c` (the `#!` interpreter lookup) and `posix/execvp.c`
  (`{ "", .ntp, .nap, .ncp }` — its `.app`/`.gtp` are dropped too, both m68k).
  `system("C:\\P10\\P10KID s3")` now runs `P10KID` (exit code 42, measured,
  `measurements/measurement-2026-10-02-port10c/`); a name whose only file is
  `LEGACY.PRG` is `-1`/`ENOENT`. An explicit extension is used as given —
  also an m68k one (the list does not gate what the caller names; whether
  `Pexec` loads it is the kernel's decision, not measured here). `_buffindfile` appends
  the lower-case string (`P10KID.ncp`); GEMDOS upper-cases names on FAT, so it
  matches `P10KID.NCP`. Host check `sdk/libc/tests/spawn-host.sh`
  (`spawn-search-host`). The PATH part of the search reads `getenv("PATH")`,
  which is empty while `environ` is (§5).
* **The execute bits are NewTOS's too** (user decision 2026-10-02,
  follow-up). `stat()`/`lstat()` (`__do_stat` = `__quickstat`), `access()`
  (`newtos-arch/access.c`), `_buffindfile`, `open()` and `glob()` take
  `st_mode` from `__quickstat`; `Fstat64` is `EINVFN` (= mintlib's
  `-ENOSYS`) on NewTOS, so its `Fsfirst` path decides, and it sets `0111` on
  a regular file by the extension **as the caller spelled it**. Pristine
  mintlib: `.app/.gtp/.ttp/.prg/.tos`, `strcmp`, lower case only — a FAT
  name `X.NCP` was not executable, `x.prg` was. The overlay
  `sdk/libc/overlay/mintlib/quickstat.c` replaces the list with `.ntp`,
  `.nap`, `.ncp`, compared with `strcasecmp`; the m68k extensions come back
  with the emulator, after the native ones; `.nac` is not executable.
  Directories keep `0755` whatever their name; `FA_RDONLY`/`FA_HIDDEN`
  still clear the write/read bits. Measured in QEMU raspi4b:
  `access("C:\\P10\\P10KID.NCP", X_OK)` and `access("c:\\p10\\p10kid.ncp",
  X_OK)` are 0, `.NTP`/`.NAP` too; `LEGACY.PRG`/`legacy.prg`, `ACC.NAC`,
  `PLAIN.TXT` are `-1`/`EACCES`; `stat().st_mode` `0100755` vs `0100644`
  (`measurements/measurement-2026-10-02-port10d/`). Host check
  `sdk/libc/tests/quickstat-host.sh` (27 checks; with the pristine file 15
  fail). The other copies of the list are not built: `mintlib/do_stat.c`
  (excluded, replaced by `newtos-arch/do_stat.c`) carries the same old list,
  and the excluded `mintlib/do_fstat.c` set `0111` for an open file whose
  first word is the m68k `0x601a`/`#!` magic — the lean
  `newtos-arch/do_fstat.c` sets no execute bit for `fstat()` at all.
* **Redirection does not reach the child yet (kernel).** The child's standard
  handles are not inherited from the parent (`loader.rs` sets `p_uft` to
  `-1`; EmuTOS `bdos/proc.c` `init_pd_files` inherits them via `ixforce`), so
  `system("prog >file")` creates `file` but the child still writes to the
  console. And `Fforce` keeps no use count (EmuTOS `bdos/fshand.c` `ixforce`
  increments `f_use`): after `system_hack` restores handle 1 with
  `Fforce(1, dup)` and closes `dup`, the caller's handle 1 is dead —
  `Fwrite(1, …)` returns `EIHNDL` (-37), so `printf` output is lost from then
  on. Both measured in `port10.c`; avoid `>`/`<` in `system()` lines until
  the kernel follows EmuTOS here.
* `/dev/null` becomes `null:` (`mintlib/unx2dos.c`, no MiNT); `Fcreate` of it
  fails on NewTOS, so `system("... >/dev/null")` returns 2 with a `perror`
  line and runs nothing. That is what DOOM's `I_Error` → `ZenityErrorBox`
  probe sees: DOOM reports the error and exits normally.

`misc/syslog.c` referenced the missing `vfork` until TASK/PROC01; every
symbol its object references is defined in `libc.a` now (`llvm-nm`,
2026-10-05) — syslog itself is not exercised.

`printf("%f")` (`stdio/printf_fp.c`) printed wrong digits and could fault
until TASK/PORT10: `stdlib/gmp-mparam.h` declared 32-bit GMP limbs while
`mp_limb_t` (`unsigned long`) is 64 bits on LP64. The overlay
`sdk/libc/overlay/stdlib/gmp-mparam.h` derives the width from `long`; host
check `sdk/libc/tests/gmp-host.sh`.

## 9. `<sys/wait.h>` status macros (TASK/PORT-WAITH)

The status word `wait`/`wait3`/`wait4`/`waitpid` store is an `int` built by
mintlib's `unix/wait4.c` with plain integer arithmetic: from the kernel's
`(pid << 16) | code` (on TOS from `__waitval`, which `tfork` sets to
`(pid << 16) | Pexec's return`, `mintlib/thread.c:146`) it swaps the low two
bytes, so the **exit code / stop signal is in bits 8..15, the core flag in
bit 7, the terminating signal in bits 0..6** — the same layout
`W_EXITCODE(ret, sig) = ret << 8 | sig` and `W_STOPCODE(sig) = sig << 8 | 0x7f`
in the header describe. That library code is layout-correct on AArch64
(the rebuilt `libc.a` is byte-identical with and without the fix).

The macros `WIFEXITED`, `WEXITSTATUS`, `WIFSIGNALED`, `WTERMSIG`,
`WIFSTOPPED`, `WSTOPSIG`, `WIFCOREDUMPED` and the BSD `union wait` members
(`w_retcode`, `w_termsig`, `w_coredump`, `w_stopsig`) read the word through
`struct __wait` bit-fields declared `junk:16, retcode:8, coredump:1,
termsig:7`. On big-endian m68k the first field takes the most significant
bits, which gives exactly the layout above; on little-endian AArch64
(AAPCS64) the first field takes the least significant bits, so `retcode` sat
in bits 16..23 and `termsig` in bits 25..31: for every status word wait4
produces (16 bits) `WEXITSTATUS` was 0, `WIFEXITED` true and
`WIFSIGNALED`/`WIFSTOPPED` false (found by VFORKT,
ARCH03 stage 2e). The overlay `sdk/libc/overlay/include/sys/wait.h` declares
the fields in reverse order when `__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__`
— same bit positions, same `int` layout, macros unchanged; the big-endian
branch is mintlib's text. Only the header changes; programs that use the
macros must be recompiled (no SDK app or overlay does — QED, TeraDesk, Doom,
cflib, gemlib checked; mintlib's own users are `sunrpc/`, `unix/test-vfork.c`
and `test-skeleton.c`, which `build-libc.sh` does not build).

Checks: host `sdk/libc/tests/waith-host.sh` (in `sdk/run-host-tests.sh`) —
74 checks: every macro on known words (exit 0/1/42/127/255, SIGINT, SIGTERM,
SIGSEGV + core, stopped SIGTSTP/SIGSTOP), `union wait` in both directions,
and mintlib's unmodified `wait4.c`/`waitpid.c`/`wait.c` with the `__waitval`
`tfork` leaves; with the pristine header 28 of the 74 fail. Target: VFORKT
(`waitpid` after `_exit(42)` → `WEXITSTATUS == 42`, `_exit(7)` → 7, POSIX
`system()` → `WIFEXITED && WEXITSTATUS == 127`), QEMU raspi4b,
`measurements/measurement-2026-10-06-waith/`.
