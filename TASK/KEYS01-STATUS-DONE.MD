# KEYS01 — Quittung

**Status: IMPLEMENTIERT · in QEMU raspi4b belegt · Abnahme durch den Tester
offen · echte Pi offen** · 2026-10-03 · Basis `desk-rebuild`, HEAD `3387f75`,
Baum vorher sauber, nicht committet · Belege
[measurements/measurement-2026-10-03-krepeat/](../measurements/measurement-2026-10-03-krepeat/RUNINFO.TXT)
(Aenderung als Diff: `method/keys01-change.diff`)

Eine gehaltene Taste wiederholt jetzt wie unter EmuTOS: nach `kb_initial`
50-Hz-Schritten (Vorgabe 15 = 300 ms) die erste Wiederholung, danach alle
`kb_repeat` Schritte (Vorgabe 2 = 40 ms), bis die Taste losgelassen wird.
Die Wiederholung laeuft im gepollten Eingabe-Pump (Variante (b)), nicht im
Timer-Interrupt.

## Nachgeprueft vor dem Bau

Die Befunde der Diagnose stimmen mit dem Quelltext ueberein:
`hid.rs:69-96` meldet nur Aenderungen, `ikbd.rs` (`decode_key`) legt pro Make
ein Wort ab und verwirft Breaks, `tick.rs:40-44` enthielt die falsche
Klammer „USB-HID / IKBD does its own“, `KB_RATE` wurde nur von `kbrate`
gelesen. EmuTOS: `bios/ikbd.c:395-536` (Kommentar, `kbrate`, `do_key_repeat`,
`kb_timerc_int`), Start `:875,891,898-909`, Stopp `:807-817`, Takt
`bios/vectors.S:521` mit Sieb `0x1111` (`bios/bios.c:463`), Vorgaben
`include/sysconf.h:68-69`, `conterm = 7` `:1100`.

## Was sich geaendert hat

* **Neu `kernel/src/key_repeat.rs`:** die Zustandsmaschine `KeyRepeat`
  (`kb_ticks`, `kb_last`, `kb_last_actual`, dazu `hz_200` des letzten Polls).
  `make()` startet mit dem aktuellen `kb_initial`. `brk()` stoppt beim Break
  der wiederholten Taste sowie bei Home/Insert, egal welche Taste wiederholt.
  `poll()` zaehlt die Vielfachen von 4, die `hz_200` seit dem letzten Poll
  ueberschritten hat (dieselbe Phase wie das EmuTOS-Sieb), und gibt hoechstens
  ein Wort zurueck. Bei geloeschtem `conterm`-Bit 1 friert der Zaehler ein,
  er wird nicht zurueckgesetzt (`:475-479`). Hat sich `shifty` geaendert, wird
  das Wort aus dem Roh-Scancode neu umgesetzt (`:502-508`). Danach gilt das
  aktuelle `kb_repeat`. Das Modul ist rein und hat keine Kernel-Abhaengigkeit;
  `tools/xtask` bindet es per `#[path]` ein, wie `bootargs.rs`.
* **`kernel/src/ikbd.rs`:**
  * Ein Make einer Nicht-Modifier-Taste ruft `make()` auf, ein Break einer
    Nicht-Modifier-Taste ruft `brk()` auf. Das gilt auch fuer den Break von
    CapsLock, wie im `default`-Arm von EmuTOS.
  * Modifier aendern nur den Shift-Zustand.
  * Neu ist `poll_repeat()`, es legt ein faelliges Wort per `push_rx` ab.
  * `key_word()` ist ausgelagert und wird fuer Make und Neuumsetzung
    gemeinsam genutzt.
* **`kernel/src/host_env.rs`:** `poll_repeat()` steht direkt **nach** jedem
  `usb::poll()`, an allen drei Stellen: `pump_input`,
  `console_read_byte_blocking` und `console_read_key_blocking`.
* **`tos-api/src/xbios/system.rs`:** `kb_rate()` liest dieselben zwei
  Bytefelder, die `Kbrate` setzt.
* **`tos-api/src/bios/chardev.rs`:** `current_conterm()` liest das `$484` des
  laufenden Prozesses ueber den Kernel-Alias von `RUN`, so wie
  `tick::publish`. Laeuft kein Prozess (Desktop, Boot), gilt 7. Dazu kommt
  `CONTERM_KEY_REPEAT`.
