# T07 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Lauf nicht erforderlich

`setjmp`/`longjmp` sind in `libc.a` (AArch64-Retarget von mintlibs m68k
`setjmp.S`), und die SDK hat eine libm (`sdk/libm/`, OpenLibm 0.8.8, Lead-
Entscheid 2026-09-21). Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis
HEAD `53f588d`, **nicht committet, nichts geflasht**. Kein Kerneleingriff.

## Was getan wurde

| Bereich | Datei | Inhalt |
|---|---|---|
| setjmp (neu) | `sdk/libc/newtos-arch/setjmp.S` | `sigsetjmp`/`setjmp`/`siglongjmp`/`longjmp` per AAPCS64; Spiegel des m68k-Kontrollflusses inkl. `__mint`-Test und `_sigmask`-Slot (Layouttabelle im Kopf: x19–x28, x29, x30, sp, d8–d15, sigmask = 22 × 8 = 176 Byte); assembliert auch fuer den macOS-Host (`.macro ADR_GLOBAL`, `SYM()`) |
| setjmp (neu) | `sdk/libc/overlay/include/setjmp.h` | mintlibs Header, `typedef long jmp_buf[22]` unter `__aarch64__`, `[14]` sonst |
| setjmp | `sdk/libc/build-libc.sh` | `arch__setjmp.o` in die Shim-Liste, Kopfkommentar |
| setjmp-Test (neu) | `sdk/libc/tests/setjmp-cases.h`, `setjmp-host.c`, `setjmp-host.sh` | 13 Checks (Rueckgabewerte, Masken-Slot exakt wie m68k, x19–x28/d8–d15 nach einem `longjmp` aus einem alle Callee-Saved-Register verschmutzenden Callee, verschachtelt, Rahmen nach vier Ebenen); Hostharness + Runner |
| setjmp-Test (neu) | `sdk/examples/setjmptst.c` | dieselben 13 Checks als `.NCP` ueber mintlibs `<setjmp.h>` (`_Static_assert(sizeof(jmp_buf) == 176)`), Verdikte per `Cconws` |
| libm (neu) | `sdk/libm/openlibm-0.8.8.tar.gz`, `openlibm-sources.sha256`, `LICENSE.openlibm.md` | gepinnter Tarball (SHA-256 `721bab18…`, Tag-Commit `5fe39974`), Provenienz, Lizenz verbatim (MIT/ISC/BSD-2/SunPro; LGPL nur Tests, nicht gebaut) |
| libm (neu) | `sdk/libm/build-libm.sh`, `expected-failures.txt` (leer) | Bauskript nach dem Muster der drei anderen: SHA-Pruefung, Manifest aus OpenLibms `Make.files` per `make` ohne Long-Double-Block/`ld128/`, Fehlerbuchhaltung, `libm.a`, Messung der haengenden Referenzen (`build/dangling.txt`), Client-Link `libm.a`+`libc.a` |
| libm (neu) | `sdk/libm/include/{math,complex,fenv}.h` | Standardnamen → `openlibm_*.h`; mintlib-Feature-Makros → `__BSD_VISIBLE`/`__XSI_VISIBLE` |
| libm-Test (neu) | `sdk/examples/mathtst.c` | 23 Checks gegen geschlossene Werte mit Toleranzen, eigener Festkomma-Drucker, `Cconws` |
| Kette | `sdk/lib-stale.sh` | `libm`: Archiv, Builder, Kette `libc → libm`, Inputs `overlay/` + Skript |
| Apps | `sdk/apps/build-qed.sh`, `build-teradesk.sh` | `lib_ensure libm`, Include-Pfade, `libm.a` vor `libc.a` in der `--start-group` |
| Image | `tools/make-sd-image.sh` | `C:\SETJMPT.NCP`, `C:\MATHTST.NCP`; `SD_SETJMPTST_AUTO=1`/`SD_MATHTST_AUTO=1` → `\AUTO\` |
| Docs | `docs/sdk/07-mintlib-limits.md` §3/§4/§6/§7, `08-porting-checklist.md`, `04-porting-anatomy.md`, `05-abi-pitfalls.md` B12, `06-image-and-testing.md`, `README.md`; `sdk/.gitignore` | nachgezogen |
| Status | `docs/STATUS.md` (Phase-15-Zeile + Recent Decisions 2026-09-21), `TASK/T07.md` Abschluss | |

## Belege (`measurement-2026-09-21-t07/`)

* `host-tests/setjmp-host.txt` — 13/13 PASS, rc 0 (macOS arm64, `clang -O2 -fno-builtin`).
  Sabotage-Gegenprobe (Scratch, nicht abgelegt): ohne `ldp d8,d9` bzw. `ldp d14,d15`
  faellt genau „8 double values in d8-d15 …"; ohne `ldp x27,x28` bzw. `ldp x19,x20`
  genau „10 long values in x19-x28 …"; ohne `ldp x29,x30` Absturz.
* `serial/qemu-raspi4b-auto-setjmptst-mathtst.log` — `[auto] running C:\AUTO\SETJMPT.NCP`
  … `setjmptst: ALL PASS (13 checks)`, `[auto] running C:\AUTO\MATHTST.NCP` …
  `mathtst: ALL PASS (23 checks)`, `[auto] 3 AUTO program(s) run`, `[desk] desktop up`.
* `host-tests/xtask-test.log` — `cargo run -p xtask -- test`: 906 passed / 0 failed / 1 ignored.
* `build/` — `build-libc-setjmptst.log` (613/11, failures match, LINK OK),
  `build-libm-mathtst.log` (175/0, 279 Symbole, 16 haengend, LINK OK, 0 undef.),
  `build-qed.log`/`build-teradesk.log` (LINK OK, 0 undef. mit `libm.a` in der Gruppe),
  `libc-dangling-before-53f588d.txt` = `libc-dangling-after.txt` (33, identisch),
  `libm-dangling.txt` (16), `*-why.txt`, `xtask-build-rpi4.log` (64/94 = Baseline),
  `make-sd-image.log`, `artifacts.sha256`.
* `method/run.sh` (Ablauf), `product-t07.diff` (geaenderte Dateien), `new-files.txt`.

## Messung der haengenden Referenzen

`libc.a`: vorher 33, nachher 33, dieselbe Liste (`__divtf3 … yyparse`) — `setjmp.S`
referenziert `_sigmask` (`signal/initsig.c`) und `__mint` (`mintlib/globals.c`),
beide im Archiv. `libm.a`: 16 — `__assert_fail _ctype memset` (aus `libc.a`),
`__addtf3 __subtf3 __multf3 __divtf3 __gttf2 __netf2 __extendsftf2 __extenddftf2
__trunctfsf2 __trunctfdf2` (binary128, kein compiler-rt: `e_powf`, `e_remainder(f)`,
`s_fdim`, `s_nexttowardf`), `__muldc3 __mulsc3` (komplexe Multiplikation: `casin`,
`casinh`, `cacosh`, `catanh` + f), `scalbnl` (`s_scalbln`).

## Offen

* Die Soft-Float-Helfer fuer binary128 (und `__muldc3`/`__mulsc3`) — im Ticket
  als eigener Punkt benannt, nicht Teil von T07. Bis dahin sind `powf`,
  `remainder(f)`, `fdim(f)`, `scalbln(f)`, `nexttowardf`, `casin`/`casinh`/
  `cacosh`/`catanh`(+f) und alle `…l`-Funktionen undefinierte Symbole.
* Pi-4-Lauf der beiden `.NCP`: nicht erforderlich (kein HW-abhaengiger Pfad;
  `setjmp` ist reines AAPCS64, libm reines C), aber wer flasht, findet sie auf
  `C:\`.
* `build-coremark.sh` bleibt ohne libm (CoreMark rechnet ohne; Flags sind Teil
  des gemeldeten Benchmark-Setups).
