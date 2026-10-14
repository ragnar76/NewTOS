# DESK43 U8b — Crate-Bildung `desktop` — Quittung

> Stand 2026-10-14 · Basis `main` @ `7699f4d7` (U6b = `df11e429`) · Aenderung **uncommitted** im gemeinsamen Baum
> (Umzug per `git mv`, im Index als Umbenennung; `programs/desktop/Cargo.toml` neu, nicht im Index)
> Auftrag: `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.1/§3.2 (Zeile U8b), §2.4 (Probe-Falle), §4.2 (G1);
> offene Punkte aus `TASK/DESK43-U8a-STATUS-DONE.MD` §7 und `TASK/DESK43-U6b-STATUS-DONE.MD` §7
> Messung: [`measurements/measurement-2026-10-14-desk43-u8b/`](../measurements/measurement-2026-10-14-desk43-u8b/RUNINFO.TXT)
> **Kein Bildschirmunterschied (0 Pixel), Orakel = U6b.** Nichts committet, keine SD-Karte, kein Pi; kein
> `git checkout/restore/clean/stash/reset`. `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: `kernel/src/desk/` ist jetzt die Bibliothek `desktop`
(`programs/desktop/`), die nur `newtos-gem` und `tos-types` kennt; der Kernel
haengt per Pfad davon ab und ruft `desktop::run`/`desktop::reset` wie vorher
den Modul-Desktop; die Driftprobe meldet das Crate (keine leere Null).
Driftprobe **4 → 4** (A 0 · B 0 · C 4 · D 0), unsafe **20 → 20**.

---

## 1. Platzierung und neues Layout

**Gewaehlt: `programs/desktop/`, Bibliothek im `programs/`-Workspace**, vom
Kernel per Pfad-Abhaengigkeit gelinkt (wie `newtos-gem` seit U6b).

Begruendung:

* **Ein Quelltext, zwei Linkziele (Review §3.1).** Das U9-Binaercrate `NEWDESK`
  wird ein `staticlib`-Programm in `programs/` (T1: `build.sh`, `crt0.S`,
  `newtos.ld`); es soll dieselbe Bibliothek linken, die der Kernel heute linkt.
  Im selben Workspace teilen beide `Cargo.lock`, Profil und Zielverzeichnis der
  Programme.
* **Die Pfad-Abhaengigkeit in das ausgeschlossene `programs/` geht ohne Umweg**
  (Review §7.4/2, in U6b mit `newtos-gem` belegt; hier erneut: `Compiling
  desktop` im Kernel-Bau, gleiche `tos-types`-Instanz, Kernelprofil). Der
  Rueckfall des Reviews (Bibliothek in den Haupt-Workspace) war nicht noetig.
* **Grenze:** Im Haupt-Workspace laege `tos-api` einen `path = "../tos-api"`
  weit entfernt — dort ebenso; die Grenze macht nicht das Verzeichnis, sondern
  die `[dependencies]`-Liste. Die wird jetzt doppelt geprueft (Probe-Gruppe D +
  Hosttest, §3).

```
programs/
├── Cargo.toml            members = ["newtos-gem", "hello-gem", "desktop"]
├── newtos-gem/           Trap-Bindungen (+ rt)
├── hello-gem/            HELLOGEM.NAP
└── desktop/              NEU (U8b) — Bibliothek, autobins = false
    ├── Cargo.toml        features board-rpi4, board-qemu-virt, selftest-desk;
    │                     deps: newtos-gem (default-features = false), tos-types
    └── src/              19 Dateien, git mv aus kernel/src/desk/:
                          mod.rs -> lib.rs, vdibind.rs -> gsx.rs, Rest namensgleich
```

Was am Quelltext geaendert wurde (sonst nur der Umzug):

