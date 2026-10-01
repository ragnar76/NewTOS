# PORT05 — STATUS: ERLEDIGT (2026-09-25)

**Auftrag:** messen, **wo** der Start stehenbleibt, wenn eine EL0-Anwendung
(TeraDesk) eine EL0-Anwendung (QED) startet — nicht raten; vorher klären, ob
sich der Effekt in QEMU `raspi4b` reproduzieren lässt.

Basis: Branch `desk-rebuild`, HEAD `c7270e4`. Der Änderungssatz liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen, keine
Pi gefahren, nichts committet, kein `git checkout/restore/clean/stash/reset`
auf dem gemeinsamen Baum; die SD-Karte unter `/Volumes/BOOT` wurde nicht
berührt.

Messung: [`measurements/measurement-2026-09-25-port05/`](../measurements/measurement-2026-09-25-port05/).

---

## 1. Das Messproblem war keins

Der Auftrag rechnete mit einer Brotkrumen-Instrumentierung, weil die Pi keine
serielle Aufzeichnung hat. Die war **nicht nötig**: der Effekt reproduziert
sich in QEMU `raspi4b` — **ohne** IKBD-Injektor, **ohne** Kratzbaum-Kopie und
**ohne** einen einzigen geskripteten Klick. Er braucht nur zwei Dinge:

* eine EL0-Anwendung, die `Pexec` ruft und danach weiterlebt, und
* ein Kind, das in einem AES-Warteruf blockiert.

Beides lässt sich in 80 Zeilen C bauen (`method/pexpar.c`, `method/pexkid.c`,
gebaut mit dem Rezept aus `sdk/apps/chaintest/build-chaintest.sh`, also mit
derselben Toolchain, denselben Headern, demselben `crt0`, `gemlib.a` und
`libc.a` wie TeraDesk und QED). Gestartet wird über die `#Z`-Autorun-Zeile in
`C:\NEWDESK.INF` — derselbe Mechanismus, der TeraDesk startet. Alles meldet
sich über `Cconws`, also über die serielle Konsole.

Damit ist auch die Frage des Tickets beantwortet: **es reproduziert in QEMU**,
und zwar deterministisch und in 16 Sekunden.

## 2. Der Befund

`serial/before-toychild.log`, gemeinsamer Baum wie er ist:

```
[PAR] 4 calling Pexec(0, C:\PEXKID.NAP)
[exec] preparing program (slot 3)
[KID] 3 entering evnt_timer(1500)      <- das Kind blockiert
[sched] switch -> slot 2               <- Slot 2 IST DER ELTER
[PAR] 1 main entry                     <- der Elter wird NEU GESTARTET
[PAR] 4 calling Pexec(0, C:\PEXKID.NAP)
[exec] preparing program (slot 4)      <- und startet das Kind noch einmal
```

Mit dem **echten QED** (`serial/before-qed.log`): **14 Neustarts** des Elters,
QED in die Slots 3…15 geladen, bis die Prozesstabelle (16 Slots) voll ist; der
14. `Pexec` antwortet `rc=0xffffffffffffffd9` = **−39 = ENSMEM**.
`screendumps/before-qed-up.png` zeigt, wie das aussieht: das Banner des Elters
rollt den Bildschirm hoch, QED kommt nie dazu, seine Menüleiste zu zeichnen.

**Der Weg ist damit belastet, und zwar genau an der Stelle, die das Ticket
vermutet hat — nur nicht aus dem vermuteten Grund.** Es ist nicht ARCH02
(FP/SIMD): kein Register geht verloren. Es ist der Scheduler.

## 3. Die Ursache

Drei Sätze, jeder am Quelltext nachlesbar:

1. **Jedes EL0-Programm, das einmal in einem `evnt_*` blockiert hat, ist
   danach `runnable`.** `sched::sleep` setzt das Bit im Leerlauf-Ausgang (a)
   auf jedem Durchgang wieder, also steht es, wenn der Warteruf endlich
   zurückkehrt (`tos-api/src/bdos/process/sched.rs`).
2. **`handle_pexec` hat den Elter in der Laufmenge stehen lassen.** Es hat den
   Rahmen auf das Kind umgebogen, `RUN`, `parent_slot`, `parent_l0_pa` und
   `parent_frame` gesetzt — aber nichts daran geändert, dass der Elter ein
   Kandidat von `pick_next` bleibt. Sein `has_ctx` ist `false` (er hat nie an
   einen Peer abgegeben), also ist er ein **`fresh`**-Ziel.
3. **Der erste blockierende AES-Aufruf des Kindes gibt ihm die CPU.**
   `sleep` → `on_yield(child)` → `pick_next` → der Elter → `apply_switch` mit
   `fresh == true`: der Elter wird **an seinem Einsprungpunkt** neu betreten.

