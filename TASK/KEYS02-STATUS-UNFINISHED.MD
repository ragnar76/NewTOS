# KEYS02 — Zwischenstand

**Status: KERNTEIL IMPLEMENTIERT · in QEMU raspi4b belegt · Doom-Seite offen ·
Abnahme durch den Tester offen · echte Pi offen** · 2026-10-04 · Basis
`desk-rebuild`, Baum vorher sauber, nicht committet · Belege
[measurements/measurement-2026-10-04-keys02/](../measurements/measurement-2026-10-04-keys02/RUNINFO.TXT)
(Aenderung als Diff: `method/keys02-change.diff`)

Ein Programm kann sich jetzt an den Tastaturvektor haengen: `Kbdvbase()`
liefert die eigene Tabelle des Prozesses, `kbdvec` liegt 8 Byte davor, und ein
eingehaengter Haken sieht jedes Make **und** jedes Break als Scancode. Verkettet
er zum alten Vektor, laufen BIOS-Schlange, AES-Tastenereignisse und
Wiederholung weiter wie bisher. Programme ohne Haken merken nichts.

Umfang = D7-Minimum der freigegebenen Spec
[`docs/api/xbios/22-kbdvbase.md`](../docs/api/xbios/22-kbdvbase.md)
(Entscheidungen D1–D8; was gebaut ist: Teil C).

## Was sich geaendert hat

* **Neu `tos-api/src/xbios/kbdvecs.rs`:** die Tabelle und ihre Vorgaben, der
  Ring je Prozess, die Entscheidung Vorgabe/Haken, `Kbdvbase`.
  * Die Tabelle liegt in Seite 0 jeder TPA bei `0x9A0`, die Slots sind 8 Byte
    breit. `Kbdvbase()` liefert `0x9B0`.
  * Alle Slots zeigen auf gueltige EL0-Stubs bei `0xA00`/`0xA08`. Der Lader
    schreibt sie bei jedem Laden (`write_defaults`).
  * Ohne laufenden Prozess antwortet eine Kernel-Tabelle gleichen Aufbaus.
* **D6 — Nummer:** Der `kbdvec_default`-Stub ruft **`Ssystem` Modus 2002**
  (`S_NEWTOS_KBDVEC`). Das ist die naechste freie Nummer im NewTOS-eigenen
  Bereich 2000+. Der Stub fuehrt den bisherigen Tastendekoder aus.
  Dokumentiert in `docs/api/gemdos/154-ssystem.md` und `ssystem.rs`.
  `gemdos_ssystem_cpuinfo.rs` prueft als freie Nachbarnummer jetzt 2003 statt
  2002.
* **`kernel/src/ikbd.rs`:**
  * `feed` fragt fuer jedes Tastenbyte `route_key`.
  * Steht die Vorgabe noch im Slot (oder laeuft kein Prozess), dekodiert es
    direkt wie bisher. Sonst kommt das Byte in den Ring (64 Plaetze, bei vollem
    Ring wird das neueste verworfen).
  * Neu ist `kbd_int` (= EmuTOS `kbd_int`).
