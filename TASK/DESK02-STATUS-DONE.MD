# DESK02 — STATUS: DONE (2026-09-24)

**Auftrag:** `Options → Set preferences` nachbauen, die Werte an die
vorhandenen Stellen binden, Persistenz ueber DESK09.

Basis: Branch `desk-rebuild`, HEAD `8d271cb`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/stash/reset`.

## Was EmuTOS an dieser Stelle tut

`desk/deskinf.c:721-818` `inf_pref()`: `deselect_all(tree1)`, dann je Gruppe
den Knopf setzen, der dem aktuellen Wert entspricht — die Doppelklickrate
kommt dabei aus `evnt_dclick(0, FALSE)` (`:751`), also vom **AES**, nicht aus
`GLOBES`. Danach `inf_show(tree1, ROOT)`, `inf_what(tree1, SPOK)`, und bei OK
jede Gruppe mit `inf_which`/`inf_gindex` zurueckgelesen; die Rate wird mit
`evnt_dclick(neu, TRUE)` **sofort** gesetzt (`:791`). Rueckgabe: `TRUE` nur
dann, wenn die Ansicht `V_TEXT` ist **und** sich Zeit- oder Datumsformat
wirklich geaendert hat (`:807-814`). `deskmain.c:885-887` ruft
`if (inf_pref()) desk_all(FALSE);`.

Der Baum `ADSETPRE` (`desk/desk_rsc.c:1369-1560`): `OUTLINED`-Wurzel 40x21,
Titel, **sechs** beschriftete Gruppen, OK/Cancel. Jede Gruppe haengt unter
einem **eigenen `G_IBOX`** — und das ist tragend, nicht schmueckend: der AES
waehlt Geschwister ueber die Kinderkette des **Elternknotens** ab
(`fm_button` → `select_radio`), zwei Gruppen unter einem Elternknoten waeren
fuer ihn **eine**.

Die `#E`-Bits stehen in `deskapp.c:86-115`; gelesen werden sie in
`app_start`s `'E'`-Arm (`:917-934`), geschrieben in `app_save` (`:1138-1160`),
und zwischen `CSAVE` und `GLOBES` bewegt von `cnx_put`/`cnx_get`
(`deskmain.c:1380-1385` / `:1441-1460`).

## Was NewTOS hatte

`Item::SetPrefs` fiel in `do_optnmenu`s `_ => {}` und war in `men_update`
abgeschaltet. `global::CSave` fuehrte nur `cs_view`/`cs_sort`/`cs_sizefit`;
der Kopfkommentar hielt ausdruecklich fest, dass fuer die uebrigen Felder
„kein Live-Zustand" existiert. Die `#E`-Bytes `e1`/`e2` wurden ungelesen und
unveraendert durch den Sitzungsspeicher geschleift.

## Was getan wurde

**`model.rs`** — `TimeFmt` (`H24`/`H12`/`Idt`) und `DateFmt`
(`Mdy`/`Dmy`/`Idt`) mit EmuTOS' Werten (`deskbind.h:65-70`), plus
`rendered()` je Typ: **eine** Regel, die den unerreichbaren `Idt`-Wert
aufloest, mit zwei Lesern (Formatierer und Dialogvorbelegung).

**`global.rs`** — `DeskState`: `g_cdelepref`, `g_cdclkpref`, `g_ctimeform`,
`g_cdateform`. `CSave`: `cs_confdel`, `cs_dblclick`, `cs_timefmt`,
`cs_datefmt` (+ `DEF_DCLICK = 2` aus `INF_E1_DEFAULT`). Der Kopfkommentar
listet jetzt, **welche** `CSAVE`-Felder weiterhin fehlen und warum.

**`inf.rs`** —

* die `#E`-Bitkonstanten `INF_E1_CONFDEL`/`INF_E1_DCMASK`/
  `INF_E2_IDTDATE`/`INF_E2_IDTTIME`/`INF_E2_DAYMONTH`/`INF_E2_24HCLOCK`;
