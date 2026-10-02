# PORT06 — Diagnose: Doom bleibt bei gehaltener Pfeiltaste stehen

Datum 2026-10-02 · Basis `desk-rebuild`, HEAD `1f6dbf4` · nur Diagnose, kein
Produktcode geaendert · Belege
[measurements/measurement-2026-10-02-port06-keys/](../measurements/measurement-2026-10-02-port06-keys/RUNINFO.TXT)

## Kurzfassung

**Ursache: NewTOS erzeugt keine Tastenwiederholung.** Eine gehaltene Taste
ergibt auf dem Pi genau **ein** Tastenwort in der IKBD-Schlange. Doom haelt
eine gewoehnliche Taste nur so lange fuer gedrueckt, wie Wiederholungen
eintreffen. Kommt nach `hold_first_ms` = 15·20+60 = **360 ms** keine, meldet
Doom „losgelassen". Der Spieler laeuft also etwa 0,36 s und bleibt dann stehen.
Loslassen und neu druecken liefert ein neues Make, und das Spiel beginnt von
vorn. Ein „Puffer, der leerlaeuft" ist es nicht: Es kommt von Anfang an nur ein
Ereignis.

Nachgestellt in QEMU raspi4b mit einem Injektor im Scratch-Kernel. Bei 5 s
Halten bekommt Doom 1 Ereignis und 0 Wiederholungen, die Freigabe kommt nach
365 ms (Lauf 2: 385 ms).

Doom liegt richtig, NewTOS weicht ab. `Kbrate(-1,-1)` liefert `0x0F02`,
wie EmuTOS es tut. Die Werte haben auf NewTOS aber keine Wirkung, und Doom
verlaesst sich auf sie.

## Beobachtete Tatsachen

1. **HID-Weg, eine Taste gehalten:** `kernel/src/drivers/usb/hid.rs:69-96`
   (`keyboard_report`) vergleicht jeden Boot-Report mit dem vorigen. Ein Make
   entsteht nur fuer eine **neue** Usage (`:88-93`), ein Break nur fuer eine
   **verschwundene** (`:79-85`). Ein unveraenderter Report, auch einer, den die
   Tastatur nach ihrer Idle-Rate erneut sendet, erzeugt kein Byte. `SET_IDLE`
   wird nicht gesendet; `xhci/mod.rs:1436-1456` setzt nur SET_CONFIGURATION und
   SET_PROTOCOL(boot). Fuer das Ergebnis ist das gleichgueltig, weil der
   Vergleich jede Wiederholung verschluckt. FreeMiNT setzt Idle sogar
   ausdruecklich auf „unendlich"
   (`references/freemint/sys/usb/src.km/udd/hid/keyboard/usb_keyboard.c:599`):
   Ein Boot-Keyboard **meldet nur Aenderungen**, die Wiederholung ist Sache
   des Betriebssystems.
2. **Dekoder:** `kernel/src/ikbd.rs:144-176` (`decode_key`, Push `:173`) legt pro Make
   genau ein Wort `scancode<<8|ascii` per `console::push_rx` ab. Ein Break
   einer gewoehnlichen Taste wird verworfen (`_ => {}`, `:175`). Einen Zaehler
   `kb_ticks`, ein `kb_last` oder einen Timer gibt es nicht.
3. **Systemtick:** `tos-api/src/bios/tick.rs:40-44`: „The 50 Hz half of the
   sieve is not implemented here. Keyboard repeat (USB-HID / IKBD does its
   own) …". Die Klammer stimmt nicht (siehe 1); der Rest beschreibt die Luecke
   richtig. Bekannt ist die Luecke als **K7** in
   `docs/STEP2-VALIDATION.md:41` und in PORT06-UNFINISHED, Punkt 6.
4. **Kbrate:** verdrahtet (`tos-api/src/xbios/mod.rs:55,122`), reiner
   Schatten `KB_RATE` mit Vorgabe `0x0F02` (`tos-api/src/xbios/system.rs:186-226`),
   Rueckgabe bytegleich zu EmuTOS `bios/ikbd.c:439-449`. Kein anderer Code
   liest `KB_RATE`. Die Rueckgabe passt also zu EmuTOS, aber nicht zu dem, was
   NewTOS tut (gar keine Wiederholung).
