# Review der Tickets mit `-STATUS-UNFINISHED.MD`-Quittung — 2026-10-17

Auftrag (Anwender, 2026-10-17): die 19 Tickets mit UNFINISHED-Quittung prüfen
(A13, A14, A15, A25, A28, A29, A32, A35, A62, ARCH03-3b, ARCH03-3c,
ARCH03-POSTE9, FS04, LDG-M3, M08, PORT09, Q06, X01, ZVIEW-M4), dazu DESK57
gegen DESK62 und PROC03 gegen DESK43/ADR-026. Pro Ticket: Quittung lesen, den
heutigen Stand gegen Code, Tests, spätere Commits/Tickets und Messläufe
prüfen. `docs/STATUS.md` und Ticketköpfe gelten als Hinweis, nicht als Beleg.

Methode und Grenzen: statisch gelesen (Code `file:line`, `git log`, Messordner,
Screendump angesehen). **Es wurde nichts gebaut und kein Test gefahren** (der
Arbeitsbaum ist geteilt, `target/` gehört anderen). Wo ein Urteil nur mit einem
Lauf fällt, steht es als NEEDS RUN. Gelesen = belegt; „Schluss" = meine
Folgerung, ausdrücklich so markiert.

## Zusammenfassung

| Ticket | Urteil | Kern in einem Satz |
|---|---|---|
| A13 | **CLOSE** | Kopf steht auf ERLEDIGT (2026-09-11); alle drei Defekte in FIX-01…06 behoben, TT-/QEMU-/Pi-Gegenmessung bitgleich; Quittung ist veraltet. |
| A14 | **CLOSE** | wie A13 (D-A14-1/-2 = FIX-03/FIX-01); L1-Messlücke wurde A24 (ERLEDIGT). |
| A15 | **CLOSE** | wie A13 (F-A15-1 = FIX-05); die vier Reststapelfälle sind Nicht-Soll (`gemsuper.c:432`). |
| A25 | **NEEDS RUN** (klein, QEMU genügt) | Code und zwei Tests belegt; es fehlt allein das Diagnoseprogramm „appl_init zweimal"; „nur Pi" stimmt nicht. |
| A28 | **CLOSE-SUPERSEDED** (DESK43 U9/U10) | Die Stelle (`desktop_relinquish` im Kernel) ist gelöscht; dieselbe Rücksetzung steht jetzt im EL0-Desktop (`programs/desktop/src/lib.rs:477`), Pi-Abnahme U9-6 deckt Start/Ende. |
| A29 | **CLOSE-SUPERSEDED** (DESK43 U9-6/U10 Pi) | `pd::close`/`close_if_open` behalten den Deskriptor (Code gelesen); Gerätehälfte durch die Pi-Abnahme des EL0-Desktops abgedeckt. |
| A32 | **CLOSE** | Kopf ERLEDIGT (2026-09-19); Pi-Lauf bitgleich, Entscheidung `DROP_SHADOW_SIZE = 2` getroffen und im Code (`window.rs:243`). |
| A35 | **CLOSE** | `A35-STATUS-DONE.MD` (2026-09-14) ersetzt die UNFINISHED-Fassung; `report_load` im Code, `#[ignore]` weg. |
| A62 | **CLOSE** (ein Rest ausgelagert) | Tabelle `MAX_PROCESSES + 1`, `KERNEL_CONTEXT` im Code; Pi-Rest durch U9-6 mitgedeckt; Rest `appl_write(1, WM_REDRAW)` ist A56-Stufe 2, ohne Ticket. |
| ARCH03-3b | **CLOSE** | Lead-Abschlussvermerk 2026-10-13 stimmt: Abnahme-RUNINFO und Commits geprüft. |
| ARCH03-3c | **CLOSE** | dito. |
| ARCH03-POSTE9 | **CLOSE** | dito; Rest in FS10 (ERLEDIGT), MEM05 (ERLEDIGT), PROC03 (offen). |
| FS04 | **CLOSE** | Kopf BEHOBEN; Pi-Messung 2182 → 467 ms, 0 Timeouts; Fix im Treiber (`Budget`) gelesen; Reste = HW05/HW06/GUI06/FS05. |
| LDG-M3 | **CLOSE-SUPERSEDED** | `LDG-M3-STATUS-DONE.MD` nach B1/B2. |
| M08 | **STILL OPEN** (S) | Tabelle `aes-m07-parameter.tsv` nicht verdrahtet; B-1 durch V17 behoben, B-2 (Fall 69, Schlangenabzug im Harness) offen. |
| PORT09 | **CLOSE** | `PORT09-STATUS-DONE.MD` (2026-10-06), Kopf ERLEDIGT. |
| Q06 | **CLOSE** (mit Vorbehalt Pi) | Die „nur Pi"-Prämisse ist überholt: der Injektor zeigt getippten Text, `*` und Z/S-Kopfzeile in QEMU (Screendump 2026-10-08). |
| X01 | **STILL OPEN** (S + Anwenderentscheid) | Zwei Entscheidungen und drei nur-Pi-Aufrufstellen stehen aus, nichts davon ist seither angefasst. |
| ZVIEW-M4 | **CLOSE-SUPERSEDED** | `ZVIEW-M4-STATUS-DONE.MD` + `ZVIEW-M4-K1-STATUS-DONE.MD`; Blocker K1 behoben. |
| DESK57 | **CLOSE-SUPERSEDED** (durch DESK62) | Selbes Symptom, Ursache `EXIT_DESKTOP`, auf der Pi am 2026-10-16 abgenommen. |
| PROC03 | **STILL OPEN** (umformulieren) | Desktop-Hälfte durch DESK43 U9 geliefert; offen bleibt nur „AES-PD als echter Prozess" (M–L). |

