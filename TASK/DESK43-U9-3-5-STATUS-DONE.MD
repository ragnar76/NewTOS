# DESK43 U9-3 + U9-4 + U9-5 — `desk=kernel`, DESKTOP-Arm startet NEWDESK (D6), Beweislaeufe — Quittung

> Stand 2026-10-15 · Basis `main` @ `18428179` (sauber) · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u9-prep-2026-10-14.md` §6 (Zeilen U9-3/U9-4/U9-5), §7 (Tests); Entscheidungen
> `TASK/DESK43.md` („U9-Vorbereitung entschieden": D6 wie `gemshlib.c:600-611`, D6-a/b/c/d, D5-a/b); offene Punkte
> aus `TASK/DESK43-U9-0-STATUS-DONE.MD` und `TASK/DESK43-U9-1-2-STATUS-DONE.MD`.
> Messung: [`measurements/measurement-2026-10-14-desk43-u9-3-5/`](../measurements/measurement-2026-10-14-desk43-u9-3-5/RUNINFO.TXT)
> **Standard ist jetzt der EL0-Desktop NEWDESK; `desk=kernel` ergibt das bisherige Verhalten byte-gleich.** Nichts
> committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`. `docs/STATUS.md` und
> `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: Boot-Argument `desk=kernel` (U9-3), DESKTOP-Arm startet das eingebettete NEWDESK ohne Sanduhr mit
EmuTOS-Absturzpolitik und Rueckfall bei Ladefehler (U9-4), und die Beweislaeufe EL0 gegen EL1 im selben Abbild (U9-5).
Driftprobe **4 → 4**, unsafe **20 → 20**.

---

## U9-3 — `desk=kernel`

* `kernel/src/bootargs.rs`: `DESK_KERNEL = b"desk=kernel"` (+1 Hosttest, Ganz-Token wie `input=aes`/`emmc=compat`).
* `kernel/src/lib.rs`: Entscheidung beim Boot direkt nach dem `input=aes`-Schalter (`boot_switch`, beide Boards),
  `shell_loop::select_desktop(..)` schreibt die Logzeile
  `[shell] desktop: EL0 program NEWDESK (embedded; desk=kernel selects the built-in one)` bzw.
  `[shell] desktop: built-in kernel desktop (boot argument desk=kernel)`.
* Beide Pfade in jedem Abbild (rpi4 1528712 B, +4120 B gegen U9-1-2).

## U9-4 — DESKTOP-Arm

* `kernel/src/shell_loop.rs` `sh_ldapp`, DESKTOP-Arm: `sh_show("")`, `p_setappdir` wie bisher; dann
  * eingebauter Desktop (`desk=kernel` oder Rueckfall): `run_kernel_desktop()` = bisheriger Code
    (`desktop::reset()` + `desktop::run()`), unveraendert;
  * sonst `DESKTOP_ACTIVE` um `launch::run_desktop_program(DEF_DESKTOP, D.s_cmd)`; `rc != 0 → wm_new()`
    (`gemshlib.c:606-609`, jeder Wert ≠ 0); Rueckgabe immer 0 (`:611`); `sh_nextapp` bleibt `DESKTOP_APP` → Neustart.
    Keine Meldung, kein Schleifenwaechter (D6-b). Logzeile `[shell] sh_ldapp: desktop 'NEWDESK' exited with rc=N`.
  * **D6-c/D6-d** (`// NEWTOS:`): `rc < 0` (Ladefehler) → Zeile
    `[shell] desktop NEWDESK could not be loaded (rc=N) — not retried; falling back to the built-in kernel desktop`,
    eingebauter Desktop fuer den Rest des Boots, kein zweiter Ladeversuch.
* `kernel/src/launch.rs`: gemeinsamer Kern `launch(path, name, tail, kind)`; `run_program_with_tail_rc` unveraendert im
  Verhalten; neu `run_desktop_program(name, cmd)` = EmuTOS `aes_run_rom_program`: **keine Sanduhr**, **leerer Tail**,
  **kein** Umschreiben von `D.s_cmd`/`ad_stail` (`shel_read` antwortet waehrend des Desktops wie vorher), keine
  `[acc] launching`-Zeile, `p_nameit` aus `D.s_cmd`, kein `set_app_dir` (macht der Arm). Gesucht wird der Desktop
  immer unter dem Registry-Namen `DEF_DESKTOP` (EmuTOS startet `deskstart`, egal was `sh_desk` sagt).
* `kernel/src/lib.rs:606` (A106-`desktop::reset()` beim AES-Neustart) nur noch, wenn der eingebaute Desktop benutzt wird.
* **selftest-desk erreicht NEWDESK** (offener Punkt 1 aus U9-1-2): xtask `PROGRAM_FEATURES = [("newdesk","selftest-desk")]`,
  `build_kernel` reicht seine Features an `build_programs` durch; +1 Hosttest. Beleg: Abschnitt 8 der RUNINFO.
* Spez: `docs/api/aes/shell-loop.md` §8, neuer Abschnitt „the DESKTOP arm starts NEWDESK (DESK43 U9-3/U9-4)".
* `programs/newdesk/src/lib.rs`: nur Doku Zeilen 3-5 (gleiche Zeilenzahl, NAP byte-gleich U9-1-2).

