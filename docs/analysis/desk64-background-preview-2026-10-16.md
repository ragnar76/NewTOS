# DESK64 — Vorschau im Dialog "Set background" (2026-10-16)

Scope: Zeigt EmuTOS im Hintergrund-Dialog eine Vorschau, und wie verhaelt sich NewTOS (`programs/desktop/`)?

## Verdikt

**Abweichung: ja.** EmuTOS hat ein Vorschau-Objekt (`BGSAMPLE`) und zeichnet es nach jeder Auswahl neu. NewTOS baut stattdessen einen eigenen, nicht aus `ADBKGND` abgeleiteten Dialog mit `-`/`+`-Schrittschaltern und Zahlenfeldern. Dieser Dialog hat weder Vorschau noch Farb-/Muster-Felder noch Desktop/Window-Umschalter. Die Ursache liegt im Desktop (fehlendes Objekt und fehlender Code), nicht in AES/VDI. Der Dialog wird durch einen Ersatzbau ersetzt, dessen Kommentar die Abweichung selbst nennt.

## EmuTOS (gelesen)

- Schalter: `CONF_WITH_BACKGROUNDS` — `references/emutos/include/config.h:1747-1749` Default **1**. Der Block `config.h:328-329` setzt 0 nur in einer Maschinen-Sonderkonfiguration davor (`#ifndef`-Kaskade, nicht der Falcon-Zweig). Die generierte Tabelle `docs/api/_emutos-config.md:173` zeigt `1 | 0`. Das heisst "fuer alle Maschinen mitkompiliert", **nicht** "im Falcon-Profil an". Ob der Falcon-/512-Build den Schalter tatsaechlich auf 1 laesst, ist hier nicht einzeln belegt (siehe offene Fragen). Die Gruppe `CONF_WITH_DESKTOP_CONFIG`/`SHORTCUTS` zeigt dieselbe Tabellenform. Menuepunkt: `deskmain.c:226-227,824-827` (`BACKGRND` -> `inf_backgrounds()`).
- Ressource `ADBKGND` (Baum 19): `desk/desk_rsc.c:2230-2452`, Indizes `desk_rsc.h:251-260`: `BGDESK`=4, `BGWIN`=5 (Radio, `desk_rsc.c:2256,2262`), `BGPAT0..7`=9..16 (`TOUCHEXIT` G_BOX, `:2288ff`), **`BGSAMPLE`=17 (G_BOX, `ob_spec` 16716033, Position 17,8 Groesse 4x2, `desk_rsc.c:2334-2339`)**, `BGCOL0..15`=19..34 (`TOUCHEXIT` G_BOX, `:2345ff`), `BGOK`=35, `BGCANCEL`=36.
- Logik: `desk/deskinf.c:825-921` (`inf_backgrounds`). Das Beispiel bekommt beim Start `DROOT`- bzw. `DROOT+1`-`ob_spec` plus `SAMPLE_BORDER_FLAGS` (`deskinf.c:53-57,855-858`). Pro Schleifendurchlauf: `draw_fld(tree, BGSAMPLE)` (`:869`), dann `form_do`. Bei `BGDESK`/`BGWIN` wird das Muster auf den jeweiligen Stand gesetzt. Bei `BGPAT0..7` / `BGCOL0..15` werden `FILLPAT_MASK` bzw. `FILLCOL_MASK` (`include/obdefs.h:126-127`) des Samples aus dem angeklickten Objekt uebernommen (`:880-891`). Bei OK gilt: `set_aes_background`, `g_patcol[index]`, `do_wredraw(DESKWH,...)` (Desktop, `:900-908`) bzw. Fenster-`ob_spec` fuer alle Fensterobjekte, Rueckgabe TRUE (`:910-916`).
- `draw_fld` = `objc_draw(tree, obj, MAX_DEPTH, <objc_offset-Rechteck>)`, `desk/deskdir.c:131-138`. Die Vorschau ist also **reiner AES-`objc_draw`** eines `G_BOX` mit Fuellmuster/-farbe. Eine eigene VDI-Logik steckt nicht darin.

## NewTOS (gelesen)

