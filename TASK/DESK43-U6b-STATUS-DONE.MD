# DESK43 U6b — `newtos-gem` + Build-Kopplung — Quittung

> Stand 2026-10-14 · Basis `main` @ `cfb6a238` (U8a = `87adb580`) · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2 (Zeile U6b), §5 (T1, Ladeform E), §6 (X03-Zeile), §7.4/2;
> Entscheidung D3 in `TASK/DESK43.md`
> Messung: [`measurements/measurement-2026-10-14-desk43-u6b/`](../measurements/measurement-2026-10-14-desk43-u6b/RUNINFO.TXT)
> **Kein Bildschirmunterschied (0 Pixel), Orakel = U8a.** Nichts committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`.
> `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: die Trap-Bindungen des Kernel-Desktops (AES-Teil von
`desk/mod.rs`, `dosbind.rs`, `vdibind.rs`-Trapteil, `xbiosbind.rs`) liegen
jetzt — zusammengefuehrt mit U6a — in `programs/newtos-gem`, und der
Kernel-Desktop (weiter EL1) ruft sie von dort; dazu `xtask build-programs
--board`, die Einbettungsmechanik mit hartem Fehler (Liste noch leer) und die
X03-Durchreichung mit Test. Driftprobe **4 → 4**, unsafe **24 → 20**.

---

## 1. Was umgezogen ist

| Vorher (`kernel/src/desk/`) | Jetzt | Bemerkung |
|---|---|---|
| `mod.rs` aesbind (36 `OP_*`, `Aes`, `gem_trap`, 40 Wrapper) | `programs/newtos-gem/src/aes.rs` | Opcodes mit EmuTOS-Namen (`APPL_INIT` … `SHEL_WDEF`, `aesdefs.h`), **einmal**; jede Bindung mit den `control[]`-Zaehlern des Desktops (= `desk/gembind.c`, je Funktion zitiert). In `desk/mod.rs` bleiben die Importe (Aufrufstellen `super::form_alert` … unveraendert) und die desktopgeformten Helfer (`wind_get_work/_rect/_top`, `objc_draw_tree/_dialog`, `form_dial_start/_finish`, `wind_calc_work_width`, `evnt_multi`-Desktopform, `evnt_multi_m1`, `g_desk`, `screen_rect`) |
| `dosbind.rs` (geloescht) | `newtos-gem/src/gemdos.rs` | EmuTOS-`gemdos.h`-Namen; `dos_conout` → `cconout` (normale Bindung, U8a-Punkt 4); U6a `cconws`/`mfree` → `dos_conws`/`dos_free` (EmuTOS-Namen); `malloc`/`pterm` ohne EmuTOS-Wrapper (`// NEWTOS:`) |
| `vdibind.rs` Trapteil (8 `OP_*`, `Vdi`, `vdi_trap`) | `newtos-gem/src/vdi.rs` | Opcodes mit EmuTOS-`aes/funcdef.h`-Namen (`POLYLINE`, `SET_WRITING_MODE` …; U8a-Punkt 3); drei ohne EmuTOS-Namen (`OPEN_/CLOSE_VWORKSTATION`, `SET_TEXT_ALIGNMENT`, markiert); klassische GEM-Signaturen (`vs_clip(h, flag, pxy)`, `vr_recfl(h, pxy)`, `vq_extnd`/`vqt_attributes` als `work_out`/`attrib`) |
| `vdibind.rs` Desktoplogik | bleibt (`HANDLE`, `small_font_cell`, `gsx_sclip/_attr_line/_pline`) | ruft jetzt `newtos_gem::vdi` |
| `xbiosbind.rs` (geloescht) | `newtos-gem/src/xbios.rs` | `keytbl(unshift, shift, caps)`, `KEYTBL_KEEP` |
| GEMDOS-Funktionsnummern (in `dosbind.rs`, `aes/shell.rs`, U6a) | **`tos_types::gemdos`** (neu) | `tos-api/src/aes/shell.rs` nutzt sie (private Kopien weg; U5b-Punkt 3) |
| `ENSMEM`/`ERROR` in `newtos-gem` | `tos_types::error` | U5b-Punkt 3 |
| `wind_create/wind_calc(kind: i16)` | `kind: u16` | U5b-Punkt 5; `hello-gem` castet nicht mehr |