Befund zur Namensgebung: das Suffix `-UNFINISHED` trägt drei Bedeutungen
(wirklich offen / HW-Abnahme fehlt / veraltete Vorfassung neben einer
DONE-Quittung). Von 21 Tickets sind nur **drei** wirklich offen (M08, X01,
PROC03), eines ist ein kleiner Lauf (A25).

---

## A13 — `wind_get(WF_FIRSTXYWH)`

**Verdict: CLOSE.**
Quittung: drei belegte Defekte (D-A13-1 Handleprüfung, -2 Zerlegungsreihenfolge,
-3 `DROP_SHADOW_SIZE`), Fixes „nicht umgesetzt, Entscheidung beim Anwender".

Heute:
- `TASK/A13.md:3` ERLEDIGT; Abschnitt „Abschluss — 2026-09-11": FIX-01…06,
  Suite 435/0, Gegenmessung QEMU gegen TT in beiden Profilen
  (`measurements/measurement-2026-09-10-qemu/` vorhanden). Die vier
  verbleibenden Fälle 127/133/144/204 sind P1-Stapelreste ohne Soll
  (`aes/gemsuper.c:432`).
- Quittungen `TASK/FIX-01…06-STATUS-DONE.MD` alle vorhanden.
- D-A13-1: `tos-api/src/aes/window.rs:3697-3709` prüft `win(handle)` vor der
  Rechteckliste (gelesen).
- D-A13-2: `window.rs:1677-1692` `subtract` erzeugt TOP, LEFT, RIGHT, BOTTOM
  mit Verweis `gemwrect.c:30-33` (gelesen).
- D-A13-3: `DROP_SHADOW_SIZE` `window.rs:243` (= 2).
- Pi: `measurements/measurement-2026-09-11-pi4/run1-rlist-fixed/` (Diff
  `DIFF-gegen-TT-und-QEMU.txt`): 166/174/183/213 bitgleich zum TT; Rest sind
  127/133/144/204 und die Geräteparameter 201/208.

Nicht mehr offen aus der Quittung, aber nie gemessen (Schluss: kein
Schließungshindernis): drei oder mehr überlappende Fenster (Weg `subtract` vs.
`everyobj`) — das war A22 (ERLEDIGT 2026-09-12); Spec-Lücken
`window-graphics.md:341,848-851` waren Researcher-Meldungen, kein A13-Auftrag.
Die UNFINISHED-Quittung ist durch den „Abschluss" im Ticket überholt.

## A14 — `WF_NEXTXYWH`

**Verdict: CLOSE.**
Quittung: F4/F5 stehen aus, L1 (keine verschränkte Zwei-Fenster-Messung).

Heute: `TASK/A14.md` Abschluss 2026-09-11 (ERLEDIGT). D-A14-2 = derselbe Fix
wie D-A13-1 (FIX-01, A23 Abschnitt 2); D-A14-1 (Cursor je Fenster) = FIX-03,
Cursor im Fenster-Slot (`window.rs:479-486`, gelesen). L1 = Messlücke
[A24](../../TASK/A24.md), Kopf ERLEDIGT (2026-09-12). L2/L3 sind in der
Quittung selbst als „ohne Soll" geführt; L4 = A22 (ERLEDIGT). Pi-Beleg wie A13.