* `env_to_prefs`/`prefs_to_env` (Gegenstuecke zu `deskapp.c:917-934` bzw.
  `:1138-1160`), in `apply()` und `build_from_state()` verdrahtet;
* `fmt_date`/`fmt_time` (aus `win.rs` hierher gezogen, wo EmuTOS sie hat,
  `deskinf.c:74-180`), von **beiden** Lesern benutzt: der Textansicht
  (`win::format_fnode`) und dem Show-Info-Kasten (`inf_dttm`,
  `deskinf.c:460-468`);
* `inf_which`/`inf_gindex`/`inf_what` (`deskinf.c:705-716`,
  `util/optimize.c:134-170`);
* `inf_pref()` selbst, 23 Objekte, vier Gruppen je unter einem `G_IBOX`.

**`main.rs`** — `cnx_put` schreibt die vier Felder in `CSave` (die Rate aus
dem Zwischenspeicher `g_cdclkpref`, **nicht** aus einem frischen Trap:
`cnx_put` laeuft bei jedem Fensterzug, `deskmain.c:1382`); `cnx_get_prefs`
liest sie zurueck und schiebt die Rate mit `evnt_dclick(rate, TRUE)` in den
AES (`deskmain.c:1460`); `do_optnmenu` bekommt den `PREFITEM`-Fall.

**`mod.rs`** — neue aesbind-Bindung `evnt_dclick(rate, setit)` (Opcode 26).

**`dir.rs`** — `confirm_deletes()` als Tor vor den beiden Ruecksprachen, die
EmuTOS' `d_doop` stellt (`deskdir.c:906`). Der **Desktopsymbol→Papierkorb**-
Abwurf bleibt ungefiltert: dort steht bei EmuTOS `wants_to_delete_files`
unbedingt davor (`deskfun.c:1049-1051`), die Einstellung erreicht erst die
Frage danach.

**`menu.rs`** — `Item::SetPrefs` freigegeben (EmuTOS listet `PREFITEM` in
keiner `ILL_*`-Tabelle, `deskmain.c:143-160`).

## Der Feldsatz, und was fehlt

Gezeigt, in EmuTOS' Reihenfolge: **Confirm deletes**, **Double-click speed**,
**Time format**, **Date format**. Drei Felder von `ADSETPRE` fehlen, und zwar
**abwesend** statt vorhanden-und-wirkungslos:

* **Confirm copies** (`SPCCYES`/`SPCCNO`) — EmuTOS' Schalter schaltet den
  Kopierdialog in `d_doop` ein (`deskdir.c:939`). NewTOS' Kopierwerk hat
  ueberhaupt keinen (der Modulkopf von `dir.rs` haelt das seit Task #9 als
  bewusste Abweichung fest).
* **Confirm overwrites** (`SPCOWYES`/`SPCOWNO`) — dasselbe eine Ebene tiefer:
  EmuTOS fragt vor dem Ersetzen (`deskdir.c:325`), NewTOS ueberschreibt immer
  (`Fcreate` kuerzt).
* **Der `Default`-Knopf beider Formatgruppen** (`SPTF_DEF`/`SPDF_DEF`) —
  „default" heisst *nimm das Format aus dem `_IDT`-Keks*, den EmuTOS mit
  `Supexec(get_idt_cookie)` liest (`desksupp.c:219`). XBIOS `Supexec` (0x26)
  ist nicht implementiert (`tos-api/src/xbios/mod.rs:129`), und der Desktop
  darf seit DESK10 an Kernelzustand nur ueber den Trap.

Die `Idt`-Werte werden trotzdem **gelesen und unveraendert
zurueckgeschrieben** (`env_to_prefs`/`prefs_to_env`), damit eine von EmuTOS
geschriebene `NEWDESK.INF` ihre Bits behaelt.

## Ein echter Defekt, den die Messung gefunden hat