* **`kernel/src/arch/aarch64/exceptions.rs`:** Zustellung mit
  `deliver_pending_kbdvec`.
  * Zugestellt wird an jeder Rueckkehr nach EL0: Syscall-Ende, Resume nach
    Scheduler-Wechsel, Resume-`Psigreturn`.
  * Je Upcall geht ein Byte an den Haken, mit `x0` = Scancode und `x1` = 0.
  * Neues Maskenbit `UPCALL_MASK_KBDVEC = 1 << 3`, Tiefe 1.
  * Das naechste Byte folgt beim `Psigreturn` des Hakens. So kommen ein Make
    und sein Break aus demselben Poll beide an, in Reihenfolge (das haette
    `post_upcall`, „last writer wins", nicht geschafft).
* **O1 — war nicht gegeben, ist gebaut.** Eine blockierende AES-Warte und die
  blockierenden Einzeltasten-Lesungen kehrten nie nach EL0 zurueck, solange die
  BIOS-Schlange leer war. Bei einem verkettenden Haken fuellt aber erst der
  Haken die Schlange: das waere eine Verklemmung. Jetzt weichen sie aus:
  * `sched::sleep` liefert `Yield(switch_restart_for_upcall())`.
  * `Cconin`/`Cnecin`/`Crawcin`/`Bconin(2)` nutzen den neuen
    `Host::console_read_key_restartable` und rufen bei `None`
    `kbdvecs::request_restart()`.
  * Der Trap-Handler bricht den Aufruf wie einen S2-Yield ab (`elr` auf das
    `svc` zurueck) und stellt den Haken zu. Danach laeuft das `svc` neu.
* **D4:** `KeyRepeat` merkt sich `ikbdsys` beim Make. Die Wiederholung stoppt
  bei einem Wechsel, so wie `ikbd.c:453-472`. `ikbdsys` wird nie gerufen.
* **D8:** `free_slot` leert den Ring. Tabelle und Stubs verschwinden mit der
  TPA.
* Weitere Dateien:
  * `process.rs`: Maskenbit, `free_slot`.
  * `sched.rs`: `Switch::upcall_restart`.
  * `host.rs`: `kbd_int`, `console_read_key_restartable`.
  * `kernel/src/host_env.rs`: gemeinsame Leseschleife `read_key_loop`.
  * `loader.rs`: Bereichskonstanten und Aufruf.
  * `xbios/system.rs`: alte Null-Tabelle entfernt.
  * `xbios/mod.rs`: Verweise.
  * Kommentare in `keytbl.rs`, `aes/init.rs`, `bios/vectors.rs`.

## Spec und Doku

* `docs/api/xbios/22-kbdvbase.md`: Statuskopf und neuer **Teil C (Bau)**.
  * Abweichung zu Teil B: die Kernel-Tabelle fuer den Fall „kein Prozess".
    B1 regelt diesen Fall nicht, er ist hier ergaenzt.
  * O1 als geklaert markiert.
* `docs/api/xbios/_state.md`:
  * §1 ist korrigiert (`drvstat` gehoert zum Vertrag) und hat jetzt die
    NewTOS-Bindung.
  * Im Abschnitt zur Wiederholung steht der `ikbdsys`-Stopp jetzt als
    nachgebildet, nicht mehr als „nicht nachgebildet".
* `docs/api/gemdos/154-ssystem.md`: Modus 2002.
* `docs/COMPATIBILITY.md` §„Sanctioned deviations": neuer Eintrag. Die Tabelle
  ist je Prozess, ein Haken endet mit seinem Prozess, und der Haken wird beim
  Syscall-Ruecklauf gerufen statt im Interrupt (D8).
* `docs/STEP2-VALIDATION.md`: K4 steht auf PARTIAL.
* `docs/STATUS.md` ist **nicht** angefasst (Vorgabe). Den Text fuer „Blocked"
  liefert der Bericht an den Lead.

## Ergebnisse

| Messung | Soll | Ist |
|---|---|---|
| Hosttests (`cargo test -p tos-api -p tos-types -p xtask --no-fail-fast`) | 997/0/1 + neu | **1010/0/1** (+10 `xbios_kbdvecs`, +3 `kernel_key_repeat` `ikbdsys`) |
| Release-Bau nach `cargo clean -p kernel -p tos-api --target … --release`, qemu-virt | 64 / 94 | **64 / 94** |
| dto. rpi4 | 64 / 94 | **64 / 94** |
| clippy qemu-virt (tos-api / kernel) | 80 / 125 | **80 / 125** |
| clippy rpi4 | 80 / 125 | **80 / 125** |
| `sh tools/desk-drift-probe.sh` | 31 | **31** |

Keine Clippy-Meldung in einer geaenderten Datei. Ein `% 8 == 0` in
`loader.rs` fiel als `manual_is_multiple_of` auf und ist behoben.

QEMU raspi4b (`RUNINFO.TXT`), Programm `sdk/examples/kbdvec.c`, Start per
`#Z`, Injektor nur in einer Scratch-Kopie:

* Eingespeist wurden Hoch-Make, Rechts-Make, Rechts-Break und Hoch-Break, die
  beiden Breaks **im selben Poll**. Danach 'a'-Make und -Break.
* Der Haken sah `48 4D CD C8 1E 9E`: genau diese Bytes, in Reihenfolge. Das
  zweite Break kam ueber die `Psigreturn`-Kette.
* `evnt_multi(MU_KEYBD)` **ohne Timer** bekam `4800`/`4D00` samt
  KEYS01-Wiederholungen. Die Verkettung funktioniert also. Wiederholungen
  erreichen den Haken nicht.
* `Bconin(2)` gab `001E0061` zurueck. **PASS.**
* Danach im Desktop oeffnet Alt-C das Fenster C:\ (Schirmabzug t27). Das gilt
  auch im Lauf 2, in dem das Programm seinen Haken **nicht** zuruecksetzt
  (D8).

## Fuer den Tester

* **Beide Boards:** booten, normale Selftests, Schirmabzuege. Erwartet ist
  keine Aenderung, denn ohne Haken nimmt jedes Tastenbyte den alten Weg.
* **Haken-Weg:** nur ueber den Injektor in QEMU raspi4b
  (`method/apply-instrumentation.py` auf eine Scratch-Kopie, Lauf wie in
  `RUNINFO.TXT`). Erwartet wird `KBDVEC: PASS`. qemu-virt hat keine
  IKBD-Quelle.
* **Nur echte Pi:** USB-Tastatur mit eingehaengtem Haken und Doom (naechster
  Schritt).

## Offen

1. **Doom-Seite (naechster Agent):** Doom haengt sich an `kbdvec`, verkettet und
   wertet Make/Break direkt aus statt der Wiederholungs-Heuristik.
   * Der Haken darf **keine FP/SIMD-Register** benutzen, denn ein Upcall
     sichert sie nicht (siehe 3).
   * Abnahme wie in `KEYS02.md`.
2. **Lesungen ohne Ausweichen:** `Cconrs`, konsolen-`Fread` und
   `v_string` (Anfragemodus) weichen nicht aus. Sie haben vor ihrem Ende
   bereits Wirkung (Echo, Puffer), ein Neustart wuerde Eingaben verlieren.
   * Ein Programm, das mit eingehaengtem, verkettendem Haken darin wartet,
     haengt.
   * Modale AES-Schleifen (`form_do`, Menue, Ziehen) stellen erst an ihrem Ende
     zu, mit 64 Plaetzen Puffer.
3. **FP/SIMD bei Upcalls:** `deliver_upcall` sichert nur die GPRs. Das gilt
   seit jeher fuer alle Upcalls (Rad, Userdef, `etv_term`). Bei `kbdvec`
   tritt es aber bei jedem Tastendruck auf.
   * Ein C-Haken, den der Compiler mit SIMD-Registern uebersetzt, kann
     `v0-v7`/`v16-v31` des unterbrochenen Codes zerstoeren.
   * Nicht gebaut; eigene Entscheidung (Speicher je Slot).
4. **D7-Luecke:** Die uebrigen Slots und `mousexvec` werden nicht gerufen. Den
   „Blocked"-Eintrag setzt der Lead.
5. TT TOS 3.06 hat `kbdvec` nur abgeleitet (O3, Spec). Fuer die Doom-Abnahme
   auf der TT relevant.