## U9-5 — Beweise (QEMU, ein Abbild, EL0 Standard gegen `desk=kernel`)

| Beweis | Ergebnis |
|---|---|
| 8 Views on/off/strict (Kernel `kv`) | je **7/8 byte-gleich**; einziger Unterschied „Free RAM"-Feld (s. u.) |
| `desk=kernel`-Views gegen U9-0 | **8/8 byte-gleich** |
| Serie desktop/APITEST/TeraDesk/QED on/off/strict/on-asid | EL0 = EL1 **5/5 byte-gleich** je Modus; APITEST 106/0/5, 0 Zeilen Diff |
| `desk=kernel` gegen U9-1-2 (on/off/strict) | Screens 15/15 byte-gleich; APITEST 0 Diff; seriell nur Bootzeile + Kernelgroesse |
| Orakel G2 (svc-seq-trace, Kernel `ks`) | Kernelkontext-Teil (305 Aufrufe) identisch; Desktop-Teil identisch bis auf + `Mshrink`, + 4 `Malloc`, `Keytbl`-Adresse; EL1-Log = U9-0 bis auf die `Keytbl`-Kerneladresse |
| „Save desktop" | geschriebene `NEWDESK.INF` EL0 = EL1 **byte-gleich** (672 B) |
| qemu-virt on/off/strict | Endschirm EL0 = EL1 = U9-1-2 byte-gleich |
| reproduzierbar | Clean-Bau an anderem Pfad: beide Abbilder, alle NAPs 0 Bytes Unterschied |

### Jeder sichtbare Unterschied EL0 gegen EL1, mit Ursache

1. **„Free RAM: 10240 KB + N KB"** im Desktop-Konfigurationsdialog: 915432 → 914748 KB (on/strict), 925092 → 924408 KB
   (off), −684 KB. NEWDESK ist ein Prozess: TPA (Abbild + 64 KiB Stack, PRGFLAGS 7 → Alt-RAM) und Heap-Hunks sind
   belegt. Erlaubt (Prep §5.6).
2. **Sanduhr**: in keinem Dump sichtbar, in beiden Pfaden nicht — der Arm setzt keine (EmuTOS), der NORMAL-Arm setzt sie
   fuer Programme wie bisher. Kein Unterschied gemessen.
3. **Mauszeiger waehrend Desktop-Rechenphasen** (IRQ-Pumpe nur bei Unterbrechung aus EL0, `input_gate`): in den Laeufen
   kein Fall (keine Mausbewegung waehrend einer Rechenphase). Nicht gemessen, nur aus der Quelle.
4. Nicht sichtbar, aber verschieden (aus der Quelle, ungemessen): `appl_find("NEWDESK")` — EL0-Desktop hat einen per
   `p_nameit` benannten AES-Deskriptor (wie EmuTOS `EMUDESK`), der eingebaute `pd::DESKTOP` einen leeren Namen.
5. Seriell: `[launch] EL0: slot 0 …` je Desktopstart, `desktop 'NEWDESK' exited with rc=N` je Desktopende.

## Absturztests (D6)

| Fall | Ergebnis |
|---|---|
| NEWDESK-Panic beim Start (Scratch), on/off/strict | Meldung + „Press any key"; Taste → `rc=65535` → `wm_new` → Neustart → Panic → **wartet**. 2 Starts in 45 s, Schirm steht. Keine Schleife. |
| Blob mit zerstoertem ELF-Kopf | 1× `load failed: BadMagic`, Rueckfallzeile `rc=-66`, eingebauter Desktop; nach HELLOGEM-Ende **kein** zweiter Ladeversuch; Schirme = `desk=kernel`-Zwilling |
| Registry ohne `NEWDESK` | dasselbe mit `NotFound`, `rc=-33` (EFILNF) |
| HGPANIC (Rust-Panic) unter EL0-Desktop, on/off/strict | `rc=65535`, Desktop zurueck; Schirme = EL1 |
| CRCHILD (Schreibfehler, Bericht + Taste), on/off/strict | `rc=65535`, Desktop zurueck; Schirme = EL1 |
| CRASHTST (6 Abstuerze im Enkel) | `CRASHTST: PASS`, `rc=0`, Desktop zurueck; Schirme = EL1 |
| HELLOGEM, QED, TeraDesk, APITEST (106/0/5), SYSACC (AC_OPEN) unter EL0-Desktop | Schirme = EL1 byte-gleich; Doom laeuft in beiden (Demo-Frames verschieden) |

**Mess-Hinweis:** `tools/inject-arm.py`s `launch:`-Ausloeser feuert unter dem EL0-Desktop nicht, wenn das gestartete
Programm denselben Slot (0) innerhalb eines Injektor-Ticks erbt (Abfrage sieht keine Luecke). Erste F1-Laeufe
dadurch ohne Taste (Programm wartete). Behoben in einer **Kopie** im Messverzeichnis (`method/inject-arm-u935.py`,
Ausloeser am Slot-Veroeffentlichungspunkt), Kernel `kf2`; das Werkzeug selbst nicht angefasst.