Der erste Probelauf (`screendumps/probe-C-prefs-BEFORE-idt-fix.png`) zeigte
`DMY` vorbelegt auf einer Maschine, deren Textansicht `MDY` druckte. Ursache:
die Ersatzregel fuer `Idt` stand **zweimal** im Baum — einmal in `fmt_date`
(`Idt` → `Mdy`) und einmal als `!= DateFmt::Mdy` in der Vorbelegung (`Idt` →
der zweite Knopf, also `Dmy`) — und die beiden Kopien widersprachen sich. Das
Startabbild traegt eine 21-Byte-Platzhalter-`NEWDESK.INF`, also greift
`InfData::default()` mit `INF_E2_DEFAULT = 0xE0`, und beide `IDT`-Bits sind
gesetzt: der Fall war der **Normalfall**, nicht ein Randfall.

Behoben mit `TimeFmt::rendered()`/`DateFmt::rendered()`: eine Regel, durch die
beide Leser gehen. `screendumps/prefs-p-C-prefs.png` ist derselbe Lauf danach
(`MDY` vorbelegt).

## Zwei benannte Abweichungen

1. **`desk_all(VIEW_CHANGED)` statt EmuTOS' `desk_all(FALSE)`.** EmuTOS
   kommt mit einem reinen Neuzeichnen aus, weil seine Textzeilen **beim
   Zeichnen** vom `G_USERDEF` `dr_fnode` formatiert werden
   (`deskinf.c:310-326`). NewTOS baut die Zeile einmal in `win_bldview` →
   `format_fnode` und legt sie ins Objektetikett, braucht also einen
   **Neuaufbau**. `inf_pref`s Rueckgabe bleibt EmuTOS' enge
   (`deskinf.c:807-814`) — verglichen wird gegen den *gerenderten* alten
   Wert, damit ein Wechsel von `Idt` auf das `H24`, das ohnehin auf dem
   Schirm stand, keinen Neuaufbau bestellt.
2. **Der Datumstrenner bleibt pro Aufrufstelle, wie er war.** EmuTOS
   uebergibt an beiden Stellen `DATEFORM_SEP` (`'/'`, `deskbind.h:64`);
   NewTOS' Textansicht druckt seit ihrer Entstehung `MM-DD-YY` mit
   Gedankenstrich. DESK02 bindet die **Reihenfolge** an die Einstellung und
   laesst den Trenner unangetastet, damit sich kein Schirm aendert, den
   dieses Ticket nicht aendern sollte. Der Gedankenstrich ist als eigener
   Befund gemeldet.

`fmt_time` schreibt uebrigens immer **7 Zeichen**: `hh:mm` plus `am`/`pm`
oder plus die zwei Leerzeichen, die EmuTOS im 24-Stunden-Fall anhaengt
(`char *suffix = "  ";`, `deskinf.c:77`). Die feste Feldbreite ist, was der
Textansicht ihre Spalten erlaubt; im 24-Stunden-Fall schneidet der
Leerzeichen-Trimmer sie wieder weg, die Vorgabezeile ist also **bytegleich**
zu der vor DESK02.

## Pruefung

**Hosttest (neu): `tos-api/tests/aes_desk_prefs_radio.rs`, 3 Faelle.** Er
prueft die eine AES-Eigenschaft, auf die dieser Dialog neu angewiesen ist —
dass eine Radiogruppe von ihrem **Elternknoten** begrenzt wird —, gefahren
durch den echten Dispatcher mit `form_button` (56):

* `a_radio_click_selects_one_button_of_its_own_group` — Klick in Gruppe A
  laesst Gruppe B unberuehrt;
* `each_group_keeps_its_own_selection_across_clicks` — jede Gruppe behaelt
  ueber alle Klicks hinweg genau **eine** Auswahl (was `inf_which`/
  `inf_gindex` brauchen, die sonst `-1` liefern);
* `without_its_ibox_a_group_takes_the_whole_parent_with_it` — das
  Gegenbeispiel: ohne die `G_IBOX`en loescht ein Klick die Auswahl der
  anderen Gruppe. Damit ist der `G_IBOX` nicht als „unsichtbar, also
  entbehrlich" entfernbar.

