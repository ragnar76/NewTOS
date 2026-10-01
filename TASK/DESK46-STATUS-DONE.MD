# DESK46 — STATUS: DONE (2026-09-27) — Befund 1 ist das gewaehlte Verhalten, Befund 2 war ein Defekt und ist behoben

**Auftrag:** die zwei Befunde der Pi-Runde 2026-09-26 trennen — ein `.NCP` vom
Desktop „blitzt weiss", und aus EmuCON heraus friert die Maschine nach einem
`.NCP` ein (`TASK/DESK46.md`, P1).

Basis: Branch `desk-rebuild`, HEAD `4d00e83`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum (zwei Dateien). Nichts geflasht, nichts
ausgeworfen, keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`. `/Volumes/BOOT` nicht angefasst.
`docs/STATUS.md`, `TASK/DESK46.md`, `TASK/DESK43.md` und der Plan sind
unberuehrt. Die SD-Abbilder und alle Instrumentierung lagen im Job-Scratch und
sind geloescht.

Messung: [`measurements/measurement-2026-09-27-desk46/`](../measurements/measurement-2026-09-27-desk46/).

---

## 1. Befund 1 — **kein Defekt**, das Ticket schrumpft auf Befund 2

Der Textschirm wird betreten **und** das Programm kommt zum Zeichnen. Beides
steht im Bild: `measurements/measurement-2026-09-27-desk46/screendumps/a3-3-console.png` ist
der Konsolenschirm im Augenblick, in dem `C:\D46\SYSINFO.NCP` — per Doppelklick
vom Desktop gestartet — fertig ist und bevor `sh_chgrf` zurueckschaltet. Alle
sieben Ausgabezeilen stehen da, schwarz auf weiss.

Dafuer haelt ein **Scratch**-Eingriff den Schirm drei Sekunden fest; im Produkt
gibt es die Pause nicht. Wie lang der Blitz wirklich ist, misst ein zweiter Lauf
mit `D46_HOLD=0` und 200-Hz-Marken an `sh_chgrf`
(`measurement-.../serial/A2-serial.log`):

```
[d46] sh_chgrf: want=0 gl_shgem=1 t=4333     <- Schirm wird alpha (geraeumt)
[shell] sh_ldapp: starting C:\D46\SYSINFO.NCP, sh_nextapp=0, sh_isgem=0
SYSINFO: ... SYSINFO: done.
[d46] sh_chgrf: want=1 gl_shgem=0 t=4338     <- Schirm wird wieder grafisch
```

**5 Ticks = 25 ms**, Programmlauf und Ausgabe inbegriffen. Das ist genau die
Folge der Entscheidung vom 2026-09-26 (`console_pause_for_key` entfaellt,
DESK43-U3 §6) und EmuTOS-konform — nichts zu reparieren. „Zu schnell, kann ich
nicht sagen" ist bei 25 ms die einzig moegliche Antwort.

> Will der Anwender die Ausgabe doch stehen sehen, ist das **seine** offene
> Entscheidung aus U3 §6 und braucht einen EmuTOS-fremden Traeger — nicht
> dieses Ticket.

---

## 2. Befund 2 — Defekt, headless reproduziert, Ursache belegt, behoben

### 2.1 Reproduziert, im Produktpfad

Lauf `B3`: Ctrl-Z (die `#K`-Tastenabkuerzung von *File → Execute EmuCON*)
oeffnet die Konsole, `pwd` antwortet, `run sysinfo.ncp` bringt die volle
Ausgabe und `[program exited]` — und dann bricht die Eingabeaufforderung nach
`C:\` ab. `screendumps/b5-frozen-console.png` ist dieses Bild.
`kernel/src/shell.rs` war in diesem Lauf **unangetastet**; was da haengt, ist
das Produkt.

### 2.2 Die Frage des Tickets nach der Tastatur: **nein, sie reagiert nicht**

Der Kernel nimmt einen Daten-Abort und parkt: `unhandled` →
`arch::aarch64::halt()` (`kernel/src/arch/aarch64/exceptions.rs:1856-1903`).

```
[EXC] synchronous (non-SVC) exception (Data Abort (same EL)):
      ESR=0x0000000096000004 ELR=0xffff0000000d1b68 SPSR=0x0000000060000004
