# Review der offenen Ticketliste — 2026-10-19

Auftrag (Anwender, 2026-10-18): die P3-Liste aus `docs/STATUS.md` „Offen" erneut
prüfen — viele Tickets könnten durch die Kernarbeit vom 17.–19.10. (A112, A113,
RST01, PROC03, PROC04, CON01) erledigt oder verändert sein. Anders als am
[17.10.](open-tickets-review-2026-10-17.md) wird dort, wo nur ein Lauf
entscheidet, gelaufen.

Methode und Grenzen: Basis HEAD `e8d245bd`. Code `file:line`, EmuTOS unter
`references/emutos`, FreeMiNT unter `references/freemint` (MEM06/07/08/10,
ARCH03 2b, ADR-027). Läufe: Hosttests über den echten GEMDOS-Dispatcher
(`tos-api/tests/fatvol`, Probe vorübergehend in `tos-api/tests/`, danach
entfernt), ein Belastungslauf für T15, ein QEMU-raspi4b-Lauf für LIBC-BUF.
Belege: [`measurements/measurement-2026-10-19-ticket-review/RUNINFO.TXT`](../../measurements/measurement-2026-10-19-ticket-review/RUNINFO.TXT).
Keine Pi, keine Karte, kein Produktcode geändert. Kennzeichnung:
**[gelesen]** = Stelle in dieser Sitzung gelesen; **[Lauf]** = in dieser
Sitzung gefahren; **[Schluss]** = Folgerung ohne Lauf. `docs/STATUS.md` und
Ticketköpfe gelten als Hinweis, nicht als Beleg.

„Basis" = nötig für getreues EmuTOS-Verhalten; „Erweiterung" = über EmuTOS
hinaus oder reine SDK-/Werkzeugfrage.

## Zusammenfassung

