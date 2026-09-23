# DESK03 — STATUS: DONE (2026-09-24)

**Auftrag:** `Options → Install icon` und `Options → Remove desktop icon`
nachbauen, an die Desktop-Symboltabelle binden, Persistenz ueber DESK09.

Basis: Branch `desk-rebuild`, HEAD `edbb5cc`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/clean/stash/reset`.

DESK03 und DESK04 sind **ein** Arbeitspaket; diese Quittung beschreibt das
Ganze, [DESK04-STATUS-DONE.MD](DESK04-STATUS-DONE.MD) nur, was allein
`ins_devices` betrifft.

## Der Befund, der vor der Aufgabe stand

Die Aufgabe liess sich nicht als zwei Handler erledigen, weil es nichts gab,
worauf sie haetten wirken koennen.

`app_blddesk` **leitete** die Desktop-Symbole bei jedem Aufruf ab: ein
`G_ICON` je gesetztem Bit von `dos_sdrv(dos_gdrv())`, dazu ein Papierkorb und
ein Drucker als feste Zutat; die `#M`/`#T`/`#O`-Zeilen der `NEWDESK.INF`
steuerten nur die Rasterstelle bei (`inf_desk_icons: Vec<(u8,u8,u8,u8)>`).
Drei Folgen:

* *Install devices* war bauartbedingt wirkungslos — jedes Laufwerk der Karte
  hatte immer schon ein Symbol;
* *Remove icon* konnte nichts entfernen — der naechste `app_blddesk` holte es
  zurueck;
* *Install icon* hatte kein Ziel, in das es schreiben koennte.

## Was EmuTOS an dieser Stelle tut

`G.g_ahead` ist eine Liste (`ANODE`, `deskapp.h:83-99`), und `app_blddesk`
(`deskapp.c:1337-1404`) laeuft sie ab und erfindet nichts. Was in der Liste
steht, entscheidet `app_start` einmalig — aus der `EMUDESK.INF`
(`app_parse`s Tag-Schalter, `:295-346`, plus der Raster→Pixel-Umrechnung bei
`:979-987`) oder, wenn es keine gibt, aus `build_inf` (`:696-752`). Danach
bearbeiten `ins_devices`, `ins_icon` und `rmv_icon` (`deskins.c`) dieselbe
Liste, `move_drvicon` (`deskact.c:146-151`) schreibt die gezogene Stelle
hinein, und `app_save` (`:1216-1262`) schreibt sie zurueck.

## Was getan wurde

**`global.rs`** — `Anode` (der `AF_ISDESK`-Zuschnitt von `ANODE`:
`a_type`/`a_letter`/`a_aicon`/`a_pappl`/`a_xspot`/`a_yspot`/`a_obid`) und
`DeskState::g_ahead: Vec<Anode>` an der Stelle von `inf_desk_icons`.

**`app.rs`** — `app_blddesk` laeuft `g_ahead` ab, schreibt `pa->a_obid`
zurueck (`deskapp.c:1369`; ohne das kann `app_afind_by_id` ein angeklicktes
Objekt nicht auf seinen Knoten abbilden) und bricht bei erschoepftem
Objektvorrat ab statt zurueckzukehren (`:1364-1368`). Neu:
`app_afind_by_id` (`:1408-1419`), `snap_icon` in Pixeln (EmuTOS' Einheit,
`deskins.c:146-184`) neben dem vorhandenen `snap_icon_grid`,
`default_icon_kind` (das typgerechte Ersatzbild).

**Neu: [`kernel/src/desk/ins.rs`](../kernel/src/desk/ins.rs)** — `deskins.c`:
`grid_free`/`ins_posdisk`, `install_drive`/`remove_drive`/`ins_devices`,
`install_desktop_icon` (der `ADINSDSK`-Baum), `ins_icon`/`rmv_icon`, dazu ein
kopfloser `selftest()`.