`newtos-gem` hat ein Feature **`rt`** (Standard an): `#[global_allocator]`
und `#[panic_handler]`. Der Kernel bindet die Crate mit
`default-features = false` (eigener Allokator/Panic-Handler). `hello-gem`
ist an die vereinigte API angepasst (gleiches Verhalten, s. §4).

**Review §7.4/2 beantwortet:** eine Pfad-Abhaengigkeit des Kernels in das per
`exclude` ausgeschlossene `programs/` funktioniert ohne Umweg — Cargo baut
`newtos-gem` als Nicht-Mitglied-Pfadpaket mit Profil und Ziel des Kernels;
die `edition.workspace`-Vererbung loest es aus `programs/Cargo.toml`, und
`tos-types` ist dieselbe Crate-Instanz. 0 rustc-Warnungen aus `newtos-gem`.

## 2. Bau-Kommandos und Abhaengigkeitsgraph

```
cargo xtask build-programs --board rpi4|qemu-virt
    -> bash programs/build.sh <crate> target/programs/<board>/<NAME>.NAP -- --features board-<board>
       (je Eintrag von xtask PROGRAMS; heute hello-gem -> HELLOGEM.NAP)
cargo xtask build --board <b> [--release] [--features ...]
    -> build-programs <b>, dann cargo build -p kernel (build.rs erzeugt die Einbettungstabelle)
```

```
kernel ──► tos-api ──► tos-types
   │                      ▲
   └──► newtos-gem ───────┘        (programs/newtos-gem, default-features = false)
hello-gem ──► newtos-gem (rt) ──► tos-types      (programs/-Workspace)
```

`kernel/desk/` nennt `newtos_gem::`, `tos_types::`, `core::`, `alloc::` und
sich selbst — keinen `tos_api::`/`crate::`-Pfad (Driftprobe A 0, B 0).

## 3. Einbettungsmechanik (D3) — vorbereitet, Liste leer

* `kernel/embed.rs` (rein `std`, von `build.rs` per `mod` und von xtask per
  Pfad eingebunden): `EMBEDDED` (heute `&[]`), `programs_dir(ws, board)` =
  `target/programs/<board>/`, `generate()` = Quelltext der Tabelle oder der
  **harte Fehler** mit Dateiname und Kommando.
* `kernel/build.rs`: Board aus dem Feature, `rerun-if-changed` je Blob,
  `$OUT_DIR/embedded_programs.rs` = `pub static EMBEDDED_PROGRAMS: &[(&[u8],
  &[u8])]` mit einem `include_bytes!` je Eintrag; fehlt ein Blob → `panic!`
  im Build-Skript = Bauabbruch.
* `kernel/src/embedded.rs`: `register()` traegt jeden Eintrag ins Registry
  ein; Aufruf in `kernel_main_rust` vor `enter_aes()`. Bei leerer Liste tut
  er nichts.

**Warum leer und nicht HELLOGEM als Platzhalter im Produkt:** der Beweis
brauchte es nicht (s. u.), und ein eingebettetes Programm ueberschattet eine
gleichnamige Kartendatei (`registry::lookup` vor FAT) — eine
Verhaltensaenderung. U9 traegt `Embedded { name: "NEWDESK", file:
"NEWDESK.NAP" }` ein und loescht den Test `nothing_embedded_before_u9`.

**Beweis** (`method/embed-proof.sh`, Scratch-Kopien): (A) frisches `target/`:
`cargo xtask build` baut erst `hello-gem`, dann den Kernel — Abbilder
**byte-gleich** zu den Abbildern des gemeinsamen Baums (rpi4 und qemu-virt).
(B) Kopie mit `EMBEDDED = [HGEMBED ← HELLOGEM.NAP]` (Platzhaltername, nicht
`NEWDESK`, nichts startet es): `cargo build -p kernel` ohne build-programs
**bricht ab** (rc 101, Meldung nennt Datei und Kommando); `cargo xtask build`
baut, die Tabelle nennt den Blob, die 101 088 Bytes stehen byte-genau in
`kernel8.img`. Dazu 6 Hosttests in `kernel/embed.rs`.

## 4. X03-Durchreichung und Test

* `build-programs` reicht `--features board-<b>` an jedes Programm; jedes
  Programm deklariert `board-rpi4` und `board-qemu-virt` (Cargo weist ein
  unbekanntes Feature ab). `hello-gem` hat nichts Board-Spezifisches; der
  Desktop (U8b/U9) haengt seine Maschinenzeile daran.