5. **Schlange:** `kernel/src/console.rs:32,59-71`: 128 Woerter, SPSC, bei
   Ueberlauf wird verworfen. Fuer dieses Fehlerbild spielt sie keine Rolle (es
   kommt nur 1 Wort). Ein endlicher Zaehler oder Timeout, der Wiederholungen
   „nach einer Weile" abschaltet, existiert im Pfad USB → `hid.rs` →
   `ikbd::feed` → `push_rx` → `rx_pop_key` → AES nicht. Abgebrochen wird nur
   durch Doom selbst (`doomgeneric_gem.c:1493-1503`).
6. **Messung** (`serial/run1-hold-serial.log`, RUNINFO):
   ```
   K06 Kbrate=0f02 hold_first=360 hold_repeat=100 ms
   [k06] feed 0x48 at hz_200 6006            (Taste runter)
   [k06] push_rx 0x4800 at hz_200 6006       (ein Wort)
   K06 evnt MU_KEYBD kc=4800 ks=0 at 30030 ms
   K06 put PRESS key 173 at 30030 ms
   K06 put RELEASE key 173 at 30395 ms       (Doom gibt nach 365 ms auf)
   [k06] feed 0xc8 at hz_200 7005            (Taste hoch, 5 s spaeter; nichts mehr gemeldet)
   ```
   Lauf 2 (2 s Halten): PRESS 38030, RELEASE 38415 ms.

## Was EmuTOS tut (Referenz)

* `bios/ikbd.c:395-423` (Kommentar) und `:451-536`: Eine Software-Wiederholung
  in `do_key_repeat()`, aufgerufen von `kb_timerc_int()` mit **50 Hz** (jeder
  vierte Timer-C-Interrupt, `bios/vectors.S:521`).
* Ein Make einer Nicht-Modifier-Taste setzt `kb_ticks = kb_initial` (`:891`)
  und merkt sich `kb_last` (shifty, scancode, ascii) und `kb_last_actual`
  (`:875,898-903`).
* Laeuft `kb_ticks` ab, wird `kb_last` erneut in `ikbdiorec` gelegt
  (`push_ikbdiorec`, `:524`) und `kb_ticks = kb_repeat` gesetzt (`:527`). Das
  wiederholt sich **unbegrenzt**, bis ein Break von `kb_last_actual` kommt
  (`:816-817`), außerdem bei Alt-Numpad/Home/Insert (`:791-813`).
* Aendert sich `shifty` waehrend der Wiederholung, wird der Scancode neu
  umgesetzt (`:502-508`). `conterm` Bit 1 schaltet die Wiederholung global ab
  (`:475`). Wechselt `ikbdsys`, bricht die Wiederholung ab (`:453-472`).
* Vorgaben 15/2 (≈300 ms / 40 ms), `kbd_init` `:1091-1093`; `Kbrate` liest und
  schreibt nur die beiden Globals (`:439-449`).
* **Nur die zuletzt gedrueckte Taste wiederholt** (`kb_last` ist ein einziger
  Eintrag). Wird eine zweite Taste gedrueckt, endet die Wiederholung der
  ersten, auch wenn sie gehalten bleibt.
* Die AES holt Tasten ueber `chkkbd()` aus der BIOS-Schlange
  (`aes/gemdisp.c:156-177`). EmuTOS-Wiederholungen kommen deshalb bei
  `evnt_multi(MU_KEYBD)` an.
* Loslassen sehen TOS-Programme nur ueber die IKBD-Vektoren. `kbdvec` liegt
  direkt **vor** `kbdvecs` (TOS ≥ 2.0, undokumentiert; `bios/aciavecs.S:44-53,129`)
  und wird mit jedem Nicht-Paket-Byte aufgerufen, also auch mit Breaks (Bit 7),
  a0 = iorec (`:405-413`). Daneben `ikbdsys` (`:139`), das ACIA-Systemvektor-Byte.
  Standard ist `kbdvec = _kbdvec` → `kbd_int` (`:372-377`).

## Antworten auf die vier Fragen

**1. Wo entsteht die Wiederholung, passt sie zu EmuTOS, stimmt Kbrate?**
Auf dem Tastaturweg des Pi entsteht **nirgends** eine Wiederholung: weder im
Geraet (Belege 1, 6), noch in `hid.rs`, `ikbd.rs` oder einem Tick
(`tick.rs:40-44`). Das weicht von EmuTOS ab (unbegrenzte Wiederholung nach
300 ms im 40-ms-Takt). `Kbrate(-1,-1)` = `0x0F02` ist als Rueckgabe korrekt,
hat aber keine Wirkung.

