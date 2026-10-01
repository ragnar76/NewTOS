# T06 — STATUS: DONE (Hosttests + QEMU-Selbstbeleg) / Pi-Lauf nicht erforderlich

Alle Stellen der Liste in einem Paket bearbeitet 2026-09-21, Branch
`desk-rebuild`, Basis HEAD `75b2cc3`, **nicht committet, nichts geflasht**.
Kernel-Produktcode: ein Doc-Kommentar (`launch.rs`), harte Asserts in
`selftest_emmc.rs` (nur unter dem neuen Feature `selftest-emmc` erreichbar),
eine `cfg`-Aufrufstelle in `lib.rs`. Warnzahlen unveraendert.

## Checkliste

| # | Stelle | Zustand |
|---|---|---|
| 1 | `sdk/crt0.S`, `sdk/libc/crt0.S`, `heap_init.c` 0x100-Layout | erledigt (0x1000 / Segmentlayout / Sysvar-Seite) |
| 2 | `docs/ARCHITECTURE.md` Trap-ABI + Layout | erledigt (`svc #2`, x0/x1-Kontrakt, Rueckgaben; Userspace-Layout aktuell) |
| 3 | `kernel/src/launch.rs` Doc | erledigt (AUTO = `.NCP`, Grund) |
| 4 | `build-qed.sh` Overlay-Kommentar | erledigt (`memory.c` + `set.c`) |
| 5 | `make-sd-image.sh` Overlay-Pfad | erledigt (`sdk/overlays/qed/`) |
| 6 | `qed.rsc` in `build-qed.sh` | erledigt; bytegleich zur vorhandenen Datei (`cmp`; 41378 B, SHA-256 `30ac6bf2…` — die Ticket-Zahlen `41322 B / 5a2cefa2…` sind der Stand vor `9acfb94`) |
| 7 | `build-gemtest.sh`, `build-chaintest.sh` → `lib_ensure` | erledigt (`lib_ensure gemlib`) |
| 8 | `make-sd-image.sh` Meldung DUMPVDI/VDIDRAW | erledigt (getrennte, praezise Meldungen; Lauf im Log) |
| 9 | `obspec_bits.c:26` Bauzeile | erledigt (`-I sdk/libc/mintlib/include`) |
| 10 | `gemtest_verify.c` Builder-Hinweis | erledigt (Zielprogramm ohne Builder, nicht im Runner) |
| 11 | Hosttests verdrahten | erledigt: `sdk/run-host-tests.sh` — 7/7 gruen; `docs/sdk/06-image-and-testing.md` |
| 12 | `selftest_emmc.rs` `fmeta_test` hart | erledigt (9 Asserts mit Fallname, aufraeumen vorher, wiederholbar) |
| 13 | Selbsttest verdrahten | erledigt: Kernel-Feature `selftest-emmc`, eine `cfg`-Stelle in `lib.rs`; QEMU gruen, Boot laeuft weiter |

## Belege (`measurements/measurement-2026-09-21-t05-t06/`)

* `host-tests/run-host-tests.log` — `obspec_bits`, `evnt_timer_words`,
  `wind_set_addrin`, `intin_long_words` (ASan), `ptr_word_count`,
  `aesglobal_fields`, `setjmp-host`: **7 passed, 0 failed, 0 skipped**.
* `serial/selftest-emmc-serial.log` — `cargo xtask build --board rpi4
  --features selftest-emmc`, QEMU raspi4b: `[emmc] Fattrib get=0x0 set(0x21)=0x21
  get=0x21 ; set(SUBDIR)=-36`, `dir GET("C:\CLIPBRD\")=0x10 dir GET("C:\CLIPBRD")=0x10
  ; file GET=0x0 ; dir SET=-36 ; missing GET=-33`, kein `[PANIC]`, danach
  `[auto] 1 AUTO program(s) run`, `[desk] desktop up`.
* `build/make-sd-image.log` — `! tosdump/build/VDIDRAW.NAP not built …; skipping VDIDRAW`
  bei installiertem DUMPVDI (die alte Sammelmeldung ist weg).
* `build/build-qed-AFTER.log` — `resource -> …/build/qed.rsc (41378 bytes)`;
  `after-bin-sha256.txt` `30ac6bf2…`.
* `build/xtask-build-rpi4.log`, `xtask-build-qemu-virt.log`,
  `xtask-build-rpi4-selftest-emmc.log` — tos-api 64 / kernel 94 in allen drei;
  `clippy-kernel-rpi4.log`, `clippy-kernel-rpi4-selftest-emmc.log` — kernel 125
  (Baseline 125), keine Treffer in `selftest_emmc.rs`/`launch.rs`.
* `host-tests/xtask-test-AFTER.log` — 909/0/1.

## Offen

* Die uebrigen `selftest_emmc`-Bloecke drucken weiterhin Erwartungen des alten
  Phase-09-Abbilds (u. a. „expect -34 EPTHNF", was laut `3d-fopen.md` nicht
  einmal stimmt) statt zu pruefen — ausserhalb des Tickets, eigener Punkt.
* `.init_array`-Konstruktoren (mintlibs `init_clock`) werden von crt0 nicht
  ausgefuehrt — Beobachtung aus T05, kein T06-Punkt.