[EXC] FAR=0x0000000000000000
```

Der Ersatz fuer den Maus-Trenner ist ein Herzschlag aus **derselben Schleife,
die die Taste lesen wuerde** (`console_read_key_blocking`): eine Zeile je
Sekunde. In `serial/B3-serial.log` laeuft er waehrend EmuCON (`hb 5s`, `6s`,
`7s` …) und hoert mit dem `[EXC]` auf; der danach eingeplante `ver`-Befehl
kommt nie an. Es laeuft also keine Schleife mehr, die eine Taste lesen koennte.

### 2.3 Wo es stehenbleibt, und warum

Ausfuehrlich in `measurement-.../analysis/where-it-stops.txt`. Kurz:

```
[d46] finish_terminate: slot=2 from_el0=0 full=0 pf.spsr=0x80000004 rc=0
[d46]   child frame : x19=0x0 x29=0x0 lr=0x1010 elr=0x1018
[d46]   parent saved: x19=0xffff000000149fd5 x29=0xffff000000338b40 …
```

`finish_terminate` (`kernel/src/arch/aarch64/exceptions.rs`) hat die Register
des Eltern-Rahmens nur wiederhergestellt, wenn

```rust
let from_el0 = (pf_spsr & 0xF) == 0;
if from_el0 || full { … }
```

— und `parent_frame_full` setzt **nur** `adopt_kernel_park_as_parent`
(der Accessory-Fall, ACC03), `handle_pexec` setzt es nie. Fuer EmuCON, das den
GEMDOS-`Pexec`-Trap **aus EL1** aufruft (`kernel/src/shell.rs:331-347`,
`cmd_run` :120-137), traf also keiner der beiden Arme: `elr`/`spsr`/`sp_el0`
kamen zurueck, **kein einziges GPR**. EmuCON lief mit dem Registersatz des
Kindes weiter — ein soeben beendetes EL0-Programm hat x19…x29 auf 0 —, und der
erste Zugriff ueber ein callee-saved Register las aus Adresse 0. Im Produktlauf
war das `ldrb w9, [x25, x8]` mit `x25 = &buf` in `prompt()`
(`kernel/src/shell.rs:264-265`); welches Register es trifft, haengt an der
Registerbelegung, **dass** es eines trifft, nicht (drei Laeufe, drei Stellen,
`analysis/where-it-stops.txt` §3).

Zweite, stillere Folge derselben Zeile: `frame.gpr[0] = rc` lief ebenfalls nur
im `from_el0`-Arm. Ein EL1-Aufrufer bekam nie den Beendigungscode des Kindes,
obwohl `docs/api/gemdos/process/4b-pexec.md` („Trap interface"; Ergebnis bei
Modus 0/4/6 ist der Beendigungscode, geliefert ueber den gespeicherten
Trap-Rahmen des Aufrufers) ihn verlangt.

### 2.4 **Kein U3-Rueckschritt**

`git log -S"if from_el0 || full {"` nennt `03394c2` (Accessories), und der
`if from_el0`-Gate davor ist aelter (`git show 03394c2^:…`, Zeile 937). U3
(`e0069e5`) und U4 (`5a485be`) fassen weder `exceptions.rs` noch `process.rs`
noch `shell.rs` an. EmuCONs `run` gibt es seit `a3c4329`; vor U3 war es ueber
`sh_run_pending_console` erreichbar, seit U3 ueber `sh_ldapp`s
`CONSOLE_APP`-Arm — der Weg dorthin hat sich geaendert, der Defekt am Ende
nicht. Die zweite genannte Nachbarschaft (PORT05) ist ebenfalls **nicht** die
Ursache: PORT05s `suspend_parent_for_child`/`resume_parent_of_child` laufen
hier korrekt, der Fehler sitzt eine Ebene tiefer im Registerrestore.

---

## 3. Der Fix

Zwei Dateien, **+80 / −30** Zeilen (`method/product-desk46.diff`).

### 3.1 `tos-api/src/bdos/process.rs` — `Process::parent_frame_pexec`

Ein neues, ausdrueckliches Kennzeichen: „dieser `parent_frame` ist der
Trap-Rahmen eines **`Pexec`-Aufrufers**". Mit Initialisierung und Ruecksetzung
in `free_slot` (`tos-api/src/bdos/process.rs:944-960`) neben
`parent_frame_full`.

Es gibt drei Arten von `parent_frame`, und `SPSR.M` kann nur zwei davon
trennen, weil zwei von ihnen `EL1t` sind. Genau daran ist der Defekt
gehangen; deshalb sagt jetzt der **Schreiber** des Rahmens, welche Art es ist:

| Art | gesetzt von | Restore |
|---|---|---|
| `Pexec`-Aufrufer (EL0 **und** EL1) | `handle_pexec` | alle GPR + `lr`, `x0` = Beendigungscode |
| adoptierter Kernel-Park (Accessory, ACC03) | `adopt_kernel_park_as_parent` | alle GPR + `lr`, `x0` **unveraendert** (der `svc` laeuft nochmal) |
| Kernel-Start (`crate::launch`, Selbsttests) | `enter_el0_with_resume_capture` | nur `elr`/`sp_el0`/`spsr` |

### 3.2 `kernel/src/arch/aarch64/exceptions.rs`

* `handle_pexec` setzt `parent_frame_pexec = true`, sobald es einen Start
  umleitet — gleich von welcher Ausnahmestufe der Aufrufer kam.
* `adopt_kernel_park_as_parent` setzt es ausdruecklich auf `false` (die beiden
  Arten schliessen sich aus; wer den Rahmen schreibt, sagt beides).
* `finish_terminate` entscheidet an den beiden Kennzeichen statt an
  `SPSR.M`: `if pexec_caller || full { <GPR + lr> ; if pexec_caller { gpr[0] = rc } }`.
  Der Park-Abbau (`set_kernel_parked(false)`) bleibt am `full`-Arm, wo er
  hingehoert. `from_el0` wird in `finish_terminate` nicht mehr gebraucht und
  ist dort weg (in `handle_pexec` bleibt es: dort unterscheidet es weiter
  `ldtrb` von gewoehnlichen Ladebefehlen beim Lesen der Argumente).

Der lange Kommentarblock ueber der Stelle ist entsprechend neu geschrieben: er
beschrieb bisher eine Fallunterscheidung nach `SPSR.M`, die es nicht mehr gibt.

---

## 4. Was nachgewiesen ist (headless, QEMU `raspi4b` 640×480)

| Lauf | Kernel | Ergebnis |
|---|---|---|
| `B3` | **ohne** Fix, `shell.rs` unberuehrt | Daten-Abort + `halt()` nach `Pexec`; `screendumps/b5-frozen-console.png` |
| `B4` | **mit** Fix, gleiches Skript, gleiches Abbild | `pexec_caller=1` → `[program exited]` → Eingabeaufforderung → `ver` antwortet → `exit` → `[desk] desktop up`; **0** Treffer auf `EXC\|PANIC\|panic` |
| `A2` | mit Fix, `D46_HOLD=0` | Desktop-Startweg unveraendert: `finish_terminate` meldet fuer `\AUTO\HELLO.NCP` **und** `C:\D46\SYSINFO.NCP` `pexec_caller=0 full=0` (der Teilrestore), Desktop kommt mit seinem Fenster zurueck |
| `A3` | mit Fix, `D46_HOLD=600` | Befund-1-Bild (`a3-3-console.png`) + Desktop danach (`a3-4-desktop-back.png`) |
| `qemu-virt-boot` | Produktbaum, Board `qemu-virt` | bootet bis `[desk] desktop up (0 drives, …)`, **0** `[EXC]` |

`B3` gegen `B4` ist die Vorher-/Nachher-Probe: gleiches Tastenskript, gleiches
Kartenabbild, einziger Unterschied der Fix.

---

## 5. Die Zahlen

| | |
|---|---|
| Bauen `--board rpi4`, frisch | tos-api **64** / kernel **94** (Basis 64/94, **Delta 0**), rc 0 |
| Bauen `--board qemu-virt`, frisch | tos-api **64** / kernel **94** (Basis 64/94, **Delta 0**), rc 0 |
| clippy, beide Feature-Saetze | tos-api **80** / kernel **125** (Basis 80/125, **Delta 0**) |
| clippy-Treffer in `exceptions.rs` / `process.rs` | **0** |
| `cargo run -p xtask -- test` | **956 / 0 / 1** (Basis 956/0/1) in zwei von drei Laeufen |
| `sh tools/desk-drift-probe.sh` | **32** (Basis 32) |

Der dritte Testlauf zeigte
`sched_pexec_parent_suspend::blocking_child_never_yields_to_its_own_parent`
als Fehlschlag — das ist **`TASK/T11`**, nicht diese Aenderung: der Test laeuft
auf einer reinen `sched`-Fixture, die der geaenderte Code nicht beruehrt, und
die beiden folgenden Laeufe sind sauber. Protokoll:
`measurement-.../build/xtask-test-T11-flake.log`.

---

## 6. Was die echte Pi 4 zeigen muss

1. Aus dem Desktop *File → Execute EmuCON* (oder Ctrl-Z).
2. `run c:\sysinfo.ncp` (oder `run sysinfo.ncp`, wenn ein Fenster offen war —
   die Konsole startet im Verzeichnis des obersten Fensters).
   * **Erwartet:** die Ausgabe, darunter `[program exited]`, und danach eine
     **vollstaendige** Eingabeaufforderung `C:\…>`, auf die die Tastatur wieder
     antwortet.
   * **Der alte Fehler sah so aus:** die Eingabeaufforderung bricht nach `C:\`
     ab und nichts geht mehr — `screendumps/b5-frozen-console.png`.
3. Noch ein Kommando tippen (`ver`), dann `exit`.
   * **Erwartet:** der Desktop kommt komplett neu gezeichnet zurueck.
4. Zur Gegenprobe ein `.NCP` vom Desktop doppelklicken: kurzes Weiss mit der
   Programmausgabe (~25 ms), dann sofort der Desktop. Das ist **richtig** so.

---

## 7. Offene / gemeldete Punkte

1. **Kein automatischer Regressionstest fuer diesen Pfad.** Der Restore liegt
   in `kernel/src/arch/aarch64/exceptions.rs` und ist auf dem Host nicht
   uebersetzbar; die Vorher-/Nachher-Probe ist der QEMU-Lauf `B3`/`B4`. Ein
   `selftest_pexec_el1`-Modul (Sentinel in x19…x28, `svc #1` mit `x8 = 0x4B`
   auf ein Registry-Programm, Vergleich innerhalb desselben `asm!`-Blocks)
   waere die richtige Form — aber **die vorhandenen `selftest_pexec*`-Module
   haengen in keinem Bootpfad** ([[unwired-selftests-rot-silently]]), ein
   weiteres unverdrahtetes verrottet mit. Verdrahten aendert das Bootverhalten
   jedes Laufs; das gehoert entschieden, nicht nebenbei gemacht. **Gemeldet,
   nicht gebaut.**
