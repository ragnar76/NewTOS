# A28 — Quittung

**Zustand: UNFINISHED** · 2026-09-14 · Commit `d93e115`

**UNFINISHED heißt hier: die Hardwareabnahme fehlt, nicht die Implementierung.**
Die fünf Abnahmepunkte des Tickets sind erfüllt; offen bleibt die eine Geste, die
nur eine echte Pi 4 ausführen kann (Abschnitt 6).

Gepinnte EmuTOS-Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.
NewTOS-Basis: `desk-rebuild`, HEAD vor der Arbeit `9bb9157`, sauberer Baum.
Messverzeichnis: [`measurements/measurement-2026-09-14-a28/`](../measurements/measurement-2026-09-14-a28/RUNINFO.TXT).

---

## 1. Die beiden Rücksetzungen, an der jeweils entsprechenden Stelle (Punkt 1)

Zeilennummern selbst nachgeprüft, die des Tickets waren verschoben.

| EmuTOS | Inhalt | NewTOS |
|---|---|---|
| `desk/deskmain.c:2159-2166` | `menu_bar(NULL,0)`; `wind_set(DESKWH, WF_NEWDESK, NULL,0,0)`; **`appl_exit();`** | `desk::main::desktop_relinquish` — die ersten beiden standen schon da, `super::appl_exit()` ist **neu** und steht **zuletzt**, wie in EmuTOS |
| `aes/gemaplib.c:251-261` | `ap_exit`: `if (rlr->p_qindex) ap_rdwr(MU_MESAG, …)`, dann `p_flags &= ~AP_OPEN` | `pd::close` — unverändert, seit A29 richtig |
| `aes/gemshlib.c:633-636` | `p_nameit(rlr, fname); p_setappdir(…); **rlr->p_flags = 0;**` | `pd::register` — `p_nameit` stand schon da, `t[idx].flags = 0` ist **neu** |
| `desk/deskmain.c:1953` | `gl_apid = appl_init();` beim Wiedereintritt in `deskmain` | `desk::main::desktop_resume` — `super::appl_init()` ist **neu**, siehe Abschnitt 3 |

**Nicht** durch Wiedereinbau des `queue.clear()` in `pd::open` — `pd::open` ist
unverändert und ORt weiterhin nur `AP_OPEN` (A25). Die Warteschlange wird da
geleert, wo EmuTOS sie leert: in `ap_exit`, aufgerufen vom `appl_exit()` des
Desktop-Epilogs. `rlr->p_flags = 0` fasst die Schlange nicht an, also fasst
`pd::register` sie auch nicht an.

**Warum `pd::register` die entsprechende Stelle ist:** EmuTOS setzt `p_flags = 0`
unmittelbar neben `p_nameit`, für den Deskriptor, den das Kind benutzen wird. Bei
uns liegt genau dieses `p_nameit` in `pd::register`, gerufen aus
`kernel/src/launch.rs` in dem Moment, in dem das Kind seinen Prozess-Slot bekommt
und seine AES-Id damit feststeht. Beide Startpfade laufen dort durch: der
Doppelklick des Desktops (`run_gem_app`) und der Shell-Loop
(`sh_ldapp`, NORMAL/AUTORUN).

## 2. Der umgekehrte A/B-Nachweis, beide Mechanismen getrennt (Punkt 2)

Gefahren über den echten LP64-Dispatcher, nur öffentliche Einstiegspunkte, in der
Reihenfolge von `kernel/src/desk/main.rs` und `kernel/src/launch.rs`
(`tos-api/tests/aes_launch_boundary_reset.rs`, drei Stufen in **einer**
Testfunktion — Deskriptortabelle und Fenstertabelle sind prozessweite Statics).

**Ausgangslage, echt erzeugt:** Desktop `appl_init` → zwei überlappende Fenster →
`wind_set(w1, WF_TOP)`, die erste Hälfte des Doppelklicks. Danach steht **eine**
ungelesene `WM_REDRAW` in der Desktop-Schlange.

### Mechanismus A — die Grenze leert die Schlange