## A15 — Listenende und `OUT_LEN`

**Verdict: CLOSE.**
Quittung: F-A15-1 (P2-Pfad) ausstehend, P2 nicht gemessen.

Heute: `TASK/A15.md` Abschnitt „Nachtrag — F-A15-1 ist umgesetzt (Stufe 5)"
(FIX-05) und „Abschluss — 2026-09-11". `window.rs:3711-3740` schreibt bei
`Owns::Exhausted` vier Wörter, bei `Owns::Empty` nur `w`/`h` (gelesen) —
genau das P1/P2-Modell aus `gemwmlib.c:1230-1241`. Offen blieb in der
Quittung die Messung von P2 auf echtem EmuTOS: das braucht eine neue
Falltabelle und ist Messdesign, kein A15-Defekt; Rest (zufälliger Stapelinhalt
in `int_out[5..6]`) ist Nicht-Soll. Kein Schließungshindernis.

## A25 — `appl_init` Wiederaufruf

**Verdict: NEEDS RUN** (klein, kein Hardwarelauf nötig).
Quittung: implementiert und hostverifiziert, „reale Gegenprobe fehlt", nicht
deployed/committet.

Belegt:
- Fix im Code: `tos-api/src/aes/pd.rs:355-374`, `slot.flags |= AP_OPEN`, kein
  `queue.clear()` (EmuTOS `aes/gemaplib.c:54-63`).
- Tests vorhanden: `tos-api/tests/aes_appl_init.rs`, Unit-Test in `pd.rs`
  (`flags == AP_OPEN | AP_MESAG`, `pd.rs:400`). Arbeitsbaum zu `tos-api/` ist
  sauber (`git status`), der Fix ist also committet.
- A28/A29 bauen darauf auf und trennen die „Erhaltung" (A25) von der
  Rücksetzung an der Startgrenze ausdrücklich (`aes_launch_boundary_reset.rs`
  Phase 2).

Offen, einzige Zeile: der Diagnoselauf „appl_init, Nachricht an eigene PID,
appl_init, lesen, appl_exit". Gefunden habe ich keinen späteren Beleg
(`sdk/apps/apitest/apitest.c` hat nur T28 für `ap_private`, keinen
Wiederaufruf). Schluss (nicht gelesen, gefolgert): der Pfad ist reine
AES-Software ohne Geräteanteil; ein `#Z`-Autorun in QEMU raspi4b ohne Eingabe
genügt. Kleinster Schritt: ein APITEST-Fall (~20 Zeilen C) plus ein QEMU-Lauf.
Größe S. Alternativ Anwenderentscheid „durch den Hosttest über den echten
Dispatcher hinreichend belegt" — das widerspräche aber dem Satz im Ticket
„Abschluss erst mit tatsächlich ausgeführter Hardware-Gegenprobe".

## A28 — Zustandsrücksetzung an der Grenze Desktop → Kind

**Verdict: CLOSE-SUPERSEDED** (DESK43 U9/U10, ADR-026).
Quittung: Implementierung (`desktop_relinquish`/`desktop_resume`, `register`
mit `flags = 0`) fertig; Abnahme am Doppelklick-Pfad nur auf echter Pi.

Heute:
- Die Quittungsstelle gibt es nicht mehr: `kernel/src/desk/` ist gelöscht
  (`ls` → nicht vorhanden; `desktop_relinquish`/`desktop_resume` finden sich
  nur noch in Tests/Doku). DESK43 U10 `0a3b9be9`.
- Die EmuTOS-Rücksetzung steht jetzt dort, wo EmuTOS sie hat — im Epilog des
  Desktops selbst: `programs/desktop/src/lib.rs:455-482`
  (`menu_bar(…,0)`, `wind_set_newdesk(0,0)`, **`appl_exit()` als letzter
  Schritt**, `deskmain.c:2159-2166`); der Kommentar `:472-476` nennt A28.
- Zweite Hälfte: `tos-api/src/aes/pd.rs:613-630,703-707` setzt
  `flags = 0` im `register` (`gemshlib.c:636`).
- Tests behalten das Modell: `tos-api/tests/aes_launch_boundary_reset.rs`
  (zuletzt angefasst in `0a3b9be9`, `desktop_relinquish` als Testhilfe `:339`).