| Ticket | Urteil | Art | Größe | Beleg | Kern in einem Satz |
|---|---|---|---|---|---|
| **FS17** | **STILL OPEN — Punkt 3 ist ein Datenverlust** | Basis | M | [Lauf] | Eine offene Datei, im selben Ordner umbenannt, dann beschrieben und geschlossen, hat Länge 0, und ihre Cluster sind verloren (6 Cluster bei 3000 Byte). Punkte 1, 2, 5, 6 reproduziert. |
| FS14 | STILL OPEN | Basis | S | [Lauf] | `Frename(C:\FS14\X.TXT,"C:\")` → `0` und ein Eintrag ohne Namen; EmuTOS `EPTHNF`. |
| FS15 | STILL OPEN | Basis | S | [Lauf] | `Fopen`/`Fcreate("CON:")` usw. → `-33`/`-34`; EmuTOS liefert die Gerätehandles. |
| FS16 | STILL OPEN (Anwenderentscheid nach Messung) | Basis | M | [gelesen] | `Fwrite` ruft weiter nach jedem Aufruf `cache::flush()` auf (`file.rs:552`). |
| FS12 | STILL OPEN | Basis | S–M | [gelesen] | `MAX_DRIVES = 16`, `BLKDEVNUM = 16`; EmuTOS 26. |
| A115 | STILL OPEN | Basis | M | [gelesen] | `pick_next` geht weiter nach Slot-Reihenfolge vor, nicht nach `rlr`/`drl`. |
| A116 | **NEEDS USER** | Basis | M | [gelesen] | Weiter `0` bei voller Queue; Schritt 1 des Tickets ist eine Anwenderentscheidung. |
| RST02 | STILL OPEN | Basis | L | [gelesen] | Warmstart = Kaltstart (PM-Watchdog), `kernel/src/reset.rs:33-45`. |
| INPUT02 | STILL OPEN | Basis | M | [gelesen] | Die Reset-Prüfung läuft nur in `decode_key`, und das läuft nur auf der Trap-Seite. |
| CON03 | **NEEDS USER** | Basis | L | [gelesen] | Weiter 11 Befehle in `shell.rs:118-135`; offen ist, wo EmuCON hin soll. |
| DESK51 | STILL OPEN | Basis | S | [gelesen] | Die AES hat `graf_shrinkbox` (74); es fehlen nur das `newtos-gem`-Binding und zwei Aufrufstellen. |
| DESK52 | STILL OPEN | Basis | S | [gelesen] | `do_fopen` weiter ohne Laufwerksprobe und ohne `STDEEPPA`. |
| DESK53 | STILL OPEN (jetzt trivial) | Basis | S | [gelesen] | Seit DESK48 gibt es `refresh_window`; ESC ruft aber weiter `refresh_top` auf, das nicht neu einliest. |
| DESK54 | STILL OPEN | Basis | S | [gelesen] | `do_xyfix` weiter ohne die 3D-Korrektur. |
| DESK55 | STILL OPEN | Basis | S | [gelesen] | Der Titel ist weiter das Verzeichnis; überschneidet sich mit DESK67 Punkt 3. |
| DESK61 | STILL OPEN | Basis | S | [gelesen] | Frühe Rückkehr vor `f_selected` unverändert; einen Fall mit sichtbarer Wirkung habe ich nicht gefunden. |
| DESK63 | STILL OPEN | Basis | S | [gelesen] | `_ => illegal_op_msg()` in `fun_desk2desk` unverändert. |
| DESK66 | STILL OPEN | Basis | M | [gelesen] | Kein Weg vom EL0-Desktop zu `set_aes_background`. |
| DESK67 | STILL OPEN | Basis | M | [gelesen] | Alle fünf Punkte stehen noch im Code. |
| REF03 | STILL OPEN | Hygiene | S | [gelesen, PDF] | Die drei falschen §2.2.15-Zitate in `emmc.rs` stehen noch; der Sweep gegen die 3.00-PDF findet keine weiteren. |
| T15 | STILL OPEN (Abnahmezahl erfüllt) | Prüfstand | S | [Lauf] | 300/300 je Datei unter Last ohne Riegel; Punkt 1 (kann der Pfeil die Prüfstellen treffen?) bleibt ungeklärt. |
| DESK49 | STILL OPEN | Erweiterung (Werkzeug) | S | [gelesen] | `filetypes` in `make-sd-image.sh:694-736` weiter ohne `*.NAP/NCP/NTP/NAC`. |
| LIBC-IOCTL | STILL OPEN | Erweiterung (SDK) | S–M | [gelesen] | Der Shim reicht weiter nur an `Fcntl` weiter. |
| LIBC-BUF | **CLOSE empfohlen** (Restentscheid beim Anwender) | Erweiterung (SDK) | — | [Lauf] | Eine `fopen`-Datei mit `fprintf` als erster Ausgabe bekommt 32 KiB; betroffen ist nur `stdout` (laut Ticket auch umgelenkt). |
| MEM07 | **CLOSE** | Erweiterung | — | [gelesen] | Einziger Verbraucher ist `pexec.rs:404`; obere Grenze 2×16 = 32 ≤ 64; wird die Grenze erreicht, kommt ein Fehler zurück. |
| MEM08 | STILL OPEN (zuerst messen) | Erweiterung | S | [gelesen] | Lineare Suche unverändert, keine Messung im Baum. |
| MEM10 | STILL OPEN (Entwurf zur Freigabe) | Erweiterung | M | [gelesen] | Messung liegt vor (23,3 s gegen 0,06 s); `memprot.rs` unverändert seit `d2e3c384`. |
| MEM06 | STILL OPEN (Idee) | Erweiterung | — | [gelesen] | Nichts zu tun ohne Freigabe. |
| ARCH03 2b | STILL OPEN | Erweiterung (FreeMiNT) | M | [gelesen] | `pexec.rs:193` → `UnknownMode`; zu den Modi 100+ gibt es keine Spec (Regel 5). |
| SND01, SND02, SDL01, RTC01, IO01, DESK65 | STILL OPEN, weiter Erweiterung | Erweiterung | — | [gelesen] | Nichts davon ist gebaut (siehe Einzeiler). |
| GUI01 | **CLOSE nach Anwenderentscheid**, Rest S | Basis-Abweichung (keine Erweiterung) | S (Doku) | [gelesen] | Der Anwender hat am 2026-10-18 entschieden. Der im Ticket behauptete COMPATIBILITY-Eintrag (DESK64) **existiert nicht**. |

Ergebnis: Von 29 P3-Tickets plus 7 Erweiterungen sind **2 schließbar** (MEM07,
LIBC-BUF in der Prüffrage) und **GUI01 nach einem fehlenden Doku-Eintrag**.
**2 brauchen eine Anwenderentscheidung** (A116, CON03), FS16 nach der Messung.
Alle übrigen sind wirklich offen.

**Die Kernarbeit vom 17.–19.10. hat keines dieser Tickets erledigt.** Sie hat
zwei davon kleiner gemacht: DESK53, weil DESK48 `refresh_window` gebaut hat,
und MEM07, das sich als Fehlalarm herausstellt.

Der wichtigste Befund ist **neu in seiner Schwere: FS17 Punkt 3.** Der
Developer hat ihn als „Schlüssel `dir_slot` veraltet" vermerkt. Tatsächlich
gehen dabei Daten verloren und Cluster leck, und die Ursache betrifft jedes
`Frename` im selben Ordner (Abschnitt FS17).

---

## FS17 — kleinere GEMDOS-Datei-Abweichungen (aus PROC04 Punkt 5)

**Urteil: STILL OPEN.** Punkt 3 empfehle ich als eigenes Ticket mit höherer
Priorität. Basis, M.

Alle Punkte habe ich mit der Probe `method/zz_review_probe_fs.rs` gefahren
(Ausgabe in `hosttests/fs-probe.txt`) [Lauf]:

