# A29 — Quittung

**Zustand: UNFINISHED** · 2026-09-14 · Commit `7c28493`

**UNFINISHED heißt hier: die Hardwareabnahme fehlt, nicht die Implementierung.**
Die vier Abnahmepunkte des Tickets sind erfüllt; was offen bleibt, ist die eine
Geste, die nur eine echte Pi 4 ausführen kann (unten, Abschnitt 6).

Gepinnte EmuTOS-Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.
NewTOS-Basis: `desk-rebuild`, HEAD vor der Arbeit `a006386`, sauberer Baum.
Messverzeichnis: [`measurements/measurement-2026-09-14-a29/`](../measurements/measurement-2026-09-14-a29/RUNINFO.TXT).

---

## 1. Die praktische Wirkung — zuerst festgestellt (Abnahmepunkt 1)

Der Desktop läuft im Kernel-Bootkontext und bekommt AES-Id 1 (`current_pid` bei
leerer BASEPAGE, `tos-api/src/aes/appl.rs`). Das erste von ihm gestartete
Programm landet in GEMDOS-Slot 0, dessen AES-Id `slot + 1 = 1` ist
(`aes::pid_of_slot`). Beide benutzen **denselben** Deskriptor. Das ist nicht der
Defekt, sondern EmuTOS-treu (`aes/gemshlib.c:633-636`, wie das Ticket richtig
sagt).

Beide Endepfade haben den Deskriptor gelöscht:

    appl_exit()          -> pd::close          -> *slot = Aespd::EMPTY
    close_client_if_open -> pd::close_if_open  -> *slot = Aespd::EMPTY

Danach ist `pd::by_pid(1)` `None`. **Was dadurch verloren geht**, abgelesen an
den Poststellen, nicht vermutet — alle laufen über `pd::post_msg`/`post_words`
und damit über `by_pid`:

| Was der Anwender tut | Was passiert |
|---|---|
| Menüpunkt anklicken | `menu_intercept` (`aes/event.rs`) postet `MN_SELECTED` und meldet dem Aufrufer „erledigt"; der holt sie über den regulären MU_MESAG-Zweig ab. Ohne Deskriptor: Menü klappt auf, Klick wird verbraucht, **nichts passiert**. |
| Fenster schließen / vollbild | `window::intercept` postet `WM_CLOSED`/`WM_FULLED`. Das Kästchen blinkt, dann nichts. |
| Fenster verschieben / grössen | `intercept` zeichnet das Gummiband und postet `WM_MOVED`/`WM_SIZED`. Der Desktop setzt das Rechteck erst auf die Nachricht hin (`moved_msg` → `wind_set(WF_CURRXYWH)`). Ohne sie **springt das Fenster zurück**. |
| Auf ein unteres Fenster klicken | `intercept` postet `WM_TOPPED`; das Hochholen ist ausdrücklich Sache des Besitzers. Ohne Nachricht bleibt das Fenster unten. |
| Pfeile / Schieber | `WM_ARROWED`/`WM_HSLID`/`WM_VSLID` — kein Blättern. |
| Fensterinhalt freilegen | `send_redraw` postet `WM_REDRAW` — Inhalt bleibt alt. |

**Still oder sichtbar falsch?** Beides, nacheinander. Im Moment der Rückkehr
**still**: `desk::main::desktop_resume` malt den ganzen Schirm direkt neu
(`redraw::redraw_desktop` → `win_shwall_nocursor`), das Bild ist also intakt.
**Sichtbar falsch** beim nächsten Handgriff — der Desktop sieht aus wie immer
und reagiert nicht mehr.

Gemessen durch den echten LP64-Dispatcher, in der Reihenfolge von
`kernel/src/desk/main.rs` (`run_gem_app`) und `kernel/src/launch.rs`
(`tos-api/tests/aes_app_return_descriptor.rs`):

    appl_write(1, msg)                          0  statt 1
    Desktop macht wind_open -> appl_read        0, Puffer [0,0,0,0,0,0,0,0]
                                                statt 1 + dem WM_REDRAW