| Stelle | Aenderung | Grund |
|---|---|---|
| `lib.rs` (ex `mod.rs`) | `#![no_std]`, `extern crate alloc`, `#![deny(unsafe_op_in_unsafe_fn)]`, `#![warn(missing_docs)]` | Crate-Wurzel; die Lints, die der Modul-Desktop vom Kernel erbte — keine Diagnose aendert sich |
| `lib.rs` | `#![allow(special_module_name)]` | `mod main` (= `deskmain.c`) loest im Crate-Wurzel `special_module_name` aus; auf dem `mod`-Item greift das `allow` nicht (gemessen), daher Crate-Ebene. `autobins = false` verhindert, dass Cargo `src/main.rs` als Binaerziel nimmt |
| `lib.rs` | Wurzel-`pub(super)` → `pub(crate)` | `pub(super)` ist in einer Crate-Wurzel ungueltig; Reichweite bleibt „der Desktop" |
| `lib.rs` | Desktop-Helfer `evnt_multi(flags, msg)` → **`evnt_multi_msg`** | U6b-Punkt 5: Namensgleichheit mit `newtos_gem::aes::evnt_multi` (andere Signatur) |
| `lib.rs`, `redraw.rs` | Alias `wind_get_rect` entfernt, `do_wredraw_nocursor` ruft `wind_get_grect` | U6b-Punkt 5: reiner Alias; die Doku des Rechteck-Gangs steht am Aufrufer |
| `gsx.rs` (ex `vdibind.rs`) | Modulname `gsx` | U6b-Punkt 4: keine Bindung mehr, sondern die `gsx_*`-Logik |
| Doku-Links | `[`crate::shell_loop…`]`, `[`crate::initinfo…`]`, `[`tos_api::…`]` → Klartext `kernel::…`/`tos_api::…` | wuerden im Crate ins Leere zeigen; Modultitel `desk::x` → `desktop::x` |
| `inf.rs` | `compile_error!`-Text `desk::inf` → `desktop::inf` | nur Fehlermeldung |

`DeskStatic` (U8a) und `selftest.rs` (hinter `selftest-desk`) sind
unveraendert mitgezogen; `MACHINE_NAME` (X03) haengt am Board-Feature des
Crates.

**Kernel:** `pub mod desk;` entfaellt; `kernel/Cargo.toml`:
`desktop = { path = "../programs/desktop" }`, `board-rpi4 =
["desktop/board-rpi4"]`, `board-qemu-virt = ["desktop/board-qemu-virt"]`,
`selftest-desk = ["desktop/selftest-desk"]` (U6b-Punkt 1); Aufrufe
`desktop::reset()` (`lib.rs` A106, `shell_loop.rs` DESKTOP-Arm) und
`desktop::run()`.

## 2. Abhaengigkeitsgraph — Beweis „Umgehung = Compilerfehler" (G1)

`cargo tree` (`checks/u8b-deptree-*.txt`):

```
desktop v0.1.0 (programs/desktop)
├── newtos-gem v0.1.0 (programs/newtos-gem)
│   └── tos-types v0.5.0 (tos-types)
└── tos-types v0.5.0 (tos-types)

kernel v0.5.0 ──► desktop, newtos-gem, tos-api, tos-types, xhci
```

Gegenprobe auf einer Scratch-Kopie (`method/g1-proof.sh`,
`checks/u8b-g1-proof.txt`), `cargo check -p kernel`:

| Eingriff in den Desktop | Ergebnis |
|---|---|
| keiner | baut |
| `tos_api::aes::reinit();` | `error[E0433]: cannot find module or crate `tos_api`` in `programs/desktop` |
| `kernel::shell_loop::…` | `error[E0433]: cannot find module or crate `kernel`` |
| `crate::kprintln!("x");` | `error[E0433]: cannot find `kprintln` in `crate`` |
| `tos-api = { path = … }` in `programs/desktop/Cargo.toml` | Probe Gruppe D = 1 (TOTAL 5); Hosttest `desktop_depends_only_on_newtos_gem_and_tos_types` **FAILED** mit der Abhaengigkeit |

Neue Hosttests (`tools/xtask/src/programs_tests.rs`, +3):
`desktop_depends_only_on_newtos_gem_and_tos_types` (Liste exakt, transitiv:
`newtos-gem` → nur `tos-types`, `tos-types` → nichts; `newtos-gem` ohne `rt`),
`desktop_takes_the_kernel_board_and_selftest_features` (Desktop deklariert
beide Boards + `selftest-desk`, der Kernel reicht jedes durch),
`dependency_scanner_reads_every_table_form`. Der bestehende
`desktop_and_boot_screen_strings_agree` findet `MACHINE_NAME` usw. jetzt im
Crate (`emutos_consts::desktop_src`).

