# DESK43 U9-0 — Voraussetzungen U9-0a … U9-0d — Quittung

> Stand 2026-10-14 · Basis `main` @ `7708a653` (Produktcode = U8b `d7c5efa2`) · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u9-prep-2026-10-14.md` §6 (U9-0a … U9-0d), Entscheidungen `TASK/DESK43.md`
> („U9-Vorbereitung entschieden“: REG exakt bis NUL, PROC05 korrigiert, T10 Slot-Veroeffentlichung), `TASK/PROC05.md`, `TASK/T10.md`
> Messung: [`measurements/measurement-2026-10-14-desk43-u9-0/`](../measurements/measurement-2026-10-14-desk43-u9-0/RUNINFO.TXT)
> **Kein Bildschirmunterschied (0 Pixel), Orakel = U8b.** Einzige sichtbare Aenderung: die serielle Zeile
> `[shell] sh_ldapp: '…' exited with rc=N` zeigt den echten Exitcode. Nichts committet, keine SD-Karte, kein Pi; kein
> `git checkout/restore/clean/stash/reset`. `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: die vier kleinen Vorarbeiten fuer den EL0-Desktop —
Registry-Name bis NUL, Kind-Exitcode an die Shell, veroeffentlichter
Shell-Kind-Slot als Schluessel des „desktop up“-Orakels, und ein
wiederverwendbarer Injektor, der auf Programmidentitaet bewaffnet.
Driftprobe **4 → 4**, unsafe **20 → 20**.

---

## U9-0a — Registry: Name endet am ersten NUL

* `tos-api/src/bdos/process/registry.rs`: `lookup` kuerzt Schluessel **und**
  Eintragsnamen am ersten NUL und vergleicht exakt (kein Case-Folding, REG-a);
  leerer Name trifft nichts. Doku von `register`/Modul angepasst (Prep §4,
  Variante „beide Seiten“).
* Hosttests (Modul-Test): blanker Name, 128-Byte-`D.s_cmd`-Puffer, `NAME\0X`
  treffen; `NAME2`, `NAM`, Kleinschreibung, `C:\NAME`, `""`, 128×NUL nicht.
* `shell_loop.rs:434`-Weg: `&cmd` (128 Byte) findet jetzt einen Registry-Namen.
  `accs.rs:210`: `acc.path` ist `[u8; 32]` NUL-gepolstert, aber immer mit
  Laufwerk (`C:\ACC\NAME.NAC`) — trifft nie einen Registry-Namen, vorher wie nachher.
* **Heute ohne Laufzeitwirkung:** die Registry ist beim Boot leer
  (`embedded.rs`: `EMBEDDED = &[]`; die `b"C"`-Selbsttests `selftest_pexec*`
  werden von keinem Bootpfad aufgerufen). Beleg = Hosttest; die QEMU-Laeufe
  zeigen „keine Aenderung“.

## U9-0b — PROC05: Exitcode des Kindes

Siehe [PROC05-STATUS-DONE.MD](PROC05-STATUS-DONE.MD). Kurz: `finish_terminate`
(Kernelstart-Zweig) → `launch::record_kernel_child_exit` (UWORD zero-extended) →
`run_program_with_tail_rc` liefert `0 … 65535` bzw. den negativen Ladefehler;
`sh_ldapp` NORMAL/AUTORUN `ret < 0 ? ret : 0` (kein Alert), DESKTOP-Arm
unveraendert (Kernelaufruf, kein Exitcode). Spezifikation
`docs/api/aes/shell-loop.md` §8; 4 Hosttests.

## U9-0c — `launch::shell_child_slot()` und Orakelschluessel