- `programs/desktop/src/inf.rs:3699-3808` (`set_background`): handgebauter Baum aus 12 Objekten (`:3745-3757`): Titel, `Pattern:`/`Colour:`-Strings mit Zahl, vier `-`/`+`-Buttons, OK, Cancel. Kein `G_BOX` mit Fuellmuster, keine Swatches, kein Desktop/Window-Umschalter.
- Der Kommentar `inf.rs:3727-3733` bestaetigt: "`ADBKGND` has no stepper pair — it offers eight pattern and sixteen colour swatches" — die Abweichung ist also bekannt und benannt, nicht aus Versehen.
- Redraw: `inf.rs:3761-3766,3790` — `draw_dial(tree_ptr)` zeichnet den ganzen Dialog neu. Dass ein `draw_fld(tree, BGSAMPLE)` fehlt, steht ebenfalls im Kommentar.
- Nur Desktop-Hintergrund; das `#Q`-Fenster-Bucket wird mitgefuehrt, hat aber keinen Verbraucher (`main.rs:2509-2520`). Der Fenster-Hintergrund ist also nicht einstellbar.
- Zusatz-Abweichung: NewTOS speichert sofort `cnx_put(); app_save(true)` (`inf.rs:3800-3807`), EmuTOS erst beim naechsten "Save desktop" (selbst im Kommentar benannt).

## Ursache und Schicht

Fehlender Code und fehlendes Objekt im Desktop (`programs/desktop/`): Der Ressourcenbaum `ADBKGND` wird nicht nachgebaut. AES/VDI-Rendering ist nicht verdaechtig. `objc_draw` auf `G_BOX` mit Fuellmuster/-farbe benutzt jedes Desktop-Fenster und jede Icon-Box ohnehin. Ausdruecklich **nicht** getestet, siehe offene Fragen.

## Fix-Vorschlag (nach EmuTOS)

1. `ADBKGND` objektweise 1:1 aus `desk_rsc.c:2230-2452` als Baum nachbauen (37 Objekte, Rollen/Flags wie dort, 3D-Flags `FL3DBAK`/`FL3DIND`/`FL3DACT` wie dort).
2. `inf_backgrounds` nach `deskinf.c:825-921` uebernehmen: Spalten `unused`/`index` je `gl_nplanes` (`:832-847`), `HIDETREE` auf nicht verfuegbare Farben (`:850-852`), Initialisierung `BGSAMPLE` (`:855-858`), Schleife mit `draw_fld(BGSAMPLE)`+`form_do` (`:866-896`), Beendigung (`:898-918`). Ein `draw_fld`-Helfer (`deskdir.c:131-138`) ist noetig, falls in `programs/desktop/` noch nicht vorhanden.
3. Fenster-Hintergrund anschliessen: die `#Q`-Window-Spalte hat bisher keinen Verbraucher (`main.rs:2509-2520`); die EmuTOS-Rueckgabe `BACKGROUND_HAS_CHANGED` greift dann wieder (`deskmain.c:824-827`).
4. Das sofortige `app_save` entfernen oder bewusst als Erweiterung behalten (Entscheidung beim Lead; EmuTOS speichert nicht).

## Offene Fragen

- **Kein Bildbeleg erzeugt.** Es wurden weder `rscview` noch Hatari-Falcon noch ein QEMU-Dump ausgefuehrt; das Verdikt stuetzt sich vollstaendig auf den Quelltext (beide Seiten). Wer ein Bild braucht: `rscview` auf `ADBKGND` (Baum 19) und Hatari-Falcon-TC-Lauf, dann gegen NewTOS-Dump legen.
- Falcon-/make-512-Profil: ist `CONF_WITH_BACKGROUNDS` dort effektiv 1? Vermutlich ja (Default `config.h:1748`), aber der Maschinen-Block `config.h:320-330` wurde nicht auf Falcon-Zuordnung geprueft. Das ist hier **gelesen als Default, nicht als Falcon-Ergebnis**.
- Ob das Ergebnis (Vorschau-Box) in Hatari tatsaechlich sichtbar ist, entscheidet nicht der Quelltext allein, folgt aber direkt aus `objc_draw` von `G_BOX`; als Schluss (nicht gelesen) markiert.

## Out-of-scope

- Die anderen handgebauten Dialoge (`dlg.rs:94ff` u.a.) ersetzen ebenfalls `desk_rsc.c`-Baeume; nicht untersucht.