Das ist keine Geschmacksfrage, sondern ein gebrochener Kontrakt. GEMDOS
`Pexec` ist **synchron**: EmuTOS' `proc_go` setzt `run = p` und springt über
`gouser()` ins Kind (`references/emutos/bdos/proc.c:536-577`); die Register
des Elters liegen eingefroren in seiner PD, bis `xterm` des Kindes
`run = run->p_parent` macht und `termuser()` ruft (`:601-618`). EmuTOS' AES-
Rundlauf kann den Elter nicht einmal sehen, weil startende und gestartete
Anwendung sich **eine** AESPD teilen — genau so steht es in unserer eigenen
Analyse: *„GEMDOS `Pexec` … hat keine Verbindung zum AES-`dsptch()`-Scheduler.
Während die Kindanwendung läuft, laufen Accessories/Screen-Manager nicht"*
(`docs/analysis/coop-kernel/emutos-cooperative-model.md` §5.2).

NewTOS gibt Elter und Kind **getrennte** Slots. Damit muss die Regel
ausgesprochen werden, die EmuTOS geschenkt bekommt.

## 4. Der Fix

| Datei | Was |
|---|---|
| `tos-api/src/bdos/process.rs` | neues Feld `Process::parent_runnable` (+ Init in `empty()`, + Rücksetzen in `free_slot`) — am **Kind**-Slot, wie `parent_frame`/`parent_l0_pa`, weil es Zustand *dieses einen Starts* ist |
| `tos-api/src/bdos/process/sched.rs` | neu: `suspend_parent_for_child(child_slot)` und `resume_parent_of_child(child_slot)` — die Politik gehört in den Scheduler, der Trap-Handler sieht weiterhin nur einen `Switch` |
| `kernel/src/arch/aarch64/exceptions.rs` | `handle_pexec`: auf dem Startpfad den Elter aus der Laufmenge nehmen; `finish_terminate`: ihn **vor** `release_slot` genau so zurücklegen, wie er vorgefunden wurde |
| `tos-api/tests/sched_pexec_parent_suspend.rs` | neu, 6 Hosttests |

Gespeichert und zurückgegeben wird das **Bit**, nicht ein erzwungenes `true`:
ein Elter, der nie in der Laufmenge war (ein vom Kernel gestartetes Programm,
das noch in keinem `evnt_*` blockiert hat), wird nicht dadurch zum Peer, dass
er ein Kind gestartet hat. `wait_cond` wird beim Aussetzen gelöscht, sonst
könnte `wake()` den ausgesetzten Elter hinter dem Rücken des Schedulers wieder
scharf machen — das wäre dasselbe Loch noch einmal.

**Accessories laufen weiter wie bisher.** Zurückgezogen wird genau ein
Prozess, der Elter; ein geweckter Accessory ist für das blockierte Kind
weiterhin ein Peer (eigener Test).

## 5. Gemessen nach dem Fix

`serial/after-toychild-roundtrip.log` — der volle Rundweg, **einmal**:

```
[exec] preparing program (slot 3)
[KID] 3 entering evnt_timer(1500)
[KID] 4 evnt_timer returned
[KID] 5 (no appl_exit in this build), Pterm(0)
[PAR] 5 Pexec returned rc=0x0000000000000000
[PAR] 6 second evnt_timer returned
```

`serial/after-qed.log` + `screendumps/after-qed-up.png` — mit dem **echten
QED**: **ein** `[exec] preparing program (slot 3)`, **ein** `[PAR] 1 main
entry`, und QED steht mit seiner vollen Menüleiste (`qed Datei Bearbeiten
Fenster Suchen Spezial Optionen Shell`) und Pfeilzeiger da, während der Elter
im `Pexec` ausgesetzt bleibt. Kein `[EXC]`, kein `[PANIC]` in irgendeinem Lauf.

Das Vorher/Nachher ist ein echtes A/B: die „Vorher"-Läufe liefen mit einem
Kern, in dem die zwei neuen Aufrufstellen **von Hand** im Arbeitsbaum
stillgelegt waren (bearbeiten, bauen, fahren, aus einer Kopie im Job-Scratch
zurückschreiben — nie `git checkout/restore`). Der Rückbau ist per sha1
belegt: `1eb7468a8df7586fbd1fed2513af561db5cc07f0`, ohne Scratch-Marker.

## 6. Ein zweiter, **unabhängiger** Defekt — gefunden, nicht behoben

`appl_exit()` schickt AC_CLOSE an jedes Accessory und blockiert dann in
`wait_for_accs(AP_ACCLOSE)`, das auf der Weckklasse **WAIT_INPUT** schläft
(`tos-api/src/aes/acc.rs:164`). Die Accessory-Schranke treibt diese Klasse
nicht: `idle_wake_pass` weckt sie nur, wenn eine Taste kommt oder sich der
Zeiger bewegt. In einem headless-Lauf passiert beides nie, also bekommt das
Programm die CPU nicht zurück.

**Gemessen, nicht geschlossen, dass das nichts mit PORT05 zu tun hat:** ein
vom **Kernel** gestartetes Programm (kein `Pexec` im Spiel) bleibt an genau
derselben Stelle stehen, und zwar

* `serial/control-kernel-launch-unfixed.log` (ohne den Fix) und
* `serial/control-kernel-launch-fixed.log` (mit dem Fix)