* `loader.rs`: `CONTERM_DEFAULT` ist jetzt `pub(crate)`. Sonst keine
  Aenderung.
* `kernel/src/lib.rs` (`pub mod key_repeat`), `tools/xtask/src/main.rs`
  (Testeinbindung).

## Burst-Schutz: hoechstens eine Wiederholung pro Poll

Unter EmuTOS legt der Interrupt bei gehaltener Taste je `kb_repeat` Ticks ein
Wort ab, auch wenn niemand liest. Mit Polling weiss NewTOS bei einem spaeten
Poll nicht, **wann** in der Luecke die Taste losgelassen wurde. Ein Break, den
der naechste Poll sieht, kann direkt nach dem vorigen Poll passiert sein.
Wuerde die Luecke voll gutgeschrieben, kaemen Woerter fuer Zeit, in der die
Taste vielleicht nicht mehr gehalten war. Das sind genau die Geistertasten,
die Variante (b) vermeiden soll.

Darum gibt ein Poll hoechstens **ein** Wort aus. Die restlichen Schritte
verfallen, und die naechste Wiederholung ist `kb_repeat` Schritte nach diesem
Poll faellig. Solange mindestens einmal pro Tick gepollt wird (jede
AES-Warteschleife, jedes blockierende Lesen, `Bconstat` bei leerer Schlange),
ist der Takt exakt der von EmuTOS. Die Rate faellt nur bei einem Aufrufer, der
seltener pumpt. Belegt im Hosttest `long_gap_emits_at_most_one`.

## Spec und Doku

* `docs/api/xbios/_state.md`: neuer Abschnitt §„Key-repeat handler“ mit der
  EmuTOS-Ereignistabelle (Zeilenbelege), den Folgerungen und der
  NewTOS-Bindung. Darin die Abweichung, der Burst-Schutz und was nicht
  nachgebildet ist: `ikbdsys`-Wechsel, Tastenklick, Wiederholung bei der
  Mausemulation, Alt-Numpad, `convert_scancode`-Umschreibung.
* `docs/api/xbios/23-kbrate.md`: In §Pi binding war die USB-HID-Behauptung
  falsch, „not wired“ war veraltet, und „~50 Hz VBL“ war falsch (es ist der
  Timer C). Alle drei Stellen sind korrigiert, die Implementierungsnotiz ist
  aktualisiert.
* `docs/COMPATIBILITY.md` §„Sanctioned deviations“: neuer Eintrag mit Ort,
  Grund und beobachtbarem Unterschied.
* Weitere Kopien derselben Begruendung angepasst: `tos-api/src/bios/tick.rs`
  (Moduldoku), `docs/api/bios/_state.md` (Lueckenliste),
  `docs/api/bios/06-tickcal.md` (Tabelle, Text, Lueckenliste) und
  `docs/STEP2-VALIDATION.md` (K7 als FIXED markiert).
* `docs/STATUS.md` ist **nicht** angefasst (Vorgabe).

## Ergebnisse

| Messung | Soll | Ist |
|---|---|---|
| Hosttests (`cargo test -p tos-api -p tos-types -p xtask --no-fail-fast`) | 980/0/1 + neu | **997/0/1** (+15 `kernel_key_repeat`, +2 `xbios_kbrate_repeat_source`); vorher auf 3387f75 nachgemessen 980/0/1 |
| Release-Bau nach `cargo clean -p kernel -p tos-api --target … --release`, qemu-virt | 64 / 94 | **64 / 94** |
| dto. rpi4 | 64 / 94 | **64 / 94** |
| clippy qemu-virt (kernel / tos-api) | 125 / 80 | **125 / 80** |
| clippy rpi4 | 125 / 80 | **125 / 80** |
| `sh tools/desk-drift-probe.sh` | 31 | **31** |

Keine Clippy-Meldung in einer der geaenderten Dateien.

Hosttests der Zustandsmaschine:
* Verzoegerung und Rate mit den Vorgaben, Phase des Siebs.
* Das Wiederholwort ist das Make-Wort.
* Der Break stoppt; ein Break im selben Poll wie der faellige Schritt gibt
  keinen Geist.