## 3. Werkzeuge auf dem neuen Pfad

* **Driftprobe v2:** findet ohne Anpassung das Crate (`desktop source:
  programs/desktop/src/ (crate programs/desktop/Cargo.toml, 19 .rs file(s))`);
  B zaehlt jetzt `kernel::`; C zaehlt die vier Kernel→Crate-Stellen
  (`kernel/Cargo.toml` Abhaengigkeit, `lib.rs` `desktop::reset`,
  `shell_loop.rs` `desktop::reset` + `desktop::run`) — **keine leere Null**.
  Nur der Kommentar zur `UNSAFE_BASELINE` nennt den neuen Ort.
* **`desk-cite-list.sh --since`:** folgt jetzt Umbenennungen (`git diff -M`
  ueber den Baum, gefiltert auf die Desktopquelle) — sonst haette jede
  verschobene Datei als komplett neu gezaehlt.
* **`emutos-consts`:** findet das Crate ueber `desktop_src`; `SHARED_SETS`
  unveraendert; Zahlen = U6b.
* **Pfadverweise** `kernel/src/desk` → `programs/desktop/src` mechanisch in
  Code-Kommentaren, `tools/`, `tos-api/tests`, `sdk/`, `docs/api/`,
  `docs/sdk/`, `programs/README.md` (209 Zeilen; Pruefung: nach Normalisierung
  des Pfads ist jede entfernte Zeile gleich ihrer neuen). **Nicht** angefasst:
  `TASK/`, `docs/analysis/` (datierte Analysen), `docs/STATUS.md`,
  `docs/MEMORY.md`, und Zeilen, die die geloeschten `dosbind.rs`/`xbiosbind.rs`
  oder „until U6b" nennen.

## 4. Pruefungen (Einzelheiten RUNINFO)

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1357/0/1 + neue | **1360/0/1** (+3 programs_tests) |
| Release-Warnungen rpi4 / qemu-virt, je Crate | 64/94 | **tos-api 64, kernel 94, desktop 0, newtos-gem 0** — beide Boards, auch Dev-Bau und rpi4+`selftest-desk`. Die 19 Desk-Dateien trugen bei U6b ebenfalls 0 bei; nichts ist gewandert |
| Clippy Kernel-Workspace rpi4 / qemu-virt | 76/125 | **tos-api 76, kernel 125** beide; mit `selftest-desk` 76/125 |
| Clippy `programs/` | — | **desktop** board-rpi4 / board-qemu-virt / rpi4+selftest-desk: **0/0/0**; hello-gem+newtos-gem 0; newtos-gem ohne rt 0. (Desk-Dateien hatten bei U6b 0 Clippy-Befunde; im Kernel-Workspace wird das Crate als Nicht-Mitglied nicht mehr gelintet, darum der eigene Lauf) |
| Driftprobe / unsafe | 4, Crate gemeldet / ≤ 20 | **4** (A 0 · B 0 · C 4 · D 0) / **20** |
| `cargo xtask emutos-consts` | gruen | total=464 matched=332 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=130 (= U6b) |
| `desk-cite-list` | laeuft auf neuem Pfad | 476/315/5/156 (U6b 477/315/5/157: der unzitierte Alias ist weg) |
| SDK-Hosttests | 20/0/0 | **20/0/0** |
| `programs/build.sh`, `build-programs` beide Boards | baut | baut, 0 Warnungen; NAPs **byte-gleich zu U6b** (`5b4f283c…`/`5cd0ac13…`) |
| `cargo xtask build --release` beide Boards, frisches `target/` | baut | baut (erst hello-gem, dann Kernel inkl. `Compiling desktop`), Abbilder **byte-gleich** zum gemeinsamen Baum |
| QEMU raspi4b on/off/strict (12 Laeufe) | = U6b | **15/15 byte-gleich**; APITEST **106/0/5**, 0 Zeilen Diff; Serienlog nur `[mm]`-Kernelgroesse (2473984 → 2457600) |
| Injektor-Ansichten (8) | = U6b | **8/8 byte-gleich**, Metriken gleich (About-Box: rpi4-Maschinenzeile ueber das durchgereichte Feature) |
| Orakel | = U6b | **gleich** bis auf Keytbl-Rueckgabezeiger (`.bss`) und den Build-Stempel-Rev des Bootbildschirms (`(unknown)` in der Scratch-Kopie ohne `.git`) — `checks/u8b-oracle-diff.txt` |
| qemu-virt | = U6b | End-Screendump **byte-gleich**, `[desk] desktop up (0 drives)` |
| HELLOGEM | = U6b | #Z on/off/strict + F1: Ergebniszeilen und 8/8 Screendumps **= U6b** |
| Selbsttests (Feature) | = U6b | 53 PASS / 1 FAIL (vorbestehend, U8a §4); Zeilen = U6b; t75 und Gegenprobe t40 byte-gleich; Karte unveraendert |