2. **FP/SIMD-Register ueberleben `Pexec` fuer keinen Aufrufer.** Der
   Eltern-Schnappschuss (`ParentFrame`) hat nur GPR/`lr`/`elr`/`spsr`/`sp_el0`;
   `saved_fp` gehoert dem Scheduler. Das betrifft EL0-Aufrufer seit jeher
   genauso und ist nicht Teil dieses Befunds — aeltere Luecke, unberuehrt.
3. **`parent_frame_full` heisst inzwischen missverstaendlich** („whole frame")
   und meint in Wahrheit nur den adoptierten Kernel-Park. Der Doc-Kommentar ist
   entsprechend praezisiert; ein Umbenennen waere eine eigene Stufe.

## Out-of-scope-Beobachtungen (unberuehrt, je eine Zeile)

* `cargo run -p xtask -- build --board qemu-virt` erzeugt **kein**
  `kernel8.img` (nur `--board rpi4` tut das, `tools/xtask/src/main.rs:170-178`)
  — fuer die Rauchprobe von Hand mit `llvm-objcopy` erzeugt.
* `kernel::shell::execute` kennt kein blosses `<programm>` als Kommando; ein
  Programm laeuft nur ueber `run <datei>` (der Anwenderbericht „konnte
  `sysinfo.ncp` aufrufen" meint also `run sysinfo.ncp`).
* `crate::shell::cmd_run` uebergibt `Pexec` weder Kommandoschwanz noch
  Umgebung (`x2 = x3 = 0`), waehrend EmuTOS' EmuCON den Rest der Zeile als
  Schwanz weiterreicht (`cli/cmdexec.c:52`, `Pexec(0,path,cmdline,NULL)`) — aeltere Luecke, nicht Teil dieses
  Befunds.
* `docs/STATUS.md` ist auf Weisung des Leads **nicht** angefasst; der Eintrag
  zu dieser Aenderung fehlt dort noch.