1. **`Fdelete` auf einen Ordner** → NewTOS `-36`. EmuTOS: `ixunlink` sucht mit
   `scan(dn,s,FA_NORM,&pos)` (`bdos/fsopnclo.c:604`). `FA_NORM` ist `0x27`
   (`bdos/fs.h:195`), und `match` (`fsdir.c`, Attributtest am Ende) lässt
   Unterverzeichnisse und Volume-Labels nur zu, wenn sie ausdrücklich gesucht
   werden. Ergebnis dort: `EFILNF`. **Bestätigt.** [gelesen] Zusätzlich: Auch
   ein Volume-Label wäre bei NewTOS löschbar. `file.rs:745` prüft nur
   `FA_RO|0x10`, `find_in_dir` findet jeden Eintrag. Diesen Fall habe ich
   nicht gefahren.
2. **Zwei `Fopen` derselben Datei teilen sich keine Länge.** Über `h1` von 3
   auf 8 Byte verlängert, danach liefert `Fseek(0,h2,SEEK_END)` → **3**. Bei
   EmuTOS läuft das über das gemeinsame DFD-`o_fileln` und ergibt 8. Nach
   beiden `Fclose` stimmt die Länge auf der Karte (8). **Bestätigt.**
3. **`Frename` einer offenen Datei — Datenverlust.**
   - NewTOS verschiebt auch beim Umbenennen *im selben Ordner* den Eintrag in
     einen freien Slot und markiert den alten mit `0xE5`
     (`tos-api/src/bdos/fs/file.rs:972-983`). EmuTOS überschreibt im selben
     Ordner nur den Namen an Ort und Stelle (`bdos/fsdir.c:1076-1083`).
     Sichtbar wird das an der Reihenfolge: A, B, dann A→C ergibt bei NewTOS
     `B, C`, bei EmuTOS `C, B`.
   - Ablauf: offene, leere Datei D umbenannt in E, dann `Fwrite` 1000 Byte,
     dann `Fclose` → `E.TXT` hat **Länge 0**. Die Ursache ist `flush_ofd`
     (`file.rs:715-722`): Es schreibt Startcluster und Länge in den *alten*,
     gelöschten Slot `dir_slot`.
   - Gegenprobe mit 3000 Byte: freie Cluster 4990 → 4984, nach
     `Fdelete(E.TXT)` **weiter 4984**. Die **6 Cluster sind verloren** (Leck in
     der FAT, das `fsck` als verwaiste Cluster melden würde).
   - EmuTOS hat diesen Fehler nicht: Der Slot bleibt derselbe, und `ixclose`
     schreibt `o_td`/Länge in genau diesen FCB (`fsopnclo.c:530-549`).
   - Zur Einordnung: Beim Umbenennen *über Verzeichnisgrenzen* zeigt das alte
     OFD auch bei EmuTOS auf den gelöschten Slot (`ixcreat` legt einen neuen
     an, `fsdir.c:1014`). Dieser Fall ist [Schluss] EmuTOS-gleich und hier
     nicht bewertet.
   - Schwere: Dateiinhalt verloren und FAT inkonsistent, über die dokumentierte
     API erreichbar. **CONFIRMED, echter Defekt.**
4. **`Fdatime` direkt auf den Slot.** In der Folge „setzen, `Fwrite`,
   `Fclose`" kommt derselbe Wert zurück wie bei EmuTOS. Beobachtbar ist die
   Abweichung nur zusammen mit Punkt 2 (zweites Handle) oder Punkt 3 (Slot
   verschoben). Kein eigener Befund, gehört zum DFD-Umbau.
5. **`FA_ARCHIVE`.** `Fcreate(attr 0)` mit `Fclose` → Attribut `0`. Nach dem
   Löschen des Bits, Verlängern und `Fclose` → wieder `0`. EmuTOS: `Fcreate`
   setzt `O_DIRTY` (`fsopnclo.c:206`), Schreiben über das Dateiende hinaus
   ebenfalls (`fsio.c:43-46`), und `ixclose` setzt dann `attr |= FA_ARCHIVE`
   (`fsopnclo.c:541-545`) → `0x20`. NewTOS setzt das Archivbit nie
   (`flush_ofd` fasst Byte `0x0B` nicht an). **Bestätigt**, sichtbar für jedes
   Backup-Programm.
6. **Slot bei `Fcreate` über eine vorhandene Datei.** X, Y, Z angelegt, X
   gelöscht, `Fcreate` über Z → NewTOS ordnet `Z, Y`. EmuTOS beginnt die
   Suche nach freiem Platz an Zs eigener Position (`fsopnclo.c:150-186`) und
   ordnet `Y, Z`. NewTOS nimmt den ersten freien Slot (`file.rs:400`).
   **Bestätigt.**
7. **`tmpfile()` linkt nicht.** In `sdk/libc/libc.a` sind `getgrent`,
   `getpwuid` und `getgroups` weiter nicht definiert (`llvm-nm`), `tmpfile`
   schon. Unverändert. [gelesen]