sind **bis auf die Uhrzeile identisch**. Beide neuen Aufrufe kehren sofort
zurück, wenn der Slot keinen Elternslot hat — genau der Fall auf diesem Pfad.

Vor dem Fix war dieser Stillstand im `Pexec`-Fall **verdeckt**, weil die
Neustartschleife weiter Ausgabe erzeugte. Interaktiv fällt er nicht auf, weil
eine bewegte Maus die Klasse weckt — deshalb hat ihn bisher niemand gesehen.
**Das gehört in ein eigenes Ticket.** (`serial/control-kernel-launch-serial-
byte.log`: ein eingespeistes serielles Byte hat den Lauf nicht freigegeben; ob
das Byte im Gast ankam, ist **nicht** belegt — die Aussage über die
Weckklasse steht auf dem Quelltext, nicht auf dieser Messung.)

## 7. Was **nicht** belegt ist

* **Der Einfrierer der Pi selbst.** Reproduziert ist die *Ursache* — die
  unbegrenzte Neustartschleife bis ENSMEM —, nicht der harte Stillstand mit
  toter Maus. In QEMU gab es kein `[EXC]`/`[PANIC]`. Auf der Pi kommt oben
  drauf, was das Messprogramm nicht tut: ein neu gestartetes TeraDesk lädt
  seine RSC neu, baut seine Fenster neu auf, belegt Pool-Speicher neu — 14 mal.
  Dass das in einem `halt()` endet (und damit in genau „auch die Maus bewegt
  sich nicht mehr", weil `unhandled()` → `halt()` alles anhält), ist
  **plausibel und unbewiesen**.
* **TeraDesk selbst** wurde nicht gefahren. Der Reproduzierer ist TeraDesks
  Handlung (chdir + `Pexec` + am Leben bleiben), nicht TeraDesk.
* **`TERADESK.INF`.** Der vom Ticket verlangte Rückbau ist nicht gemacht
  worden und bleibt offen — er ist durch diesen Befund aber unwahrscheinlich
  geworden: der Defekt tritt ohne TeraDesk und ohne dessen `.INF` auf.
* **ARCH02** (FP/SIMD auf dem `Pexec`-Weg) ist **nicht** berührt und bleibt
  offen. Es war hier nicht beteiligt.

## 8. Die Zahl, die auf der Pi zeigt, dass es gewirkt hat

Aus TeraDesk QED starten. **QED kommt genau einmal hoch, mit seiner eigenen
Menüleiste, und das System bleibt bedienbar** (Maus bewegt sich, TeraDesks
Menü reagiert nach QEDs Ende). Vorher: QED erscheint gar nicht oder nur kurz,
TeraDesk zeichnet sich mehrfach neu, dann Stillstand.

Wenn eine Zahl gewünscht ist: **QED darf genau einen Prozessslot belegen.**
Vor dem Fix waren es in QEMU 13 (Slots 3…15) plus ein `Pexec`, das mit −39
(ENSMEM) scheitert.

## 9. Bau + Prüfung

| | Ergebnis |
|---|---|
| `cargo xtask build --board rpi4` (frisch) | OK — `tos-api` 64, `kernel` 94 Warnungen (Grundstand) |
| `cargo xtask build --board qemu-virt` (frisch) | OK — `tos-api` 64, `kernel` 94 Warnungen (Grundstand) |
| `cargo clippy` `board-rpi4` / `board-qemu-virt` | 80 / 125 auf **beiden** (Grundstand, ±0) |
| `cargo run -p xtask -- test` | **950 passed / 0 failed / 1 ignored** (944 + 6 neue) |

Belege: `measurements/measurement-2026-09-25-port05/build/`.

## 10. Verifikationsübergabe an den Tester

* **Greift:** `board-rpi4`, QEMU `raspi4b`. Kommando steht in
  `method/` — `seed-pex.sh` + `run-pex-shot.sh`; ein Lauf dauert ~40 s.
* **Erwartet:** in `after-qed.log` genau **ein** `[exec] preparing program`
  und genau **ein** `[PAR] 1 main entry`; im Screendump QEDs Menüleiste.
* **Neuer Hosttest:** `tos-api/tests/sched_pexec_parent_suspend.rs`, läuft
  in `cargo run -p xtask -- test` mit (6 Fälle, keine Verdrahtung nötig).
* **Kein neuer Selftest** im Kernel — der Fall braucht einen EL0-Elter, der
  vorher in einem AES-Warteruf blockiert hat; das ist in `selftest_pexec`
  nicht darstellbar, ohne die AES mitzuziehen.
* **Regressionsfläche:** Accessory-Umschaltung (ACC02/ACC03) und der
  Kernel-Startweg (`launch.rs`) — beide gehen durch dieselben zwei neuen
  Aufrufe, die dort aber **no-ops** sind (kein Elternslot). Der
  Kontrollbeleg dafür liegt als Paar identischer Logs in `serial/`.
* **Nur echte Pi:** der harte Einfrierer (§7) und die Abnahme mit dem
  echten TeraDesk.