* Hosttests `tools/xtask/src/programs_tests.rs`: jedes `staticlib`-Mitglied
  von `programs/` steht in `PROGRAMS` und hat beide Features; das
  Bau-Kommando traegt das Board-Feature hinter `--` und schreibt ins
  Board-Verzeichnis; **Review §6/X03:** `VERSION_STRING`,
  `INDEPENDENCE_NOTICE` und die `MACHINE_NAME` je Board des Desktops sind
  gleich den Texten von `kernel/src/initinfo.rs` (Desktopquelle ueber
  `emutos_consts::desktop_src`, findet nach U8b das Crate).

## 5. Pruefungen (Einzelheiten RUNINFO)

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1347/0/1 + neue | **1357/0/1** (+10: 6 embed, 4 programs_tests) |
| Release-Warnungen rpi4 / qemu-virt | 64/94 | **64/94** beide, 0 Fehler; Verteilung je Datei = U8a |
| Clippy rpi4 / qemu-virt | 76/125 | **76/125** beide; Verteilung je Datei = U8a; mit `selftest-desk` 76/125 |
| Clippy `programs/` (+ `newtos-gem` ohne `rt`) | — | **0 / 0** |
| Driftprobe / unsafe | 4 (D 0) / ≤ 24 | **4** (A 0 · B 0 · C 4 · **D 0**) / **20** (Baseline im Skript auf 20 gesenkt) |
| `cargo xtask emutos-consts` | gruen | total=464 matched=332 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=130 |
| SDK-Hosttests | 20/0/0 | **20/0/0** |
| `cargo xtask build --release` beide Boards, frisches `target/` | baut | baut, Abbild **byte-gleich** |
| QEMU raspi4b on/off/strict (12 Laeufe) | = U8a | **15/15 byte-gleich**; APITEST **106/0/5**, 0 Zeilen Diff; Serienlog nur Kernelgroesse (+8 KiB) |
| Injektor-Ansichten (8) | = U8a | **8/8 byte-gleich**; Metriken gleich |
| Oracle | = U8a | **gleich** bis auf Keytbl-Rueckgabezeiger (`.bss`, +8 KiB Kernel) |
| qemu-virt | = U8a | End-Screendump **byte-gleich** |
| HELLOGEM | laeuft | #Z on/off/strict + F1 vom Desktop: Ergebniszeile und Fensterbild **= U6a** |
| Selbsttests | = U8a | 53 PASS / 1 FAIL (vorbestehend), Zeilen = U8a; Gegenprobe Karte unveraendert |

**Nicht in der Spur sichtbar, statt dessen begruendet:** `graf_mouse`
deklariert jetzt `naddrin = 1` wie EmuTOS (`gembind.c:650-654`) statt 0, mit
`addr_in[0] = NULL` — tos-api liest `addr_in` nur fuer `USER_DEF`, ein nicht
deklarierter Slot kommt als 0 an (`Aespb::from_raw`) → gleiche Eingabe. Das
`global[]` ist jetzt das eine statische von `newtos-gem` statt ein genulltes
je Aufruf; die AES schreibt es nur in `appl_init`/`rsrc_*` (Desktop ruft kein
`rsrc_*`).

## 6. Guards (`docs/api/desktop/_rebuild-plan.md` §12)

```
Guards (docs/api/desktop/_rebuild-plan.md §12):
  DRIFT-PROBE TOTAL: 4 (before: 4)   DRIFT-PROBE UNSAFE: 20 (baseline 24 -> lowered to 20)
  EMUTOS-CONSTS: total=464 matched=332 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=130
                 (EmuTOS snapshot 1a7484ab unchanged; SHARED_SETS += tos-types/src/gemdos.rs, newtos-gem aes/gemdos/vdi/xbios)
  DESK-CITE: fns=477 cited=315 newtos=5 uncited=157            (whole desktop; before 548/359/5/184 -- the bindings left)
  DESK-CITE --since cfb6a238: 26 fns touched, 23 cited, 2 NEWTOS, 1 uncited:
    uncited: dir.rs scan_folder -- comment edit only (dosbind -> newtos_gem::gemdos), pre-existing
    NEWTOS (pre-existing markers, touched by the rename dos_conout -> cconout): selftest.rs ConOut::write_str, line
    new desktop helpers in mod.rs all cite EmuTOS (deskmain.c:1049, desksupp.c:227, deskinf.c:356, deskwin.c:95, ...)
    outside the scanned desktop, marked // NEWTOS: in newtos-gem: ap_id, cconout, malloc, malloc_avail, pterm,
    dos_getcookie, KEYTBL, SET_TEXT_ALIGNMENT, OPEN_/CLOSE_VWORKSTATION; kernel/src/embedded.rs register
  Oracle: run (unit changes how the desktop calls): identical to U8a except the Keytbl pointer value;
          measurements/measurement-2026-10-14-desk43-u6b/checks/u6b-oracle-diff.txt
```