**`inf.rs`** — `apply` fuellt `g_ahead` aus den `#M`/`#T`/`#O`-Zeilen (mit
EmuTOS' Raster→Pixel-Umrechnung und `snap_icon`), `build_inf`/
`build_desk_icons` saeen sie aus der Laufwerkskarte, `build_from_state`
schreibt sie zurueck — aus der **Liste**, nicht mehr aus dem Objektbaum, und
mit echter Bildnummer.

**`act.rs`** — `move_drvicon` schreibt die gerasterte Stelle in den Knoten
(`deskact.c:146-151`); ohne das spraenge ein gezogenes Symbol beim naechsten
Neuaufbau zurueck und `app_save` wuerde die alte Stelle sichern.

**`win.rs`** — `win_isel` (`deskwin.c:825-840`) und
`count_selected_desk_icons` (`men_update`s `ndesk`).

**`menu.rs`** — `men_update` zaehlt `ndesk` mit; *Remove desktop icon* haengt
daran, *Install icon* und *Install devices* sind immer verfuegbar.

**`main.rs`** — die drei `do_optnmenu`-Faelle (`deskmain.c:847-881`),
`first_selected()` (= `win_isel(G.g_screen, G.g_croot, 0)`, `:842`) und
`rebuild_desktop_icons()` (`app_blddesk` + das `do_wredraw(DESKWH, &G.g_desk)`,
das EmuTOS ans Ende von `app_blddesk` haengt, `deskapp.c:1402`, + `men_update`,
weil die Auswahl mit den Objekten gestorben ist).

**`dir.rs`** — `draw_fld` (`deskdir.c:131-139`), das EmuTOS an genau dieser
Stelle hat; **`mod.rs`** — die `objc_offset`-Bindung (44), die es braucht,
und `mod ins`.

**`icons.rs`** — EmuTOS' `IG_*`-Nummern und die Abbildung auf unsere Raster.

## Der Defekt, den die Messung gefunden hat

Der erste Startlauf zeigte einen Desktop **ohne jedes Symbol**.

Ursache: die Liste muss beim Start gesaet werden, und ich hatte die Bedingung
dafuer als „gibt es die Datei?" gebaut. EmuTOS entscheidet ueber die
**Signatur**: `read_inf_file(inf_data); if (inf_data[0] != '#')
build_inf(…)` (`deskapp.c:829-834`) — `shel_get` liefert so oder so einen
Puffer, also ist das `#` das Einzige, was „INF-Daten" von „keine" trennt. Das
Testabbild traegt eine 21-Byte-`newdesk placeholder\r\n` als `NEWDESK.INF`,
womit der Fall der **Normalfall** war, nicht ein Randfall.

Die Gegenrichtung ist genauso tragend und wird von derselben Bedingung
getragen: eine Datei, die INF-Daten *ist* und keine Symbolzeile fuehrt,
beschreibt einen Desktop **ohne** Symbole — genau das schreibt *Remove desktop
icon* + *Save desktop*, und `build_inf` darf es nicht rueckgaengig machen.
Beide Richtungen sind gemessen (`cold-desktop.png` / `cold2-desktop.png`).

## Eine sichtbare Folge, die eine Lead-Entscheidung verdient

Weil die Liste gesaet werden muss, ist `build_inf`s Anordnung jetzt die
Vorgabeanordnung des Desktops — und sie ist **nicht** die, die NewTOS bisher
zeichnete:

| | bisher | jetzt (`deskapp.c:715-754`) |
|---|---|---|
| Laufwerke | Spalte 0, untereinander | reihenweise von links (`icon_index % xcnt`) |
| Papierkorb | `(cols-1, 0)`, oben rechts | `(0, ycnt-1)`, unten links — nach rechts nur, wenn die Laufwerke die Reihe schon erreicht haben |
| Drucker | `(cols-1, 1)` | unten rechts, und nur wenn der Papierkorb links blieb |

Die bisherige Regel hatte **keinen** Quellenbeleg (der Kommentar verwies auf
`deskapp.c:721`, wo nur `IG_HARD`/`IG_FLOPPY` steht). Die neue ist zeilenweise
belegt. Bei 640x480 mit einem Laufwerk ist der Unterschied, dass Papierkorb
und Drucker von oben rechts in die unterste Reihe wandern — `boot-desktop.png`
gegen `measurement-2026-09-24-desk01b/screendumps/about-A-idle.png`.

**Nebenwirkung fuer die Messmethode:** die Ecke unten rechts, auf die die
bisherigen Desk-Laeufe den Zeiger parken (`parkbr`), haelt jetzt den Drucker.
Die Laeufe dieses Tickets parken bei (600, 240).

## Das Symbolbild ist jetzt echter Zustand

Die `#M`/`#T`/`#O`-Zeile trug bisher immer `FF FF`, weil es nichts zu
schreiben gab. Jetzt steht dort `a_aicon`, in EmuTOS' `IG_*`-Nummerierung —
und die ist **Dateiformat**, nicht ein interner Index: `app_parse` liest sie
(`deskapp.c:359-365`), `app_save` schreibt sie (`:1258`), `desk_inf_data2`
buchstabiert `07`/`02`/`06`.

**`IG_REMOV` (5) fehlt uns.** Das Wechselplatten-Bild ist das einzige der acht
`IG_*`, fuer das NewTOS kein Raster hat (die Bitmaps stammen aus
`desk/icon.rsc`, dieses war nie dabei). Statt es zu ersetzen:

* das Schrittwerk laeuft ueber `icons::BUILTIN_ICONS` (sieben Eintraege), nicht
  ueber `0 .. G.g_numiblks-1` (acht) — es kann also nicht auf ein Bild
  springen, das es nicht gibt;
* eine `a_aicon`, die nicht aufloest, faellt in `app_blddesk` auf das Bild
  zurueck, das der Knotentyp vorgibt (`app::default_icon_kind`), statt leer zu
  zeichnen.

**Die Diskette wird nur angeboten, wenn sie benutzbar ist**: `IG_FLOPPY` faellt
aus der Schrittliste, solange die Laufwerkskarte kein `A:`/`B:` meldet — die
Maschine hat keinen Diskettenanschluss, und das Menue laesst *Format* aus
demselben Grund weg. Eine Ausnahme: ein Knoten, der `IG_FLOPPY` **schon**
traegt (eine anderswo geschriebene `NEWDESK.INF`), behaelt es in der Liste,
damit der Dialog von seinem eigenen Bild wegsteppen kann, statt es still zu
ueberschreiben.

## Der `ADINSDSK`-Baum

Siebzehn Objekte mit **denselben Nummern**, die `desk_rsc.h:100-112` ihnen
gibt, damit man die beiden nebeneinanderlegen kann. 3D-Rollen nach DESK01bs
Tafel; die zwei Zeilen, die dort ausdruecklich als „nicht das, was man raten
wuerde" stehen, sind genau die dieses Baums: ein Schrittgadget ist
`FL3DACT` (`desk_rsc.c:990,996`).

Zwei Kastenwerte, aus demselben Grund, aus dem DESK01b `ADDINFO`s nennen
musste (`just_draw` ersetzt die Flaeche nur fuer einen **hohlen weissen**
Kasten, `gemoblib.c:619-636`): die Wurzel `69888L` = `(1<<16)|0x1100` (ein
Pixel Rahmen), `ID_IBOX` **`4352L` = `0x1100`** — dasselbe Farbwort mit
Dicke **0**, also die 3D-Flaeche ohne eigenen Rahmen. Die erste Fassung gab
`ID_IBOX` den Wurzelwert und zeichnete einen schwarzen Rahmen, den EmuTOS
nicht hat; das ist korrigiert und nachgemessen.

**`ID_ID` traegt kein `EDITABLE`, und das ist die Quelle, keine Auslassung.**
`desk_rsc.c:936-940` gibt ihm `FL3DBAK | 0x4000` (das zweite ist der
desktop-eigene `RIGHT_ALIGNED`-Hinweis, `deskmain.c:106`, den
`align_objects` wieder abraeumt) — und `install_desktop_icon` uebergibt
trotzdem `start_fld = ID_ID` an `form_do` und liest das Feld mit `inf_sget`
zurueck. Es funktioniert, weil `fm_inifld` `start_fld` unveraendert
durchreicht (`aes/gemfmlib.c:153-160`) und `ob_edit` die Flagge nicht prueft:
der Schreibmarker **startet** im Kennungsfeld. Was nicht geht, ist
zurueckkommen — `fm_button` haengt den Klick an `EDITABLE`, und der
Pfeilwechsel besucht nur wirklich editierbare Felder. Woertlich nachgebaut,
Flagge und Startfeld.

EmuTOS' `ID_DOWN`-Verschiebung um `ADJ3DSTD` vor und nach dem Dialog
(`deskins.c:826-831`, `:966-971`) hat kein Gegenstueck: sie gleicht eine
Ressource aus, die fuer flache Knoepfe gesetzt wurde. Dieser Baum wird bei
jedem Aufruf frisch gebaut, mit dem Wuchs schon eingerechnet.

## Benannte Verengungen

1. **Die Liste haelt nur Desktop-Symbole.** EmuTOS fuehrt **eine** `ANODE`-
   Kette fuer die Desktop-Symbole *und* die Dateityp-/Anwendungszuordnungen.
   NewTOS hat die beiden getrennt, als Install-Application gebaut wurde: die
   Zuordnungshaelfte liegt in `desk::inf`s Sitzungsmodell. Beide Haelften
   landen in derselben Datei, in derselben Zeilengrammatik.
2. **Keine Fenstersymbole und keine Desktop-Verknuepfungen.**
   `CONF_WITH_WINDOW_ICONS` (`install_window_icon`, `ADINSWIN`) und
   `CONF_WITH_DESKTOP_SHORTCUTS` (`ins_shortcut`) haengen einen `ANODE` an
   einen **Dateinamen**, und NewTOS hat dafuer keinen Leser: ein Fensterobjekt
   nimmt sein Symbol aus der Endung (`ftype::classify` in `win_bldview`). Ein
   `ADINSWIN` waere ein Bedienelement ohne Wirkung. `ins.rs` hat deshalb die
   Form eines `CONF_WITH_WINDOW_ICONS = 0`-Baus: `ins_icon(0)` geht direkt auf
   `install_desktop_icon(NULL)` (`deskins.c:1262-1263`, der `#else`-Zweig),
   statt `STICNTYP` zu fragen. **Gemeldet, nicht gefaelscht.**
3. **`app_revit` hat kein Gegenstueck.** EmuTOS dreht die Kette vor
   `app_blddesk` und vor `app_save` um (`:1352`, `:1211`), weil `app_alloc`
   **vorne** anhaengt (`:215-216`). Ein `Vec` mit `push` steht schon in
   Entstehungsreihenfolge; beide Laeufe sind Vorwaertsschleifen, gleiche
   Reihenfolge ohne die Listenoperation.
4. **Die Beschriftungen bleiben NewTOS'.** EmuTOS schreibt `DISK C` / `TRASH`
   / `PRINTER` (`desk_rsc.c:2481-2483`), NewTOS' Desktop druckt seit seiner
   Entstehung `Hard disk` / `Floppy disk` / `Trash` / `Printer`, und jeder
   Screendump, gegen den die Desktoparbeit gemessen wird, traegt diese vier.
   Die Abweichung ist **aelter als dieses Ticket**; sie steckt jetzt in vier
   Konstanten an einer Stelle, und `install_drive` liest dieselben wie
   `build_inf`, damit ein per *Install devices* angelegtes Symbol genauso
   beschriftet ist wie eines vom Start. EmuTOS' Wortlaut zu uebernehmen ist
   eine Vierzeilen-Aenderung und eine eigene Entscheidung.

## Pruefung

**Hosttest: keiner moeglich, und das ist ein Befund.** Die gesamte Aenderung
liegt in `kernel/`, das `forced-target = "aarch64-unknown-none-softfloat"`
traegt; `cargo run -p xtask -- test` faehrt nur `tos-api`, `tos-types`,
`xtask` (`tools/xtask/src/main.rs`, `HOST_TEST_PACKAGES`). Derselbe Befund
steht in den Quittungen von DESK09 und DESK10. Der Nachweis ist deshalb in
QEMU und im kopflosen `selftest()` gefuehrt.

**`desk::ins::selftest()` (neu, unverdrahtet wie die uebrigen `selftest_*`):
14 Faelle, 14 bestanden.** Er deckt ab, was QEMU nicht stellen kann — ein
Laufwerk, das aus der Karte verschwindet — und die Invarianten, die ein Bild
nicht zeigt: ein Symbol je Kartenbit, kein Rasterplatz doppelt belegt,
`IG_HARD` gegen `IG_FLOPPY` nach `drive > 1`, Papierkorb und Drucker von
`ins_devices` unberuehrt, die angebotene Bilderliste ohne Loch und die
Diskette genau dann darin, wenn ein Diskettenlaufwerk existiert. Er stellt die
Liste, die er vorfand, vor der Rueckkehr wieder her.

**QEMU raspi4b** (640x480, `-display none`, QMP-Screendump, frische
Bildkopie je Lauf, Scratch-Kopie mit dem DESK08/09-IKBD-Injektor; **sieben**
Laeufe, alle **0** `[EXC]`/`[PANIC]`):

1. **Start** — `boot-desktop.png`: `Hard disk` (C) oben links, `Trash` unten
   links, `Printer` unten rechts.
2. **Menuezustand** — `men-C-sel.png` (ein Symbol ausgewaehlt): *Remove
   desktop icon* **schwarz**; `men-E-nosel.png` (nichts ausgewaehlt):
   **grau**. *Install icon* und *Install devices* in beiden Faellen schwarz.
3. **Entfernen + Geraete einrichten** — `rmv-B-selected.png` (Symbol
   invertiert), `rmv-C-removed.png` (weg, Papierkorb/Drucker unberuehrt,
   rueckstandsfrei), `rmv-D-reinstalled.png` gegen `rmv-A-idle.png`:
   **pixdiff 0**.
4. **Install icon** — `ico-B-dialog.png`: der Kasten misst 328x316 bei
   (156, 91) = `form_center`s Wert (`(640-328)/2 = 156`,
   `19 + (480-19-316)/2 = 91`), `Drive` vorgewaehlt, Festplattenbild.
   `ico-C-step1` / `ico-D-step2`: das Schrittwerk laeuft Festplatte → Ordner →
   Papierkorb, **ohne** Diskette. `ico-E-trashtype`: `Trash can` gewaehlt,
   `Drive` geloescht (die Radiogruppe unter ihrem `G_IBOX` arbeitet).
   `ico-F-installed`: ein neues Papierkorb-Symbol an der ersten freien
   Rasterstelle (1,0), ohne Beschriftung (es wurde keine getippt).
5. **Die geschriebene Datei** (`inf/NEWDESK.INF.after-install`):

   ```
   #M 00 00 00 FF C Hard disk@ @
   #T 00 09 03 FF   Trash@ @
   #O 06 09 04 FF   Printer@ @
   #T 01 00 03 FF   @ @
   ```

   `#E 1A E0 00 00 60` und `#Q 41 40 43 40 43 40` unveraendert.
6. **Kaltstart** von diesem Abbild, Produktkernel (`cold-desktop.png`): alle
   vier Symbole stehen wieder da, jedes an seiner Rasterstelle.
7. **Entfernen ueberlebt** — `rms-*` schreibt eine `NEWDESK.INF` ganz ohne
   `#M`-Zeile (`inf/NEWDESK.INF.after-remove`), und der Kaltstart daraus
   (`cold2-desktop.png`) zeigt Papierkorb und Drucker **ohne** `C:`.

`fsck_msdos -n` auf der FAT16-Partition der drei geschriebenen Abbilder:
Phase 1/2/3 ohne Beanstandung.

**Bau:** frisch je Board (`cargo clean -p kernel -p tos-api` davor);
`cargo xtask build --board rpi4` und `--board qemu-virt`: **tos-api 64 /
kernel 94 auf beiden** (Basis 64/94, Delta 0). Clippy: tos-api **80**, kernel
**125** auf beiden (Basis 80/125, Delta 0), **null** Treffer in
`kernel/src/desk/`. `cargo run -p xtask -- test`: **932 bestanden /
0 fehlgeschlagen / 1 ausgelassen** (Basis 932/0/1).

## Artefakte

* `kernel/src/desk/ins.rs` (neu) und
  `kernel/src/desk/{act,app,dir,global,icons,inf,main,menu,mod,win}.rs` —
  uncommitted im Baum.
* `docs/api/desktop/deskins.md` — der bis dahin leere Abschnitt „NewTOS
  implementation notes" ist gefuellt: die Zustandsumstellung, die
  `Anode`-Feldtafel, die Signaturbedingung, die `IG_*`-Tafel mit dem Loch,
  der `ADINSDSK`-Objektbaum, die `ID_ID`-Eigenart, `ins_devices`, die
  Menuezustaende.
* `docs/api/desktop/_hw-test-2026-09-17.md` — neuer Punkt **38** (zwoelf
  Schritte).
* `docs/STATUS.md` — Phase-14-Zeile + eine Zeile in *Recent Decisions*
  (2026-09-24).
* `measurement-2026-09-24-desk03-desk04/` — `RUNINFO.TXT`, `method/`
  (vier Aktionslisten + Injektortabellen + der vollstaendige Diff),
  `screendumps/`, `serial/`, `inf/`, `build/`, `manifest.sha256`.
  Kein `*.img`.

## Offen

* **Pi-4-Sichtabnahme** — HW-Checkliste Punkt **38**. Vier Punkte kann QEMU
  grundsaetzlich nicht fahren: die **vier** Laufwerke der Karte, jede
  **Tastatureingabe** (Laufwerkskennung, Beschriftung — der Injektor kann
  Tasten, aber ein Tippfeld gehoert an eine echte Tastatur), das **Ziehen**
  eines Symbols und die 1920x1080-Optik des Kastens.
* **Lead-Entscheidung:** die geaenderte Vorgabeanordnung (Papierkorb und
  Drucker unten statt oben rechts) — belegt, aber sichtbar.
* **Lead-Entscheidung:** EmuTOS' Beschriftungen (`DISK C` / `TRASH` /
  `PRINTER`) statt der heutigen.
* **Gemeldete Luecke:** Fenstersymbole (`ADINSWIN`) und
  Desktop-Verknuepfungen — brauchen erst einen Leser im `win_bldview`.

---

## Nachtrag des Leads (gleiche Arbeitseinheit)

Die Symbolbeschriftungen sind nach diesem Bericht auf EmuTOS' Wortlaut
umgestellt worden: `desktop_str_addr(STDISK)` + Laufwerksbuchstabe fuer beide
Laufwerksarten, `TRASH`, `PRINTER` (`desk_rsc.c:2481-2483`, gebaut in
`build_inf` `deskapp.c:723` und `install_drive` `deskins.c:247`). Die oben
protokollierten Laeufe und Screendumps zeigen daher noch `Hard disk` /
`Floppy disk` / `Trash` / `Printer`; die `#M`-Zeile lautet jetzt
`#M 00 00 00 FF C DISK C@ @`. Alles uebrige des Berichts ist unberuehrt.