**Kernel 16 KiB kleiner** (rpi4-Abbild 1197160 → 1180520 Bytes): Folge der
Crate-Trennung im Codegen (anderes Inlining/Ausduennen ueber die
Crate-Grenze). Ursache im Einzelnen **nicht untersucht**; Verhalten durch die
Pixel-, Orakel- und APITEST-Gleichheit belegt.

**Messmethode, die U8b selbst beruehrt:** Die Injektoren, die bisher in
`desk/` `crate::kprintln!`/`crate::host_env` riefen (Views-Metrikzeile,
Selbsttest-Schaerfung), gehen jetzt ueber einen `extern "C"`-Haken, den die
Scratch-Kernelkopie mit `#[no_mangle]` definiert — der Desktop kann den Kernel
nicht mehr nennen, das ist der Zweck der Einheit. Gleiche Stelle, gleiche
Zeile.

## 5. Guards (`docs/api/desktop/_rebuild-plan.md` §12)

```
Guards (docs/api/desktop/_rebuild-plan.md §12):
  DRIFT-PROBE TOTAL: 4 (before: 4)   DRIFT-PROBE UNSAFE: 20 (baseline 20)
                     source reported: programs/desktop/src/ (crate programs/desktop/Cargo.toml), C = 4 kernel -> crate
  EMUTOS-CONSTS: total=464 matched=332 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=130
                 (EmuTOS snapshot 1a7484ab unchanged)
  DESK-CITE: fns=476 cited=315 newtos=5 uncited=156            (whole desktop; before 477/315/5/157)
  DESK-CITE --since 7699f4d7 (rename-aware): 26 fns touched, 23 cited, 1 NEWTOS, 2 uncited:
    uncited, touched only mechanically: lib.rs wind_get_work (pub(super) -> pub(crate));
                                        global.rs patcol_all (comment line below it: crate::shell_loop -> kernel::shell_loop)
    NEWTOS (pre-existing marker, comment edit above it): global.rs DeskStatic::deref
    no fn added; one removed (wind_get_rect, uncited alias)
  Oracle: run (unit changes how the desktop is linked): identical to U6b except the Keytbl pointer (.bss) and the
          boot-screen build-stamp rev; measurements/measurement-2026-10-14-desk43-u8b/checks/u8b-oracle-diff.txt
```

## 6. Offen fuer U9 (und spaeter)

1. **Registry vs. Shell-Pfad (U9-Blocker, aus U6b §7/2, unveraendert):**
   `sh_ldapp` uebergibt den ganzen 128-Byte-Puffer `s_cmd` an `Pexec`
   (`kernel/src/shell_loop.rs`, `&cmd`), `registry::lookup` vergleicht die
   ganze Scheibe — ein Registry-Name `NEWDESK` trifft aus dem Shell-Pfad nie.
   Vor dem Umschalten bis zum NUL vergleichen (oder den Namen kuerzen).