Rot, mit dem Modell des HEAD-Kerns (`desktop_relinquish` = `save_windows`, kein
`appl_exit`) — `measurements/measurement-2026-09-14-a28/host-test-red-mechanism-A.log`:

    phase 1: the desktop's queue must already be empty when it gives up the
    machine — EmuTOS drains it in appl_exit (deskmain.c:2166 -> ap_exit,
    gemaplib.c:256-257), before the child exists
      left: 1   right: 0

Dieselbe Lage mit Sondenausgabe (`host-test-red-child-inherits.log`):

    A28-PROBE queue at the boundary -> 1
    A28-PROBE child appl_read -> 1, body [20, 1, 0, 1, 40, 40, 402, 302]
    phase 1: the child must start with an empty message queue; it read a
    message of type 20 …
      left: (1, 20)   right: (0, 0)

`20` = `WM_REDRAW`, `msg[1] = 1` = Absender-pid (das Kind selbst, es kann den
Fremdursprung nicht erkennen), `msg[3] = 1` = ein Fensterhandle des Desktops, das
`save_windows()` bereits aus der lebenden Tabelle genommen hat. Deckt sich mit der
Messung des A25-Audits (`[20, 1, 0, 1, 11, 53, 276, 154]`).

Grün mit dem Fix: die Schlange ist **schon vor** `register_process` und **vor**
dem `appl_init` des Kindes leer, und `appl_read` des Kindes liefert 0.

### Die Kontrolle — `appl_init` ist nicht der Löscher

Ohne sie wäre „0 Nachrichten" nicht unterscheidbar von einem Rückbau von A25.
Stufe 2 legt dem Kind nach seinem `appl_init` eine Nachricht hin, ruft
`appl_init` ein zweites Mal und liest sie unverändert zurück:

    phase 2: appl_init must NOT discard a pending message (EmuTOS ap_init,
    aes/gemaplib.c:54-63) — so phase 1's empty queue can only come from the
    boundary reset

Grün. `ap_init` besteht aus drei Anweisungen, die einzige Zustandsänderung ist
`rlr->p_flags |= AP_OPEN` — nachgelesen, nicht übernommen.

### Mechanismus B — der Übergang löscht die Flags

Diese Stufe läuft gegen **unveränderten Produktivcode**, ohne jedes Modell, weil
`pd::register` in `tos-api` liegt. Rot vor dem Fix:

    phase 3: the launcher's 'appl_init without appl_exit' cleanup fired for a
    program that never called appl_init, and drained the desktop's own queue —
    the descriptor was handed over with a stale AP_OPEN (EmuTOS clears every
    flag at the handover, aes/gemshlib.c:636)
      left: 0   right: 1

Der Weg dorthin ist ein echter Kernelpfad: der Desktop ist AES-Anwendung
(`AP_OPEN` gesetzt) und hat eine ungelesene Nachricht; der Anwender startet ein
`.NCP`. `desk::main::launch` schickt das **nicht** durch `run_gem_app`, also gibt
es dort kein `appl_exit`, und ein Konsolenprogramm ruft `appl_init` nie. Die
`sh_ldapp`-Nachsorge (`gemshlib.c:650-657`, `close_client_if_open`) fragt „hat
**dieses** Programm `appl_init` ohne `appl_exit` gerufen?" — sie las das Flag des
**vorherigen** Belegten und leerte darum die Schlange des Desktops. Mit
`flags = 0` beim Übergang ist sie richtig ein No-op.

## 3. Der `shell_loop.rs`-Kommentar (Punkt 3) — aufgelöst

Der alte Text hielt fest, `rlr->p_flags = 0` sei „item 10 of the plan's gap list"
und zurückgestellt. Das stimmt nicht mehr. Neu gefasst
(`kernel/src/shell_loop.rs`, an `p_nameit_and_appdir`): alle drei Zeilen von
`gemshlib.c:633-636` laufen, verteilt auf zwei Orte nach dem Zeitpunkt, zu dem
jede gebraucht wird — `p_setappdir` hier, weil `sh_find` das Verzeichnis **vor**
dem Start durchsucht; `p_nameit` **und** `p_flags = 0` zusammen in
`pd::register`, weil der zu benennende und zu löschende Deskriptor erst dort
feststeht. Als Abweichung steht dort jetzt nur noch die Deskriptor-**Identität**
(Punkt 10), und ausdrücklich, dass das Verhalten, das EmuTOS aus diesem Modell
zieht, seit A28 vorhanden ist.