**2. Endlicher Puffer, Zaehler oder Timeout?** Im Kernel und in der AES
nicht. Das 360-ms-Fenster liegt in Doom (`doomgeneric_gem.c:1381-1396`,
`:1493-1503`) und wirkt, weil keine Wiederholung kommt. Die 128er-Schlange
laeuft hier nicht voll.

**3. Passt Doom zur Lieferung?** Gegen EmuTOS ja, gegen NewTOS nein.
Gemessen an EmuTOS passen Dooms Fenster: 300 ms + 60 ms Rand fuer die erste,
40 ms + 60 ms fuer jede weitere Wiederholung. Der Rand schuetzt auch langsame
Bilder, weil Doom die Schlange **vor** der Zeitpruefung leert
(`:1457-1485` vor `:1493`). Die Abweichung liegt bei NewTOS. Zwei echte
Schwaechen der Heuristik bleiben, die auch auf TT/EmuTOS auftreten:
(a) Weil nur die letzte Taste wiederholt, gilt bei *Vorwaerts halten + kurz
Links* die Vorwaertstaste nach ≤ 100 ms als losgelassen, obwohl sie gehalten
ist. (b) Doom meldet das Loslassen erst bis zu 100 ms spaeter, der Spieler
laeuft also etwas nach. Beides laesst sich nur ueber die IKBD-Vektoren loesen,
nicht ueber eine andere Heuristik.

**4. Kann ein Programm auf NewTOS Druecken und Loslassen TOS-konform sehen?**
**Nein.**
* `Kbdvbase` (`tos-api/src/xbios/system.rs:98-134`) liefert einen Zeiger auf
  9 Slots in nativer Breite, alle **0**. Es gibt **keinen** `kbdvec`-Slot
  davor.
* Der Zeiger ist eine Kernel-Adresse in der oberen Haelfte
  (`kernel/src/arch/aarch64/link-rpi4.ld:24`, `VA_OFFSET 0xFFFF…`), gemappt
  nur fuer EL1 (`kernel/src/arch/aarch64/mmu.rs:82-83,179`, `AP_RW_EL1`). Ein
  EL0-Programm, das ihn dereferenziert, laeuft nach Lage der Mappings in einen
  Berechtigungsfehler (nicht ausgefuehrt).
* `ikbd::feed`/`decode_key` rufen keinen Vektor auf, der Mausweg geht direkt
  nach `vdi::feed_relative` (`ikbd.rs:117-122`). Die einzige Ausnahme ist
  `mousexvec`, kernelintern (`tos-api/src/bios/vectors.rs:180-215`).