Ausserdem prueft `desk::inf::selftest()` (unverdrahtet, wie die uebrigen
`selftest_*`) den `#E`-Rundlauf jetzt auch fuer die vier
Einstellungsfelder — zwei neue Faelle, in Speicher und auf Platte.

**Bau:** frisch je Board (`cargo clean -p kernel -p tos-api` davor);
`cargo xtask build --board rpi4` und `--board qemu-virt`: **tos-api 64 /
kernel 94 auf beiden**, rc 0 (Basis 64/94, Delta 0). Clippy tos-api **80**,
kernel **125** auf beiden (Basis 80/125), **null** Treffer in
`kernel/src/desk/`. `cargo run -p xtask -- test`: **932 bestanden /
0 fehlgeschlagen / 1 ausgelassen** (Basis 929/0/1 + 3 neue).

**QEMU raspi4b** (Scratch-Kernel mit dem DESK08/09-IKBD-Injektor, 640x480,
`method/desk-desk02.actions`): `Options` → `Set preferences...` →
`Date format: DMY` → `OK` → `Options` → `Save desktop...`.

* `prefs-p-C-prefs.png`: der Dialog steht gemessen bei
  **(129, 143, 382, 212)** — `form_center`s Wert
  (`(640-382)/2 = 129`, `19 + (480-19-212)/2 = 143`). Vorbelegt sind
  `Yes` / `3` / `24H` / `MDY` (je Knopfrechteck als Schwarzanteil gemessen:
  0.87 / 0.91 / 0.85 / 0.85 fuer die gewaehlten, 0.05–0.15 fuer die uebrigen).
* `prefs-p-D-dmy.png`: nach dem `DMY`-Klick ist `MDY` geloescht, `DMY`
  gesetzt, und die **drei anderen Zeilen sind unveraendert**.
* `prefs-p-E-closed.png` gegen `prefs-p-A-idle.png`: **bytegleich**
  (md5 `710d7f24…`, pixdiff 0).
* `inf/NEWDESK.INF.after-prefs`, aus dem Abbild ausgelesen (SHA-256
  `8a6f9dd6…`): `#E 1A 86 00 00 60` — `e1 = 0x1A` unveraendert
  (`CONFDEL|CONFCPY|dclick 2`), `e2 = 0x86` =
  `BLITTER|DAYMONTH|24HCLOCK`, **beide `IDT`-Bits geloescht**, weil der
  Benutzer eine ausdrueckliche Wahl getroffen hat.
* **Kaltstart** von diesem Abbild (`cold-C-prefs.png`): der Dialog oeffnet
  mit `DMY` vorbelegt.
* **Bindungsbeleg** (`method/desk-desk02-text.actions`, derselbe Kernel, zwei
  Abbilder): `t-mdy.png` (Startabbild) druckt `09-23-26`, `t-dmy.png` (das
  gespeicherte) `23-09-26` — die Einstellung erreicht also `format_fnode`
  und nicht nur den Dialog. Die Zeitspalte ist beidemal `09:58`.
* `fsck_msdos -n` auf der Partition: Phase 1/2/3 ohne Beanstandung.
* Null `[EXC]`/`[PANIC]` in allen Laeufen.

## Artefakte

* `kernel/src/desk/{model,global,inf,main,mod,menu,dir,win}.rs` —
  uncommitted im Baum.
* `tos-api/tests/aes_desk_prefs_radio.rs` — neu.
* `docs/api/desktop/deskinf.md` — neuer Abschnitt „NewTOS implementation
  notes — `inf_pref` and the date/time formatters (DESK02)": Baumtafel, die
  drei fehlenden Felder mit Begruendung, die Tafel „wo jedes Feld lebt", die
  beiden Abweichungen, die Messwerte.