**Die dritte Änderung, die im Auftrag nicht stand.** `desktop_resume` ruft jetzt
`appl_init()` (`deskmain.c:1953`). Ohne sie hätte **mein eigener Fix** eine neue
Abweichung erzeugt: der Desktop verlässt die AES beim Start jedes Programms und
wäre danach dauerhaft ohne `AP_OPEN` gelaufen — ein Zustand, in dem EmuTOS'
laufender Desktop nie ist (dort endet der Desktop-Prozess und `sh_ldapp` betritt
`deskstart` neu, `gemshlib.c:600-612,710-727`). `ap_init` ORt das Flag nur zurück;
Deskriptor, Name und Schlange bleiben unberührt, die Leerung der Grenze wird also
nicht rückgängig gemacht.

## 4. Regressionstest (Punkt 4)

Neu: `tos-api/tests/aes_launch_boundary_reset.rs`, eine Testfunktion, drei
benannte Stufen (oben). Zwei davon sind vor dem Fix rot, die dritte ist die
Kontrolle, die verhindert, dass der Fix durch einen A25-Rückbau „bestanden" wird.

**Was der Test modelliert und was er fährt — ehrlich getrennt.** Stufe 3 fährt
Produktivcode (`pd::register`, `close_client_if_open`). Stufe 1 **modelliert** die
Kernelseite: `desktop_relinquish` liegt in `kernel/`, das auf dem Host nicht
testbar ist, also ruft der Test dieselbe `appl_exit`-Trap in derselben Reihenfolge
wie `desk/main.rs`. Der ungeprüfte Rest ist die eine Zeile „der Kernel ruft es" —
belegt durch Quelltext und den Maschinenlauf, nicht durch den Test. Das ist
dasselbe Verfahren, mit dem das A25-Audit den Defekt überhaupt gemessen hat.

**Ein bestehender Test war zu ändern, nicht grün zu machen:**
`tos-api/tests/aes_app_return_descriptor.rs` (A29) bildete die Grenze in der alten
Reihenfolge ab und sagte im Kommentar ausdrücklich „no second `appl_init`. The
desktop is resumed, not restarted." Das ist seit dieser Arbeit falsch. Das Modell
ist nachgezogen (`appl_exit` in `relinquish`, `appl_init` in `resume`); der Test
wird dadurch **strenger** — er belegt jetzt zusätzlich, dass der Deskriptor auch
das `appl_exit` der Grenze überlebt. Er war vorher grün und ist es geblieben.

## 5. Kompensationsprüfung nach A23 (Punkt 5)

Vollständige Liste, im heutigen Baum selbst erhoben.

**`flags` schreiben:** `pd::open` (`*slot = EMPTY` für einen frischen Slot;
`|= AP_OPEN`), `pd::close` (`&= !AP_OPEN`), `pd::close_if_open` (`&= !AP_OPEN`),
`pd::register` (`*slot = EMPTY` für einen frischen Slot; **neu `= 0`**),
`appl::appl_read` und `event.rs` (zwei Stellen) — jeweils `|= AP_MESAG`.
**`flags` lesen:** **genau eine** Stelle im ganzen Baum —
`pd::close_if_open` (`s.flags & AP_OPEN`). (`event.rs:60` `m.flags` gehört zu
einer anderen Struktur, dem Maus-Rechteck.) Dazu die Zusicherungen des
`pd.rs`-Unittests.
**`queue` schreiben:** `post_msg`, `post_words`, `pop_into` (`appl_read`), `pop`
(`evnt_mesag`/`evnt_multi`), `clear()` in `close` und `close_if_open`,
`window::test_clear_msg` (Testhaken), die `EMPTY`-Zuweisungen. **`pd::register`
fasst die Schlange nicht an** — wie `rlr->p_flags = 0`.
**`queue` lesen:** `window::test_pending_msg`, `window::test_msg_count` (beides
Testhaken). **`name` schreiben:** `pd::register` (`p_nameit`), die
`EMPTY`-Zuweisungen. **`name` lesen:** `pd::find_by_name` (`appl_find`).

