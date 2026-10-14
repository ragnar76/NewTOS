# DESK43 U9-1 + U9-2 — Instrumente, EL1-Basislinie, NEWDESK-Programm + Einbettung — Quittung

> Stand 2026-10-14 · Basis `main` @ `c53e9267` (sauber) · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u9-prep-2026-10-14.md` §5.1/§5.2, §6 (Zeilen U9-1, U9-2); Entscheidungen
> `TASK/DESK43.md` („U9-Vorbereitung entschieden": D3, D6-a, STK nach Messung); offene Punkte aus
> `TASK/DESK43-U8b-STATUS-DONE.MD` §6 und `TASK/DESK43-U9-0-STATUS-DONE.MD`.
> Messung: [`measurements/measurement-2026-10-14-desk43-u9-1-2/`](../measurements/measurement-2026-10-14-desk43-u9-1-2/RUNINFO.TXT)
> **Bootpfad unveraendert** (DESKTOP-Arm startet weiter den EL1-Desktop): U9-0-Serie 15/15 byte-gleich, APITEST
> 106/0/5. Nichts committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`.
> `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: Stack-/Latenz-Instrumente als Scratch mit EL1-Basislinie (U9-1), und das duenne
Programm `NEWDESK` um die Desktop-Bibliothek mit Laufzeit + `panic-wait-key`, gebaut fuer beide Boards
und unter dem Registry-Namen `NEWDESK` pfad-reproduzierbar ins Kernelabbild eingebettet (U9-2).
Driftprobe **4 → 4**, unsafe **20 → 20**.

---

## U9-1 — Instrumente (nur Scratch) und EL1-Basislinie

`method/u91_instr.rs` + `method/u91-instr.py` (auf rsync-Kopien angewandt, vertraegt sich mit `tools/inject-arm.py`):

* **Stack, bemalen und messen** (EmuTOS `CONF_DEBUG_DESK_STACK`): EL1 = Hauptkernelstack (SPSel 0; jede
  Ausnahme, auch das eigene `svc`, laeuft auf SP_EL1), bemalt `[__stack_bottom_inclusive, SP−1 KiB)` direkt vor
  `desktop::run()`. EL0 = crt0-Stack `[B+round16(_end−B), +MINKEEP)`, `_end = p_bbase+p_blen`, vom Launcher vor dem
  Sprung nach EL0 bemalt (Kernelalias). Meldung bei „desktop up", sekundlich bei Wachstum, am Ende.
* **Latenz** (CNTVCT, µs): A `sh_ldapp`-Eintritt, B Sprung nach EL0, U „desktop up" (EL1: Kernelzeile; EL0: erstes
  `evnt_multi` des Shell-Kindes nach `appl_init`), X Pterm des vorigen Kindes.
* Instrumentierter Basis-Kernel: Views 8/8 **byte-gleich** U9-0 (Instrument aendert keine Pixel).

| EL1-Basislinie (Basisbaum, QEMU raspi4b) | Wert |
|---|---|
| Stack bei „desktop up" | 2112 B |
| Stack max, 8 Views (Alt-C, Text/Icons, About, Prefs, Hintergrund, Konfiguration) | 8000 B; 150-Datei-Verzeichnis 8192 B |
| Stack max, 4 Selbsttests | 3296 B |
| Start nach Boot, A→U | Median 15.9 ms (15.0 … 17.5; ein Ausreisser 101 ms) |
| Start nach Programmende (10× HELLOGEM per F1), A→U / X→U | 4.0 ms (3.6 … 4.6) / 6.1 ms (5.6 … 7.5) |

## U9-2 — `NEWDESK`

**Layout.** `programs/newdesk/` (`staticlib`, Mitglied des Programm-Workspace): `main(basepage) → desktop::run(); 0`.
Abhaengigkeiten **`desktop` + `newtos-gem` (mit `rt`, Feature `panic-wait-key`)**; Features `board-rpi4`,
`board-qemu-virt`, `selftest-desk` → `desktop/*` (wie der Kernel). Doku im Crate: was vom DESKTOP-Arm-Prolog im
Programm entfaellt — `sh_show("")`/`p_setappdir` bleiben Schritte der Shell (`gemshlib.c:603-605`),
`desktop::reset()` ersetzt der frische Prozess (Lader nullt `.bss`), Mshrink/Stack macht crt0
(`deskstart.S:28-41`), `restart:`-Schleife = `desktop::run()`, `Pterm(0)` = `main` gibt 0 zurueck.

**`panic-wait-key` (D6-a)** in `newtos-gem` (Standard aus; `= ["rt"]`): Meldung per `Cconws`, dann wie `dopanic`
(`bios/kprint.c:581-585`) — anstehende Tasten verwerfen (`Cconis`/`Crawcin`), `*** Press any key to continue ***`,
`Crawcin`, `Pterm(-1)`. `Crawcin`/`Cconis` statt `Cconin`: kein Echo, kein Ctrl-C (Gegenstueck zu
`bconstat2`/`bconin2`). Neue Bindungen `crawcin()`/`cconis()`, Konstanten `CRAWCIN`/`CCONIS` in `tos-types`
(`bdosbind.h:23,25`).

**Einbettung (D3).** xtask `PROGRAMS` += `("newdesk", "NEWDESK.NAP")`; `kernel/embed.rs`
`EMBEDDED = [NEWDESK / NEWDESK.NAP]`; Test `nothing_embedded_before_u9` geloescht, ersetzt durch
`newdesk_is_the_one_embedded_program`; neue Hosttests `newdesk_is_the_desktop_with_the_program_runtime`,
`the_embedded_desktop_is_built_and_named_as_the_shell_names_it` (Name = `DEF_DESKTOP`). Kernel-Quelltext
unveraendert (nur Doku in `embedded.rs`). Erreichbar per `#Z 01 NEWDESK@`.

**Reproduzierbarkeit — Entscheidung: feste Crate-Metadaten, kein Strip.** Befund: cargo hasht fuer
`tos-types` (Pfadpaket ausserhalb `programs/`) den **absoluten Pfad** in `-C metadata`; alle Abhaengigen erben es.
Daraus entstehen die Symbol-Hashes — und die **Reihenfolge des Codes**: NEWDESK.NAP aus zwei Pfaden unterschied
sich in 815 Bytes des *geladenen* Abbilds. Strippen hilft daher nicht, ein zusaetzliches `-C metadata` auch nicht
(rustc hasht alle). Loesung: `programs/rustc-stable-metadata.sh` als `build.rustc-wrapper`
(`programs/.cargo/config.toml`) ersetzt cargos Wert durch einen Hash aus Name, Version, Crate-Typ, Ziel und
Features. Beleg: Clean-Bau an anderem Pfad — `kernel8.img` und qemu-virt-Abbild **0 Bytes Unterschied**, beide
NAPs beider Boards 0. Nicht gestrippt: `.symtab`/`.debug_*` (~110 KB, nicht geladen) bleiben fuer die Aufloesung
eines Absturz-PCs. Nebenwirkung: `HELLOGEM.NAP` hat einen neuen Hash (`97f14224…`, vorher `5b4f283c…`) —
Verhalten identisch (HELLOGEM-#Z-Lauf: Screendumps und Ergebniszeilen = U9-0).

**Binaereigenschaften** (`llvm-readobj -h -d -r -l`, beide Boards gleich gebaut): ET_DYN, Entry 0x1000;
PT_LOAD R-X 201152 B, PT_LOAD RW 15136/15200 B, PT_DYNAMIC, PT_NOTE. DT: FLAGS_1 DEBUG RELA RELASZ RELAENT
RELACOUNT SYMTAB SYMENT STRTAB STRSZ GNU_HASH HASH INIT_ARRAY INIT_ARRAYSZ FINI_ARRAY FINI_ARRAYSZ NULL —
**nur Positivliste** (`reloc.rs:265-268`); 276 Relokationen, **alle `R_AARCH64_RELATIVE`**. Datei rpi4 344344 B,
qemu-virt 344168 B; Kernelabbild rpi4 1180520 → 1524592 B. Der Lader hat das Binaerprogramm ohne Aenderung angenommen.

## EL0-Laeufe (QEMU raspi4b, `#Z`, on/off/strict)

Zwillingskarten: EL0-Karte `#Z 01 NEWDESK@` (Registry) bzw. `#Z 01 C:\NEWDESK.NAP@` (+ Datei), EL1-Zwilling
dieselbe Zeile als `#z` (von AES und Desktop ignoriert) → gleiche Dateien, Groessen, Daten. Gleicher Kernel,
Injektor `up:NEWDESK` bzw. `desktop` mit dem U8b-Views-Skript (Alt-C-Verzeichnisfenster, Ansichten, About-Box …).

* Registry und Kartendatei, je on/off/strict, dazu 150-Datei-Verzeichnis: der EL0-Desktop kommt hoch und ist
  bedienbar; **7 von 8 Ansichten byte-gleich** zum EL1-Desktop. About-Box zeigt die rpi4-Maschinenzeile.
* **Einziger Pixelunterschied** (jedes Paar, x=357 y=355 w=30 h=12, 122–150 px): die zweite Zahl von
  „Free RAM: 10240 KB + N KB" im Desktop-Konfigurationsdialog, on/strict 915432 → 914748 KB, off 925092 →
  924408 KB (−684 KB), grosses Verzeichnis −716 KB. **Ursache:** NEWDESK ist ein Prozess — sein TPA (Abbild
  220 KB + 64 KB Stack, PRGFLAGS 7 → Alt-RAM) und seine Malloc-Heap-Hunks sind nicht mehr frei. Erwartet
  (Prep §5.6). Sanduhr/Mauszeiger: kein sichtbarer Unterschied in den Dumps.
* Erster Desktop-Bildschirm (Produktkernel, Kartendatei, t25) = U9-0-Desktop **byte-gleich**.
* Selbsttests im EL0-NEWDESK (Scratch-NAP mit `selftest-desk`): Ergebniszeilen = EL1 = U8b, 53 PASS / 1 FAIL (vorbestehend).
* **Panic-Build** (Scratch-`panic!` vor `desktop::run()`): Meldung + Prompt auf Schirm, wartet; Taste (Injektor,
  +8 s) → `exited with rc=65535` → EL1-Desktop. Gegenprobe ohne Taste: 35 s am Prompt, kein Ende.
* 0 `[EXC]`/`[crash]`/PANIC/„load failed" in allen Logs des Laufs.

## Stack und Latenz EL0 gegen EL1 (QEMU, nicht Pi)

| | EL1 | EL0 NEWDESK |
|---|---|---|
| Stack bei „up" | 2112 B | 3328 B |
| Stack max Views / grosses Verz. / Selbsttests | 8000 / 8192 / 3296 B | 8120 / 9152 / 4496 B |
| Start nach Boot, A→U | 15.9 ms | Registry 47.1 ms (Laden 23.6 + Init 23.5; off 21.3), Kartendatei 99.5 ms (Laden 76) |
| Start nach Programmende, X→U | 6.1 ms | 45.5 ms (Kartendatei warm: Laden 26.9 + Init 16.8) |

**STK-Empfehlung:** Hochwasser EL0 **9152 B von 65536 B (14 %)**, weit unter der 32-KiB-Regel → `MINKEEP`
**belassen** (Option 3), kein Build-Parameter, kein Canary in U9. Vorbehalt: tiefe Verzeichnisrekursion
(Kopieren eines tiefen Baums) nicht gefahren; in U9-6 auf der Pi mit den Instrumenten nachmessen.

## Pruefungen

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1366/0/1 ± | **1368/0/1** (+2 programs_tests; Embed-Test ersetzt) |
| Warnungen rpi4 / qemu-virt | 64/94 | 64/94 beide; build-programs 0 |
| Clippy Kernel rpi4 / qemu-virt / +selftest-desk | 76/125 | 76/125 alle; Programme (inkl. newdesk ×3, newtos-gem +pwk) 0 |
| Drift / unsafe | 4 / ≤ 20 | **4** (A 0 · B 0 · C 4 · D 0) / **20** |
| emutos-consts | gruen | 466/332/2/2/0/0/132 (+2 = CRAWCIN/CCONIS, wie CCONOUT ohne EmuTOS-`#define`) |
| SDK | 20/0/0 | 20/0/0 |
| build-programs + xtask build, beide Boards, frisches target, anderer Pfad | baut, reproduzierbar | baut; Abbilder und NAPs 0 Bytes Unterschied |
| U9-0-Serie on/off/strict | byte-gleich | 15/15 byte-gleich; APITEST 106/0/5, 0 Zeilen Diff; Logs nur `[mm] kernel`-Groesse |
| qemu-virt | = U9-0 | Endschirm byte-gleich; Log nur `[mm] kernel` |

## Offen fuer U9-3 / U9-4 (und spaeter)

1. **`selftest-desk` erreicht NEWDESK nicht**: `cargo xtask build --features selftest-desk` baut die Programme nur mit
   dem Board-Feature (`build-programs` nimmt nur `--board`). Mit U9-4 (EL0-Desktop im Bootpfad) muss das Feature an
   `build-programs` durchgereicht werden, sonst laufen die Selbsttests nur im `desk=kernel`-Rueckfall.
2. **U9-4 setzt `DESKTOP_ACTIVE` um den Launcher-Start** (U9-0 Punkt 1); `launch.rs` setzt dabei heute die
   Sanduhr (Review §3.3(1): EmuTOS-DESKTOP-Arm hat keine) — in U9-4 entscheiden.
3. **EL0-Neustart aus der Registry nach Programmende** ist vor U9-4 nicht erreichbar; Schaetzung aus den Teilen
   ~24 ms Laden + ~17 ms Init (QEMU), gegen 6 ms EL1. Pi-Messung in U9-6 (Instrumente liegen in `method/`).
4. **Speicher:** der EL0-Desktop belegt ~684 KB Alt-RAM (TPA 284 KB + Heap ~400 KB) — sichtbar im „Free RAM"-Feld.
5. **Driftprobe** zaehlt die Einbettung nicht (Review §3.2 Fussnote 2, „Kernel bettet ein") — Probe-Erweiterung offen.
6. **Wrapper und alte `target/programs/`**: cargo verfolgt den Wrapper nicht im Fingerprint; ein vorhandenes
   Programm-Zielverzeichnis braucht einmal `cargo clean` (in `programs/`), sonst bleiben alte Artefakte mit
   Pfad-Hashes (README dokumentiert es).
7. qemu-virt: EL0-NEWDESK nicht gefahren (keine Karte → kein `#Z`); TeraDesk/QED/Accessories unter EL0-NEWDESK nicht gefahren.
8. Strip-Option: das eingebettete Blob traegt ~110 KB nicht geladene Symbole/Debug-Abschnitte; wer das Abbild
   kleiner will, kann in `build.sh` `--strip-all` setzen (Reproduzierbarkeit haengt nicht davon ab).

## Ausserhalb des Auftrags (je eine Zeile, nicht angefasst)

* `programs/hello-gem` bemalt seinen Stack selbst — ein externes Stack-Instrument sieht dort 65536 B (Artefakt).
* `kernel/src/launch.rs:21-27`: verwaister Doc-Kommentar vor `busy_cursor` (schon U9-0 notiert).
* Erster Boot-Start eines EL1-Desktops dauerte einmal 101 ms statt ~15 ms (QEMU-Hostjitter, Views-Phase verschoben) — Messrauschen, kein Befund.

## Selbstpruefung

Aufgabe in einem Satz: U9-1-Instrumente (Scratch) mit EL1-Basislinie, und `NEWDESK` als duennes Programm um
`desktop::run()` mit `rt` + `panic-wait-key`, in `PROGRAMS` und eingebettet als `NEWDESK` fuer beide Boards,
reproduzierbar, Bootpfad unveraendert. Geaendert: `programs/newdesk/` (neu), `programs/rustc-stable-metadata.sh`
(neu), `programs/{Cargo.toml,Cargo.lock,.cargo/config.toml,README.md}`, `programs/newtos-gem/{Cargo.toml,src/lib.rs,
src/gemdos.rs}`, `tos-types/src/gemdos.rs`, `kernel/embed.rs`, `kernel/src/embedded.rs` (Doku),
`tools/xtask/src/{main.rs,programs_tests.rs}`; diese Quittung; das Messverzeichnis. Beide Boards bauen
(64/94; Clippy 76/125; Programme 0).