- Gerätehälfte: DESK43 U9-6 (echte Pi 4, Kern `f73bb075`, Anwender „alles ohne
  Fehler"; Programmstart/-ende QED, Doom, zView, Ball, Present, CoreMark,
  EmuCON, TeraDesk → QED → zurück) und U10 auf der Pi am 2026-10-16
  (`TASK/DESK43.md` Abschnitte „Hardware-Abnahme U9-6", „U10").
- Seit ADR-026 endet der Desktop vor dem Kind wie in EmuTOS (D6); die in der
  Quittung genannte Nebenbeobachtung „`.NCP`-Pfad relinquisht nicht" gibt es
  im alten Sinn nicht mehr.

Vorbehalt (Schluss): der exakte Auslöser aus der Quittung — verdecktes Fenster
anklicken (ungelesene `WM_REDRAW`), daraus QED starten — ist in der
U9-6-Prüfliste nicht ausdrücklich genannt. Die Rücksetzung ist jedoch im
Code und im Hosttest belegt; wer ganz sicher sein will, nimmt den Griff als
einen Punkt in die ohnehin geplante Pi-Sammelabnahme.

## A29 — Programmende löscht den Desktop-Deskriptor

**Verdict: CLOSE-SUPERSEDED** (DESK43 U9-6/U10 auf der Pi).
Quittung: implementiert (`7c28493e`), Hardwareabnahme fehlt.

Heute:
- Code: `pd::close` (`tos-api/src/aes/pd.rs:548-555`: Queue leeren, `AP_OPEN`
  löschen, Slot bleibt) und `close_if_open` (`:589-…`) — EmuTOS `ap_exit`
  `gemaplib.c:251-261`, `sh_ldapp` `gemshlib.c:650-657`.
- Test: `tos-api/tests/aes_app_return_descriptor.rs` vorhanden.
- Folge der Deskriptorerhaltung (Obergrenze) wurde A62.
- Gerätehälfte: der Zustand „Desktop empfängt nach der Rückkehr noch
  Nachrichten" ist seit ADR-026 anders gelagert (der Desktop wird nach dem
  Kind neu gestartet), und die Pi-Abnahmen U9-6/U10 (Menüs, Fenster, Starts,
  Rückkehr) liefen ohne Fehler.