**Erreichbarkeit — und die erste Korrektur an meinem Auftrag.** Der Defekt
hängt nur am Doppelklick-Pfad des laufenden Desktops (`run_gem_app`), denn nur
der ruft **kein** `appl_init` mehr nach. Der **`#Z`-Autorun-Pfad hat den Fehler
nicht**: dort läuft das Programm **vor** dem Desktop, und `sh_ldapp`s
DESKTOP-Arm ruft danach `desk::run()`, dessen erste Handlung `appl_init()` ist
(`kernel/src/desk/mod.rs`). EmuTOS macht genau dasselbe aus genau demselben
Grund: `sh_main` betritt `deskstart` nach jeder Anwendung neu
(`aes/gemshlib.c:710-727`). Der im Auftrag vorgeschlagene Maschinennachweis
(„TeraDesk oder QED über den `#Z`-Autorun starten, beenden lassen") kann den
Defekt daher **nicht** zeigen — er ist ein Kontrolllauf, kein Beweis. Er wurde
trotzdem gefahren, als Nichtregressionsbeleg (Abschnitt 5).

## 2. EmuTOS an derselben Grenze (Abnahmepunkt 2)

Selbst gelesen in der gepinnten Fassung, Zeilennummern nachgeprüft:

| Ort | Inhalt |
|---|---|
| `aes/gemaplib.c:251-261` | `ap_exit`: `wm_update(BEG)`, `mn_cleanup()`, `wait_for_accs(AP_ACCLOSE)`, `if (rlr->p_qindex) ap_rdwr(MU_MESAG, rlr, rlr->p_qindex, (WORD *)D.g_valstr);`, `wm_update(END)`, `all_run()`, `rlr->p_flags &= ~AP_OPEN;` |
| `aes/gemshlib.c:650-657` | `sh_ldapp`, direkt nach `dos_exec`: `if (rlr->p_flags&AP_OPEN) { mn_cleanup(); if (rlr->p_qindex) ap_rdwr(MU_MESAG, …); rlr->p_flags &= ~AP_OPEN; }` |
| `aes/gempd.c:32-35` | `pd_index(i)` — die AESPDs sind feste Einträge von `D.g_int[]`/`D.g_acc[]`, über den Index geholt |
| `aes/gempd.c:38-57` | `fpdnm(pname, pid)` — läuft über `totpds` Einträge, **ohne jede Lebendprüfung** |
| `aes/gemsuper.c:102-105` | `APPL_WRITE` löst sein Ziel mit genau diesem `fpdnm(NULL, AP_RWID)` auf |
| `aes/gemshlib.c:633-636` | die geteilte Prämisse: `p_nameit(rlr,…)`, `p_setappdir`, `rlr->p_flags = 0` — **derselbe** `rlr` wird umbenannt (die `p_flags = 0`-Hälfte ist A28) |

**Beide Endepfade tun genau zwei Dinge — Queue leeren, `AP_OPEN` löschen — und
geben nichts frei.** EmuTOS *könnte* einen AESPD gar nicht freigeben: er ist ein
Feldeintrag, und der Zielsucher prüft keine Lebendigkeit. Deshalb findet eine an
eine pid adressierte Nachricht dort immer ihren Deskriptor, vor wie nach
`appl_exit`.

**Die Spec war bereits richtig, der Code wich von ihr ab.**
`docs/api/aes/application.md` § `appl_exit (19)` listet die sieben Schritte von
`ap_exit` samt Drain (Schritt 4) und `AP_OPEN`-Löschen (Schritt 7) — und kein
Freigeben. `docs/api/aes/shell-loop.md` § „Arm 3" zitiert `:650-657` wörtlich.
Es war **keine** Spec-Korrektur nötig; ergänzt wurde nur eine Notiz, die
festhält, was der Code stattdessen tat.

## 3. Der Fix (Abnahmepunkt 3)

`tos-api/src/aes/pd.rs`:

```rust
pub unsafe fn close(pid: i16) {            // appl_exit -> ap_exit
    if let Some(slot) = … {
        slot.queue.clear();                // ap_rdwr(MU_MESAG, rlr, rlr->p_qindex, …)
        slot.flags &= !AP_OPEN;            // rlr->p_flags &= ~AP_OPEN
    }
}

pub unsafe fn close_if_open(pid: i16) {    // sh_ldapp's AP_OPEN cleanup
    if let Some(slot) = … /* unveränderter AP_OPEN-Wächter */ {
        slot.queue.clear();
        slot.flags &= !AP_OPEN;
    }
}
```

Vorher stand an beiden Stellen `*slot = Aespd::EMPTY;`. Sonst nur Doc-Comments
(`tos-api/src/aes/appl.rs` zu Opcode 19, `tos-api/src/aes/mod.rs` zu
`close_client_if_open`) und die Spec-Notiz.

**Warum beide und nicht nur der eine, den das Ticket nennt — die zweite
Korrektur an meinem Auftrag.** Der Sachverhalt des Tickets nennt
`close_client_if_open`. Das ist aber der *Absturz*-Pfad: `sh_ldapp`s Nachsorge
greift nur, wenn das Programm `appl_init` ohne `appl_exit` gerufen hat. Der Pfad,
den echte Anwendungen nehmen, ist `appl_exit` — TeraDesk
(`sdk/apps/teradesk/main.c:2124`, `:2069`, `startprg.c:272`) und QED (über cflib
`appexitg.o`) rufen es beide. Der neue Test fällt vor dem Fix an **jedem der
beiden einzeln** (Abschnitt 4). Ein Fix nur an `close_if_open` hätte den P1 für
jede reale Anwendung stehen lassen. Beide Stellen sind dieselbe EmuTOS-Invariante
an derselben Grenze, also ein Änderungssatz.

## 4. Kompensationsprüfung nach A23

Vollständige Liste der Stellen, die die Lebensdauer eines Deskriptors schreiben
oder lesen (`in_use` kommt außerhalb von `pd.rs` nirgends vor — die Treffer in
`vdi/vwk.rs` und `window.rs` gehören zu anderen Strukturen):

**Schreiber:** `pd::open` (`*slot = EMPTY` + `in_use = true` für einen frischen
Slot), `pd::register` (dito), `pd::close` (geändert), `pd::close_if_open`
(geändert).
**Leser:** `pd::by_pid` (einziger Zugang für `post_msg`, `post_words`,
`evnt_multi`, `evnt_mesag`, `appl_read`, `appl_write`), `pd::find_by_name`
(`appl_find`), die Slot-Suche in `open`/`register`.
**Einziger Aufrufer von `pd::close`:** `appl_exit` (`appl.rs`).
**Einziger Aufrufer von `close_client_if_open`:** `kernel/src/launch.rs`.

Ergebnis — **keine Stelle hat sich auf das Löschen eingerichtet**, drei
Beziehungen ändern sich und alle drei werden dadurch EmuTOS-treuer:

1. `appl_find(name)` findet den Namen eines beendeten Programms jetzt noch.
   EmuTOS ebenso (`ap_exit` fasst `p_name` nicht an). Die beiden eingefrorenen
   `appl_find`-Fälle (`aes-m05-addrin.tsv`, Opcode 13) fragen nach
   `ABCDEFGH`/`HIJKLMNO`, die nie registriert werden — unberührt. Opcode 10, 11,
   12 und 19 kommen in **keiner** eingefrorenen Tabelle vor (nachgezählt: die
   Referenzfälle decken 0, 13, 24, 48, 77, 79, 100–109 ab).
2. `appl_write` an eine beendete pid liefert jetzt 1 statt 0. EmuTOS ebenso
   (`fpdnm` findet den pd, `ap_rdwr(MU_SDMSG)` reiht ein).
3. `desk::run()`s `appl_init()` (`desk/mod.rs`) ist **keine** Kompensation,
   sondern EmuTOS' `deskmain.c:1953`; es ist der Grund, warum der
   `#Z`-Autorun-Pfad den Defekt nie hatte.

Ein **vorhandener Test wird nicht rot** — `xtask test` 545 → 546, 0 rot (der
einzige Zuwachs ist der neue Test). `kernel/src/selftest_aes.rs` prüft an
`appl_exit` nur den Rückgabewert 1, der unverändert ist.

**Rot vor dem Fix, grün danach** (`measurements/measurement-2026-09-14-a29/host-test-red-before.log`;
`tos-api/src/aes/pd.rs` dafür von Hand auf den HEAD-Stand gesetzt und wieder
zurückkopiert, kein `git checkout`):

    round 1: appl_write to the desktop was refused — its descriptor did not
    survive the program
      left: 0   right: 1          <- das Kind ruft appl_exit (TeraDesk, QED)

    round 2: appl_write to the desktop was refused — …
      left: 0   right: 1          <- das Kind stürzt ab, close_client_if_open

    wind_open posted no WM_REDRAW to the desktop (body [0,0,0,0,0,0,0,0]) — its
    descriptor is gone, so every window message is dropped
      left: 0   right: 1          <- die Wirkungshälfte allein, ohne die
                                     appl_write-Probe, die sie sonst verdeckt

Alle drei grün gegen den reparierten Stand.

## 5. Maschinennachweis

QEMU `raspi4b`, headless, 640×480, `#Z`-Autorun (`method/prep-chain.sh`,
`method/boot-qmp.sh`). Autorun-Programm ist `CHAINTST.NAP` — die einzige
mitgelieferte GEM-Anwendung, die ohne Eingabe von selbst endet: `appl_init()`,
`shel_read`, `shel_write`, `appl_exit()`, `Pterm(0)`. Sie kettet einmal auf sich
selbst und dann auf `HELLO.NCP`; danach startet der Desktop. **Vier Programme
starten und enden**, über den echten `svc`-Pfad.

Zwei Läufe über denselben Datenträger: einmal mit dem HEAD-Kern
(`kernel8.img` sha256 `eeb6e505…`, byte-identisch mit dem, den
`measurements/measurement-2026-09-14-a19` für HEAD festgehalten hat), einmal mit dem
reparierten (`868c34e7…`).

    Seriell:   chain-prefix-BOOT-SERIAL.txt == chain-postfix-BOOT-SERIAL.txt
               (diff ohne Ausgabe), beide enden auf
               "[desk] desktop up (1 drives)"
    Bild:      640×480 = 307200 Bildpunkte, je drei Bytes, Punkt für Punkt
               verglichen -> 0 verschieden
    Screendump screendumps/chain-desktop-{prefix,postfix}.png — Menüleiste,
               C:-Icon, Papierkorb, offenes C:\-Fenster „25 items  381616
               bytes used" mit Verzeichnisliste

Das ist das **erwartete** Ergebnis und der Grund steht in Abschnitt 1: der
Autorun-Pfad hatte den Defekt nie. Der Lauf belegt Nichtregression, nicht den
Defekt.

`qemu-virt` Rauchprobe: bootet, `[desk] desktop up (0 drives)`, kein Fault
(`qemuvirt-BOOT-SERIAL.txt`).

**Builds und Warnungen** — Sollstand gehalten, Delta 0:

    rpi4       ELF sha256 5e673d02…   kernel8.img 868c34e7…  (787584 B)
    qemu-virt  ELF sha256 dea16065…
    Jedem Bau ging ein `touch tos-api/src/aes/pd.rs` voraus, beide Protokolle
    zeigen „Compiling tos-api" UND „Compiling kernel"; die beiden ELF-Hashes
    sind verschieden = der zweite Bau war ein echter Bau. `kernel8.img` kam in
    jedem Neubau dieser Sitzung bitgleich heraus.
    rustc   tos-api 64 / kernel 94   (beide Boards)
    clippy  tos-api 81 / kernel 125  (kernel auf beiden Boards)
    Die neue Testdatei zieht keine eigene clippy-Meldung.

    cargo run -p xtask -- test    546 bestanden / 0 fehlgeschlagen / 1 ignoriert
                                  (545 vorher, plus der eine neue Test)

## 6. Was offen bleibt — echte Pi 4

**Der Defektpfad selbst ist in keinem Emulator fahrbar, und das ist hier
gemessen, nicht angenommen.** Ein Programm lässt sich vom *laufenden* Desktop
nur per Doppelklick (`hndl_button` → `launch_native` → `run_gem_app`) oder über
die Tastatur (`hndl_kbd`) starten; beides braucht HID-Eingabe. Die beiden
Emulatoren geben Eingabe **oder** C:-Speicher, nie beides, und beide Protokolle
dieses Laufs sagen es:

    raspi4b   "[pcie] RC not present (no response at RC_BASE); USB disabled"
              — und C: funktioniert (25 Einträge gelistet)   -> Speicher, keine Eingabe
    virt      usb-kbd und usb-mouse melden sich an, dann
              "[desk] desktop up (0 drives)"                 -> Eingabe, kein Speicher

**Für den Anwender, auf echter Pi 4** (eine Sitzung, in dieser Reihenfolge):

1. Desktop hochfahren, ein Verzeichnisfenster offen lassen.
2. `C:\Apps\TERADESK\DESKTOP.NAP` (oder `QED.NAP`) per Doppelklick starten.
3. Die Anwendung regulär beenden (sie ruft `appl_exit`).
4. Zurück auf dem Desktop, **ohne Neustart**: einen Menüpunkt wählen
   (z. B. *Desk → Desktop Info*), ein Fenster am Titelbalken verschieben, das
   Schließkästchen drücken, ein unteres Fenster anklicken.
   **Soll:** alle vier tun etwas. **Vor dem Fix:** keiner von ihnen.

Ohne diesen Lauf steht die Wirkung als Hostmessung über den echten Dispatcher
fest, aber nicht am Gerät.

## 7. Verhältnis zu A28 (Abnahmepunkt 4)

**Nicht mitgemacht, und nachgemessen, dass es nicht mitgemacht wurde.** Ein
Probetest (nach dem Lauf gelöscht) legt dem Desktop eine ungelesene Nachricht
hin und startet dann das Kind:

    A28-PROBE child appl_read -> 1, body [18193, 1, 0, 1, 2, 3, 4, 5]

Das Kind erbt sie weiterhin, also ist A28 unverändert offen — weder repariert
noch verdeckt. Unberührt geblieben sind genau die drei Stellen, die A28 nennt:
`desk::main::desktop_relinquish` (kein `appl_exit()` eingebaut), `pd::register`
(kein `p_flags = 0`), `kernel/src/shell_loop.rs:341-357` (der Kommentar, der die
Auslassung von `gemshlib.c:636` festhält).

**Reihenfolge, die dabei herauskam:** A29 ist Voraussetzung für A28, nicht
umgekehrt. A28s erster Punkt will `appl_exit()` in `desktop_relinquish`. Solange
`pd::close` den Deskriptor löschte, hätte genau dieser Aufruf den Desktop
**vor** dem Start des Kindes seines Deskriptors beraubt; A28s gewünschte Wirkung
(„das Kind startet mit 0 Nachrichten") wäre eingetreten, aber aus dem falschen
Grund und mit einem toten Desktop danach. Jetzt tut `appl_exit()` an dieser
Stelle das, was `deskmain.c:2166` tut. Der Lead hatte recht, die beiden zu
trennen.

## 8. Nebenbeobachtungen (nicht verfolgt, nichts geändert)

- **Deskriptor-Obergrenze.** `MAX_APPS = 8`, `MAX_PROCESSES = 16`, und ein
  Deskriptor wird jetzt nie mehr frei. Praktisch gebunden: GEMDOS-Slots werden
  beim `Pterm` freigegeben, das nächste Programm bekommt wieder Slot 0 und damit
  dieselbe pid und denselben Deskriptor; nur echt verschachtelte Starts erzeugen
  weitere. EmuTOS bindet das strukturell (`totpds` feste Einträge, Index statt
  pid); die saubere Entsprechung wäre, die Deskriptortabelle über den
  GEMDOS-Slot zu indizieren — das ist Punkt 10 der Lückenliste in
  `kernel/src/shell_loop.rs:341-357` und ein eigener Auftrag.
- `appl_init` kann bei voller Tabelle `-1` liefern; EmuTOS' `ap_init` kann nicht
  scheitern. Vorbestehend, vom A25-Audit gemeldet.
- `evnt_mesag` hat einen Zweig „kein Deskriptor → melde 1 und kehre zurück"
  (`aes/event.rs`), für den es in EmuTOS keine Entsprechung gibt, weil dort
  immer ein pd existiert. Nach diesem Fix schwerer erreichbar, aber weiterhin da.
- `ap_exit`s `wm_update(BEG/END_UPDATE)` und `all_run()` sind hier nicht
  nachgebildet: sie klammern den Drain gegen EmuTOS' eigene Nebenläufigkeit, mit
  der ein synchroner Drain auf dem Einzel-Trap-Pfad nicht konkurrieren kann.
  Genannt, nicht weggeredet.

## 9. Aufräumen

Zwei temporäre Testdateien angelegt und beide per `rm` gelöscht
(`tos-api/tests/zz_a29_a28_probe_tmp.rs`, `tos-api/tests/zz_a29_effect_tmp.rs`).
`tos-api/src/aes/pd.rs` wurde für die Rot-Läufe von Hand auf den HEAD-Stand
gesetzt und aus einer Kopie zurückgeschrieben — **kein** `git checkout`,
`restore`, `clean`, `stash` oder `reset`. Keine Hardwareaktion, nichts geflasht,
keine Karte angefasst. Der Baum enthält außer dieser Arbeit nichts Verändertes.
