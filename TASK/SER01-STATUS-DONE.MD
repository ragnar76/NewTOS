# SER01 — Quittung

**Status: IMPLEMENTIERT · in QEMU raspi4b belegt · Abnahme durch den Tester
offen · echte Pi offen** · 2026-10-02 · Basis `desk-rebuild`, HEAD `c1f504c`,
Baum vorher sauber · Belege [measurements/measurement-2026-10-02-ser01/](../measurements/measurement-2026-10-02-ser01/)

Auf `board-rpi4` sendet die UART0 nur noch, wenn die Boot-Argumente das ganze
Token `serial=on` enthalten. Panic und fatale Ausnahmen schalten die Ausgabe
immer ein. RX ist unberuehrt. `board-qemu-virt` sendet weiter immer.

## Was der Anwender in `cmdline.txt` schreibt

Auf der Boot-Partition der Karte die Datei `cmdline.txt` (eine Zeile) mit

```
serial=on
```

Weitere Optionen duerfen in derselben Zeile stehen, getrennt durch Leerzeichen.
Ohne das Token (oder ohne Datei) bleibt die serielle Ausgabe aus. Gross- und
Kleinschreibung zaehlt, `serial = on` oder `serial=ON` gelten nicht. Es ist
**nichts** auf die Karte geschrieben worden.

## Wie die Boot-Argumente den Kernel erreichen

Ueber den VideoCore-Mailbox-Tag `0x0005_0001` „Get command line“, **nicht**
ueber `/chosen/bootargs` im Device Tree:

* Quelle: `references/circle/lib/koptions.cpp:52-61` (Circle liest seine
  `cmdline.txt`-Optionen genau so), Tag-ID `include/circle/bcmpropertytags.h:72`
  und `addon/linux/raspberrypi-firmware.h:113`, Wertpuffer 2048 Byte
  `bcmpropertytags.h:305-310`, Antwortregeln `lib/bcmpropertytags.cpp:49-71`,
  Format `doc/cmdline.txt:3-4`. Eingetragen in
  `docs/hardware/mailbox-framebuffer.md` §3e.
* Warum nicht der Device Tree: keine lokale Quelle belegt, was die Pi-Firmware
  in `/chosen/bootargs` schreibt (Circle liest es nie), und QEMU `raspi4b` mit
  `-kernel` uebergibt gar keinen Device Tree (x0 = `0x100` ohne FDT-Magic, jedes
  raspi4b-Log sagt `[mm] device tree: no usable /memory node`). Der Mailbox-Weg
  laeuft auf Pi und QEMU gleich.
* Beobachtet (QEMU 11.0.2): das Firmware-Modell von `raspi4b` beantwortet den Tag
  mit dem `-append`-String. `-append serial=on` ist damit das QEMU-Gegenstueck zu
  `cmdline.txt`.

## Was getan wurde

| Datei | Stelle | was |
|---|---|---|
| `kernel/src/bootargs.rs` | neu | `has_token` (Trennung an ASCII-Leerraum, Ende am ersten NUL, nur ganze Tokens), `SERIAL_ON`, 5 Host-Tests |
| `kernel/src/drivers/bcm2711/mailbox.rs` | `with_command_line` | Tag `0x00050001`, 2048-Byte-Puffer; `None` bei Fehler, ohne Antwort, leer oder voll (moeglicherweise abgeschnitten) |
| `kernel/src/console.rs` | `SerialConsole`, `CONSOLE`, `SERIAL_TX_DEFAULT`, `serial_tx_enabled`, `force_serial_tx_on`, `configure_serial_tx` | `CONSOLE` ist jetzt ein Wrapper um die Board-PL011; nur `write_byte` ist geschaltet, `init`/RX/IRQ gehen durch. Vorgabe rpi4 aus, qemu-virt an |
| `kernel/src/lib.rs` | `kernel_main_rust(dtb_phys, entry_el)`, Moduldeklaration | Schalter direkt nach `CONSOLE.init()`, vor dem ersten `kprintln!`; eine neue Logzeile `[serial] output on: …` wenn an |
| `kernel/src/arch/aarch64/boot.rs` | `BOOT_HEARTBEAT_PRE_MMU`, `_start` | Der Vor-MMU-Herzschlag `RB<EL>` wurde direkt ins Datenregister geschrieben, am Schalter vorbei. Auf rpi4 entfaellt der Direktschreibzugriff; die EL geht in `x1` an `kernel_main`, die Konsole spielt `RB<EL>` nach, wenn die Ausgabe an ist. qemu-virt unveraendert |
| `kernel/src/panic.rs` | `on_panic` | `force_serial_tx_on()` zuerst |
| `kernel/src/arch/aarch64/exceptions.rs` | `unhandled` | `force_serial_tx_on()` zuerst (deckt auch den Stack-Overflow-Zweig ab) |
| `kernel/src/hal/uart.rs` | `UartWriter::write_str` | nur ein Kommentar: `bootlog::capture` sitzt ueber dem Schalter und zeichnet weiter auf |
| `tools/xtask/src/main.rs` | `cmd_run_rpi4`, Testmodul | `-append serial=on`; `kernel/src/bootargs.rs` per `#[path]` als Testmodul, damit die Tests im Host-Lauf mitlaufen |
| `tools/qemu-serial-on/qemu-system-aarch64` | neu | Shim fuer alte Messskripte, s. u. |
| `docs/hardware/mailbox-framebuffer.md` | §3e, Quellentabelle | Tag, Quellen, QEMU-Beobachtung |
| `docs/sdk/06-image-and-testing.md` | raspi4b-Abschnitt | `-append serial=on` in beiden Rezepten und ein Hinweis auf den Schalter und den Shim |