Nicht gefahren: eine gezielte Pi-Messung nur für A29. Schluss: nicht nötig,
weil die Quittungshypothese („still verlorene `MN_SELECTED`/`WM_*` nach
Rückkehr") im neuen Aufbau durch jeden Menüklick nach einer Rückkehr
beobachtet wird.

## A32 — Abnahme der Rechtecklisten-Reparatur auf echter HW

**Verdict: CLOSE.**
Quittung (2026-09-12): 3 von 4 Punkten belegt, Punkt 4 (`DROP_SHADOW_SIZE`
2 → 4) offen mit Frage an den Anwender.

Heute: `TASK/A32.md:3` ERLEDIGT (2026-09-19). Punkt 4 entschieden: bleibt 2
(„zu viel", Anwenderzitat im Ticketkopf); im Code `window.rs:243` = 2 mit
`SHADOW_TH = DROP_SHADOW_SIZE / 2`. Punkte 1–3 durch
`measurements/measurement-2026-09-11-pi4/run1-rlist-fixed/` belegt. Einzige
Kleinigkeit in der Quittung: der Pixelvergleich der alten Screendumps gegen
`measurement-2026-09-10-qemu/` wurde „nicht ausdrücklich bestätigt" — Stufe 6
ändert kein Pixel; Schluss: kein Hindernis.

## A35 — `ap_ptree`/`ap_rscmem`/`ap_rsclen`

**Verdict: CLOSE.**
Quittung (2. Fassung, 2026-09-13): „WEITERHIN NICHT IMPLEMENTIERT", zwei
Adressfragen offen.

Heute: `TASK/A35-STATUS-DONE.MD` (2026-09-14) hat beide Fragen entschieden und
umgesetzt (NRSC v7/v8-Zeigertabelle `c60daa7f`; EL1-Pfad schreibt ehrlich
nichts). Code: `report_load` `tos-api/src/aes/resource.rs:2478`; Tests
`tos-api/tests/aes_rsrc_global_fields.rs` (4 Tests, **kein `#[ignore]`** mehr),
`aes_global_layout.rs`. Die UNFINISHED-Datei ist veraltet und widerspricht dem
Kopf (ERLEDIGT). Rest aus der DONE-Quittung: direkter Maschinenbeleg der drei
Wörter (APITEST-Fall, nicht gebaut) und interaktiv auf der Pi (TeraDesk ist
der Leser, `icon.c:2692`). TeraDesk läuft auf der Pi (U9-6). Schluss:
Rest ist Komfort, nicht Schließungsbedingung.

## A62 — AES-Prozesstabelle vs. GEMDOS-Tabelle

**Verdict: CLOSE** (ein Rest ausgelagert).
Quittung: Stufe 1 und 2 implementiert, QEMU-Belege, Pi-Abnahme offen.

Heute:
- Code: `tos-api/src/aes/pd.rs:64` `KERNEL_CONTEXT = MAX_PROCESSES` (Tabelle
  `MAX_PROCESSES + 1`, Identität = GEMDOS-Slot; umbenannt von `DESKTOP` durch
  DESK43/ADR-026, `d11ed66b`). Test `tos-api/tests/aes_pd_by_slot.rs`.
- QEMU-Beleg: `measurements/measurement-2026-09-20-a62/` (`NESTTST` Tiefe
  1–16, `serial/nest-serial.log`).
- Pi: nicht per `NESTTST` gefahren; die verschachtelte Startkette TeraDesk →
  QED lief auf der Pi (U9-6, „alles ohne Fehler"). Schluss: die Tabellenlogik
  hat keinen Geräteanteil.
- Ausgelagerter Rest (nicht A62): Stufe 2/A56 nennt `appl_write(1,
  WM_REDRAW)` → `w_redraw_desktop` (`hctl_msg`, `gemctrl.c:380-384`) als
  weder spezifiziert noch gebaut; Code bestätigt es:
  `tos-api/src/aes/appl.rs:429-439` („has no spec entry yet and is not
  reproduced here"). EmuDESK ruft `appl_write` nie
  (`references/emutos/desk/` nur `gembind.c:130`), Wirkung also nur für
  Drittprogramme. Kein Ticket gefunden. Vorschlag: bei Bedarf Researcher-Spec
  + kleiner Bau (S), nicht im A62-Kopf halten.

## ARCH03-3b / ARCH03-3c / ARCH03-POSTE9

**Verdict: CLOSE** (alle drei).
Alle drei Quittungen tragen oben einen Lead-Vermerk vom 2026-10-13 „Abschluss";
ich habe ihn nachgeprüft statt übernommen:
- Commits existieren und liegen auf `main`: `ae097d0f` (E10, 2026-10-05),
  `7e97a682` (p_parent, 2026-10-06), `50207c5f` (crt0 `exit()`),
  `9882f5ec` (mintlib-`tfork`), Abnahmestand `f721c5af` (2026-10-09).
- E10-Verdrahtung im Code: `tos-api/src/aes/object.rs:2756-2791`
  (`memprot::grant_temp`/`revoke_temp` um das fremde `G_USERDEF`).
- Abnahme: `measurements/measurement-2026-10-13-arch03-acceptance/RUNINFO.TXT`
  — MPROT3B, E10USR, PORT10 (44), VFORKT, ASID-Reihen PASS in allen vier
  Modi; S2ALL nur durch eine veraltete Sonde (ENVKID) rot, mit ENVKIDJ PASS,
  Verweigerungen in strict beobachtet (Zeilen ~63-90 der Datei).
- Rest aus POSTE9: `Fseek` auf Konsolenhandles → `FS10` ERLEDIGT 2026-10-13;
  MD-Erschöpfung → `MEM05` ERLEDIGT 2026-10-14; `p_parent` Weg B → PROC03
  (unten, offen).
- Echte Pi: die Vorführkarte vom 2026-10-08 enthält diese Commits (Angabe im
  Lead-Vermerk, nicht von mir geprüft); **`asid=on` auf echter Pi ist offen**
  und steht im ARCH03-Kopf, nicht in diesen Quittungen.
Nur noch Aufräumen: die Dateien tragen den Suffix „UNFINISHED", obwohl sie
abgeschlossen sind (Vermerk sagt, der Name bleibe wegen Verweisen).

## FS04 — Kopieren über die Partitionsgrenze

**Verdict: CLOSE.**
Quittung: Messung vollständig, Schicht nicht lokalisiert, Pi-Lauf nötig.

Heute: Kopf BEHOBEN (2026-09-25, auf der Pi). Beleg:
`measurements/measurement-2026-09-25-fs04/raw/pi05-FS04-after-fix.TXT`
(2182 → 467 ms, 0 Timeouts, 0 Fehler). Fix im Treiber gelesen:
`kernel/src/drivers/bcm2711/emmc.rs:306-312,425-448` — die Wartephasen je
Sektor nutzen `Budget` (echte Dauer über `CNTFRQ_EL0`), `SPIN_LIMIT` bleibt
nur für die Initialisierung (mit ausdrücklichem Kommentar). Commits
`0b179c31` (Ticket), `6642409b`. Reste sind ausgeliehene Tickets: HW05
(Schreibdurchsatz, ZURÜCKGESTELLT), HW06 (ZURÜCKGESTELLT), GUI06 (BEHOBEN,
Pi-Abnahme offen), FS05 (OFFEN, P3) — alle vorhanden.

## LDG-M3 — WinDom + LDG-Demo

**Verdict: CLOSE-SUPERSEDED.**
`LDG-M3-STATUS-DONE.MD` (2026-10-06) nennt die beiden Blocker B1
(`EV_MB`-Tastenzustand → `LDG-M3-B1-STATUS-DONE.MD`) und B2 (Modul-Heap →
`LDG-M3-B2-STATUS-DONE.MD`) als behoben, Abnahme in QEMU an `EX_LIB.NLD` und
`POPUP.NLD` (sinus 0.479426 / cosinus 0.877583) in `memprot` on/strict/off,
LDGTEST 69/69, Belege `measurement-2026-10-06-aes-evbutton`,
`-libshare`. Rest dort („Informations" leer, `RsrcXload`, `appl_tplay`) ist
ausdrücklich „nicht Teil von M3". Die UNFINISHED-Quittung bleibt Vorgeschichte.

## M08 — `mkrusttests`/Pufferspalte

**Verdict: STILL OPEN** (Größe S).
Quittung: Werkzeug fertig, zwei Tabellen verdrahtet, dritte
(`aes-m07-parameter.tsv`) wegen B-1 (Mauszeiger parkt 1 px daneben) und B-2
(Fall 69) nicht.

Heute:
- B-1: behoben, V17 `02903d55` (2026-09-14), `vdi/mouse.rs::park_position`.
- B-2: offen. `tos-api/tests/aes_reference.rs:11-30` sagt es selbst: die
  Tabelle läuft allein **20 von 20 grün**, ist aber nicht verdrahtet, weil Fall
  69 (`evnt_multi(MU_MESAG|MU_TIMER)` mit gepflanztem `MME_PBUFF`) neben
  `cases_m05_addrin` rot wird — `wm_new` leert die Nachrichtenschlange nicht
  (`gemwmlib.c:1874-1892`), ein übrig gebliebenes `WM_REDRAW` feuert.
- Es gibt kein `cases_m07_parameter`-Modul im Ordner
  (`tos-api/tests/aes_reference/` hat nur `cases_m05_addrin.rs` und `cases_m06_sysvar.rs`).
Kleinster nächster Schritt: im Harness vor Fall 69 die Schlange per
`evnt_multi(MU_MESAG|MU_TIMER, timer 0)` bis „nur Timer" abziehen (so schon in
der Quittung §4 vorgeschlagen), Modul erzeugen, `--shuffle` zehnmal fahren.
Danach Tester-Abnahme (steht ebenfalls aus). Kein Produktcode.

## PORT09 — `environ`/ARGV

**Verdict: CLOSE.**
`TASK/PORT09.md:3` ERLEDIGT 2026-10-06; `PORT09-STATUS-DONE.MD`: `cmdargs.c` ist
mintlibs `parseargs()`, Vorbedingung (Kind liest `p_env`) durch ARCH03 Stufe 1
erfüllt, alte Probe läuft durch (alle 17 Einträge). Die UNFINISHED-Quittung
(2026-10-02, „gestoppt vor der Umsetzung") ist Vorgeschichte und steht im Kopf
des Tickets als solche.

## Q06 — QED `restore_edit`

**Verdict: CLOSE** (Vorbehalt: kein ausdrücklicher Pi-Eintrag).
Quittung: Ursache bestimmt (`setmin`/`setmax` mit 32-Bit-`long`/Big-Endian),
Overlay `sdk/overlays/qed/set.c`; Pi-Abnahme offen, „in QEMU nicht auslösbar
mangels Eingabe".

Heute:
- Overlay vorhanden (`sdk/overlays/qed/set.c`), `sdk/apps/build-qed.sh:22`
  nennt es.
- Die Prämisse „QEMU hat keine Eingabe" ist überholt: seit 2026-09-17 gibt es
  den Scratch-IKBD-Injektor. Der Lauf
  `measurements/measurement-2026-10-08-keyowner/` (`qedo`-Serie) hat QED
  Tippen+Speichern+Öffnen gefahren; ich habe
  `screendumps/head-qedo-Q03_EDITED.png` angesehen: Fenstertitel
  `*C:\APPS\QED\NOTE.TXT` (Stern), Kopfzeile `Z: 2  S: 1`, getippte Zeichen
  `ab` sichtbar in Zeile 1. Das sind genau die drei Vorhersagen der Quittung
  (Zeichen, `*` im Titel, Z/S-Kopfzeile folgt dem Cursor).
- Pi: HW-TEST-LOG führt am 2026-09-12 „getippter Text erzeugt keine Zeichen"
  (vor dem Fix); ein späterer Pi-Eintrag zu getipptem Text fehlt. Schluss
  (gefolgert): die Ursache war reine Anwendungsarithmetik, hardwareunabhängig;
  der QEMU-Beweis genügt. Ist dem Anwender ein Pi-Eintrag wichtig, ist es ein
  Punkt für die Sammelabnahme (Zeichen tippen, Stern, Kopfzeile).

## X01 — `Super`/`Supexec`

**Verdict: STILL OPEN** (Größe S; Entscheidungen beim Anwender).
Quittung: Umsetzung im Overlay, QEMU-Belege (SUPER 10/0/0); offen zwei
Entscheidungen, drei nur-Pi-Aufrufstellen, Tester-Abnahme.

Heute:
- Umsetzung vorhanden: `sdk/libc/overlay/include/mint/osbind.h` (Commit
  `e503efdc`), Hinweis `:804-805` auf die scharfe Abschaltung.
- Kernel unverändert (`tos-api/src/xbios/mod.rs:131` nur Kommentar); die Spec
  `docs/api/xbios/26-supexec.md` steht auf „decision deferred".
- Nichts davon seither angefasst: kein Commit mit `X01` nach `e503efdc`, keine
  `lobo`/Shutdown-Erwähnung in HW-TEST-LOG, TASK-Quittungen, `docs/analysis`
  (gesucht). U9-6 hat Menüs, Programmstarts, Ctrl-C geprüft, **nicht**
  „Quit → Shutdown".
Was bleibt:
1. Anwenderentscheid: TeraDesks Abschaltpfad (`main.c:2118` `Supexec(rv)` →
   `lobo()`, Endlosschleife) ist jetzt tatsächlich scharf.
2. Anwenderentscheid: Cookie-Spiegel als Momentaufnahme (ADR-012-Rest).
3. Pi: `main.c:2090/2118`, `file.c:945/969`, `video.c:289/318` nur auf echter
   Pi erreichbar (nicht gefahren).
4. Tester-Abnahme.
Kleinster Schritt: Entscheid 1 (eine Frage), dann eine Pi-Sitzung mit drei
Griffen (Quit → Shutdown, Medienwechsel, Videodialog). Größe S plus
Anwenderzeit.

## ZVIEW-M4 — zView + Codecs

**Verdict: CLOSE-SUPERSEDED.**
Blocker K1 (`graf_handle`/`vq_extnd`) behoben:
`ZVIEW-M4-K1-STATUS-DONE.MD`; `ZVIEW-M4-STATUS-DONE.MD` (2026-10-07)
meldet 15/15 Muster pixelgenau (memprot on), 3/3 strict, Beleg
`measurements/measurement-2026-10-07-k1/`. Ausdrücklich Rest dort (nicht
Schließungshindernis): Upstream-Fehler U1 (`tgaldg.c`), tiff und PDF nicht
gebaut, Browser/Speichern/Diashow ungefahren. zView lief außerdem in der
Pi-Abnahme U9-6.

## DESK57 — Datei auf Desktop-QED-Symbol

**Verdict: CLOSE-SUPERSEDED** (durch DESK62).
DESK57 (2026-10-09, Pi): Zoom-Animation, Sanduhr, dann nichts. DESK62
(2026-10-16, Pi-Abnahme U9-6) beschreibt dasselbe: Verknüpfung auf dem
Desktop, Datei darauf gezogen, Animation, dauerhafte Sanduhr, QED startet nicht.
Ursache (`c5018c19`): `fun_file2desk`/`fun_desk2win` verwarfen die Antwort
von `do_aopen`, `fun_drag` gab `false`. Fix `a8b03c81` (2026-10-16): modulweites
`EXIT_DESKTOP`, im Code `programs/desktop/src/dir.rs:1658,1683,1706,1989,2103,2173`
(EmuTOS `deskfun.c:61,967,1034,1249,1304,1338`). Belege
`measurements/measurement-2026-10-16-desk62{,-fix}/`; auf der Pi abgenommen:
Drop auf Verknüpfung startet QED mit Datei, Pfeil kehrt zurück. DESK57s
Hypothesen (Speicherschutz, fehlender Tail, geschluckter Fehler) sind damit
beantwortet: weder noch, sondern verworfenes Rückgabekennzeichen. Zu tun:
DESK57-Kopf auf „Duplikat von DESK62" setzen.

## PROC03 — Desktop/AES als echter EL0-Prozess

**Verdict: STILL OPEN — umzuformulieren auf die AES-Hälfte** (Größe M–L).
Ticket: „Weg B", Desktop/AES als echter Prozess mit eigenem PD.

Heute:
- Desktop-Hälfte geliefert: der Desktop ist der EL0-Prozess NEWDESK (ADR-026,
  `docs/MEMORY.md:2229`; DESK43 U9/U10, Pi-abgenommen 2026-10-16). Er und jeder
  Programmstart laufen über `kernel/src/launch.rs` `launch()`
  (`run_desktop_program` `:152` und `run_program_with_tail_rc` `:127` rufen
  beide `launch`), das `p_parent` bei `:257` mit
  `pexec::set_kernel_parent` auf den AES-PD-Stellvertreter setzt (EmuTOS
  `gemshlib.c:645`). Das war PPARENT (Weg A, `7e97a682`).
- Offen bleibt die AES-Hälfte: der Shell-Loop läuft weiter als Kernel-Kontext;
  die beiden PDs (`initial_basepage`, AES-PD) sind **nie der laufende
  Prozess** (`tos-api/src/bdos/process/standin.rs:1-30`: „Way B — the AES as a
  real process — is `TASK/PROC03.md`"). Folgen laut
  `docs/api/gemdos/process/_parent-and-environment.md:180,208`: EmuCON-`Pexec`
  bekommt den AES-PD statt eines EmuCON-PD; AES-PD hat `p_hitpa = p+0x100`,
  `p_tbase = 0`; `docs/COMPATIBILITY.md:716` verweist auf PROC03.
- Der Ticketkopf ist veraltet (er behauptet „NewTOS hat weder AES-PD noch
  `initial_basepage`", „Lader schreibt 0"): schon `docs/analysis/
  desk43-u5-u10-review-2026-10-14.md` (Zeile 301) und Entscheidung D7 in
  `TASK/DESK43.md:221` haben „PROC03 umformulieren (nur noch AES-Hälfte)"
  vorgesehen; umgesetzt ist das im Ticket nicht.
- Lesehinweis: dass beide Pfade durch `launch` gehen, habe ich gelesen
  (`kernel/src/launch.rs:127-157,257`); dass NEWDESK tatsächlich den
  Desktop-Arm nimmt, folgt aus dem Funktionskopf und der DESK43-Doku, ich habe
  den Aufrufer im Shell-Loop nicht einzeln nachverfolgt.
Kleinster nächster Schritt: PROC03 neu fassen (nur AES-PD als echter Prozess,
Voraussetzung DESK43 U9 erfüllt); kein Zeitdruck, kein sichtbarer Defekt
(Beobachtbar nur über `p_parent`-Ketten und Parent einer EmuCON-`Pexec`).

---

## Außerhalb des Scopes (je eine Zeile)

- ARCH03: `asid=on` auf echter Pi und Stufe 2b offen (im Kopf von
  `TASK/ARCH03.md`), nicht Teil dieser Quittungen.
- `docs/api/aes/window-graphics.md` ~1224-1240 beschreibt noch die in U10
  gelöschten `wm_new_keep_desktop`/`desktop_resume` (steht in `DESK43.md` als
  offen).
- Spec-Lücken aus den A13-A15-Quittungen (`window-graphics.md:341,848-851`,
  Zwei-Wort-Regel von `w_owns`) habe ich nicht auf Behebung geprüft.
- Dateinamen `*-UNFINISHED.MD` bleiben als Vorgeschichte stehen; wenn die
  Lead-Anwendung der Urteile gelingt, sollten die Ticketköpfe der geschlossenen
  Tickets einen Satz „Quittung veraltet, siehe …" bekommen (A13-A15, A35,
  FS04, PORT09, LDG-M3, ZVIEW-M4).