## 7. Offen fuer U8b (und spaeter)

1. **Desktop-Crate (U8b) braucht die Board-Features** und nimmt
   `MACHINE_NAME` mit; der Kernel reicht `board-* = ["desktop/board-*"]`
   durch, das Binaer-Crate `NEWDESK` (U9) bekommt sie von `build-programs`.
   `programs_tests` prueft nur `staticlib`-Mitglieder — eine Bibliothek
   `desktop` faellt nicht darunter; dort pruefen, oder den Test erweitern.
2. **Registry vs. Shell-Pfad (U9-Blocker):** `sh_ldapp` uebergibt den ganzen
   128-Byte-Puffer `s_cmd` an `Pexec` (`shell_loop.rs:434` `&cmd`),
   `registry::lookup` vergleicht die ganze Scheibe — ein Registry-Name trifft
   aus dem Shell-Pfad **nie**. Vor `NEWDESK` aus dem Registry: bis zum NUL
   vergleichen (oder den Namen dort kuerzen).
3. **NAP nicht pfad-reproduzierbar:** Symbolnamen tragen Crate-Hashes, die vom
   Checkout-Pfad abhaengen (Kernelabbilder sind es). Mit Einbettung (U9) wird
   das Kernelabbild pfadabhaengig — Symbole strippen oder Metadaten festlegen.
4. `desk/vdibind.rs` ist keine Bindung mehr (gsx-Logik) — beim `git mv` in
   U8b umbenennen (z. B. `gsx.rs`).
5. Desktop-Helfer `evnt_multi(flags, msg)` heisst wie
   `newtos_gem::aes::evnt_multi` (andere Signatur); `wind_get_rect` ist ein
   reiner Alias von `wind_get_grect`. Beim Umzug bereinigen.
6. `shell_loop.rs` `AES_APPL_INIT`/`AES_EVNT_MULTI` (Kernel-Beobachter, U8a)
   sind noch eine eigene Opcode-Kopie — nicht angefasst (Kernel soll in U10
   nicht an `newtos-gem` haengen).
7. VDI-Farb-/Modus-Konstanten weiter mehrfach (`Pel` in tos-api,
   `initinfo.rs`, jetzt `newtos_gem::vdi`) — U5b-Punkt 1 unveraendert.
8. `hello-gem` setzt den Fenstertitel ueber `wind_set_ptr` (die typsichere
   `wind_set_name(&'static CStr)` von U6a ist zugunsten einer Bindung je
   Form entfallen); eine sichere Huelle waere ein spaeterer Zusatz.
9. `cargo xtask build`/`run` braucht jetzt `clang` (crt0) — von D3 so
   gewollt, aber neu fuer reine Kernelbauten.

## 8. Selbstpruefung

Aufgabe in einem Satz: Trap-Bindungen aus `desk/` nach `newtos-gem`, der
EL1-Desktop nutzt sie (0 Pixel, Orakel gleich), `build-programs --board` +
Einbettung mit hartem Fehler + X03-Durchreichung mit Test, ohne
Boot-Aenderung. Geaendert: `programs/newtos-gem/*` (+`xbios.rs`),
`programs/hello-gem/*`, `programs/README.md`, `kernel/Cargo.toml`,
`kernel/build.rs`, `kernel/embed.rs` (neu), `kernel/src/embedded.rs` (neu),
`kernel/src/lib.rs` (2 Zeilen + Aufruf), `kernel/src/desk/*` (2 Dateien
geloescht), Kommentarzeilen in `dbg_qedtrace.rs`/`exceptions.rs`/
`tos-api/src/uaccess.rs`, `tos-api/src/aes/shell.rs` (Importe),
`tos-types/src/gemdos.rs` (neu), `tools/xtask/*`, `tools/desk-drift-probe.sh`
(Baseline 20), `docs/api/desktop/gembind.md`/`deskinf.md` (Pfade), beide
`Cargo.lock`, diese Quittung, das Messverzeichnis.
