# INPUT01 — Quittung: der Mauszeiger bewegt sich auch, während ein Programm arbeitet

Datum 2026-10-08 · Basis `desk-rebuild`, HEAD `3ffb55fc` (= Tag `ofam-2026` `10b1d302` + Doku),
Baum vorher sauber · **nicht committet** · Belege
[measurements/measurement-2026-10-08-input01/](../measurements/measurement-2026-10-08-input01/RUNINFO.TXT)
· Ticket [INPUT01](INPUT01.md)

Abschaltbar ohne Kernelwechsel: `input=aes` in `cmdline.txt` = Verhalten des Tags `ofam-2026`
(in QEMU in allen Regressionsläufen bildgleich zu `ofam-2026` nachgewiesen). Standard = neues
Verhalten, weil alle Prüfungen bestanden sind.

## Ursache (belegt, QEMU raspi4b, Injektor + Trap-Profil)

* Die Maus wurde nur in der Pumpe des Trap-Pfads abgefragt (`Host::pump_input` aus AES-Wartestellen,
  blockierende Konsolen-Lesungen, `Bconstat`); in jedem Trap sind IRQs maskiert.
* zView-Start aus EmuDESK: **830 ms** ohne Pumpe, davon **96 %** in Kernel-Traps (20 `Pexec`
  635 ms, längster 136 ms; 10 `Mshrink` 159 ms), 29 ms EL0.
* Vorschaubilder (8 Fotos 2048×1536): **180 ms** je Bild, 55 % `Fread`-Traps (je ≤ 8 ms), 45 % EL0.
* EmuTOS: IKBD-Interrupt → `mouse_int` (`bios/aciavecs.S:249`), Cursor im VBL
  (`vdi/vdi_mouse.c:602,668-686`), GEMDOS-Trap ohne Interruptmaske (`bdos/rwa.S:158-204`).

## Bau

* `kernel/src/input_gate.rs` (neu, rein, Hosttests): Modus (`input=aes`), Entscheidung
  `irq_pump_allowed` (IRQ aus EL0, Prozess läuft, kein Upcall, AES-Fiber nicht angehalten),
  `pure_motion`, `HeldBytes`, `PumpLock` (Einmal-Wächter). Moduldoku = Begründung.
* `kernel/src/drivers/gic.rs`: nach dem Timer-Tick und EOI → `host_env::irq_input_pump(spsr)`.
* `kernel/src/host_env.rs`: `pump_devices` (Trap-Seite: zurückgehaltenen Bericht dekodieren
  **statt** neu zu ziehen, sonst `usb::poll`; dann Wiederholung) für `pump_input` und beide
  Lese-Schleifen; `pump_devices_irq` (Interrupt: einen Bericht in die Erfassung ziehen);
  `irq_input_pump` (Gate).
* `kernel/src/ikbd.rs`: Erfassung `capture_begin`/`capture_end` (reine Zeigerbewegung sofort
  dekodieren, sonst zurückhalten), `release_held`; der alte Dekoder heißt `decode_byte`.
* `kernel/src/bootargs.rs` (`INPUT_AES` + Test), `kernel/src/lib.rs` (Modus beim Booten, Zeile
  `[input] …`), `tools/xtask/src/main.rs` (Hosttests von `input_gate`),
  `tos-api/src/aes/thread.rs` (`is_suspended`).
* Nur Kommentare/Spec: `usb/mod.rs`, `key_repeat.rs`, `tos-api` `aes/input.rs`, `aes/mod.rs`,
  `bios/tick.rs`, `xbios/kbdvecs.rs`; `docs/api/xbios/_state.md` (Wiederholungs-Bindung unverändert,
  benannt), `docs/api/xbios/22-kbdvbase.md` (B0-Nachtrag).

Was **nicht** gemacht wurde (Begründung `input_gate.rs`): keine IRQs während Kernelarbeit, keine
Pumpe an „sicheren Punkten“ in langen Traps (`Pexec`, SD-Lesen, memprot) — dort ist der Prozess
evtl. halb angelegt und das AES evtl. der Aufrufer; die AES-Handler laufen ohne Fork-Queue inline.
Kein IRQ-Pumpen aus EL1 (der Desktop kann mitten in einem `objc_draw` stehen).

## Prüfungen

| Prüfung | Ergebnis |
|---|---|
| Hosttests | 1261/0/1 (Basis 1249/0/1, +12 neu) |
| Release-Bau rpi4 + qemu-virt | tos-api 64 / kernel 94, 0 Fehler (= Basis) |
| Clippy rpi4 + qemu-virt | 76 / 125 (= Basis) |
| Drift / SDK-Hosttests | 31 / 18-0-0 |
| Stillstand zView-Start | 830 ms → 135 ms (ein `Pexec`) |
| Stillstand Vorschaubilder | 180 ms (7×) → 30 ms |
| BIG.JPG öffnen | 85 ms → 85 ms (ein AES-Aufruf, unverändert gewollt) |
| Regression `irq-` vs `aes-` (gleiche Karten) | bildgleich in allen Läufen außer DOOM-Demobildern |
| `aes-` vs `ofam-2026` | bildgleich bis auf Dateidaten (desk) und Uhr (desk3a), DOOM-Demobilder |
| DOOM (kbdvec) | Start, Spiel, Esc/Up/Return/Y, `rc=0`, Desktop zurück — beide Modi |
| APITEST | 106/0/5 (irq, aes, strict, Paketkernel beide Modi) |
| `memprot=strict` | desk, zView-Browser, APITEST wie `ofam-2026` |
| Serielle Logs | 0 `[EXC]`/PANIC/„Memory violation“ in 39 Logs |

## Pi-Paket

`measurements/measurement-2026-10-08-input01/bin/kernel8.img` (gitignored, sha256
`50f922a804e56748bc6d6f57de15f00c847f6873f897864f55c1b2c996347e7c`) + `bin/LIESMICH.TXT`.
Nicht auf eine Karte kopiert.

## Offen

* Nur auf der echten Pi: `usb::poll` aus dem Timer-Interrupt gegen den echten VL805/xHCI;
  Zeichenreste am Cursor bei Programmen, die direkt zeichnen ohne die Maus zu verstecken
  (EmuTOS-gleich, aber neu sichtbar, weil der Zeiger jetzt zwischen zwei VDI-Aufrufen springen kann).
* Restliche Stillstände: ein einzelner langer Trap (auf der Pi ein Codec-`Pexec` mit Kartenlesen,
  evtl. länger als QEMUs 130 ms) und Arbeit des eingebauten Desktops selbst (EL1).
* Entscheidung für Lead/Anwender: EmuTOS stellt auch Knopfwechsel aus dem Interrupt zu; hier
  bewusst nicht (Klicks behalten ihren alten Zeitpunkt). Eine Angleichung würde ändern, was ein
  gerade beschäftigtes Programm von einem Klick sieht.
* Kernel-Banner des Pakets zeigt `3ffb55fc` (nicht committet).