2. **`NEWDESK`-Binaercrate:** duennes `staticlib`-Crate in `programs/`
   (`#[no_mangle] main` → `desktop::run()`, `Pterm`), haengt an `desktop` und an
   `newtos-gem` **mit** `rt` (der Desktop selbst nimmt `rt` nicht); Eintrag in
   xtask `PROGRAMS` (der bestehende Test erzwingt das fuer jedes `staticlib`)
   und in `kernel/embed.rs` (`Embedded { name: "NEWDESK", file:
   "NEWDESK.NAP" }`, Test `nothing_embedded_before_u9` loeschen). Die
   Board-Features reicht `build-programs` schon durch; das Binaercrate muss sie
   an `desktop/board-*` weitergeben, `selftest-desk` ebenso.
3. **Erster PIC-Bau des Desktops:** `cargo build -p desktop --release` im
   Programm-Profil (PIC, `opt-level = "s"`) uebersetzt; gelinkt als `.NAP`
   (fat LTO, `-z text`, `DT_*`-Positivliste) ist er **noch nicht** — Groesse und
   Relokationen pruefen (`llvm-readobj -d`, wie U6a).
4. **Desktop-up-Schluessel** (U8a §7/3): `observe_desktop_gem_call` erkennt den
   Desktop an `Space::Kernel` + DESKTOP-Arm; als EL0-Prozess „der vom
   DESKTOP-Arm gestartete Prozess".
5. **`lib.rs` A106 (`desktop::reset()` nach `tos_api::aes::reinit()`)** und der
   `desktop::reset()`/`run()` im DESKTOP-Arm fallen mit U9 (Prozessende gibt den
   Speicher frei); Probe-Gruppe C sinkt dann, Gruppe D/„Kernel bettet ein"
   kommt hinzu (Review §3.2 Fussnote 2).
6. **NAP nicht pfad-reproduzierbar** (U6b §7/3): mit Einbettung wird das
   Kernelabbild pfadabhaengig. (Kernelabbilder sind es heute nicht: der
   Clean-Bau in anderem Pfad ist byte-gleich.)
7. `DeskStatic`-Invariante (ein Kontrollfluss) bleibt im EL0-Prozess wahr.
8. Ein blankes `cargo build`/`cargo clippy` ueber den ganzen
   `programs/`-Workspace bricht am `compile_error!` des Desktops ohne Board ab
   (gewollt wie beim Kernel; in `programs/README.md` dokumentiert). Wer das
   aendern will: `default-members` ohne `desktop`.
9. Kernelseitige Opcode-Kopie `AES_APPL_INIT`/`AES_EVNT_MULTI` in
   `shell_loop.rs` (U6b §7/6) — unveraendert.
10. `docs/analysis/`- und `TASK/`-Texte nennen weiter `kernel/src/desk/`
    (datiert/historisch, bewusst nicht umgeschrieben); `docs/STATUS.md` und
    `docs/MEMORY.md` ebenso (nicht Teil dieses Auftrags).

## 7. Selbstpruefung

Aufgabe in einem Satz: `kernel/src/desk/` per `git mv` zur Bibliothek
`desktop` machen, die nur `newtos-gem` + `tos-types` kennt, den Kernel davon
abhaengen lassen und die Probe im selben Schritt auf das Crate umstellen —
ohne Verhaltensaenderung. Geaendert: 19 Dateien verschoben
(`programs/desktop/src/`, davon inhaltlich `lib.rs`, `gsx.rs`, `main.rs`,
`redraw.rs`, `global.rs`, `inf.rs`, `model.rs` + Titelzeilen), neu
`programs/desktop/Cargo.toml`; `programs/Cargo.toml`, `programs/README.md`,
beide `Cargo.lock`; `kernel/Cargo.toml`, `kernel/src/lib.rs`,
`kernel/src/shell_loop.rs`; `tools/desk-cite-list.sh`,
`tools/desk-drift-probe.sh` (Kommentar), `tools/xtask/src/programs_tests.rs`;
Pfadverweise (§3); diese Quittung; das Messverzeichnis. Beide Boards bauen
(tos-api 64 / kernel 94 / desktop 0; Clippy 76/125, desktop 0).