* `kernel/src/launch.rs`: `SHELL_CHILD_SLOT` (privat) + `pub fn shell_child_slot() -> Option<usize>`;
  gesetzt unmittelbar vor `enter_el0…` (neben `[launch] EL0: slot …`),
  zurueckgesetzt sofort nach der Rueckkehr. Gilt fuer jeden Launcher-Start
  (`sh_ldapp` und `\AUTO\`).
* `kernel/src/shell_loop.rs`: `is_desktop_caller(args)` —
  Shell-Kind vorhanden → `Space::User && running_slot() == Kind` (EL0-`NEWDESK`
  ab U9); kein Shell-Kind → `Space::Kernel` (heutiger eingebauter Desktop, spaeter
  `desk=kernel`). Ein Schluessel fuer beide Formen hinter `DESKTOP_ACTIVE`, ohne
  zweites Flag. Zeilentext und Anker `kprintln!(\n                "[desk] desktop up ({} drives)",`
  **byte-gleich** (alle U8a/U8b-Injektoren und `tools/inject-arm.py` finden ihn).
* Lesezugriff im EL0-Fall: vor dem Dispatch (wie bisher), nicht nachher —
  Prep §3.4 empfahl (a) „nach dem Dispatch“; das geht fuer `evnt_multi` nicht,
  weil der Aufruf blockiert und die Zeile (und jede Injektor-Bewaffnung daran)
  erst nach dem ersten Ereignis kaeme. Stattdessen begruendet: der Dispatcher
  liest dieselben Woerter (`Aespb::from_raw`: `block[0..6]`, `control[0..5]`);
  ein schlechter Zeiger beendet das Programm genauso, das Orakel aendert sein
  Schicksal nicht (nur welcher der beiden Lesezugriffe gemeldet wird). Doku am
  Funktionskopf.
* Beleg EL1-Pfad: Orakel (G2) `SVCSEQ` 688 Zeilen / 23752 Byte, **diff leer**
  gegen U8b; „desktop up“ in allen Laeufen wie U8b.

## U9-0d — T10: `tools/inject-arm.py`

* Neues, gepflegtes Werkzeug (statt Kopien je Messlauf): patcht eine
  **Scratch**-Kopie mit einem IKBD-Injektor (gleiche Stelle wie die bisherigen,
  nach dem USB-Poll in `pump_devices`), je Skript ein Ausloeser:
  `desktop` (Kernel-Ereignis „desktop up“), `launch:NAME` (Launcher
  veroeffentlicht ein Shell-Kind mit Namen NAME — `p_nameit`-Form aus `D.s_cmd`),
  `up:NAME` (erstes `evnt_multi` dieses Kindes nach `appl_init`). Jeder Ausloeser
  einmal; Uhr startet am ersten Tick nach dem Ereignis (= Semantik der alten
  Injektoren). Skriptdatei `<ticks> <byte>…`.
* Ersetzt die `RUN.is_null()`-Bewaffnung **fuer neue Laeufe**; kein bestehendes
  Messskript angefasst.
* Benutzt in diesem Lauf: Views (`desktop`, 8/8 = U8b, Fuetterplan identisch),
  HELLOGEM (`desktop` + `launch:HELLOGEM`/`launch:HGPANIC`, 8/8 = U8b, nur der Arm
  des Programms auf der Karte feuert), TeraDesk (`up:DESKTOP` + `launch:DESKTOP`,
  beide feuern mit `slot Some(0) name DESKTOP`, t35 = U8b).

## Zahlen (RUNINFO §1)

| | erwartet | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1360/0/1 + neu | **1366/0/1** (+2 Registry, +4 PROC05) |
| Warnungen release rpi4 / qemu-virt | tos-api 64, kernel 94 | 64 / 94 beide |
| Clippy kernel rpi4 / qemu-virt / +selftest-desk | 76 / 125 | 76 / 125 |
| Driftprobe / unsafe | 4 / ≤ 20 | 4 / 20 |
| emutos-consts | gruen | 464/332/2/2/0/0/130 (= U8b) |
| SDK-Hosttests | 20/0/0 | 20/0/0 |
| build-programs, NAPs | rc 0 | rc 0, 0 Warnungen, NAPs = U8b byte-gleich |

## Beweis „keine sichtbare Aenderung“ (QEMU raspi4b, on/off/strict; RUNINFO §2-7)

* Serie 15/15 Screendumps byte-gleich U8b; APITEST **106/0/5** alle Modi, 0
  abweichende Ergebniszeilen; serielle Logs desktop/apitest/teradesk/qed 0
  abweichende Zeilen (hex-normalisiert).
* Views 8/8, Seq 2/2, HELLOGEM 8/8 byte-gleich U8b; qemu-virt-Endschirm
  byte-gleich (Log nur `[mm] kernel`-Groesse).
* Panic-Build: `exited with rc=65535` (vorher `rc=0`), kein Alert, Schirm = Desktop.

## Offen fuer U9-1 / U9-2 (und spaeter)

1. **U9-4 muss `DESKTOP_ACTIVE` um den Launcher-Start setzen** — der neue
   Schluessel erkennt den EL0-Desktop dann ohne weitere Aenderung; vorher (U9-2/-3)
   sicherstellen, dass waehrend des eingebauten Desktops nie ein Shell-Kind lebt
   (heute wahr: der Desktop startet seit U3 nichts selbst).
2. **Injektor-Name fuer den EL0-Desktop:** `launch:NEWDESK` / `up:NEWDESK` greifen,
   sobald `sh_ldapp` den Registry-Namen `NEWDESK` startet (`D.s_cmd` = `NEWDESK`).
   `desktop` bleibt der empfohlene Ausloeser fuer Desktop-Laeufe (deckt EL1 und EL0).
3. Name wird beim ersten Tick nach Erscheinen des Slots aus `D.s_cmd` gelesen;
   ruft das Kind vorher `shel_write(SHW_EXEC)`, saehe der Injektor den neuen
   Namen (in allen Laeufen nicht aufgetreten; dokumentiert im Werkzeugkopf).
4. **T10-Abnahme im Ticketsinn** (ein alter a78-Lauf nachgefahren) nicht gemacht;
   nachgefahren wurden die U8b-Views und -HELLOGEM-Laeufe mit identischen Marken.
   Ob das T10 schliesst, entscheidet der Lead (keine T10-Quittung angelegt).
5. PROC05-Werte `Pterm(1)`, `Pterm(0x12345)`, `#Z` auf Nicht-ELF nur hostgetestet.
6. U9-1 (Stack/Latenz-Instrumente) kann `launch::shell_child_slot()` und die
   `[launch] EL0`-Zeile als Stempelpunkte nutzen; `tools/inject-arm.py` liefert die
   Uhr ab „desktop up“.

## Ausserhalb des Auftrags (je eine Zeile, nicht angefasst)

* `kernel/src/launch.rs:21-27`: verwaister Doc-Kommentar („Load and run …“) vor `busy_cursor`.
* `Pexec`-Aufrufer erhalten nach Ctrl-C `-32` sign-extended statt EmuTOS' 65504 (`finish_terminate`, `pexec_caller`-Zweig).
* `docs/api/aes/shell-loop.md` §7 nennt noch `kernel::desk::reset()` (seit U8b `desktop::reset`).