## Zahlen

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1368/0/1 + neu | **1370/0/1** (+1 bootargs, +1 programs_tests) |
| Warnungen rpi4 / qemu-virt | 64/94 | 64/94 beide; build-programs 0 |
| Clippy Kernel rpi4 / qemu-virt / +selftest-desk | 76/125 | 76/125 alle; Programme 0; xtask --tests 8 (alle vorbestehend) |
| Drift / unsafe | 4 / ≤ 20 | **4 / 20** |
| emutos-consts | gruen | 466/332/2/2/0/0/132 (= U9-1-2) |
| SDK | 20/0/0 | 20/0/0 |
| NAPs | — | = U9-1-2 byte-gleich |
| Selbsttests (selftest-desk → NEWDESK) | 53/1 | 53 PASS / 1 FAIL (vorbestehend), Zeilen = U8b |

## HW-Pruefliste fuer U9-6 (echte Pi 4)

1. Boot ohne `desk=kernel`: Zeile `[shell] desktop: EL0 program NEWDESK …`, `[launch] EL0: slot`, „desktop up"; Desktop bedienbar (Menues, Alt-C, Ansichten, Dialoge).
2. Boot mit `desk=kernel` in `cmdline.txt`: eingebauter Desktop, Zeile nennt den Pfad; Verhalten wie U8b.
3. Programmstart/-ende aus dem Desktop (HELLOGEM, QED, TeraDesk, EmuCON): Desktop kommt zurueck; **Latenz** Programmende → Desktop (QEMU-Schaetzung ~41 ms gegen 6 ms EL1) mit den U9-1-Instrumenten messen.
4. **Absturzaufforderung auf echter Hardware** (Tastatur im Ausnahmekontext, ARCH03 0c auf Pi nie gefahren): CRCHILD per F1 → Bericht → Taste → Desktop zurueck.
5. **NEWDESK-Panic-Scratch-Blob** (nur Scratch-Abbild!): Prompt erscheint, eine Taste → genau ein Neustart → wartet wieder; ohne Taste keine Schleife.
6. Rueckfall bei Ladefehler (Scratch-Abbild mit zerstoertem Blob): Rueckfallzeile, eingebauter Desktop, kein Flackern.
7. „Free RAM" im Konfigurationsdialog: plausibel (≈ −700 KB gegen `desk=kernel`); Speicher nach ≥ 20 Programmzyklen stabil (Leck/Slot).
8. Stack-Hochwasser EL0 (U9-1-Instrument) mit tiefem Verzeichnisbaum kopieren.
9. Mauszeiger bewegt sich waehrend Desktop-Rechenphasen (grosses Verzeichnis oeffnen, dabei Maus bewegen); mit `input=aes` nicht.
10. Accessory (SYSACC) oeffnen/schliessen unter EL0-Desktop; „Save desktop" und Neustart lesen die INF wieder.

## Offen / nicht gemacht

* Echte Pi (U9-6). `memprot=verify`, `asid=tiny`, `input=aes` nicht gefahren.
* Latenz/Stack des EL0-Neustarts aus der Registry nach Programmende nicht gemessen (keine Instrumente in diesen Kerneln).
* TeraDesk → QED-Kette und TeraDesk-Ende zurueck zum EL0-Desktop; Programm, das mit offenen Fenstern abstuerzt
  (Sichtbarkeit von `wm_new`); *Read .INF file*; `appl_find("NEWDESK")` nicht gefahren.

## Ausserhalb des Auftrags (je eine Zeile, nicht angefasst)

* `tools/inject-arm.py`: `launch:`/`up:` erkennen kein neues Kind, wenn es denselben Slot im selben Tick erbt (Fix-Muster in `method/inject-arm-u935.py`).
* Eingebauter Desktop: `pd::DESKTOP` traegt keinen Namen → `appl_find("NEWDESK")` weicht von EmuTOS ab (nur im `desk=kernel`-Pfad).
* `kernel/src/launch.rs:21-27` verwaister Doc-Kommentar vor `busy_cursor` (seit U9-0 notiert).
* `docs/api/aes/shell-loop.md` §7 nennt noch `kernel::desk::reset()` (seit U8b `desktop::reset`).
* Datei-Zeitstempel auf den QEMU-Karten 1979-12-31 (keine Uhrzeit ohne `kernel8.img` auf der Karte) — beide Pfade gleich.

## Selbstpruefung

Aufgabe in einem Satz: `desk=kernel` als Boot-Argument, DESKTOP-Arm startet das eingebettete NEWDESK nach
`gemshlib.c:600-611` mit D6 und Rueckfall, Beweislaeufe EL0 gegen EL1 im selben Abbild. Geaendert:
`kernel/src/{bootargs,lib,shell_loop,launch}.rs`, `tools/xtask/src/{main,programs_tests}.rs`,
`programs/newdesk/src/lib.rs` (Doku), `docs/api/aes/shell-loop.md`; diese Quittung; das Messverzeichnis. Beide Boards
bauen (64/94, Clippy 76/125), reproduzierbar.