Überschneidung: Die Punkte 2, 4, 5 und FS16 haben eine gemeinsame Ursache.
NewTOS hat kein geteiltes DFD mit `O_DIRTY`, das erst `ixclose`
zurückschreibt. Ein Umbau nach EmuTOS (DFD je Verzeichnisposition,
`O_DIRTY`, Zurückschreiben in `ixclose`) erledigt alle vier und nimmt Punkt 3
den Boden, sobald der Schlüssel die Verzeichnisposition ist und `Frename` im
selben Ordner an Ort und Stelle schreibt.

## FS14 — `Frename` über Verzeichnisgrenzen auf einen leeren Namen

**Urteil: STILL OPEN.** Basis, S. [Lauf]
`Frename("C:\FS14\X.TXT","C:\")` liefert `0`, und das Wurzelverzeichnis listet
danach einen Eintrag `""`.

EmuTOS [gelesen]: `ixsfirst` auf den leeren Namen findet nichts (`fsdir.c:912`).
Im Zweig über Verzeichnisgrenzen folgt `hnew = ixcreat(p2,att)`
(`fsdir.c:1014`). `ixcreat` liefert bei leerem Namen `EPTHNF`
(`fsopnclo.c:120-121`), und `xrename` macht aus **jedem** `hnew < 0` ein
`EPTHNF` (`fsdir.c:1017-1018`).

NewTOS: `fcb_name("")` = elf Leerzeichen, `find_free_slot_grow` und der
Eintrag wird geschrieben (`file.rs:924-975`).

Zusatz [Schluss, nicht gefahren]: Aus derselben Zeile `fsdir.c:1017-1018`
folgt, dass EmuTOS bei vollem Zielverzeichnis über Verzeichnisgrenzen
ebenfalls `EPTHNF` liefert. NewTOS liefert dort `EACCDN` (`file.rs:972-975`).
Das sollte derselbe Fix mit erledigen.

## FS15 — Gerätenamen bei `Fopen`/`Fcreate`

**Urteil: STILL OPEN.** Basis, S. [Lauf]
`Fopen("CON:"|"con:"|"AUX:"|"PRN:",1)` → `-33`, `Fcreate(…)` → `-34`.

EmuTOS [gelesen]: `osif` vergleicht für `Fopen`/`Fcreate` den **ganzen**
Pfad mit `specname_table` (`bdos/bdosmain.c:101-110`, `:648-663`, `strcmp`,
nur ganz groß oder ganz klein) und liefert `0x0000FFFF` (CON),
`0x0000FFFE` (AUX) und `0x0000FFFD` (PRN), **ohne** die Funktion aufzurufen.

Achtung für die Spec: Der Rückgabewert ist ein positives LONG; das WORD
darin ist das Handle -1/-2/-3. Gemischte Schreibung (`Con:`) geht den
normalen Pfad. Kein Zweig dafür in `tos-api/src` [gelesen].

Überschneidung mit IO01: AUX/PRN landen auf den „nicht vorhanden"-Geräten
von FS11.

## FS16 — `Fwrite` leert den Cache nach jedem Aufruf

**Urteil: STILL OPEN.** Basis, M. [gelesen] `tos-api/src/bdos/fs/file.rs:552`
`super::cache::flush()?` am Ende jedes `ixwrite`; unverändert.

Das Ticket verlangt erst eine Messung, dann einen Vorschlag, dann den
Anwenderentscheid. Die Messung habe ich nicht gemacht (sie ist Ticketarbeit,
nicht Review). Siehe die gemeinsame Ursache unter FS17.

## FS12 — 16 statt 26 Laufwerke

**Urteil: STILL OPEN.** Basis, S–M. [gelesen]
`tos-api/src/bios/drive.rs:11` `MAX_DRIVES = 16` und
`tos-api/src/bdos/fs/drive.rs:20` `BLKDEVNUM = 16`; EmuTOS
`include/sysconf.h:43` `BLKDEVNUM 26`. Unverändert. Hinweis: Das
v0.6-Readme nennt „Laufwerke bis P:" (`3167c5f2`); mit dem Fix wird es
veraltet.

## A115 — Bereitliste nach `rlr`

**Urteil: STILL OPEN.** Basis, M. [gelesen]
`tos-api/src/bdos/process/sched.rs:445-454` `pick_next(after)` geht die
Slots ab `after+1` reihum durch. Seit A112 (`1762e837`) gibt es keinen
Commit an `sched.rs`. Eine `rlr`/`drl`-Liste gibt es nicht.

Überschneidung mit A116: Beide brauchen dieselbe Weckinfrastruktur in
`sched.rs`, sinnvoll als ein Arbeitspaket.

## A116 — `appl_write` auf eine volle fremde Queue