**Ergebnis: keine Stelle hat sich auf die fehlende Rücksetzung eingerichtet.**
Drei Beziehungen ändern sich, alle drei werden EmuTOS-treuer:

1. **`close_if_open` beurteilt jetzt das richtige Programm.** Vorher konnte es auf
   dem `AP_OPEN` des vorherigen Belegten auslösen; das ist der Defekt aus Stufe 3.
   EmuTOS fragt mit `rlr->p_flags & AP_OPEN` nach dem Programm, das gerade
   zurückgekehrt ist (`gemshlib.c:650`), und stellt das durch `p_flags = 0` vor
   dem `dos_exec` sicher.
2. **`AP_MESAG` läuft nicht mehr in das nächste Programm über.** EmuTOS löscht es
   mit (`p_flags = 0` löscht alle Bits, `ap_exit` nur `AP_OPEN`). In NewTOS wird
   `AP_MESAG` gesetzt, aber **nirgends gelesen** — die Angleichung ist also
   derzeit nicht beobachtbar. Genannt, nicht als Wirkung verkauft.
3. **Die Schlange des Desktops ist beim Programmstart leer.** Der einzige Verlust
   wäre eine Nachricht, die der Desktop noch hätte lesen wollen — genau die, die
   EmuTOS in `ap_exit` wegwirft. `desktop_resume` malt danach ohnehin den ganzen
   Schirm neu (`redraw_desktop`), eine verworfene `WM_REDRAW` hat also kein
   Nachspiel.

**Kein vorhandener Test kodiert den alten, falschen Wert.** Der einzige Test, der
die Grenze überhaupt abbildet, ist A29s (Abschnitt 4) — er war grün und ist grün
geblieben; die Änderung dort ist eine Modellkorrektur, keine Grünmachung.
`kernel/src/selftest_aes.rs` prüft an `appl_exit` nur den Rückgabewert 1
(unverändert). `xtask test` 546 → 547, **0 rot**; der einzige Zuwachs ist der neue
Test.

## 6. Läufe, Bauten, Warnungen

    cargo run -p xtask -- test    547 bestanden / 0 fehlgeschlagen / 1 ignoriert
                                  (Sollstand vorher 546/0/1 selbst nachgemessen)

    rpi4       ELF sha256 18eb2ad7…   kernel8.img e23991f0…
    qemu-virt  ELF sha256 292d0505…
    Jedem Bau ging ein `touch` beider Crate-Wurzeln voraus; beide Protokolle
    zeigen „Compiling tos-api" UND „Compiling kernel", die ELF-Hashes sind
    verschieden = der zweite Bau war ein echter Bau.
    rustc   tos-api 64 / kernel 94    (beide Boards)
    clippy  tos-api 81 / kernel 125   (kernel auf beiden Boards)
    Delta 0. Keine Meldung zeigt auf aes/pd.rs, desk/main.rs, shell_loop.rs.

**Maschinenlauf — ausdrücklich ein Kontrolllauf.** QEMU `raspi4b`, headless,
640×480, `#Z`-Autorun mit `CHAINTST.NAP` (Rezept und Karte aus A29), zweimal über
denselben Datenträger: HEAD-Kern `868c34e7…` gegen reparierten `e23991f0…`.
Seriell **bitgleich** (`diff` ohne Ausgabe), Bild **0 von 307200 Bildpunkten**
verschieden, beide enden auf `[desk] desktop up (1 drives)`.
`qemu-virt`-Rauchprobe: bootet, `[desk] desktop up (0 drives)`, kein Fault.

