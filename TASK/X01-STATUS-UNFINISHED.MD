# X01 — Quittung: IMPLEMENTIERT, zwei Entscheidungen und die echte Pi offen

Datum 2026-09-14 · Basis `desk-rebuild`, HEAD `a9a5f8a`, Baum vorher sauber
· Belege [measurement-2026-09-14-x01/](../measurement-2026-09-14-x01/)

**UNFINISHED heisst hier: die Umsetzung steht und ist in QEMU belegt.** Offen
sind (a) zwei Entscheidungen, die ausdruecklich beim Anwender liegen, (b) drei
Aufrufstellen, die nur eine echte Pi 4 erreicht, und (c) die unabhaengige
Abnahme durch den Tester.

## Was tatsaechlich getan wurde

| Datei | Funktion | was |
|---|---|---|
| `sdk/libc/overlay/include/mint/osbind.h` | `Super(ptr)` | m68k-Trap 0x20 → EL0-Fassung: `1L` → `0L` (Abfrage), sonst Nulloperation mit `SP_EL0` als Marke. Herleitung aller sechs m68k-Faelle gegen `references/emutos/bdos/rwa.S:159-182,277-339` steht im Kommentar. |
| dieselbe | `SuperToUser(ptr)` | m68k-Inline-Assembler → `((void)Super(ptr))` |
| dieselbe | `Supexec(funcptr)` | XBIOS-Trap 0x26 → direkter Aufruf, Ergebnis als LONG (EmuTOS `bios/vectors.S:1376` `jmp (a0)`, ohne die Ebene) |
| `sdk/libc/build-libc.sh` | `EXCLUDE` | `unix/ioctl.c` aufgenommen: es uebersetzt jetzt (sein einziger Hinderungsgrund war der m68k-`SuperToUser`) und darf nicht benutzt werden — sein Rumpf ist der m68k-tty-Pfad. Das Archiv traegt weiterhin genau ein `ioctl`, unseres. |
| `sdk/apps/apitest/apitest.c` | Stapelloser Modus | `C:\APITEST.CFG` mit dem Wort `BATCH` schaltet jedes `Cconin()` ab (Seitenumbruch und Schluss). Ohne die Datei bleibt das Verhalten unveraendert — genau so, wie es die echte Pi braucht. Ein Schalter geht nicht: ein `\AUTO`-Programm bekommt in TOS keine Befehlszeile. |
| dieselbe | Gruppe `SUPER` | zehn Proben SX01–SX10: Abfrage, Marke, die Form von TeraDesks `get_tosversion()`, `Supexec`-Aufruf + Ergebnis, cflibs `get_cookie_adr`-Form, Glas-Abtastung, Spiegel gegen Kernglas |

**Kein Rust angefasst.** GEMDOS 0x20 liefert weiter `EINVFN`, XBIOS 0x26
weiter `38`.

## Pruefung, die gelaufen ist

Sechs Laeufe QEMU `raspi4b`, headless, 640x480, mit **demselben Kern**
(`kernel8.img` sha256 `e23991f0…`): je Zustand Standarddesktop, `#Z`-Autorun
TeraDesk, `#Z`-Autorun QED. Werkzeuge in `measurement-2026-09-14-x01/method/`.

* `C:\APITEST.LOG` **vorher**: SUPER 3 bestanden / 6 gefallen / 1
  uebersprungen — `Super(1L)` = `-32`, `Super(0L)` = `0xFFFF…FFE0`,
  `Supexec(f)` = `0x26` und der Rumpf lief nicht (`calls=0`).
* **nachher**: SUPER 10 / 0 / 0 — `Super(1L)` = `0`, Marke `0x003FFEE0`,
  `Supexec(f)` = `0x5EC0FFEE` mit `calls=1`, `Supexec(get_cookie_adr)` =
  `0x5F0`. Gesamt 85/6/6 → 92/0/5.
* **SX04 ist der Kontrollwert:** die Form von `gettos.c:38-44` liefert vor
  wie nach der Aenderung `os_version = 0x0404`.
* Bildpunktvergleich (`method/compare-png.py`): TeraDesk **0 von 307200**,
  QED **0 von 307200**. Der Standarddesktop weicht in 134 Punkten ab —
  ausschliesslich die Ziffern von „385373 bytes used" → „384984 bytes used",
  weil die Karte ein kleineres `APITEST.NCP` traegt.
* `QED.NAP` ist bitgleich (sha256 `c2173506…`): QED erreicht beide Funktionen
  nicht. `DESKTOP.NAP` 282192 → 282096 Bytes.
* `libc.a`: 611/14 → 613/11. Neu uebersetzt: `mintlib/setsysvar.c`,
  `unix/sysinfo.c`.
* `cargo run -p xtask -- test` → **547 bestanden / 0 gefallen / 1
  uebersprungen**.
* Beide Boards frisch `--release` (je `touch` der Crate-Wurzeln, beide
  Protokolle zeigen „Compiling tos-api" und „Compiling kernel"). Warnungen
  Delta 0: rustc tos-api 64 / kernel 94, clippy tos-api 81 / kernel 125.
* `qemu-virt`-Rauchprobe: bootet, „[desk] desktop up (0 drives)", kein Fault.

## Was offen bleibt

1. **Entscheidung — TeraDesks Abschaltpfad wird scharf.** `main.c:2118`
   `Supexec(rv)` ruft jetzt wirklich `lobo()` (`main.c:1723`), eine
   Endlosschleife, die die Maschine absichtlich festsetzt. Bisher lieferte
   XBIOS 0x26 `38` und TeraDesk kehrte sauber zurueck. TOS-treu ist das Neue;
   ob das vier Wochen vor der Vorfuehrung erwuenscht ist, entscheidet der
   Anwender. Gefahren wurde der Pfad **nicht**.
2. **Entscheidung — der Cookie-Spiegel ist eine Momentaufnahme.** Er entsteht
   beim Laden (`tos-api/src/bdos/process/loader.rs`, `populate_sysvars`);
   `Ssystem(S_SETCOOKIE)` (`tos-api/src/gemdos/ssystem.rs:88`) zieht ihn nicht
   nach. Fuer jeden heute uebersetzten Aufrufer traegt er (SX07–SX10 gruen);
   einen Schreibpfad hat keine Anwendung (`install_cookie` liegt im `#if 0`).
   **Nicht repariert** — das ist ADR-012s offener Punkt
   (`docs/api/bios/_cookie-jar.md`, Abschnitt „OFFEN").
3. **Nur echte Pi 4.** `raspi4b` hat keine Eingabe („[pcie] RC not present …
   USB disabled"), also sind `main.c:2090/2118` (Menue „Quit → Shutdown"),
   `file.c:945/969` (`force_mediach` nach Medienwechsel) und
   `video.c:289/318` (Videodialog, Overscan-Umschalter) nicht fahrbar.
   `video.c` schreibt dort auf `0xFFFC00` (ST-ACIA) — das faellt mit wie ohne
   diese Aenderung um, ist also kein neuer Befund, aber die Stelle ist jetzt
   erreichbar statt vom `Super`-Fehler abgeschirmt.
4. **Abnahme durch den Tester** steht aus.

## Nicht committet

Die Arbeit liegt unversioniert im gemeinsamen Baum. Fremde unversionierte
Aenderungen liegen in denselben Verzeichnissen nicht vor; der Baum war vor
dieser Arbeit sauber.