**Urteil: NEEDS USER.** Basis, M. [gelesen]
`tos-api/src/aes/appl.rs:572-581` liefert weiter `0`, und der Kommentar
verweist selbst auf A116. `docs/COMPATIBILITY.md:501-545` (A75) trägt weiter
die alte Begründung („hoechstens ein AES-Client"), die seit A113 nicht mehr
stimmt. Einen Weckschlüssel „Queue hat wieder Platz" gibt es nicht.

Schritt 1 des Tickets ist ausdrücklich der Anwenderentscheid. Danach M:
Weckschlüssel, Blockieren, COMPATIBILITY eingrenzen.

## RST02 — Warmstart EmuTOS-treu

**Urteil: STILL OPEN.** Basis, L (Track B: Neueinstieg in den Kern ohne
Firmware, MMU und Caches neu). [gelesen] `kernel/src/reset.rs:33-45`: `warm`
gleich `cold`, also Watchdog-Neustart (`pm::reboot`). Unverändert seit
`f63c39c1`.

## INPUT02 — Ctrl+Alt+Del bei hängendem Programm

**Urteil: STILL OPEN.** Basis, M. [gelesen]
Die Reset-Prüfung `reset_key` steht nur in `decode_key`
(`kernel/src/ikbd.rs:247-257`). `decode_key` wird nur aus `decode_byte`
gerufen. Im Interrupt (`CAPTURE`) landen Tastenberichte in `HELD`
(`ikbd.rs:97-106`, `capture_end` `:123-136` dekodiert nur reine
Mausbewegung). Dekodiert wird erst über `release_held` auf der Trap-Seite.

Ein EL0-Programm ohne Systemaufruf erreicht die Prüfung also nie. Die
Code-Lage entscheidet; ein Hängelauf war nicht nötig.

## CON03 — die Kernel-Konsole ist nicht EmuCON

**Urteil: NEEDS USER.** Basis, L. [gelesen]
`kernel/src/shell.rs:118-135` bietet `exit/quit`, `help`, `ver`, `cls`,
`pwd`, `cd`, `dir/ls`, `type/cat`, `run`, `bench`, `init` und den
Laufwerkswechsel `X:`. Unverändert außer der Versionszeile (`f7cb0fd9`).

Schritt 2 des Tickets ist eine Anwenderentscheidung: Befehlssatz in der
Kernel-Konsole nachrüsten oder EmuCON als EL0-Programm nach dem
ADR-026-Muster bauen. Bezug: GUI01 (`more` ist der einzige Textbetrachter in
EmuCON).

## DESK51 — `graf_shrinkbox`

**Urteil: STILL OPEN.** Basis, S. [gelesen]
Die AES hat die Funktion: `tos-api/src/aes/graf.rs:367-374`, Dispatch
`aes/mod.rs:883` (Opcode 74). Es fehlt das Binding in
`programs/newtos-gem/src/aes.rs` (nur `graf_growbox`). Die Aufrufstellen
`do_close_folder` und `refresh_drive` vermerken das selbst
(`programs/desktop/src/main.rs:2629`, `:2671`).

EmuTOS: `w_setpath` schrumpft von der Fenstermitte (`deskfun.c:809-825`),
`fun_full_close` nur bei `w_obid > 0` (`deskfun.c:828-856`). Der Desktop ist
seit dem 17.10. außer dem Release-Commit unverändert.

## DESK52 — `do_fopen` ohne Laufwerksprobe und `STDEEPPA`

**Urteil: STILL OPEN.** Basis, S. [gelesen]
`programs/desktop/src/main.rs:3474-3516`: nur die Probe für den Zweig
`WN_DESKTOP`. EmuTOS prüft vorher `set_default_path(root) < 0` →
`fun_close(pw, CLOSE_WINDOW)` und `strlen >= LEN_ZPATH` → `STDEEPPA`
(`desksupp.c:1097-1108`).

## DESK53 — ESC liest nicht neu ein

**Urteil: STILL OPEN, jetzt trivial.** Basis, S. [gelesen]
`hndl_kbd` ESC → `refresh_top()` (`main.rs:1189-1192`, `:3614-3626`):
`win_bldview` auf der vorhandenen Liste, kein Neueinlesen.

Neu seit DESK48: `refresh_window(handle)` mit `do_fopen(pw,0,p_spec,FALSE)`
existiert (`main.rs:2708-2720`, Aufrufer bisher nur `refresh_drive`). EmuTOS
ESC = `refresh_window(win_ontop(), TRUE)` (`deskmain.c:1213-1218`). Der
`force_mediach`-Arm betrifft nur A:/B:.

Damit ist der Fix ein Einzeiler: ESC ruft `refresh_window` für das oberste
Fenster auf. Überschneidung mit DESK52 (beide `do_fopen`).

## DESK54 — `do_xyfix` 3D-Korrektur

**Urteil: STILL OPEN.** Basis, S. [gelesen] `main.rs:2801-2813`, der
Kommentar nennt die Auslassung selbst. EmuTOS `desksupp.c:262-275` unter
`CONF_WITH_3D_OBJECTS` (Referenzbau 1).

## DESK55 — Fenstertitel

**Urteil: STILL OPEN.** Basis, S. [gelesen]
`spec_and_title` (`main.rs:3107-3119`) und `set_window_mask` (`:2208-2215`)
titeln mit dem Verzeichnis, die Maske nur bei echtem Filter. EmuTOS
`win_sname`: `" " + p_spec + " "` (`deskwin.c:846-858`).

Überschneidung: NewTOS hält die Maske in `p_mask` statt in `p_spec`. Genau
das verursacht auch DESK67 Punkt 3. Wer `p_spec` wie EmuTOS mit Maske führt,
behebt beide.

## DESK61 — `act_chg` und `f_selected`

**Urteil: STILL OPEN.** Basis, S. [gelesen]
`programs/desktop/src/act.rs:75-98`: `if was == set { return false; }` steht
vor der `f_selected`-Zuweisung. EmuTOS setzt `fn->f_selected` immer
(`deskact.c:447-449`) und liefert `TRUE` für jedes nicht deaktivierte Objekt
(`:465`). NewTOS nutzt den Rückgabewert intern als „geändert" fürs
Neuzeichnen (`main.rs:345`). Das ist eine interne Zerlegung, keine
API-Wirkung.

Einen Fall, in dem `ob_state` und `f_selected` auseinanderlaufen, habe ich
nicht gefunden. Die einzigen Schreiber von `ob_state |= SELECTED` ohne
`f_selected` sind Selbsttests (`act.rs:526`, `:547`) und `dir.rs:2692`, das
`f_selected` mitsetzt. Angleichen bleibt nach ADR-022 richtig; die sichtbare
Wirkung ist nicht belegt.

## DESK63 — Verknüpfung auf Verknüpfung

**Urteil: STILL OPEN.** Basis, S. [gelesen]
`programs/desktop/src/dir.rs:2284-2350`: Ziel weder Laufwerk noch Papierkorb
→ `illegal_op_msg()`. EmuTOS ruft für jede Quelle `fun_file2any(sobj, NULL,
target, …)` (`deskfun.c:1297`).

## DESK66 — Hintergrund an die AES melden

**Urteil: STILL OPEN.** Basis, M. [gelesen]
In `programs/desktop/src` gibt es nur den Hinweiskommentar (`inf.rs:3737`).
`set_aes_background` ist `pub(crate)` im Kern (`tos-api/src/aes/init.rs:211`).
Der erste Schritt (trägt ein vorhandener AES-Aufruf den Wert?) ist nicht
getan.

## DESK67 — Reste aus DESK48

**Urteil: STILL OPEN.** Basis, M. [gelesen] Punkt für Punkt:
1. `set_window_mask` → `redraw::redraw_desktop()` (`main.rs:2223`), die Suche
   ebenso (`:2422`, `:2425`). Offen.
2. `act_bsclick`/Gummiband: nicht einzeln nachgeprüft.
3. `fun_rebld`/`fun_mark_for_rebld` vergleichen das Verzeichnis
   (`dir.rs:2445-2456`, `:2503-2519`; der Kommentar sagt es selbst). Offen,
   siehe DESK55.
4. `fun_rebld_marked` läuft über `windows.slots`, nicht über `zorder`
   (`dir.rs:2460-2480`). Offen.
5. `do_new_folder` → `dir::rebuild_window(cwin)` (`main.rs:1925`). Offen.

## REF03 — falsche Spec-Zitate

**Urteil: STILL OPEN.** Hygiene, S. [gelesen, gegen die PDF]
`references/sd-spec/archive/PartA2_SD_Host_Controller_Simplified_Specification_Ver3.00.pdf`:
§2.2.14 Clock Control, **§2.2.15 Timeout Control**, **§2.2.16 Software
Reset**, §2.2.17/18 Interrupt Status; §2.2.2/3/5/7/10/11 wie in `emmc*.rs`
zitiert.

Falsch sind weiter `kernel/src/drivers/bcm2711/emmc.rs:71`, `:934`, `:983`
(§2.2.15 für Software Reset) und `TASK/HW03-STATUS-DONE.MD:216`. F3 steht
unverändert (`emmc.rs:986-990`, „the abort sequence of §3.8 … never asks
for"); §3.8.1 heißt in 3.00 „Asynchronous Abort".

Der Sweep findet **keine weiteren** verschobenen §2.2.x-Zitate in
`kernel/src/drivers/bcm2711/`. `emmc.rs` ist seit dem 17.10. unverändert.

## T15 — Zeichentests ohne Riegel

**Urteil: STILL OPEN, Abnahmezahl erfüllt.** Prüfstand, S. [Lauf]
`method/t15-stress.sh 300`: `aes_cicon_draw`, `aes_object_font` und
`aes_object_nicelines` je 300-mal, alle drei gleichzeitig mit
`--test-threads=32`, **0 Fehlschläge** (`hosttests/t15-stress-300.txt`). In
den drei Dateien gibt es weiter keinen Riegel [gelesen].

Punkt 1 (kann der Pfeil bei x 64..71, y 48..61 die Prüfstellen treffen?)
und Punkt 3 (drei Namen für einen Mechanismus) sind nicht erledigt. Ohne
Fehlschlag in 900 Läufen bleibt das Risiko theoretisch.

## DESK49 — `filetypes`

**Urteil: STILL OPEN.** Erweiterung (Werkzeug, nur das QEMU-Abbild), S.
[gelesen] `tools/make-sd-image.sh:694-736`: `*`, `*.*`, `*.PRG`, `*.APP`,
`*.GTP`, `*.TOS`, `*.TTP`, `*.ACC`, `*.TXT`, `*.IMG`. Die Prüffrage des
Tickets (Maskenvorgabe, keine Starterkennung) hat das Review vom 17.10.
beantwortet.

## LIBC-IOCTL

**Urteil: STILL OPEN.** Erweiterung (SDK), S–M. [gelesen]
`sdk/libc/newtos-arch/ioctl.c` ist weiter `return (int) Fcntl(fd, arg,
request);`, unverändert seit `bbf45898`.

## LIBC-BUF — 1-KiB-Puffer nach dem ersten `printf`

**Urteil: CLOSE empfohlen** (die Prüffrage ist beantwortet). Erweiterung
(SDK). [Lauf] QEMU raspi4b, `C:\AUTO\LIBCBUF.NCP`
(`method/libcbuf.c`), Ausgabe `serial/libcbuf-on.log`:

- `fopen`-Datei A, erste Ausgabe `fprintf`: `__bufsize` 0 → **0x8000**,
  Puffer angelegt; nach `fputs` weiter 0x8000.
- `fopen`-Datei B, erste Ausgabe `fputs`: 0 → 0x8000.
- `stdout` (hier Konsole): 0x400 → **0** nach dem ersten `printf`, wie im
  Ticket.

Die FS08-Regel (32 KiB für Dateien) wird also für `fopen`-Dateien **nicht**
unterlaufen. Damit ist meine statische Vermutung vom Anfang dieses Reviews
widerlegt. Es bleibt der Fall, den das Ticket beobachtet hat: Ein
**umgelenktes** `stdout` bekommt nach dem ersten `printf` 1 KiB statt 32 KiB.
Das ist Upstream-mintlib-Verhalten (`buffered_vfprintf`,
`sdk/libc/overlay/stdio/vfprintf.c:2246-2265`) und kostet nur Tempo. Ob ein
Overlay-Fix dafür lohnt, entscheidet der Anwender.

## MEM07 — `MAX_SHARED_BLOCKS = 64`

**Urteil: CLOSE.** Erweiterung. [gelesen]
Der verbliebene Auftrag vom 17.10. („weitere Verbraucher auszählen") ist
erledigt:
- Die Tabelle `TABLE` (`tos-api/src/bdos/mem/region.rs:46-72`) wird nur über
  `attach` gefüllt. Einziger Aufrufer außerhalb von `region.rs` ist
  `pexec.rs:404` (Basepage und Environment eines `Pexec`-4/6-Kindes, auch
  über `hand_over` `:378-386`).
- `F_KEEP`/`Ptermres` benutzen eine **eigene**, wachsende Liste (`kept()`,
  `reserve_keep`/`mark_keep` `:189-212`), nicht `TABLE`.
- Einträge werden bei `detach`/`forget` geleert (`:339`, `:357`, `:429`).
- Grenze: 2 × `MAX_PROCESSES` (16, `bdos/process.rs:67`) = 32 ≤ 64. Ist sie
  doch erreicht, liefert der Code `RegionError::Full` → `NoOwnerRoom` →
  `Pexec`-Fehler.

Optional (S): eine Compile-Zeit-Zusicherung `2 * MAX_PROCESSES <=
MAX_SHARED_BLOCKS`, damit eine künftige Anhebung von `MAX_PROCESSES` die
Grenze nicht still überholt.

## MEM08 — lineare Suche

**Urteil: STILL OPEN** (zuerst messen). Erweiterung, S. [gelesen]
`region.rs`/`traps.rs` unverändert seit `d2e3c384`; keine Messung zu `Pterm`
mit vielen Blöcken in `measurements/` (grep MEM08: nur die MEM05-Läufe).

## MEM10 — Speicherschutz-Sync ohne volle Momentaufnahme

**Urteil: STILL OPEN.** Erweiterung, M. [gelesen]
`memprot.rs` baut die Momentaufnahme `SNAP` weiter je Sync
(`:997-1042`, `sync_now` `:772`). Die Messung liegt vor:
`measurements/measurement-2026-10-13-mem05-kernel/RUNINFO.TXT:141-142`
(23,3 s gegen 0,06 s) und `…-mem05-acceptance/RUNINFO.TXT:32` (21,0 s gegen
0,05 s). Nächster Schritt laut Ticket: Entwurf zur Freigabe.

Überschneidung mit MEM08 (beide Laufzeit bei vielen Blöcken).

## MEM06 — reservierter virtueller Heap

**Urteil: STILL OPEN** als Idee, nichts zu tun ohne Freigabe. Erweiterung.
Unverändert.

## ARCH03 Stufe 2b — FreeMiNT-Modi 100+

**Urteil: STILL OPEN.** Erweiterung (FreeMiNT, ADR-027), M. [gelesen]
`tos-api/src/bdos/process/pexec.rs:179-194` bedient die Modi 0/3/4/5/6/7,
alles andere geht nach `UnknownMode` → `EINVFN`. FreeMiNT:
`references/freemint/sys/k_exec.c:210-234` (`case 100`, `200`, `106`,
`104`). In `docs/api/gemdos/process/4b-pexec.md` gibt es keine Spec für
100+, also zuerst die Spec (Regel 5). Hinweis [Schluss]: 100/104/106 sind
asynchrone Starts; sie setzen ein Kind voraus, das neben dem Aufrufer
weiterläuft. Das berührt das kooperative Modell und ist mehr als eine
Lader-Erweiterung.

## Erweiterungen — Einzeiler

- **SND01 / SND02:** nicht gebaut. `kernel/src/drivers/bcm2711/` enthält
  `clock_tags`, `emmc*`, `framebuffer`, `mailbox`, `pl011`, `pm`, keinen
  PWM-/DMA-/HDMI-Audio-Treiber; kein `soundcmd`/`buffoper`/`devconnect` in
  `tos-api/src`. Weiter Erweiterung.
- **SDL01:** nichts in `sdk/`. Weiter Erweiterung, hängt für Ton an SND01.
- **RTC01:** kein `ds3231`/`i2c`/`bsc1` in `kernel/src`/`tos-api/src`. Weiter
  Erweiterung.
- **IO01:** nur `pl011` als Konsole, kein AUX-/MIDI-/PRN-Treiber. Weiter Idee.
- **DESK65:** nur der Hintergrunddialog speichert sofort (`inf.rs:3809`,
  `:4796`; `main.rs:1896` ist „Desktop sichern"). Weiter Erweiterung.
- **GUI01:** Der Anwender hat am 2026-10-18 bestätigt: kein eingebautes
  `show_file`, Betrachter über die Anwendungszuordnung. Das Ticket behauptet
  „Abweichung dokumentiert (DESK64)". **Das stimmt nicht:** In
  `docs/COMPATIBILITY.md` gibt es keinen Eintrag zu `CONF_WITH_SHOW_FILE`
  (grep `show_file`/`SHOW_FILE`/`DESK64` → nichts), und DESK64 behandelt den
  Hintergrunddialog. Im Referenzbau ist der Schalter `1`
  (`references/emutos/include/config.h:1842-1843`). Damit ist GUI01 eine
  **benannte Basis-Abweichung**, keine Erweiterung. Zum Schließen fehlt der
  COMPATIBILITY-Eintrag (S, Doku).

## Überschneidungen und Doppelungen

- **FS17 Punkte 2/4/5 + FS16:** eine gemeinsame Ursache (kein geteiltes DFD,
  kein `O_DIRTY`/Zurückschreiben in `ixclose`). FS17 Punkt 3 und FS17 Punkt 6
  hängen an der Slotwahl von `Frename`/`Fcreate`. Vorschlag: ein Arbeitspaket
  „DFD und Slotwahl wie EmuTOS", Punkt 3 vorneweg als Defekt.
- **FS14 + FS17 Punkt 3:** beide in `frename` (`file.rs:890-990`).
- **DESK55 + DESK67 Punkt 3:** Maske in `p_mask` statt in `p_spec`.
- **DESK52 + DESK53:** beide `do_fopen`; DESK53 ist mit `refresh_window`
  ein Einzeiler.
- **A115 + A116:** dieselbe Weckinfrastruktur in `sched.rs`.
- **RST02 + INPUT02:** beide Reste aus RST01, aber unabhängig voneinander
  lösbar.
- **CON03 + GUI01:** `more` als einziger EmuCON-Textbetrachter.
- **FS15 + IO01:** `AUX:`/`PRN:` führen auf die „nicht vorhanden"-Geräte.
- **MEM08 + MEM10:** Laufzeit bei vielen Blöcken; MEM07 entfällt.

## Spec- und Doku-Befunde über die Urteile hinaus

1. `TASK/GUI01.md` „Abweichung dokumentiert (DESK64)" ist falsch (siehe
   oben).
2. `docs/COMPATIBILITY.md:501-545` (A75) trägt weiter die seit A113 falsche
   Begründung „höchstens ein AES-Client". Die Spec ist revidiert
   (`application.md`), COMPATIBILITY nicht. Das Ticket A116 sagt es selbst.
3. FS15: Die künftige Spec muss den Rückgabewert `0x0000FFFF` usw. (positives
   LONG) festhalten, nicht `-1`.

## Außerhalb des Auftrags (je eine Zeile, nicht verfolgt)

- Dateien, die auf einer QEMU-Karte ohne `C:\KERNEL8.IMG` angelegt werden,
  tragen 1980-01-01 00:00 (Uhr ohne Startwert). Für dieses Kartenlayout
  erwartet, nur vermerkt.
- `Frename` sucht die Quelle mit `find_in_dir` (jedes Attribut). EmuTOS
  sucht mit `FA_NORM|FA_SUBDIR` (`fsdir.c:936`), ein Volume-Label ist dort
  nicht umbenennbar. Nicht gefahren.
