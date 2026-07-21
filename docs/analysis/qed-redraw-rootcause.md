# QED-Redraw — Root-Cause-Analyse

Read-only-Untersuchung. Warum QED (eine echte EL0-GEM-App über den svc/AES-Trap)
auf echter HW massive Redraw-Fehler zeigt (Menü-Artefakte, „Hintergrund/Screen
kommt nicht sauber"), während der NewTOS-**Desktop** sauber neu zeichnet.

Belege: `references/emutos/aes/` (Ground Truth), `tos-api/src/aes/`,
`kernel/src/desk/`, der Real-HW-Trace `/Volumes/BOOT/QEDTRACE.LOG` und die
Real-HW-RSC `/Volumes/BOOT/qed.rsc` (aus dem QED-Menübaum geparst).

---

## Verdikt zur zentralen Hypothese

**Die zentrale Hypothese ist WIDERLEGT.** Der App-seitige Redraw-Round-Trip
(WM_REDRAW-Zustellung + Rechteckliste WF_FIRSTXYWH/WF_NEXTXYWH + wind_update) ist
**FULL** für ein einzelnes, unverdecktes Fenster und wird von QED erfolgreich
benutzt. Beweis aus `QEDTRACE.LOG`:

- Zeile 379: `evnt_multi -> 16` `msg=[20,1,0,2,836,509,248,80]` → **WM_REDRAW
  (msg-Typ 20)** wird an QED zugestellt.
- Zeile 1021–1023: `evnt_multi -> 18` `msg=[20,1,0,1,24,60,640,960]` → WM_REDRAW
  für Fenster 1 mit dem korrekten Work-Rechteck.
- Zeile 433–434 / 978–979: `wind_get handle=1 field=11 -> (24,60,640,960)`
  (WF_FIRSTXYWH = Work-Area), danach `field=12 -> (0,0,0,0)` (WF_NEXTXYWH =
  Listenende). Klassischer Rechtecklisten-Walk, **korrekt**.
- Zwischen 436 und 977: hunderte `vs_clip`/`vr_recfl`/`v_gtext` — QED zeichnet
  seinen Fensterinhalt in die gelieferten Rechtecke.

Die Redraw-Fehler liegen also **nicht** in Area 1. Sie liegen in **Area 2
(Menü-Dropdown Save/Restore)**: dieses Save/Restore ist die **einzige**
Bildschirm-Aufräumung unter einem QED-Dropdown, und es weicht von EmuTOS
`menu_sr` ab. Der Desktop verdeckt denselben Code, weil er nach jeder Auswahl ein
volles `do_wredraw`/`full_redraw` fährt — QED hat dieses Sicherheitsnetz nicht.

---

## FULL / PARTIAL / MISSING — App-seitige Redraw-Maschinerie

| Baustein | Status | EmuTOS | NewTOS | Beleg |
|---|---|---|---|---|
| WM_REDRAW-Erzeugung bei wind_open/set(CXYWH)/move/size | **FULL** | `w_update`→`w_redraw` `ap_sendmsg(WM_REDRAW)` | `send_redraw` | gemwmlib.c:887-907,1004-1048 / window.rs:571-591,1018,1231 |
| WM_REDRAW-Zustellung via evnt_multi (MU_MESAG) | **FULL** | `ev_multi`→`ev_mesag` | `msg_pending`→pbuff | gemevlib.c:222-229 / event.rs:331-349 |
| Rechteckliste WF_FIRSTXYWH/WF_NEXTXYWH | **FULL** | `w_owns` über `w_rlist` | `compute_rlist`/`rlist_next` | gemwmlib.c:1230-1241,1556-1562 / window.rs:526-568,1069-1080 |
| Okklusion (höhere Fenster abziehen) | **FULL** | `newrect`/`mkrect`/`brkrct` | `subtract` | gemwrect.c:101-205 / window.rs:488-552 |
| WM_REDRAW pro Rechteck + VDI-Clip setzen | **FULL** | `do_walk` `gsx_sclip`+`ob_draw` | `do_wredraw` (Desktop) / App macht vs_clip | gemwmlib.c:286-312 / redraw.rs:69-93 |
| wind_update(BEG/END_UPDATE) Lock + Maus-Hide | **PARTIAL** | Mutex `wind_spb`, `gsx_moff` bei Draws | **No-op** (`ret=1`, kein Lock, kein Hide) | gemwmlib.c:1778-1797 / window.rs:1299-1301 |
| WM_REDRAW an verdeckte Fenster bei wind_close | **MISSING** | `wm_close`→`draw_change`→`w_update`→`w_redraw` sendet WM_REDRAW an untenliegende Owner | `wind_close` malt nur bg+Frames (`redraw_clip`), **kein** `send_redraw` | gemwmlib.c:1438-1459,1004-1048 / window.rs:1027-1042 |
| Menü-Dropdown Save/Restore (`menu_sr`) | **PARTIAL** | `bb_save`/`bb_restore` der um `MENU_THICKNESS` **erweiterten** Box; Puffer schirmt ganzen Screen | rettet **nur** `abs_rect(box)`, keine Marge; 256×400-Cap → Weiß-Erase-Fallback | gemmnlib.c:208-222 / menu.rs:465-518,563-572 |
| Menü-Save-Puffer für Bildschirmgröße | **MISSING** | `gl_pmpmlo`/`gl_mlen` (voller Screen) | Fester `256×400`-Puffer, still verworfen bei Überlauf | gemmnlib.c:208-222 / menu.rs:429-482 |
| DISABLED-Guard beim Item-Highlight | **PARTIAL** | `do_chg(chkdisabled=TRUE)` verweigert Highlight disabled Items | `rehilite`/`child_at` setzen SELECTED ohne Guard | gemmnlib.c:145-172,192-200 / menu.rs:280-303,207-227 |

---

## Ranked Problem-Liste

### P1 — Menü-Save/Restore rettet nur `abs_rect(box)`, deckt aber nicht die tatsächlich gezeichnete Fläche (Outside-Border + überlappende, unterschiedlich große Boxen). [CONFIRMED, Bug]

Dominante Ursache für Artefakt #1 (Fremdzeichen-Spalte rechts) und #2
(überdimensionierte Box + versetztes zweites Rechteck).

**(1) EmuTOS.** `menu_sr` (gemmnlib.c:208-222) rettet/restauriert nicht die nackte
Box, sondern eine um `MENU_THICKNESS` **auf jeder Seite erweiterte** Fläche:
```c
ob_actxywh(tree, imenu, &t);
t.g_x -= MENU_THICKNESS; t.g_w += 2*MENU_THICKNESS; t.g_h += 2*MENU_THICKNESS;  // :215-217
if (saveit) bb_save(&t); else bb_restore(&t);
```
`MENU_THICKNESS = 1` (gemmnlib.h:14). Diese +1px-Marge existiert **genau**, um den
1px-Außenrand der Dropdown-Box (Boxen mit negativer Randstärke) und Overscan
abzudecken. `bb_save`/`bb_restore` sind **unbegrenzt** (Puffer `gl_pmpmlo` für den
gesamten Screen dimensioniert). Der Submenü-Pfad rettet sogar `MENU_THICKNESS +
ADJ3DSTD` (gemmnext.c:146-149). `menu_down` rettet **vor** dem Zeichnen
(gemmnlib.c:239-241); der Titelwechsel in `mn_do` restauriert die alte Box, bevor
die neue gezogen wird (gemmnlib.c:663-671 bzw. 462-468).

**(2) NewTOS.** `open_box` rettet und `close_box`/`erase_box_inner` restaurieren
**exakt `abs_rect(imenu)`** — ohne jede Marge:
```rust
let (x, y, w, h) = object::abs_rect(tree, imenu);   // menu.rs:513
save_under(&MBACK, backend, x, y, w, h);            // menu.rs:514
object::draw_obj(tree, imenu, MAX_DEPTH, backend);  // menu.rs:515 — voller Screen-Clip
```
`draw_obj` clippt **nicht** auf die Box (voller Screen-Clip, object.rs:1378), und
`just_draw` zeichnet einen negativen Rand **außerhalb** von `(x,y,w,h)`
(`draw_signed_border`, object.rs:1721). Save/Restore kennen diese Außenpixel nicht.

**(3) Divergenz.** NewTOS lässt bei jedem Menü-Schließen/-Wechsel einen 1px-Rahmen
ungesichert stehen (Marge fehlt), und `draw_obj` ist nicht auf die Box begrenzt.
Bestätigt aus `qed.rsc` (32-Byte-`NRSC`-Objekte): QEDs Dropdown-Boxen haben
`ob_spec` Randstärke-Byte `0xFF = -1` (**Außenrand**), z. B. rec 16 (Datei)
`th=-1`, xywh `(16,0,168,128)`; rec 25 `(56,0,192,256)`; rec 42 `(112,0,184,336)`.
Die Boxen **überlappen in X** und haben **stark verschiedene Höhen** (128/256/336),
weil ihre X-Position unter ihrem Titel liegt (THEMENUS-Kinder). Wechselt man von
einer höheren/breiteren Box (z. B. rec 42, 184×336) zur Datei-Box (168×128), so
ragt der Teil der Vorgänger-Box, der Datei nicht überdeckt, hervor — mitsamt
seinem 1px-Außenrahmen und (rechts) seiner Kürzel-Spalte, die nicht restauriert
wird. Genau das Foto: „überdimensionierte Box + versetztes zweites Rechteck +
Fremdzeichen-Spalte rechts, hinter einer Vertikallinie".

**(4) Desktop-vs-QED-Delta.** Der Desktop baut seine Menü-Boxen selbst
(`desk/menu.rs build_menu`): `ob_state = 0` (kein OUTLINED/SHADOWED), `mk()` setzt
`ob_state:0`, Box-Höhe `= n_items*MENU_H` (exakt), keine Überlappung, positiver
Randinhalt — die gezeichnete Fläche liegt **vollständig in `abs_rect`**, also
restauriert das identische Save/Restore sauber (desk/menu.rs:254-266,437-473). QEDs
autorisierte RSC-Boxen haben `th=-1`-Außenränder und überlappen — die gezeichnete
Fläche liegt **außerhalb** von `abs_rect`. Zusätzlich: Nach einer Auswahl fährt der
Desktop `hndl_menu`→`full_redraw()`/`do_wredraw` (desk/main.rs:89-101,751; redraw.rs:247-256)
und übermalt jeden Save/Restore-Rest; QED bekommt nach Menü-Schließen **kein**
WM_REDRAW (klassisches AES verlässt sich zu 100 % auf `menu_sr`), also bleibt der
Rest stehen. Selbst der Desktop-**Cancel**-Pfad (kein `full_redraw`) bleibt sauber,
weil die Desktop-Boxen dekorationsfrei in `abs_rect` liegen — das beweist, dass das
Restore für undekorierte Boxen korrekt ist und die QED-Artefakte geometriebedingt
sind.

**(5) Fix-Richtung.** `open_box`/`close_box`/`close_submenu`/`popup_track` müssen
die Save/Restore-Fläche wie EmuTOS `menu_sr` um `MENU_THICKNESS` (bei 3D +
`ADJ3DSTD`) je Seite erweitern **und** die tatsächliche gezeichnete Box-Fläche
(inkl. negativer Ränder/OUTLINED/SHADOWED) abdecken — alternativ `draw_obj` beim
Menü-Zeichnen auf genau diese Save-Fläche clippen, damit gezeichnet == gerettet.
Der Titelwechsel muss die **volle** Fläche der alten Box restaurieren, bevor die
neue gezogen wird (Reihenfolge ist bereits korrekt; nur die Fläche ist zu klein).

---

### P2 — Fester 256×400-Save-Puffer mit stillem Weiß-Erase-Fallback. [CONFIRMED-Divergenz, latent für QED]

**(1) EmuTOS.** `bb_save`/`bb_restore` sind unbegrenzt; der Menü-/Alert-Puffer ist
für die größte mögliche (bildschirmgroße) Fläche dimensioniert (gemmnlib.c:208-222).

**(2) NewTOS.** `MB_W=256, MB_H=400` (menu.rs:429-430). `save_under` verwirft still
(`valid=0`), wenn `w>256 || h>400` (menu.rs:466-469); `erase_box_inner`/
`close_submenu` machen dann **Weiß-Erase** von `abs_rect` (menu.rs:567-571,393-397)
statt echter Restaurierung.

**(3) Divergenz.** Ist eine Box breiter als 256 oder höher als 400 px (bei 1080p
gut möglich, z. B. ein langes Menü), wird der darunterliegende Inhalt (QED-Text
oder Hintergrund) **weiß gelöscht statt restauriert**. Passt zu „Hintergrund kommt
nicht sauber".

**(4) Delta.** QEDs konkrete Boxen (max 336 px hoch, 192 breit) liegen **noch**
unter dem Cap — daher für die aktuellen Datei-Artefakte **nicht** die Hauptursache,
aber eine scharfe latente Bombe für längere Menüs/andere Apps; der Desktop maskiert
es via `full_redraw`.

**(5) Fix.** Cap entfernen bzw. Puffer auf Screen- oder tatsächliche
Box-Draw-Größe dimensionieren; keinen Weiß-Erase-Fallback, der Inhalt zerstört.

---

### P3 — Kein DISABLED-Guard beim Item-Highlight → schwarzer Rest-Block. [PLAUSIBLE, Bug]

Passt zu Artefakt #3 („kleiner schwarzer Block nach ‚Letzte Fassung'").

**(1) EmuTOS.** `menu_select`→`do_chg(..., chkdisabled=TRUE)` verweigert das
Setzen von SELECTED auf einem DISABLED-Item (gemmnlib.c:159-160,192-200); ein
Separator/gesperrtes Item wird nie invertiert.

**(2) NewTOS.** `rehilite` setzt SELECTED **ohne** Disabled-Prüfung
(`object::set_state_bit(tree, new, SELECTED, true)`, menu.rs:292-293); `child_at`
liefert auch DISABLED-Kinder (menu.rs:207,1340-1346). Ein SELECTED-G_STRING wird
schwarz gefüllt (object.rs:1786).

**(3) Divergenz.** Fährt der Cursor über einen gesperrten Separator (QEDs Datei-Box
hat DIS-Items, z. B. rec 18), wird dieser schwarz invertiert. Wird beim Verlassen/
bei Auswahl das alte Item nicht exakt (dieselbe Fläche) geweißt — oder liegt der
Separator im nicht-restaurierten Außenband (P1) — bleibt ein schwarzer Block.

**(4) Delta.** Der Desktop übermalt es via `full_redraw`; QED nicht.

**(5) Fix.** DISABLED-Guard in `rehilite`/Item-Auswahl übernehmen (EmuTOS
`chkdisabled`); Alt-Item-Weißung an die exakte gezeichnete Fläche binden (folgt aus
P1). Reproduktion des exakten Blocks braucht die HW-Foto-/Live-Pixel-Bestätigung.

---

### P4 — wind_update ist ein No-op (kein Lock, kein Maus-Hide). [CONFIRMED-Divergenz, im Single-Tasking tolerierbar]

**(1) EmuTOS.** `wm_update` nimmt bei BEG_UPDATE das Mutex `wind_spb`
(`tak_flag`/`ev_block(MU_MUTEX)`), gibt es bei END_UPDATE frei; BEG/END_MCTRL →
`fm_own` (Maus-/Ctrl-Ownership) (gemwmlib.c:1778-1797). Draws laufen mit
`gsx_moff()` (Maus aus).

**(2) NewTOS.** `wind_update` setzt nur `ret=1` (window.rs:1299-1301). Der Trace
zeigt QED ruft es hunderte Male (op=107) — es tut nichts.

**(3) Divergenz.** Kein Screen-Lock, keine Maus-Ausblendung über den App-Draw. Im
aktuellen Single-Tasking (ein AES-Prozess, kein konkurrierender Zeichner) über die
**dokumentierte API nicht beobachtbar** — Menü/Fenster-Tracking blendet die Maus
lokal selbst aus (`cursor_hide`/`cursor_show`). Grenzt an einen Stub, ist aber ein
definierter, den beobachtbaren Vertrag wahrender No-op, solange es keinen zweiten
Zeichner gibt.

**(4) Delta.** Desktop wie QED nutzen dasselbe No-op; keine Artefakt-Differenz.

**(5) Fix.** Erst relevant mit echtem Multitasking/konkurrierenden Draws; dann
echtes Lock + `gsx_moff`-Äquivalent nachrüsten. Aktuell **acceptable deviation**.

---

### P5 — wind_close sendet verdeckten unteren Fenstern kein WM_REDRAW. [CONFIRMED-Divergenz, im Trace nicht getriggert]

**(1) EmuTOS.** `wm_close`→`draw_change`→`w_update` läuft die Fenster von oben nach
unten und ruft `w_redraw` (WM_REDRAW an den Owner jedes freigelegten Fensters)
(gemwmlib.c:1438-1459,1004-1048).

**(2) NewTOS.** `wind_close` setzt `open=false` und ruft `redraw_clip(rect)` — malt
Desktop-Hintergrund + Fenster-**Frames** neu, aber **kein** `send_redraw` an
untenliegende Owner (window.rs:1027-1042). Der Kommentar delegiert an
„desktop owner's damage_redraw" — das gilt nur für den Desktop-Owner.

**(3) Divergenz.** Öffnet eine App (z. B. QED) ein eigenes Fenster/Dialog **über**
ihrem Hauptfenster und schließt es, bekommt das Hauptfenster **kein** WM_REDRAW für
die freigelegte Fläche → Inhalt bleibt zerstört, bis zufällig ein anderer
WM_REDRAW kommt.

**(4) Delta.** Der Desktop repariert das über sein eigenes `damage_redraw`/
`full_redraw` nach Fenster-Ops; eine Fremd-App nicht.

**(5) Fix.** In `wind_close` (analog `wind_open`/`WF_CXYWH`) `send_redraw` an die
freigelegten untenliegenden Fenster fahren (EmuTOS `w_update`-Fan-out). Im
`QEDTRACE.LOG` **nicht** ausgelöst (QEDs Fortschritts-Dialog, Handle 2, liegt über
dem Desktop, und das Hauptfenster Handle 1 öffnet erst danach) → hier latent.

---

## Top-3 Root-Causes (nach Impact)

1. **P1 — Menü-`menu_sr`-Fläche zu klein (nur `abs_rect`, keine `MENU_THICKNESS`-
   Marge, `draw_obj` nicht box-geclippt) bei QEDs `th=-1`-Außenrand- und
   überlappenden, ungleich hohen Boxen — ohne App-Repaint-Sicherheitsnetz, das nur
   der Desktop hat.** Erklärt Artefakt #1 + #2. CONFIRMED.
2. **P3 — Fehlender DISABLED-Guard beim Item-Highlight** → schwarzer Rest-Block
   (Artefakt #3). PLAUSIBLE.
3. **P2 — 256×400-Save-Cap mit Inhalt-zerstörendem Weiß-Erase** → „Hintergrund
   nicht sauber" / latente Bombe für große Menüs. CONFIRMED-Divergenz, für QEDs
   aktuelle Boxgrößen unterhalb der Schwelle.

---

## Vorgeschlagene serielle Fix-Reihenfolge

- **Gruppe 1 (Area 2, dominant — P1+P2 zusammen).** Menü-Save/Restore auf die
  **tatsächlich gezeichnete Fläche** bringen: Save/Restore-Rechteck wie EmuTOS
  `menu_sr` um `MENU_THICKNESS` (bei 3D `+ADJ3DSTD`) je Seite erweitern **und** die
  Box-Dekoration (negativer Rand / OUTLINED / SHADOWED) einschließen — oder
  `draw_obj` beim Menü-Zeichnen exakt auf die Save-Fläche clippen (gezeichnet ==
  gerettet). Den 256×400-Cap entfernen (Puffer = Screen- bzw. echte
  Box-Draw-Größe). Titelwechsel: volle Alt-Box-Fläche restaurieren, dann neue
  zeichnen. Betrifft `menu.rs` `open_box`/`close_box`/`erase_box_inner`/
  `open_submenu`/`close_submenu`/`popup_track`/`save_under`/`restore_under`.
- **Gruppe 2 (P3).** DISABLED-Guard aus EmuTOS `do_chg(chkdisabled)` in
  `rehilite`/Item-Auswahl nachziehen; Alt-Item-Weißung an die exakte gezeichnete
  Item-Fläche binden.
- **Gruppe 3 (Area 1 Härtung — P5, niedriger, latent).** `wind_close` (und
  allgemein jede Fläche freigebende Fenster-Op) muss `send_redraw` an die
  freigelegten untenliegenden Fenster fahren (EmuTOS `w_update`→`w_redraw`), damit
  eine App, die eigene Fenster stapelt, Inhalts-Redraws bekommt.
- **Gruppe 4 (P4, erst mit Multitasking).** `wind_update` zum echten Lock +
  Maus-Hide ausbauen, sobald konkurrierende Zeichner existieren.
- **Gruppe 5 (Verifikation).** Real-HW-Foto-Retest nach Gruppe 1; erwartet: keine
  Fremdzeichen-Spalte, kein versetztes zweites Rechteck, kein schwarzer Block.

---

## Source-proven vs. Real-HW-nötig

**Source-/Trace-/RSC-bewiesen:**
- App-seitiger WM_REDRAW + Rechtecklisten-Pfad ist FULL und wird von QED benutzt
  (`QEDTRACE.LOG`: msg-Typ 20; WF_FIRSTXYWH=11→Work-Area, WF_NEXTXYWH=12→(0,0,0,0)).
- Desktop maskiert Save/Restore-Reste via `full_redraw`/`do_wredraw`; QED nicht
  (desk/main.rs, redraw.rs).
- Menü-Save/Restore rettet nur `abs_rect`, ohne `MENU_THICKNESS`-Marge, `draw_obj`
  nicht box-geclippt (menu.rs, object.rs).
- QEDs Dropdown-Boxen: Außenrand `th=-1`, überlappende X-Positionen, ungleiche
  Höhen (128/256/336) — geparst aus `qed.rsc` (`NRSC`-Format).
- 256×400-Cap mit Weiß-Erase-Fallback (menu.rs).
- Desktop-Boxen dekorationsfrei in `abs_rect`, nicht überlappend (desk/menu.rs).

**Braucht das Real-HW-Foto / einen Live-Pixel-Diff zur endgültigen Zuordnung:**
- Welcher der P1-Teilmechanismen (fehlende Marge vs. nicht-restaurierte
  Vorgänger-Box beim konkreten Menüwechsel) den sichtbaren Fremdzeichen-Anteil von
  Artefakt #1 dominiert.
- Der exakte Entstehungspfad des schwarzen Blocks (Artefakt #3, P3): welches
  DIS-/SELECTED-Item, und ob es im nicht-restaurierten Außenband liegt.
- Bestätigung, dass P2/P5 bei QED aktuell nicht auslösen (Boxgrößen < Cap; Dialog
  über Desktop) — durch Trace bereits gestützt, endgültig per HW.

Interaktives QED-Redraw-Verhalten ist real-Pi4-only (Projekt-Memory
`desktop-interactive-verification-real-hw`); der statische Save/Restore-Defekt ist
oben aus Quelle + RSC + Trace bewiesen.