**Und warum das kein Beleg ist:** der `#Z`-Autorun-Pfad betritt
`desktop_relinquish` nie — dort läuft das Programm **vor** dem Desktop. Der A29-
Entwickler hat das für seinen Defekt gemessen; für A28 gilt es genauso, also
Mechanismus A wird nicht gefahren. Mechanismus B **wird** gefahren — die Kette
startet vier Programme nacheinander in GEMDOS-Slot 0, also viermal `register` auf
demselben Deskriptor (Seriell, Zeilen 36–57) —, seine Wirkung ist dort aber null,
weil jedes der vier Programme sein `appl_exit` selbst ruft und `AP_OPEN` damit
schon gelöscht ist. Erwartet war Bitgleichheit; Bitgleichheit ist eingetreten.
Der Lauf belegt **Nichtregression**.

## 7. Was offen bleibt — echte Pi 4

Der Defektpfad braucht HID-Eingabe **und** ein C:-Medium gleichzeitig; die beiden
Emulatoren geben je eines, und beide Protokolle dieses Laufs sagen es
(`raspi4b`: „[pcie] RC not present … USB disabled" bei funktionierendem C:;
`virt`: usb-kbd/usb-mouse melden sich, „desktop up (0 drives)").

Für den Anwender, eine Sitzung, in dieser Reihenfolge:

1. Desktop hochfahren, zwei Verzeichnisfenster offen, eines teilweise verdeckt.
2. Auf das verdeckte Fenster klicken (damit steht eine ungelesene `WM_REDRAW` in
   der Desktop-Schlange) und aus ihm heraus `QED.NAP` (oder
   `TERADESK\DESKTOP.NAP`) per Doppelklick starten.
   **Soll:** die Anwendung zeichnet nur ihre eigene Oberfläche.
   **Vor dem Fix:** sie bekommt als allererste Nachricht ein `WM_REDRAW` für ein
   Fensterhandle des Desktops und zeichnet in ein Rechteck, das ihr nicht gehört.
3. Die Anwendung beenden und den Desktop weiter bedienen — Menüpunkt wählen,
   Fenster verschieben, Schließkästchen. Alle drei müssen etwas tun (das ist A29s
   Abnahme; `desktop_resume` ruft `appl_init` nach, verschlechtert sie also nicht).
4. Ein `.NCP` (z. B. `HELLO.NCP`) doppelklicken, zurückkehren, dann ein verdecktes
   Fenster anklicken. Der Desktop muss reagieren — das ist Stufe 3 am Gerät.

## 8. Nebenbeobachtungen (nicht verfolgt, nichts geändert)

- **Der `.NCP`-Pfad relinquisht nicht.** `desk::main::launch` schickt ein
  Konsolenprogramm an `launch::run_program_with_tail` vorbei an `run_gem_app`, es
  gibt dort also kein `appl_exit` und die Desktop-Schlange überlebt den Start.
  EmuTOS' Desktop endet auch vor einem Zeichenmodus-Programm. Eigener Auftrag;
  A28 nennt die GEM-Grenze.
- `kernel/src/selftest_sleepwake_switch.rs::run()` hat **keinen Aufrufer** — ein
  unverdrahteter Selftest (bekanntes Muster). Er benutzt `register_process` und
  ist von dieser Änderung sachlich unberührt (frischer Deskriptor, Flags ohnehin
  0); er baut weiter auf beiden Boards.
- `appl_init` kann bei voller Tabelle `-1` liefern, EmuTOS' `ap_init` kann nicht
  scheitern. Vorbestehend, vom A25-Audit gemeldet.
- `AP_ACCLOSE` (0x04) fehlt ganz; ohne Accessories ohne Wirkung. `p_flags = 0`
  würde es mit löschen, sobald es existiert.

## 9. Aufräumen

Keine temporäre Datei im Arbeitsbaum hinterlassen. Für die beiden Rot-Läufe wurde
**der neue Test** von Hand auf das Modell des HEAD-Kerns gesetzt und von Hand
zurückgeschrieben — **kein** `git checkout`, `restore`, `clean`, `stash` oder
`reset`, kein Produktivcode dafür angefasst. Keine Hardwareaktion, nichts
geflasht, keine Karte angefasst; der QEMU-Lauf benutzt eine **Kopie** der
A29-Karte, das Original ist unverändert. Der Baum enthielt vor der Arbeit nichts
Fremdes und enthält außer dieser Arbeit nichts Verändertes.
