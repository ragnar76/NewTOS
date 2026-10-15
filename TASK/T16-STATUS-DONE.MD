# T16 — STATUS: DONE (2026-10-16) — behoben, in QEMU rot → gruen belegt; nicht committet

**Auftrag:** `tools/inject-arm.py` verpasst den `launch:NAME`-Ausloeser, wenn das Kind innerhalb eines Ticks denselben Slot erbt
(F1-Programm in Slot 0 nach NEWDESK); korrigierte Logik aus `measurement-2026-10-14-desk43-u9-3-5/method/inject-arm-u935.py`
uebernehmen, CLI kompatibel, dokumentieren, mit einem Lauf belegen (`TASK/T16.md`).

## Aenderung

`tools/inject-arm.py` (einzige Datei):
* neues `inj_arm::launched(slot)` im generierten Modul (Name aus `D.s_cmd`, `CHILD_INIT` zuruecksetzen, `launch:`-Ereignis);
* Schritt 4: Anker `SHELL_CHILD_SLOT.store(child.slot, Ordering::Release);` in `kernel/src/launch.rs` (genau einmal, sonst Abbruch),
  dahinter der Aufruf `crate::host_env::inj_arm::launched(child.slot); // INJ-ARM-TEMP` — immer, unabhaengig von den Arms;
* `track_child` (Tick-Abfrage) bleibt als Rueckfall und ruft bei Slotwechsel dasselbe `launched`;
* Docstring: `launch:` = jede Veroeffentlichung ist ein Start, auch bei wiederverwendetem Slot; Abschnitt „LAUNCH EVENT, NOT SLOT CHANGE“
  mit Grund und Belegverweis; `launch.rs` in der Ankerliste. CLI (`<tree> [--tag TAG] ARM...`, Trigger `desktop|launch:|up:`) unveraendert.

Alle vier Kopien `inject-arm-u935.py` (10-14, 10-15, desk58, desk59) sind identisch (sha1 4b2c6c44); keine spaetere Fassung.
Erzeugter Code Werkzeug vs. Kopie bei gleichen Arms: `launch.rs`/`shell_loop.rs` identisch, `host_env.rs` nur 3 Kommentarzeilen
anders (`checks/gen-tool-vs-u935-copy-kf.txt`).

## Beleg (QEMU raspi4b, EL0-Standarddesktop, mode on, Karte f1hg, Arms `desktop=f1.script launch:HELLOGEM=ret.script`)

| Kernel | Ergebnis |
|---|---|
| `kf-red` (Werkzeug vor T16, `method/inject-arm-pre-t16.py`) | HELLOGEM in Slot 0 gestartet, **kein** `[u6a] launch:HELLOGEM armed`, kein Return; 2. `desktop up` fehlt nach 60 s (rc=3), Enddump = HELLOGEM-Fenster „Press a key…“ |
| `kf` (Werkzeug nach T16) | `launch:HELLOGEM armed at hz_200 3227 (slot Some(0) name HELLOGEM)`, Return bei +1600, `quit by key 0x1c0d`, rc=0, NEWDESK wieder oben; Enddump = Desktop |

App-Dumps kf/kf-red byte-gleich; 0 EXC/PANIC. Zusaetzlich nutzt der DESK60-Lauf (`kxtd`, `launch:DESKTOP`) und der Ansichtenkernel
(`kv`, `desktop=`) das neue Werkzeug: 45/45 Ansichten/Serien-Dumps byte-gleich zu DESK58, tdqed-Dumps byte-gleich zur DESK43-Abnahme.

## Zahlen

Hosttests 1372/0/1 (1370 + 2 DESK60), Warnungen 64/94 beide Boards, Clippy 76/125 (rpi4, qemu-virt, +selftest-desk), Programme 0,
Drift 4 / unsafe 20, emutos-consts gruen (466/333/1/1/0/0/132), SDK 20/0/0. Beleg: `measurements/measurement-2026-10-16-t16-desk60/RUNINFO.TXT`.

## Offen

* Nichts am Werkzeug. Echte Pi nicht betroffen (reines Scratch-Werkzeug).

## Ausserhalb des Auftrags (je eine Zeile, nicht angefasst)

* Die bestehenden Messordner verwenden weiter ihre Kopien `inject-arm-u935.py` (unveraenderlich); neue Laeufe koennen `tools/inject-arm.py` nehmen.

## Selbstpruefung

Aufgabe in einem Satz: `launch:` in `tools/inject-arm.py` feuert am Start-Ereignis statt am Slotwechsel, CLI gleich, dokumentiert, mit
rot/gruen-Lauf belegt. Geaendert: `tools/inject-arm.py`; diese Quittung, `TASK/T16.md`, das Messverzeichnis. Nicht committet;
`docs/STATUS.md` nicht angefasst.