* Bekannt als **K4** (`docs/STEP2-VALIDATION.md:38`, `docs/api/xbios/22-kbdvbase.md`
  §Pi binding „the table is never populated").
* Ein Zustellweg fuer EL0-Rueckrufe existiert: die Upcalls, die G_USERDEF
  benutzt (`kernel/src/arch/aarch64/exceptions.rs:538-605`). Eine
  `kbdvec`-Zustellung koennte darauf aufsetzen, aber nur polled bzw. bei der
  Rueckkehr nach EL0, nicht im Interrupt.
* Auf dem echten TT/Falcon ist `kbdvec` (TOS ≥ 2.0) bzw. das Einhaengen von
  `ikbdsys` der uebliche Weg fuer Spiele. Ein Doom-Hook darauf liefe dort.

## Nebenfrage: bremst die serielle Ausgabe das System?

**(a) Synchron und blockierend, 115200 Baud.** `kprintln!` schreibt per
`UartWriter` byteweise (`kernel/src/lib.rs:85-122`). `write_byte` wartet
aktiv, solange `FR.TXFF` gesetzt ist
(`kernel/src/drivers/bcm2711/pl011.rs:165-172`). Die Leitung laeuft mit
115200 8N1 (`pl011.rs:14-18,136-140`). Ist der FIFO voll, kostet jedes Byte
also ≈ 87 µs. Die Ausgabe von Programmen (GEMDOS-Konsole) geht ebenfalls
synchron auf UART und fbcon (`kernel/src/host_env.rs:58-68`). Sie laeuft im
SVC-Handler bei maskierten IRQs. `_hz_200` holt verpasste Ticks nach
(`tick.rs:31-39`), verliert also keine Zeit.

**(b) Welche Zeilen koennen zur Laufzeit wiederholt feuern?**

| Ort | Wann | pro Taste/Report? |
|---|---|---|
| `xhci/mod.rs:1064` „transfer timeout" | nur in `wait_transfer` ← `control()` (`:1113`) ← Enumeration; aufgerufen nur aus `usb::init` (`drivers/usb/mod.rs:61-62`) | **nein**, nur beim Booten (dort mit 50-Mio-Schleife je Timeout, `:1036`) |
| `xhci/mod.rs:699` „stray transfer event" | Event fuer kein bekanntes Geraet/Endpoint | nur im Fehlerfall |
| `xhci/mod.rs:727` „interrupt transfer … re-arming" | Report-Transfer mit Fehlercode | nur im Fehlerfall; bei einem Dauerfehler **jede** Abfrage |
| `xhci/mod.rs:615` „has no interrupt endpoint" | Re-Arm ohne Endpoint | nur im Fehlerfall |
| `hid.rs`, `ikbd.rs`, `host_env.rs::pump_input` | — | **keine** Ausgabe |
| `console.rs:174-176` `push_rx` | nur mit Feature `initinfo-diag` | im Standardbau nein |
| AES / GEMDOS Trace | nur mit `aes-trace` / `gemdos-trace`; `QED_SVC_TRACE = false` (`dbg_qedtrace.rs:71`) | im Standardbau nein |
| `exceptions.rs:899,980,984` „[sched] switch …" | jeder Kontextwechsel (Accessory wird lauffaehig, Pexec/Pterm) | nicht pro Taste; im QEMU-Lauf nach `[launch]` **0** Zeilen (2 Accessories auf der Karte, beide warten auf Nachrichten) |
| Doom selbst (`DOOM_STATS=5`) | eine Zeile alle 5 s, ca. 60 Byte | ≈ 5 ms je 5 s |

**(c) Kann das den Tastenweg oder die Bildschleife aufhalten?** Im
Standardbau gibt es auf dem Tastenweg pro Taste, Report, IKBD-Paket,
AES-Ereignis oder VDI-Aufruf **keine** Ausgabe. Eine gehaltene Taste erzeugt
keine Zeile. Die serielle Ausgabe **erklaert das Fehlerbild nicht**: Die
Messung zeigt den Abbruch nach 360 ms schon ohne jeden seriellen Engpass
(QEMU). Eine Bremse entsteht nur, wenn eine Fehlerzeile in Dauerschleife
feuert. Kandidat ist `:727` bei einem dauerhaft fehlschlagenden
Interrupt-Transfer, ≈ 5 ms pro Zeile. Das zeigt nur ein serielles Log vom Pi.
**Headless nicht zeigbar:** QEMU raspi4b hat keine USB-Tastatur (kein xHCI-
Report-Pfad), und QEMUs PL011 bildet die Baudrate nicht nach. Die Kosten der
seriellen Ausgabe lassen sich in QEMU also nicht messen.

## Fix-Optionen (Rangfolge)

### 1. Kernel: EmuTOS-Tastenwiederholung nachbauen (schliesst K7) — empfohlen
Inhalt: `kb_ticks`/`kb_last`/`kb_last_actual` mit EmuTOS-Semantik.
* Start bei einem Nicht-Modifier-Make.
* Stopp beim Break von `kb_last_actual` sowie bei Alt-Numpad/Home/Insert.
* Neu umsetzen bei einer `shifty`-Aenderung.
* Gesperrt, wenn `conterm` Bit 1 fehlt (`$484` des laufenden Prozesses,
  `tos-api/src/bios/chardev.rs:55-58`).
* Takt aus `KB_RATE` (Accessor in `xbios::system`).

Die Entscheidung ueber den Ort liegt beim Lead/User:
* **(a) Wie EmuTOS:** 50-Hz-Teil im Timer-IRQ. Problem: `push_rx` ist SPSC,
  und `usb::poll` laeuft im Pump bei **demaskierten** IRQs
  (`host_env.rs:84-119`). Damit gaebe es zwei Produzenten, und die Schlange
  muesste IRQ-fest werden. Schwerer wiegt: Der Break kommt auf dem Pi nur
  ueber die *gepollte* USB-Abfrage. Pollt niemand, stapelt der IRQ weiter
  Wiederholungen, und nach dem Loslassen kommen Geistertasten (bis zu 128).
* **(b) Gepollt im Eingabe-Pump:** erst `usb::poll` (sieht den Break), dann
  die seit dem letzten Pump verstrichenen 50-Hz-Schritte aus `hz_200`
  abarbeiten. Keine Geistertasten, keine zweiten Produzenten. Das waere eine
  benannte Abweichung („Eingabe auf dem Pi ist gepollt, also auch die
  Wiederholung"). Unbeobachtbar ist nur, dass sich ohne Pump nichts in der
  Schlange sammelt. Jeder Leser pumpt aber: `fill_from_device`
  (`host_env.rs:49-54`), die AES-Wartepfade, `sched::sleep`
  (`tos-api/src/bdos/process/sched.rs:509-511`).

Spec zuerst: `docs/api/xbios/23-kbrate.md` §Pi binding ist falsch. Dort steht
„USB-HID does its own key-hold repeat", und „Kbrate … not in the NewTOS XBIOS
dispatcher" ist veraltet, `mod.rs:122`. Dieselbe falsche Begruendung steht in
`tos-api/src/bios/tick.rs:41`. Beide Stellen muessen mit korrigiert werden.
Aufwand: mittel, ca. 100–150 Zeilen plus Selftest (Make → nach 15 Ticks ein
Wort, dann alle 2; Break stoppt; Kbrate(0,…) wirkt). Risiko: gering bis mittel.
Jeder Tastenleser (Desktop, QED, TeraDesk, EmuCON) sieht danach
Wiederholungen. Das ist TOS-Verhalten, sollte aber auf der Pi abgenommen
werden. Ein verlorener USB-Break gaebe eine Dauerwiederholung, wie auf echter
Hardware bei einem verlorenen ACIA-Byte. Doom bleibt dabei unveraendert.

### 2. Kernel + Doom: IKBD-Vektoren fuer EL0 (schliesst K4), Doom haengt `kbdvec` ein
Doom saehe echtes Druecken und Loslassen (Bit 7), auch fuer mehrere Tasten
gleichzeitig. Damit verschwinden die Schwaechen 3(a) und 3(b), auch auf dem
TT. Kernelseitig noetig:
* eine EL0-erreichbare `kbdvecs`-Tabelle mit `kbdvec` bei −1,
* Vorbelegung mit Standard-Handlern,
* Zustellung jedes Rohbytes als Upcall,
* Klaerung der Breite (LP64-Slots gegen den 36-Byte-Vertrag),
* Klaerung des Zeitpunkts (polled statt Interrupt).

Aufwand: hoch. Risiko: mittel bis hoch (neuer Upcall-Konsument im Eingabepfad,
Wiedereintritt, `ikbdsys`-Wechsel wie EmuTOS `:453-472`). Loest das
Doom-Problem auf NewTOS **nur zusammen mit** oder **anstelle von** Option 1.
Gegen Option 1 spricht das nicht, denn jedes andere Programm braucht die
Wiederholung trotzdem.

### 3. Doom-Heuristik anpassen — kein Fix
Ohne Wiederholung und ohne Vektor gibt es auf NewTOS kein Signal, aus dem
Doom das Halten ablesen koennte. Ein laengeres `hold_first` verschiebt den
Abbruch nur, und auf einem TT liefe der Spieler dann nach. Nicht empfohlen.

## Offene Fragen

1. Option 1, Ort (a) EmuTOS-treu im IRQ (mit IRQ-fester Schlange und dem
   Geistertasten-Risiko) oder (b) gepollt im Pump als benannte Abweichung?
   Entscheidung Lead/User; die Spec-Korrektur an `23-kbrate.md` und
   `tick.rs:41` geht vorher an den Researcher.
2. Pi-Gegenprobe: Ist die beobachtete Laufstrecke wirklich ≈ 0,4 s? Wenn der
   Spieler deutlich laenger laeuft (Sekunden), gibt es eine zweite, hier nicht
   erfasste Ursache auf dem USB-Weg. Ein serielles Log vom Pi waehrend des
   Haltens wuerde zudem zeigen, ob `xhci/mod.rs:699/727` feuern.
3. Unter FreeMiNT/TT: Ist `kbdvec` als Doom-Hook dort tragfaehig, da MiNT
   `ikbdsys` ersetzt? Klaeren, bevor Option 2 fuer den m68k-Bau geplant wird.
4. Laeuft auf der Pi-Karte ein Accessory, das waehrend Doom lauffaehig wird?
   Dann fallen pro Wechsel `[sched]`-Zeilen an (≈ 3–4 ms je Zeilenpaar).
   In QEMU bleiben sie aus.
