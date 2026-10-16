# Review der offenen Ticketliste — 2026-10-17

Auftrag (Anwender, 2026-10-17: „ich glaube, viele davon sind schon erledigt"):
jedes in `docs/STATUS.md` unter „Offen (Stand 2026-10-17)" geführte Ticket
prüfen — außer MEM02 (läuft gerade) — und entscheiden, ob die Arbeit wirklich
offen ist oder das Ticket nur nie geschlossen wurde. Gleiche Methode wie
[unfinished-tickets-review-2026-10-17.md](unfinished-tickets-review-2026-10-17.md).

Methode und Grenzen: statisch gelesen (Code `file:line`, `git log`, Messordner,
EmuTOS unter `references/emutos`). **Es wurde nichts gebaut, kein Test und keine
Maschine gefahren** (geteilter Arbeitsbaum, MEM02 läuft parallel). Kennzeichnung:
**[gelesen]** = Stelle in dieser Sitzung gelesen; **[Schluss]** = meine
Folgerung; **NEEDS RUN** = nur ein Lauf entscheidet, der Lauf ist benannt.
`docs/STATUS.md` und Ticketköpfe gelten als Hinweis, nicht als Beleg. Heute
angelegte Tickets (RST01, X04, DESK63, DESK65, DESK66) sind erwartungsgemäß offen;
dort nur eine Einzeilenprüfung.

„Basis" = nötig für getreues EmuTOS-/TOS-Verhalten (Begründung je Ticket).
„Erweiterung" = über EmuTOS hinaus oder reine NewTOS-/SDK-/Werkzeugfrage.

## Zusammenfassung

| Ticket | Urteil | Art | Größe | Kern in einem Satz |
|---|---|---|---|---|
| FS06 | **CLOSE** (Prämisse widerlegt) | (Basis-Rest S) | — | EmuTOS tut bei `Frename` auf leeren Namen dasselbe (Blankname, `E_OK`); die im Ticket zitierte Abweisung `fsdir.c:912-913` ist die Existenzprüfung. Nur ein Rest: Cross-Directory-Fall (EmuTOS `EPTHNF`). |
| ARCH02 | **STILL OPEN** | Basis (Plattformnotwendigkeit) | S–M | Nur der Scheduler sichert `v0..v31`; `handle_pexec`/`finish_terminate` sichern nur die GPR. |
| DESK48 | **STILL OPEN** (teilweise) | Basis | M | `fun_desk2desk` und `fun_selectall` rufen weiter `redraw_desktop()`; `desk_clear` ohne Zeichenhälfte; `fun_rebld(path)`-Semantik fehlt. |
| RST01 | **STILL OPEN** | Basis | M–L | Kein `warm_reset`/`cold_reset`, keine Ctrl-Alt-Del-Behandlung im Baum. |
| X04 | **STILL OPEN** | Basis | S (nach RST01) | Fehlerbild unverändert; **neuer Befund** zur Ursache unten. |
| A25 | **NEEDS RUN** (QEMU) | Basis | S | Fix und zwei Tests im Baum; es fehlt allein der Diagnosefall „appl_init zweimal". |
| M08 | **STILL OPEN** | Basis-Sicherung (Test) | S | `aes-m07-parameter.tsv` weiter nicht verdrahtet; Fall 69 / `wm_new` leert die Schlange nicht. |
| PROC03 | **STILL OPEN** (umformulieren) | Basis | M–L | Desktop-Hälfte geliefert; offen: AES-PD als echter Prozess. |
| X01 | **STILL OPEN** / NEEDS RUN (Pi) | Basis | S | Entscheidungen gefallen; offen nur drei Pi-Aufrufstellen und Tester. Reboot ausgelagert in X04. |
| PORT02 | **CLOSE-SUPERSEDED** | Erweiterung | — | zView entschieden, portiert, auf der Pi abgenommen; Doom/QED/TeraDesk/CoreMark laufen. |
| SND01 | **STILL OPEN** | Erweiterung | L | Nichts gebaut; EmuTOS liefert ohne Soundchip selbst die Funktionsnummer. |
| SND02 | **STILL OPEN** | Erweiterung | M (Recherche) | Nichts gebaut, Quellenlage unverändert (Rang 6). |
| SDL01 | **STILL OPEN** | Erweiterung | L | Nichts gebaut; hängt an SND01 nur für Ton. |
| RTC01 | **STILL OPEN** | Erweiterung | M | Kein I2C/DS3231-Code, Datenblatt nicht beschafft. |
| BALL01 | **NEEDS RUN** | unklar | S | Ursache offen; QEMU-Screendump der ersten Frames bzw. Hatari-TT-Gegenprobe. |
| FS05 | **CLOSE-SUPERSEDED** (FS08 Teil 2) | Basis | — | `fwrite` steppt die Kette; Test `sequential_writes_take_one_step_per_new_cluster` (85 Lookups für 16 Cluster). |
| REF03 | **STILL OPEN** | Hygiene | S | `emmc.rs:71,934,983` zitieren weiter §2.2.15; Aussage „specification never asks" in `emmc.rs:990` steht noch. |
| T15 | **STILL OPEN** | Prüfstand | S | Drei Zeichentests weiter ohne Riegel. |
| DESK49 | **STILL OPEN** | Erweiterung (Werkzeug) | S | `filetypes` der erzeugten `TERADESK.INF` ohne `*.NAP/NCP/NTP/NAC`. |
| DESK51 | **STILL OPEN** | Basis | S–M | Kein `graf_shrinkbox`-Binding; `main.rs:2609` sagt es selbst. |
| DESK52 | **STILL OPEN** | Basis | S | `do_fopen` ohne Laufwerksprobe und `STDEEPPA`. |
| DESK53 | **STILL OPEN** | Basis | S | ESC (`refresh_top`) liest nicht neu ein. |
| DESK54 | **STILL OPEN** | Basis | S | `do_xyfix` ohne die 3D-Korrektur. |
| DESK55 | **STILL OPEN** | Basis | S | Fenstertitel = Verzeichnis, nicht ganze `p_spec`. |
| DESK56 | **CLOSE-SUPERSEDED** (DESK62) | Basis | — | `EXIT_DESKTOP` an allen drei EmuTOS-Zuweisungen gesetzt. |
| DESK61 | **STILL OPEN** | Basis | S | `act_chg` kehrt vor `f_selected` zurück; auch der Rückgabewert weicht ab. |
| DESK63 | **STILL OPEN** | Basis | S | `fun_desk2desk`: `_ => illegal_op_msg()` unverändert. |
| DESK65 | **STILL OPEN** (Idee) | Erweiterung | M | Ausdrücklich „später". |
| DESK66 | **STILL OPEN** | Basis | M | Kein Weg vom Desktop zu `set_aes_background`. |
| CON01 | **STILL OPEN** | Basis | M | `glbcolumn`/Typeahead fehlen; Code sagt es selbst. |
| CON02 | **STILL OPEN** | Basis | S–M | `Cconws` schreibt weiter auf CON statt `p_uft[1]`. |
| FS12 | **STILL OPEN** | Basis | S–M | `MAX_DRIVES = 16`, `BLKDEVNUM = 16` im Baum; EmuTOS 26. |
| FS13 | **CLOSE** (Verhalten = EmuTOS) | Basis | — | EmuTOS liefert in genau dieser Folge ebenfalls `EINTRN`. |
| LIBC-IOCTL | **STILL OPEN** | Erweiterung (SDK) | S–M | Shim reicht nur an `Fcntl` weiter. |
| LIBC-BUF | **STILL OPEN** (NEEDS RUN, QEMU) | Erweiterung (SDK) | S | Fall `fopen`-Datei ungeprüft; [Schluss]: wahrscheinlich nicht betroffen. |
| MEM06 | **STILL OPEN** (Idee, nichts zu tun) | Erweiterung | — | Richtungsnotiz. |
| MEM07 | **STILL OPEN** | Erweiterung | S | Grenze 64 unverändert; Verbraucher nicht ausgezählt. |
| MEM08 | **STILL OPEN** | Erweiterung | S (messen) | Lineare Suche unverändert, nie gemessen. |
| MEM09 | **CLOSE-SUPERSEDED** (MEM05) | Erweiterung | — | Einziger Aufrufer läuft nach `reserve()`; Invariante dokumentiert. |
| MEM10 | **STILL OPEN** | Erweiterung | M | Gemessen: 23,3 s statt 0,06 s für 8189 `Mxalloc`+`Mfree` mit Schutz. |
| IO01 | **STILL OPEN** (Idee) | Erweiterung | L | Kein Auftrag bis Freigabe. |
| ARCH03 2b | **STILL OPEN** | Erweiterung (FreeMiNT) | M | Pexec-Modi 100+ nicht im Baum. |
| ARCH03 `asid=on` Pi | **NEEDS RUN** (Pi, Anwender) | Erweiterung | S | Standard aus; kein Pi-Lauf belegt. |

Ergebnis: von 41 Tickets plus ARCH03-Rest sind **6 schließbar** (FS06, FS13, FS05,
DESK56, MEM09, PORT02), **3 brauchen einen Lauf** (A25, BALL01, ARCH03-Pi; X01
und LIBC-BUF teils), alle übrigen sind wirklich offen. Die zwei sachlichen
Überraschungen: **FS06 und FS13 sind keine Defekte** (NewTOS = EmuTOS), und
**X04/RST01 haben einen Quellenbefund**, der die bisherige Spec korrigiert
(Abschnitt X04).

---

## FS06 — `Frename` auf leeren Namen

**Verdict: CLOSE** (Prämisse des Tickets widerlegt) — optional ein kleiner Rest.

Ticket: `Frename("C:\README.TXT","C:\")` liefert `E_OK` und legt einen Eintrag
mit leerem 8.3-Namen an; „EmuTOS weist es ab: `xrename`, `fsdir.c:912-913`".

Befund [gelesen]:
- NewTOS-Seite stimmt: `fcb_name("")` liefert `Some([b' ';11])`
  (`tos-api/src/bdos/fs/dir.rs:148-173`, unverändert; kein Commit mit „FS06").
- **EmuTOS-Seite stimmt nicht.** `fsdir.c:912-913` ist
  `if (!ixsfirst(p2,FA_SUBDIR,0)) return EACCDN;` — „neuer Pfad existiert schon"
  (`ixsfirst` liefert 0 = gefunden, `fsdir.c:452-499`). Für den leeren Namen
  ist `scan` mit `builds("")` = elf Blanks erfolglos (`fsdir.c:1525-1575`;
  `builds` `:735-795`), `ixsfirst` meldet `EFILNF`, `xrename` läuft weiter.
  Im Fall „rename within directory" (`fsdir.c:1076-1083`) schreibt EmuTOS dann
  `builds(s2,buf)` — Blanks — per `update_fcb` über den Eintrag und liefert
  `ixclose` = `E_OK`. Nur `contains_illegal_characters(s2)` (`:983`) greift,
  und die lässt „" durch. **EmuTOS erzeugt denselben Blankname-Eintrag.**
- Die Abweisung leerer Namen sitzt in `ixcreat`: `if (!*s || *s=='.') return
  EPTHNF` (`bdos/fsopnclo.c:120-121`). NewTOS hat sie für `Fcreate`
  (`file.rs:336`) und `Dcreate` (`dirops.rs:50`).
- `Fopen`/`Fdelete`/`Fattrib`/`Fsfirst`: leerer Name → `scan` erfolglos →
  `EFILNF`/`EPTHNF` wie EmuTOS; das steht in `path.rs:112-130` und
  `file.rs:645-668` ausdrücklich so beschrieben.

Rest [Schluss, nicht gelaufen]: Beim **Cross-Directory-Rename** auf leeren
Zielnamen (z. B. `C:\SUB\X` → `C:\`) geht EmuTOS über `ixcreat(p2,att)` und
liefert `EPTHNF` (`fsdir.c:1041-1046` mit `fsopnclo.c:121`). NewTOS
(`file.rs:750-830`) kopiert den FCB mit `name2 = [b' ';11]` in das Zielverzeichnis
und liefert `E_OK`. Das ist die einzige Abweichung; Größe S, Basis; kein
Anwenderproblem bekannt. Der Desktop erreicht den Aufruf, weil seit DESK42 der
EmuTOS-Kasten den leeren Namen durchlässt — also genauso wie auf EmuTOS.
Wenn der Anwender trotz „1:1" einen leeren Namen im Desktop verhindern will, wäre
das eine benannte **Erweiterung** (Prüfung im Desktop-Dialog), nicht ein
Kernfix — Entscheidung beim Anwender.

Empfehlung: Ticket schließen mit Vermerk „kein Defekt, EmuTOS-gleich"; optional
Folgeticket für den Cross-Directory-Rest.

## ARCH02 — FP/SIMD-Register über `Pexec`

**Verdict: STILL OPEN.** Art: Basis (kein EmuTOS-Gegenstück — m68k hat dieses
Problem nicht —, aber ohne Fix bekommen portierte Programme falsche Daten; das ist
Plattformkorrektheit des TOS-Vertrags „Pexec kehrt mit intaktem Aufrufer zurück").
Größe S–M.

Belege [gelesen]:
- `save_fp_context`/`restore_fp_context` kommen im Kernel nur an zwei Stellen
  vor: `kernel/src/arch/aarch64/exceptions.rs:984` (`save_yield_frame`) und
  `:1223` (Switch-Wiederaufnahme).
- `handle_pexec` sichert den Aufrufer nur als GPR/`lr`/`elr`/`spsr`/`sp_el0` in
  `parent_frame` (`exceptions.rs:2041-2051`), `finish_terminate` stellt nur diese
  wieder her (`:1489-1540`). Kein `saved_fp`-Zugriff.
- Der Kernel selbst ist `nofp`, die FP-Register tragen also den Stand des
  zuletzt laufenden EL0-Kindes in den Aufrufer zurück.
- Commit `27117993` (Pexec-Eltern suspendieren) nennt ausdrücklich: „It was not
  ARCH02: no FP/SIMD register is lost, and that ticket stays open untouched."
  `84baa347` stellt GPR für EL1-Aufrufer wieder her, nicht FP.
Kleinster Schritt: Test zuerst (Programm hält FP-Konstante über `Pexec`, scheitert
vor dem Fix), dann `FpContext` in `parent_frame` neben die GPR legen. Kein Lauf
nötig für die Diagnose.

## DESK48 — vier `redraw_desktop()`-Stellen

**Verdict: STILL OPEN** (teilweise überholt). Art: Basis (EmuTOS sendet nur
`WM_REDRAW` je Fenster, `deskfun.c:607-631`, `:985-998`; Laufzeit/Optik).
Größe M. Die Sperre „nicht vor OFAM" ist mit 9./10. Oktober abgelaufen.

Pfade haben sich verschoben (Desktop ist `programs/desktop/src/`). Heute
[gelesen]:
- (1) `fun_desk2desk` (`dir.rs:2207-2274`) ruft weiter `redraw_desktop()`
  (`dir.rs:2272`). EmuTOS' Ersatz `desk_clear(DESKWH)` hat die Zeichenhälfte;
  NewTOS' `desk_clear` (`main.rs:2329-2335`) hat sie weiter nicht.
- (2) `desk_clear_all` (`main.rs:3504-3519`) ist inzwischen umgebaut: es zeichnet
  selbst (`win_shwall`, `do_wredraw(DESKWH,…)`) und ruft kein `redraw_desktop()`
  mehr. Der Ersatz liegt also in `desk_clear_all`, nicht in `desk_clear` wie bei
  EmuTOS [Schluss: Punkt 2 ist erledigt, Reihenfolge von Punkt 1 gilt weiter].
- (3) `fun_selectall` (`dir.rs:1575-1585`) ruft weiter `redraw_desktop()`
  (`:1584`); EmuTOS schickt `WM_REDRAW` über das Fensterrechteck
  (`deskfun.c:626-630`). Offen.
- (4) `fun_win2win` (`dir.rs:1957-2013`) und `fun_win2desk` (`:2019-2035`) rufen
  `rebuild_window(handle)` je Handle; EmuTOS `fun_rebld(p_spec)` (`deskfun.c:993-998`)
  baut **alle** Fenster mit diesem Pfad, und `if (datype==AT_ISFOLD)
  fun_rebld(destpath)` fehlt. Offen. (`fun_rebld(path)` selbst existiert,
  `dir.rs:2390`.)
- Neu, nicht im Ticket: `fun_del_desktop` (`dir.rs:1518`) ruft ebenfalls
  `redraw_desktop()` — nicht gegen EmuTOS geprüft.
Bereits geliefert: `b9a2444c` (GUI06-Nachfolger) hat die zwei Vollbild-Aufrufe im
Drop-Pfad entfernt.

## RST01 — Neustart (Warm/Kalt, Ctrl+Alt+Del)

**Verdict: STILL OPEN** (erwartet). Basis: EmuTOS `bios/ikbd.c:740-744`
(`cold_reset()`/`warm_reset()` bei Ctrl+Alt+Del) [gelesen]. Größe M–L (Spec,
Track B Pi-Neustart, Anwenderentscheid warm/kalt, Bau).
Einzeilenprüfung: im Baum weder `warm_reset`, `cold_reset`, Reset-Vektor noch
Watchdog (`grep` über `kernel/src`, `tos-api/src`: nur ein Kommentar in
`selftest_ec4.rs:292`). Siehe auch den Befund unter X04.

## X04 — TeraDesk „Reboot" springt auf 0

**Verdict: STILL OPEN** (erwartet), Basis, Größe S nach RST01.
Einzeilenprüfung: Beleg `measurements/measurement-2026-10-17-x04/RUNINFO.TXT`
(pc=0, `esr=02000000`); kein Commit seitdem.

**Neuer Quellenbefund zu den im Ticket offenen Fragen 1 und 2 [gelesen]:**
1. TeraDesk liest `os_start = *(long*)0x4F2` (`main.c:55-56`) und dann
   `*((long *) os_start + 4)` (`main.c:2096`, Kommentar „pointer to reset
   handler"). Auf m68k (`long`=4) ist das **Byte-Offset 0x10 = `os_rsvl`/`os_res1`**.
   Auf LP64 (`long`=8) sind es **0x20..0x27** — dort liegen bei uns `os_root`
   (+0x20) und `os_kbshift` (+0x24), beide vom Lader auf 0 gelassen
   (`tos-api/src/bdos/process/loader.rs:1156-1159`; der Lader schreibt den
   OSHEADER im 4-Byte-Fremdformat). Rückgabe 0 → `Supexec(0)` → `pc=0` passt zum Foto
   [Schluss; die genauen 8 Byte hängen an der Byte-Reihenfolge der on-wire-Felder
   und wurden nicht ausgerechnet].
2. **Korrektur der Spec:** `docs/api/structures/osheader.md:128-132` und der
   Ladekommentar `loader.rs:1150` behaupten „EmuTOS sets `os_rsvl = 0`".
   **Der Quelltext sagt etwas anderes:** `bios/startup.S:92-93` legt dort
   `os_res1: .dc.l _main` ab („reserved"), und `reseth` (+4) ist ebenfalls
   `_main` (`startup.S:80-81`). Auf EmuTOS landet TeraDeskʼs Reboot-Sprung also
   in `_main` = Neustart des Systems. `biosdefs.h:53` nennt das Feld nur
   „unused" — der Kommentar lügt, der Code nicht. Folge: die Zeile „os_rsvl = 0"
   in Spec und Lader ist falsch (**Spec-Korrektur nötig**, nicht von mir
   eingepflegt, weil außerhalb des Auftrags „keine Änderung"), und RST01 Punkt 4
   („gültiger Reset-Einsprung im OSHEADER") ist genau die EmuTOS-Treue. Auf LP64
   greift TeraDesk wegen `long`=8 allerdings ein anderes Feld; ein bloßes
   Setzen von +0x10 würde den Reboot **nicht** auslösen. Das ist ein
   Toolchain-/ABI-Punkt (Anwendung bleibt unverändert, Regel
   Projektregel „portierte Atari-Anwendung bleibt unverändert") — Entscheidung nötig, bevor RST01 baut:
   entweder LP64-Lese-Offset bedienen (OSHEADER-Mirror für LP64-Leser) oder
   Overlay-Lösung. **Needs deeper analysis**, nicht geraten.

## A25 — `appl_init` Wiederaufruf

**Verdict: NEEDS RUN** (QEMU genügt). Basis: EmuTOS `aes/gemaplib.c:54-63`
(`rlr->p_flags |= AP_OPEN`, sonst nichts) [gelesen].
Belegt [gelesen]: Fix `tos-api/src/aes/pd.rs:355-374` (`slot.flags |= AP_OPEN`,
Queue erhalten); `tos-api/tests/aes_appl_init.rs` und Unit-Test in `pd.rs`;
Arbeitsbaum zu `tos-api/` sauber, also committet. Kein Folgecommit mit „A25"
außer Statusnotizen.
Offen: ausschließlich der Diagnosefall „appl_init, Nachricht an eigene PID,
appl_init, lesen, appl_exit". `sdk/apps/apitest/apitest.c` enthält nur T28
(`ap_private`), keinen Wiederaufruf. Das Ticket verlangt „Hardware-Gegenprobe",
der Pfad ist aber reine AES-Software [Schluss]; Lauf = APITEST-Fall (~20 Zeilen C)
plus `#Z`-Autorun in QEMU raspi4b ohne Eingabe. Größe S. Alternative:
Anwenderentscheid „Hosttest über den echten Dispatcher reicht".

## M08 — `aes-m07-parameter.tsv`

**Verdict: STILL OPEN.** Art: Basis-Sicherung (Dauertest gegen gemessenes TOS-
Verhalten). Größe S.
Belegt [gelesen]: `tos-api/tests/aes_reference.rs:11-40` sagt selbst „measured but
still not here"; B-1 (Mauszeiger) durch V17 behoben, `cases_m07_parameter` läuft
allein 20/20; B-2 offen — `wm_new` (`gemwmlib.c:1874-1892`) leert die Schlange
nicht, Fall 69 sieht das `WM_REDRAW` von `cases_m05_addrin`. Im Ordner
`tos-api/tests/aes_reference/` existiert kein `cases_m07_parameter.rs`.
Kleinster Schritt: im Harness vor Fall 69 per `evnt_multi(MU_MESAG|MU_TIMER,0)`
bis „nur Timer" abziehen, Modul erzeugen, `--shuffle` zehnmal, Tester.

## PROC03 — AES als echter Prozess

**Verdict: STILL OPEN** — umformulieren auf die AES-Hälfte. Basis (`p_parent`
exakt wie EmuTOS, `proc.c:541`). Größe M–L.
[gelesen] `tos-api/src/bdos/process/standin.rs:1-30`: AES-PD und
`initial_basepage` sind nie der laufende Prozess („Way B … is TASK/PROC03.md").
Desktop-Hälfte ist durch DESK43 U9/U10 (EL0-NEWDESK, Pi-abgenommen) geliefert;
der Ticketkopf (`NewTOS hat weder AES-PD noch initial_basepage`, `Lader schreibt 0`)
ist veraltet. Kein Commit am Ticket seit dem Anlegen. Kleinster Schritt:
Ticket neu fassen (nur AES-PD als echter Prozess). Kein sichtbarer Defekt;
beobachtbar nur über `p_parent`-Ketten (wie im Vorgängerreview).

## X01 — `Super`/`Supexec`

**Verdict: STILL OPEN**, Rest = **NEEDS RUN (Pi, Anwender)** + Tester. Basis
(EmuTOS implementiert `Super` im Trap-Kopf `bdos/rwa.S:159-182`, nicht in der
BDOS-Tabelle [gelesen]). Größe S.
Heute [gelesen]: beide Entscheidungen sind gefallen (`TASK/X01.md`
„Anwenderentscheidung (2026-10-17)", Commits `80429ad1`, `360d53bd`):
Shutdown-Pfad bleibt TOS-treu, Cookie-Spiegel bleibt Momentaufnahme. Umsetzung
im Overlay `sdk/libc/overlay/include/mint/osbind.h` (`e503efdc`); Kern unverändert,
Spec `docs/api/xbios/26-supexec.md` „decision deferred".
Offen: Pi-Aufrufstellen `main.c:2090/2118` (Shutdown; hält die Maschine
absichtlich an), `file.c:945/969` (Medienwechsel), `video.c:289/318`
(Videodialog) und die Tester-Abnahme. „Reboot" ist nach X04/RST01 ausgelagert.
Kleinster Schritt: eine Pi-Sitzung mit drei Griffen (Anwender).

## PORT02 — Welche Anwendungen bis zum Termin

**Verdict: CLOSE-SUPERSEDED.** Art: Erweiterung (Bewertung, kein
EmuTOS-Verhalten).
Das Ticket verlangt „Ergebnis ist ein Dokument" mit Reihenfolge. [gelesen]:
Bewertung zView/LDG liegt vor (`docs/analysis/zview-ldg-2026-10-04.md`, Empfehlung
Option C vor dem Termin, Option A danach); zView wurde portiert
(ZVIEW01/02/04, ZVIEW-M4 DONE, 15/15 Muster pixelgenau) und lief in der
Pi-Abnahme; Doom (PORT06 ERLEDIGT, Pi 2026-10-04), QED, TeraDesk, CoreMark,
Ball, PRESENT laufen (`inventory-2026-10-08.md:117`). Die Organisatorvorschläge
sind im Ticket selbst bewertet und in SND01/SDL01/PORT06 ausgelagert. Der
Termin (9./10.10.) ist vorbei. Nicht einzeln bewertet bleiben `ztask`,
`zweather`, Thing, HighWire (blockiert durch NET01–06, NET05 nicht begonnen)
— falls gewünscht, neues Ticket „Kandidatenbewertung nach der OFAM".

## SND01 / SND02 — Ton

**Verdict: beide STILL OPEN.** Art: Erweiterung. Belegt [gelesen]:
`docs/api/xbios/_dmasound-group.md` — alle 14 Funktionen 0x80–0x8D sind in
EmuTOS real, **aber jede beginnt mit der Laufzeitprüfung auf einen Soundchip und
liefert ohne ihn die eigene Funktionsnummer**; NewTOS tut genau das, „adding sound
means deliberately leaving a correct answer behind; it is an extension"
(Commit `03b9b74c`). Im Baum kein `soundcmd`/`buffoper`/`devconnect`, kein
PWM- oder DMA-Treiber (`kernel/src/drivers/bcm2711/`: clock_tags, emmc*,
framebuffer, mailbox, pl011). Der Anwender hat Ton am 2026-09-29 auf „nach der
OFAM" gelegt. SND01 L (erst DMA-Treiber, Klinke, API), SND02 M (reine
Track-B-Recherche, „nicht anfangen zu bauen"). Reihenfolge laut Ticket: DMA →
Klinke → HDMI.

## SDL01 — SDL-Backend

**Verdict: STILL OPEN.** Erweiterung, L. Im Baum nichts (`sdk/` ohne SDL).
Video/Ereignisse/Timer sind vorhanden, Ton fehlt (SND01). Zwei Vorentscheidungen
(SDL 1.2 vs. 2; Quelle) stehen unbeantwortet im Ticket.

## RTC01 — DS3231 als BIOS-Uhr

**Verdict: STILL OPEN.** Erweiterung (Plattformtreiber; EmuTOS' `bios/clock.c`/
`nvram.c` existieren für Atari-RTCs, aber nicht für I2C — [gelesen] die Dateien
liegen vor, nicht studiert). Größe M. Im Baum weder `ds3231`, `bsc1` noch `i2c`
(`grep`), das DS3231-Datenblatt fehlt in `references/datasheets/`
(nur `RP-008248-DS-1-bcm2711-peripherals.pdf`). Ticket selbst: „nach dem
Atari-Treffen". Erste Handlung: Datenblatt beschaffen (Track B).

## BALL01 — Ball vor Hintergrund beim ersten Frame

**Verdict: NEEDS RUN.** Art: unklar — Ursache unbekannt (Demo-Reihenfolge vs.
NewTOS `Setscreen`/`Vsync`/Framebuffer-Cache).
[gelesen] `ball.c:26-36` kommentiert, dass der allererste Paint den Hintergrund
„einmal (bei zwei Bildseiten zweimal)" setzen muss; kein Commit außer dem
Anlegen (`238df87e`). Quelle liegt außerhalb des Repos. Welcher Lauf entscheidet:
(a) QEMU raspi4b, `BALL.NAP` per `#Z`-Autorun, Screendumps in kurzem Abstand ab
Start (zeigt, ob es auf NewTOS-Seite reproduzierbar ist); (b) dieselbe Binärdatei
auf Hatari-TT (Gegenprobe des Tickets). Ohne Lauf keine Aussage.

## FS05 — `fwrite` läuft die Kette je Sektor von vorn

**Verdict: CLOSE-SUPERSEDED** durch FS08 Teil 2 (`5f62fa0e`, 2026-10-07, nach dem
Ticket vom 2026-09-25). Basis.
[gelesen] `resolve_cluster` kommt im Baum nicht mehr vor; `file.rs:38,423`
führen `o_curcl`-artigen Zustand ("FS08") und lesen/schreiben wie EmuTOS `xrw`.
`tos-api/tests/gemdos_fs_chain_steps.rs:82-110`
`sequential_writes_take_one_step_per_new_cluster`: 64 Records (16 Cluster) = 85
FAT-Lookups, hergeleitet aus `nextcl`/`findfree` (`fsfat.c:208-293`) — linear.
Mess­beleg `measurements/measurement-2026-10-07-fat-curcl/` (FSBENCH 1 221 462 →
548 939 Sektoren gelesen). Einschränkung [gelesen]: die Abnahme „drei Größen"
des Tickets ist nicht wörtlich erfüllt (ein Größenpunkt im Test); die Struktur
(Schritte je neuem Cluster, kein Neustart je Sektor) ist aber der Beweis, nicht
die Zahl. Kein Schließungshindernis.

## REF03 — falsche Spec-Zitate

**Verdict: STILL OPEN.** Hygiene (kein Verhalten). S.
[gelesen] Zeilen haben sich verschoben, der Fehler nicht:
`kernel/src/drivers/bcm2711/emmc.rs:71`, `:934`, `:983` zitieren „SD-HC 3.00
§2.2.15" für Software-Reset; die Umrechnungstabelle `docs/hardware/emmc-sd.md:690-691`
sagt: 3.00 Timeout Control = §2.2.15, **Software Reset = §2.2.16**. F3:
`emmc.rs:~989-993` behauptet weiter, beide Resets in einem Schreibzugriff seien
„an optimisation the specification never asks for"; dieselbe Datei
(`emmc-sd.md:~698`) beschreibt §3.8.1 3.00 als „CMD12, then both software
resets". Die Änderung (je ein Bit pro Schreibzugriff) bleibt richtig — QEMU-Befund
in demselben Kommentar. `TASK/HW03-STATUS-DONE.MD:216` zitiert ebenfalls
§2.2.15. Der verlangte Sweep nach weiteren `§2.2.x` aus 4.20 ist nicht
nachgeprüft (die Treffer in `emmc_xfer.rs` nennen „SD-HC 3.00 §2.2.5/2.2.3/2.2.10"
mit Edition; Stichprobe plausibel, nicht gegen das PDF geprüft).

## T15 — Zeichentests ohne Riegel

**Verdict: STILL OPEN.** Prüfstand, S.
[gelesen] `aes_cicon_draw.rs`, `aes_object_font.rs`, `aes_object_nicelines.rs`:
einziger `Mutex` ist der Pixelpuffer (`px: Mutex<…>`), kein Test-Riegel
(`LOCK`/`SERIAL`/`AES_LOCK` kommt nicht vor). Kein Commit seit Anlegen.
Ob der Pfeil bei (63,47) ihre Asserts trifft, ist nach wie vor ungeprüft
(T12: 300/300). Kein sichtbarer Defekt.

## DESK49 — `filetypes` ohne die vier nativen Endungen

**Verdict: STILL OPEN.** Erweiterung (NewTOS-eigene Endungen, TeraDesk-Config).
S. [gelesen] TeraDesk `filetype.c:41,224,475-492`: `filetypes` ist die
Maskenvorgabeliste des Dateimasken-Dialogs (`wd_filemask` → `ft_dialog`),
keine Starterkennung. `tools/make-sd-image.sh:660-701` erzeugt dort `*`, `*.*`,
`*.PRG`, `*.APP`, `*.GTP`, `*.TOS`, `*.TTP`, `*.ACC`, `*.TXT`, `*.IMG` — ohne
`*.NAP/NCP/NTP/NAC`. Kleinster Schritt: im selben Heredoc ergänzen (die Frage des
Tickets „gehört es hinein" ist damit beantwortbar: es ist eine Komfortliste,
Vorgabe wäre sinnvoll). Betrifft nur das QEMU-Abbild/`TERADESK.INF`, nicht die
echte Karte.

## DESK51 — `graf_shrinkbox`

**Verdict: STILL OPEN.** Basis (Optik, `deskfun.c:809-856` [gelesen]). S–M.
[gelesen] `programs/desktop/src/main.rs:2609-2611`: „Its `graf_shrinkbox` from the
window centre is not built (no binding yet)"; `programs/newtos-gem/src/aes.rs`
kennt nur `graf_growbox` (`:494-498`). `close_window` (`main.rs:3435`) und
`do_close_folder` zeichnen keinen Schrumpf-Umriss. Kleinster Schritt: Binding in
`newtos-gem` (Opcode 74, AES-Seite zu prüfen), dann `w_setpath`/`fun_full_close`.
Hinweis: `fun_full_close` verlangt `pw->w_obid > 0` (`deskfun.c:837`).

## DESK52 — `do_fopen` Laufwerksprobe und `STDEEPPA`

**Verdict: STILL OPEN.** Basis (`desksupp.c:1089-1108` [gelesen]: `set_default_path<0`
→ `fun_close(pw,CLOSE_WINDOW)`; `strlen(pathname) >= LEN_ZPATH` → `STDEEPPA`).
S. [gelesen] `main.rs:3387-3431` `do_fopen`: nur die Desktop-Verknüpfungs-Probe
(`:3397`); weder Laufwerksprobe noch `STDEEPPA` (`grep STDEEPPA` in
`programs/desktop/src` = 0 Treffer).

## DESK53 — `refresh_window` (ESC)

**Verdict: STILL OPEN.** Basis (`desksupp.c:1670-1690`: `do_fopen(pw,0,p_spec,FALSE)`
liest neu ein [gelesen]). S. [gelesen] ESC → `refresh_top()` (`main.rs:1191`,
`:3522-3534`) ruft nur `win_bldview` auf der vorhandenen Liste und `do_wredraw`;
kein Neueinlesen, kein `do_fopen`.

## DESK54 — `do_xyfix` 3D-Korrektur

**Verdict: STILL OPEN.** Basis (`desksupp.c:262-275`: `if (*px + gl_wbox +
2*ADJ3DSTD >= V_REZ_HZ) *px -= 16;` unter `CONF_WITH_3D_OBJECTS`; `ADJ3DSTD = 2`,
`obdefs.h:136` [gelesen]; die Konfigtabelle führt `CONF_WITH_3D_OBJECTS` als 1
mit `docs/api/_emutos-config.md:166`, Spaltenbedeutung dort nicht erneut
geprüft). S. [gelesen] `main.rs:2720-2726` `do_xyfix`: nur Ausrichtung und
Menüleiste.

## DESK55 — `win_sname`

**Verdict: STILL OPEN.** Basis (`deskwin.c:846-858`: Titel = `" " + p_spec + " "`
[gelesen]). S. [gelesen] `main.rs:3020-3032` `spec_and_title` und
`set_window_mask` (`:2205-2210`) setzen den Titel auf das Verzeichnis (Maske nur
bei Filter); der Code-Kommentar benennt die Abweichung sogar („EmuTOS `win_sname`
always shows the full spec").

## DESK56 — Wurfpfade verwerfen `do_aopen`-Rückgabe

**Verdict: CLOSE-SUPERSEDED** durch DESK62 (`a8b03c81`, auf der Pi abgenommen).
[gelesen] `programs/desktop/src/dir.rs:1658` `static EXIT_DESKTOP`, gesetzt an
`:1989`, `:2103`, `:2173`, gelesen `:1706` — passend zu den drei EmuTOS-Zuweisungen
`deskfun.c:967`, `:1034`, `:1249` und `return exit_desktop` `:1338`. Das Ticket
nennt zwei verworfene Pfade in `kernel/src/desk/dir.rs` (gelöscht, DESK43 U10); die
Wirkung („verlässt der Desktop nach einem Drop-Start die Schleife?") ist DESK62.

## DESK61 — `act_chg` und `f_selected`

**Verdict: STILL OPEN.** Basis (`deskact.c:447-449`: `f_selected` wird vor dem
`old_state != curr_state`-Test gesetzt [gelesen]). S.
[gelesen] `programs/desktop/src/act.rs:75-98`: `if was == set { return false; }`
**vor** der `f_selected`-Zuweisung (`:80-82`, `:89-96`). Zusätzlich weicht der
Rückgabewert ab: EmuTOS liefert `TRUE`, sobald das Objekt nicht `DISABLED` ist
(`deskact.c:465`), NewTOS `false` bei „keine Änderung" — Aufrufer, die den Wert
als „geändert" lesen, sehen etwas anderes [Schluss; Aufrufer nicht einzeln
nachverfolgt]. Ob `f_selected` und `ob_state` real auseinanderlaufen können, ist
nicht gezeigt (Fall + Selbsttest fehlen).

## DESK63 — Verknüpfung auf Verknüpfung

**Verdict: STILL OPEN** (erwartet). Basis (`deskfun.c:1297`, `:1180`).
Einzeilenprüfung: `dir.rs:2262-2267` `_ => illegal_op_msg()` unverändert; in
`fun_desk2desk` (`:2207`) fehlt der Programm-Verknüpfungs-Zweig. S.

## DESK65 — sofort speichern (Erweiterung)

**Verdict: STILL OPEN** (Idee, ausdrücklich „später"). Erweiterung (Anwender
2026-10-17; EmuTOS speichert erst bei „Desktop sichern", `deskinf.c:919`).
Einzeilenprüfung: kein Code für die Verallgemeinerung; nur der Hintergrunddialog
speichert sofort (`inf.rs`). Einordnung laut Ticket: nach dem sauberen
EmuTOS-Basisstand.

## DESK66 — Hintergrund an die AES melden

**Verdict: STILL OPEN** (erwartet). Basis (EmuTOS `inf_backgrounds` →
`set_aes_background(curdesk & 0xff)`, `deskinf.c:904`). M.
Einzeilenprüfung: in `programs/desktop/src` nur der Hinweiskommentar
(`inf.rs:3737`); die AES-Funktion ist `pub(crate)` im Kern
(`tos-api/src/aes/init.rs:211`), vom EL0-Desktop nicht erreichbar. Zu klären
zuerst: trägt ein vorhandener AES-Aufruf den Wert (`wind_set(0,WF_NEWDESK…)`)?

## CON01 — Spaltenzähler und Typeahead

**Verdict: STILL OPEN.** Basis (`bdos/console.c:54,124,268-270` `glbcolumn`;
Typeahead [gelesen]). M. [gelesen] `tos-api/src/gemdos/console.rs:50-52,327,354,
558,590` dokumentieren die Abweichung selbst („no `glbcolumn` tracking",
„no BDOS typeahead buffer"). Kein Commit „CON01". Der Hinweis im Ticket, die
Spec `0a-cconrs.md` sei veraltet, ist nicht nachgeprüft.

## CON02 — `Cconws` folgt der Umlenkung nicht

**Verdict: STILL OPEN.** Basis (EmuTOS `xconout` → `tabout(HXFORM(run->p_uft[1]),ch)`,
`console.c:281-285`; `xconostat`/`xconstat` `:161-170` [gelesen]). S–M.
[gelesen] `console.rs:514-540` `cconws`: `host_or_eundev!()` → immer CON, kein
`p_uft`-Zugriff. Beleg `measurements/measurement-2026-10-17-libc-fstat/`.

## FS12 — 26 statt 16 Laufwerke

**Verdict: STILL OPEN.** Basis (`include/sysconf.h:43` `BLKDEVNUM 26`
[gelesen]). S–M. [gelesen] `tos-api/src/bios/drive.rs:11` `MAX_DRIVES = 16`;
`tos-api/src/bdos/fs/drive.rs:20` `BLKDEVNUM = 16`. Der Desktop prüft bereits
gegen 26 (`4b057622`, DESK58). Sichtbar erst bei >16 Laufwerken; die
Spec-Zeilen `07-getbpb.md:89,228`, `09-mediach.md:84` nicht erneut geprüft.

## FS13 — `Fclose` nach `Fforce(n,-1)` liefert −65

**Verdict: CLOSE** — kein Defekt, NewTOS = EmuTOS. Basis.
Herleitung aus EmuTOS-Code [gelesen], Folge „Fcreate, Fforce(1,fh), Kind, Kind
endet, Fforce(1,−1), Fclose(fh)":
1. `ixforce` zählt `f_use++` (`bdos/fshand.c:68-74`); das Kind erbt die forcierte
   Zuordnung über `init_pd_files` → `ixforce` (`proc.c:387-398`).
2. Das Ende des Kindes schließt sie: `ixterm` → `xclose(h)` für `p_uft[i] > 0`
   (`proc.c:117-119`); `xclose` ruft `ixclose(fd,0)` (`fsopnclo.c:482`), das den OFD
   aus `d_files` austrägt (`:552-566`).
3. `Fforce(1,-1)` im Elternteil speichert nur `p_uft[1] = -1`, ohne Zählung
   (`fshand.c:53-54`).
4. `Fclose(fh)`: `ixclose(fd,0)` findet den OFD nicht mehr in `d_files` →
   `return EINTRN` (`fsopnclo.c:565`) = −65.
NewTOS bildet dies in `ixclose` (`file.rs:1000-1013`, Feld `listed`, Kommentar
„a shared OFD … answers `EINTRN`") nach. Ohne Kind-Zwischenschritt wäre es `E_OK`.
Der Dateiinhalt stimmt, wie das Ticket sagt. Zu tun: Spec `3e-fclose.md` um die
Folge ergänzen; Ticket schließen.

## LIBC-IOCTL

**Verdict: STILL OPEN.** Erweiterung (SDK; EmuTOS hat kein `ioctl`). S–M.
[gelesen] `sdk/libc/newtos-arch/ioctl.c`: nur `return Fcntl(fd,arg,request)`;
kein TOS-Fallback für `TIOCGETP`/`TIOCSETP`/`TIOCGWINSZ`. Kein Commit
„LIBC-IOCTL". Researcher-Schritt laut Ticket (welche Programme brauchen es) offen.

## LIBC-BUF

**Verdict: STILL OPEN** — NEEDS RUN (QEMU/Host-Probe). Erweiterung (SDK). S.
[gelesen] Overlay `sdk/libc/overlay/stdio/vfprintf.c:2243-2263`
`buffered_vfprintf`: Stack-Puffer `BUFSIZ`, setzt danach `__bufsize = 0`; das ist
der im Ticket beobachtete Pfad für einen **ungepufferten** Strom. [Schluss, nicht
gelaufen]: eine `fopen`-Datei ist beim ersten `printf` nicht „ungepuffert", läuft
also nicht durch diesen Zweig, und die FS08-Regel (32 KiB über
`st_blksize`, `0abc634`) bliebe unberührt. Das entscheidet nur eine
FSTATP-Sonde mit `fprintf` auf eine `fopen`-Datei.

## MEM06 — reservierter virtueller Heap (Idee)

**Verdict: STILL OPEN** als Richtungsnotiz, nichts zu tun. Erweiterung. Status
„IDEE … kein Auftrag ohne Freigabe". Voraussetzungen (ARCH03, MEM05) sind
geliefert; die Frage nach `Malloc(-1)` und 24-Bit-Emulator bleibt Researcher-
Arbeit.

## MEM07 — `MAX_SHARED_BLOCKS = 64`

**Verdict: STILL OPEN.** Erweiterung (gemeinsame Besitzer sind eine NewTOS-/
FreeMiNT-Eigenschaft des ARCH03-Speichermodells). S.
[gelesen] `tos-api/src/bdos/mem/region.rs:46` `= 64`, Kommentar: jedes
`Pexec`-4/6-Kind braucht zwei, höchstens 16 Prozesse (`MAX_PROCESSES = 16`,
`process.rs:67`) → 32 ≤ 64 für diesen Verbraucher [Schluss]. Weitere
Verbraucher (`Ptermres`/`F_KEEP`, `reserve_keep`) nicht ausgezählt — das ist der
verbliebene Auftrag. `RegionError::Full` wird zurückgegeben (`:84`), die Grenze
scheitert also laut, nicht still.

## MEM08 — lineare Suche in `Mfree`/`Mshrink`/`detach`

**Verdict: STILL OPEN** (zuerst messen). Erweiterung — EmuTOS' `xmfree` läuft
ebenfalls über die Belegtliste; die Gewohnheit ist TOS-gleich, relevant wird sie
erst durch die auf 8192 angehobene Grenze (MEM05). S.
[gelesen] `region.rs` läuft über `mp_mal` (`:109,441,504`); keine Messung zu
`Pterm` mit vielen Blöcken im Baum. Der MEM05-Lauf misst nur die Schutzkosten
(MEM10), nicht dies.

## MEM09 — `on_alloc` ignoriert `record`

**Verdict: CLOSE-SUPERSEDED** durch MEM05 (`d2e3c384`). Erweiterung.
[gelesen] `memprot.rs:370-387`: `record(loc, mode)` wird weiter verworfen,
**aber** der einzige Aufrufer (`bdos/mem/traps.rs:426`) läuft hinter
`memprot::reserve()` (`traps.rs:374`, scheitert → Allokation `0`, nichts
geändert) und `record` dokumentiert „cannot happen for a block allocated after a
successful `reserve`" (`memprot.rs:269-272`). Die Invariante ist damit
abgesichert, der stille Fall nicht mehr erreichbar. Optional: `debug_assert!` auf
den Rückgabewert (S, Anwenderwahl).

## MEM10 — Synchronisierung ohne volle Momentaufnahme

**Verdict: STILL OPEN.** Erweiterung (Speicherschutz ist NewTOS/FreeMiNT, nicht
EmuTOS). M.
Gemessen [gelesen]: `measurements/measurement-2026-10-13-mem05-kernel/RUNINFO.TXT:140-142`
— 8189 `Mxalloc`+`Mfree` am Start: 23,3 s mit Schutz gegen 0,06 s ohne; bei
5140–8192 lebenden Blöcken 18,3 s gegen 0,02 s (QEMU raspi4b) — „documented
per-sync cost O(N·64 + N log N), MEM10". Kein Commit „MEM10". Das Ticket sagt
„erst messen": die Messung liegt vor, offen ist der Entwurf zur Freigabe.

## IO01 — AUX/MIDI/PRN (Idee)

**Verdict: STILL OPEN** (Idee, „kein Auftrag, bis der Anwender ihn freigibt").
Erweiterung (Plattformtreiber). L. Die Vorbereitung (FS11 Geräteweiche) ist
erledigt (`dbc4e22a`); Treiber nicht begonnen (nur `pl011.rs` als Konsole). Die
Frage zu MIDImaze bleibt ungeklärt.

## ARCH03 — Rest

- **Stufe 2b (FreeMiNT-Modi 100/104/106/200/204/206): STILL OPEN**, Erweiterung
  (FreeMiNT, Voraussetzung für SLB01 = ZUKUNFT). M (Plan: 3–4 Tage,
  `arch03-implementation-plan.md:534`). [gelesen] `pexec.rs:179-195`: Modi
  0/3/4/5/6/7 bedient, alles andere `UnknownMode` → `EINVFN`. Die FreeMiNT-Modi
  sind laut Plan Zeile 337 noch nicht spezifiziert (Regel 5: Spec zuerst).
- **`asid=on` auf echter Pi: NEEDS RUN (Pi, Anwender).** Erweiterung
  (ASIDs sind NewTOS-interne TLB-Pflege, kein TOS-Vertrag). S. QEMU-Belege
  (`measurement-2026-10-06-arch03-4`, `…-10-13-arch03-acceptance`) vorhanden;
  `grep` in `docs/HW-TEST-LOG.md` und den Messordnern ergibt keinen Pi-Lauf mit
  `asid=on`. Der Standardwert bleibt aus; ein Lauf wäre nur zur Freigabe als
  Standard nötig. Worker fassen die Pi nicht an.

---

## Spec-Korrekturen und Befunde, die über die Ticketurteile hinausgehen

1. **`docs/api/structures/osheader.md:128-132` und `loader.rs:1150`** („EmuTOS sets
   `os_rsvl = 0`") widersprechen `references/emutos/bios/startup.S:92-93`
   (`os_res1: .dc.l _main`). Betrifft X04/RST01; nicht von mir eingepflegt
   (Auftrag: keine Änderung außerhalb der Analysedatei).
2. **FS06 und FS13** sind Fehlalarme aus unvollständig gelesenem EmuTOS-Code;
   beide Tickets zitieren die jeweilige Stelle in falscher Bedeutung. Kein
   Hinweis auf eine Regel, aber dasselbe Muster wie bei Tickets aus Screendumps:
   erst die EmuTOS-Funktion lesen, dann ticketen.
3. FS06-Rest (Cross-Directory-Rename auf leeren Namen) und DESK48-Zusatzstelle
   `fun_del_desktop` (`dir.rs:1518`) sind neue, nicht getickerte Beobachtungen.

## Außerhalb des Scopes (je eine Zeile)

- `docs/api/aes/window-graphics.md` ~1224-1240 beschreibt noch die in U10 gelöschten
  `wm_new_keep_desktop`/`desktop_resume` (schon im Vorgängerreview vermerkt).
- Ticketköpfe von FS06, FS13, FS05, MEM09, DESK56 brauchen bei Anwendung der Urteile
  einen Satz „geschlossen am 2026-10-17, siehe …".
- `HighWire` (`sdk/apps/highwire`) hängt an NET05 (nicht begonnen) — kein Ticket
  in dieser Liste, nur Hinweis.