`docs/STATUS.md` ist absichtlich nicht angefasst (Auftrag).

## QEMU-Messwege

* `cargo xtask run --board rpi4`: gibt jetzt `-append serial=on` mit.
* `cargo xtask run` (qemu-virt): unveraendert, dort ist die Ausgabe immer an.
* `tools/make-sd-image.sh`: **nicht geaendert.** Das Image ist eine einzelne
  FAT16-Partition ohne Firmware; QEMU `raspi4b` liest daraus keine
  `cmdline.txt`. Eine dort abgelegte Datei haette keine Wirkung.
* Messskripte unter `measurements/`: nicht angefasst. **Jedes** alte Skript,
  das `qemu-system-aarch64 -M raspi4b … -kernel …` ohne `-append` startet (131
  Dateien unter `measurements/` nennen `qemu-system-aarch64`, die raspi4b-Laeufe
  darunter alle ohne `-append`), bekommt mit einem nach SER01 gebauten Kernel
  ein **leeres** serielles Log. So laufen sie wieder:
  `PATH="<repo>/tools/qemu-serial-on:$PATH" bash <skript> …`. Der Shim haengt
  `-append serial=on` an, wenn die Maschine raspi4b ist und kein `-append` dasteht,
  und startet dann das echte QEMU. Gezeigt mit dem unveraenderten
  `measurement-2026-10-02-port06-keys/method/run-doom.sh`: das Log ist identisch
  mit dem eines Laufs mit `-append serial=on`.
  Neue Skripte schreiben `-append serial=on` direkt hinein.

## Pruefung

Belege in `measurements/measurement-2026-10-02-ser01/` (RUNINFO.TXT):

1. Ohne `serial=on`: Log **0 Byte**, Desktop steht (`ser01-noargs-45s.png`).
2. Mit `-append serial=on`: Desktop steht. Gegen das Log eines HEAD-Kernels ohne
   SER01 unterscheidet es sich nur in **einer** Zeile mehr,
   `[serial] output on: boot arguments contain `serial=on``. Der nachgespielte
   Herzschlag `RB2` steht wie bisher in der ersten Zeile.
3. Ohne `serial=on`, mit Scratch-Panic (nur in einer Kopie im Scratchpad,
   `method/scratch-panic-trigger.diff`): Log enthaelt nur den Panic-Bericht (93 Byte).
4. Altes Skript ueber den Shim: Log identisch mit (2).
5. qemu-virt (release): `RB1`, Boot bis `[desk] desktop up`.

Basislinien:

| Messung | Soll | Ist |
|---|---|---|
| Host-Tests | 970/0/1 + neu | **975/0/1** (+5 `kernel_bootargs`) |
| Release-Build nach `cargo clean -p kernel -p tos-api …`, qemu-virt | tos-api 64, kernel 94 | 64 / 94 |
| dto. rpi4 | 64 / 94 | 64 / 94 |
| clippy qemu-virt | 80 / 125 | 80 / 125 |
| clippy rpi4 | 80 / 125 | 80 / 125 |
| `sh tools/desk-drift-probe.sh` | 31 | 31 |

Keine Clippy-Meldung in den geaenderten Dateien ausser der vorhandenen in
`kernel/src/lib.rs:9`. Der xtask-Testbuild meldet keine Warnungen.

## Offen

* **Echte Pi 4:** Dass die Firmware `cmdline.txt` ueber den Tag `0x00050001`
  zurueckgibt, ist durch Circle belegt, aber nicht auf unserer Hardware
  beobachtet. Abnahme auf der Pi: (a) Karte ohne `serial=on` → am
  Seriell-Adapter kommt kein Byte, auch kein `RB2`; (b) mit `serial=on` → Log wie
  bisher, plus die `[serial]`-Zeile.
* Ohne `serial=on` liefert die Pi selbst bei einer Panic vorher nichts; dass dann
  nur der Panic-Bericht ankommt, ist in QEMU gezeigt.
* Der Herzschlag `RB<EL>` kommt auf rpi4 jetzt erst nach dem MMU-Aufbau (vorher
  vor jedem Setup). Ein Fehler zwischen Einsprung und `kernel_main_rust` bleibt
  daher auch mit `serial=on` stumm. Das ist der Preis fuer „kein Byte ohne
  `serial=on`“; anders liesse es sich nur mit einem Kommandozeilen-Parser im
  Assembler-Stub vermeiden.
* `usb-diag`: `bootlog::capture` laeuft weiter ueber dem Schalter. Der Build
  `--features board-rpi4,usb-diag` uebersetzt (kernel 96 Warnungen, HEAD ohne
  SER01 ebenfalls 96). Einen `usb-diag`-Lauf hat dieser Auftrag nicht gemacht.