* `docs/api/desktop/deskmain.md` — **Erratum** im `do_optnmenu`-Absatz:
  `PREFITEM` → `desk_all(FALSE)` steht dort bei EmuTOS **bedingt**
  (`if (inf_pref())`, `deskmain.c:885-887`), der Text sagte
  „unconditionally".
* `docs/api/desktop/_hw-test-2026-09-17.md` — Punkt **37**, Schritte 3-7.
* `docs/STATUS.md` — Phase-14-Zeile + *Recent Decisions* (2026-09-24).
* `measurement-2026-09-24-desk01-desk02/`.

## Offen

**Hardwareabnahme:** Pi 4, HW-Checkliste Punkt **37**, Schritte 3-7. Zwei
Proben kann QEMU grundsaetzlich nicht fahren: die **Doppelklickrate** (sie
misst echte Klickabstaende) und die **Loeschruecksprache** (sie loescht
wirklich). Die 1920x1080-Optik des Dialogs ebenso.

Nicht gefahren: `12H` (der Lauf hat das Datumsformat geaendert, nicht das
Zeitformat) — derselbe Pfad, dieselben zwei Knoepfe, dieselbe `#E`-Haelfte;
der Bitwert steht in Punkt 37, Schritt 7.

---

## Nachtrag 2026-09-24 (DESK01b) — Ticket bleibt ERLEDIGT

Der Dialog hatte keine `FL3D*`-Flagge. Er hat jetzt `ADSETPRE`s, Objekt fuer
Objekt: Wurzel `FL3DBAK` + `ob_state = OUTLINED` + `ob_spec = (1<<16)|0x1100`
(`desk_rsc.c:1371-1375`), jeder Radioknopf `FL3DIND` (`:1396` ff.),
`SPOK`/`SPCNCL` `FL3DACT` (`:1174,1180`), die vier `G_IBOX`en und alle
`G_STRING`s flaggenlos — wie in der Quelle.

**Der Kastenwert musste mit.** `(1<<16)|0x1070` hat Muster 7 = `IP_SOLID`;
`just_draw` ersetzt die Flaeche durch die 3D-Farbe nur, wenn der Kasten **hohl
und weiss** ist (`gemoblib.c:619-636`), die Flagge waere also wirkungslos
geblieben. Jetzt `desk_rsc.c`s `0x1100`.

**Die Masse sind jetzt `ADSETPRE`s.** Ein `FL3DIND`-Knopf wird vor dem Zeichnen
um `ADJ3DSTD` (2 px je Seite) vergroessert und bekommt seinen Rahmen ausserhalb
davon, beansprucht also 3 px je Seite mehr als sein Rechteck sagt. Bei der
alten Teilung (Zeilenabstand 26, Knopfluecke 6) haetten sich die Rahmen
benachbarter Zeilen Pixel geteilt. Neu: Zeilenabstand **32** px (`ADSETPRE`
setzt seine Zeilen auf die Zeichenzeilen 3/5/7/9, `desk_rsc.c:1386` ff.) und
Knopfluecke **8** px (`:1462-1493`: 15-px-Knoepfe auf 23-px-Teilung). Der
Dialog waechst damit von **382x212** auf **390x244**; gemessen steht er bei
(125, 127) = `form_center`s Wert.

Am Feldsatz, an `inf_which`/`inf_gindex`/`inf_what`, an der `#E`-Bindung und an
den drei bewusst fehlenden Feldern aendert sich **nichts**.

**Beleg:** `measurement-2026-09-24-desk01b/` — `prefs-C-prefs.png` zeigt alle
vier Gruppen als 3D-Indikatoren (unausgewaehlt erhaben, ausgewaehlt in der
dunkleren Flaeche mit versenkter Fase und weisser Schrift, `zoom-radios.png`);
Vorbelegung unveraendert `Yes / 3 / 24H / MDY`; `Cancel` laesst den Schirm
bytegleich (pixdiff **0**, md5 `710d7f24…`).

Spec: `docs/api/desktop/deskinf.md`, Abschnitt „NewTOS implementation notes —
the 3D flag set and the About content (DESK01b)".