* Ein fremder Break stoppt nicht, ein Home/Insert-Break stoppt immer.
* Die letzte Taste gewinnt.
* Bei geloeschtem `conterm`-Bit 1 friert der Zaehler ein.
* `kb_initial` wird beim Make gelesen, `kb_repeat` bei jeder Wiederholung.
* `initial = 0` heisst keine Wiederholung, `repeat = 0` genau eine.
* Eine lange Luecke ergibt hoechstens ein Wort.
* Bei Shift-Wechsel wird neu umgesetzt.
* Ueberlauf von `hz_200` ist abgedeckt.

Kbrate:
* `(-1,-1)` liefert `0x0F02` = `kb_rate()` (15, 2).
* Setzen wirkt auf `kb_rate()`.
* Ein Wert allein setzen, Maskierung, Vorzeichenerweiterung.
* Ohne Prozess ist `conterm` = 7.

## Vorher/Nachher (QEMU raspi4b, Doom, Injektor)

Identische K06-Doom-Binaerdatei, identischer Injektor, nur der Kernel
unterscheidet sich (`RUNINFO.TXT`, `method/analyse-output.txt`):

| | vorher (3387f75) | nachher (KEYS01) |
|---|---|---|
| 5 s Pfeil hoch: Woerter | 1 | 119 = 1 Make + 118 Wiederholungen |
| erste Wiederholung | — | 59 Ticks = 295 ms nach dem Make |
| Abstand | — | 8 Ticks = 40 ms, konstant |
| Woerter nach dem Break | 0 | **0** (keine Geistertasten) |
| Schlange rein/raus | 6/6 | 167/167 |
| Doom PRESS → RELEASE (5 s) | 36050 → 36420 ms | 36040 → **41125 ms** |
| Doom PRESS → RELEASE (2 s) | 44055 → 44420 ms | 44045 → **46125 ms** |

Screendumps (`screendumps/sheet-*-t035-t043.png`): Vorher macht der Spieler
einen kurzen Schritt und bleibt stehen. Nachher laeuft er durch die Halle, bis
er an der Wand steht. Nebenbei belegt: Ein spaeter Poll (Doom laedt E1M1 und
pumpt ~300 ms nicht) sah den Break vor dem faelligen Schritt, also keine
Wiederholung von Return.

## Fuer den Tester

* Beide Boards booten, normale Selftests, Schirmabzuege: keine erwartete
  Aenderung, ausser dass eine gehaltene Taste jetzt wiederholt.
* Den Wiederholungslauf zeigt nur der Injektor-Weg
  (`method/apply-instrumentation.py` auf eine Scratch-Kopie, Lauf wie in
  `RUNINFO.TXT`). Einen verdrahteten Selftest gibt es nicht (Selftests haengen
  in keinem Bootpfad).
* Auf qemu-virt laeuft `poll_repeat` in den Schleifen mit. Ohne IKBD-Make
  (dort gibt es kein USB) bleibt es wirkungslos.
* **Nur echte Pi:** der USB-Weg selbst; ob Doom bei gehaltener Taste
  durchlaeuft; Tippen im Desktop, in EmuCON und QED mit Wiederholung (Halten
  von Buchstaben, Pfeilen, Backspace); kein Haengenbleiben einer Taste nach dem
  Loslassen.

## Offen / Fragen

1. Ein verlorener USB-Break (Report verloren) ergaebe eine Dauerwiederholung,
   so wie ein verlorenes ACIA-Byte auf echter Hardware. Abfangen liesse sich
   das nur mit einer Abweichung von EmuTOS (Zeitlimit); nicht gebaut.
2. Nicht nachgebildet, weil es auf NewTOS kein Gegenstueck gibt:
   * Stopp beim `ikbdsys`-Wechsel (haengt an K4),
   * Tastenklick,
   * Mausemulation,
   * Alt-Numpad.

   Steht in der Spec. Kommt K4 (IKBD-Vektoren fuer EL0), muss der
   `ikbdsys`-Stopp nachgezogen werden.
3. `conterm` gilt je Prozess (jede TPA hat ihr `$484`). EmuTOS hat ein
   globales `conterm`. Setzt ein Programm Bit 1 zurueck, gilt das hier nur,
   solange es laeuft. Das folgt aus dem bestehenden Sysvar-Modell und ist
   nicht Teil dieses Tickets.
