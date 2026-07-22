# AES 1:1-Divergenz-Audit — PAKET P3 (Object Manager)

**Scope (ein Satz):** Abgleich von `tos-api/src/aes/object.rs` (2333 Z.) Funktion-für-Funktion gegen EmuTOS `aes/gemoblib.c`, `aes/gemobjop.c`, `aes/gemobed.c` und die Draw-Helper in `aes/gemgraf.c` (+ `include/obdefs.h`, `include/aesdefs.h`, `aes/gemsuper.c`), inkl. aller `ob_type`-Render-Fälle, des Text-Edit-Pfads und der Support-Funktionen.

**Methode:** rein statisch, Quelle-gegen-Quelle, Zeile-für-Zeile (kein Build/QEMU — 5 parallele Auditoren).

---

## Teil 1 — Coverage-Tabelle

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| ob_add | gemoblib.c:1121 | objc_add / ob_add — object.rs:694 / 735 | impl | ~ (Return bei Invalid-Args) |
| ob_delete | gemoblib.c:1147 | objc_delete / ob_delete — object.rs:708 / 751 | impl | ja |
| ob_order | gemoblib.c:1201 | objc_order / ob_order — object.rs:722 / 784 | impl | partial (Edge-Handling) |
| ob_draw / just_draw | gemoblib.c:959 / 471 | objc_draw / render_tree / draw_tree / just_draw — object.rs:570 / 612 / 1462 / 1609 | partial | **nein** (mehrere Render-Divergenzen) |
| ob_find | gemoblib.c:1034 | objc_find / find_hit — object.rs:1284 / 2263 | impl | ja (verhaltensäquivalent) |
| ob_offset | gemoblib.c:285 | objc_offset / abs_offset — object.rs:679 / 1418 | impl | ja |
| ob_change | gemoblib.c:1253 | objc_change / change_t — object.rs:850 / 962 | partial | **nein** (Clip, kein Early-Out) |
| ob_edit | gemobed.c:335 | objc_edit / edit_field_t / edit_char — object.rs:994 / 1031 / 1076 | partial | **nein** (EDSTART, Full-Field, Skip-Ahead) |
| ob_sysvar | gemoblib.c:1350 | objc_sysvar — object.rs:892 | impl | ja |
| ob_sst | gemobjop.c:25 | inline in just_draw — object.rs:1609 ff | partial | **nein** (ob_type-Maske, INDIRECT, G_TITLE th=1) |
| ob_actxywh | gemoblib.c:313 | abs_rect / find_hit-inline — object.rs:1327 / 2290 | impl | ja (non-shadow) |
| ob_relxywh | gemoblib.c:302 | t.x/y/w/h-Getter — object.rs:198–212 | impl | ja |
| ob_setxywh | gemoblib.c:349 | — | N/A (nicht exponiert) | — |
| ob_format | gemoblib.c:361 | ob_format — object.rs:1572 | impl | ja |
| everyobj | gemobjop.c:76 | draw_tree-Rekursion — object.rs:1462 | impl | ja (Depth-Semantik geprüft) |
| get_par | gemobjop.c:145 | parent_of — object.rs:1442 | impl | ja |
| get_prev | gemoblib.c:987 | get_prev — object.rs:830 | impl | ja |
| gr_crack | gemgraf.c:612 | gr_crack — object.rs:1514 | impl | ja (Bit-Split exakt) |
| gr_gtext / gr_just | gemgraf.c:595 / 567 | draw_text — object.rs:2084 | partial | **nein** (Rundung, Overflow-Suppress, Font) |
| gr_gicon | gemgraf.c:710 | draw_icon — object.rs:2158 | partial | **nein** (WHITEBAK, Char-Offset, SMALL-Font) |
| gr_box | gemgraf.c:813 | draw_signed_border / draw_border — object.rs:2071 / 2038 | impl | ja |
| gr_rect | gemgraf.c:547 | gr_rect — object.rs:1544 | impl | ja |
| gr_inside | gemgraf.c:535 | inline | impl | ja |
| xor_color | gemoblib.c:93 | xor_color — object.rs:1526 | partial | **nein** (Palette reduziert) |
| xor_is_ok | gemoblib.c:107 | in `changecol` gefaltet — object.rs:1656 | partial | **nein** (changecol non-3D immer true) |
| add_3d_effect | gemoblib.c:220 | add_3d_effect — object.rs:1986 | impl | ~ja |
| draw_3d_outline | gemoblib.c:153 | draw_3d_outline — object.rs:2010 | impl | ~ja |
| ob_user (G_USERDEF) | gemoblib.c:433 / 792 | — | **MISSING** | **nein** |
| G_CICON-Zweig | gemoblib.c:780 | — | **MISSING** | **nein** |
| ins_char | gemobed.c:117 | ins_char — object.rs:1147 | impl | partial (Full-Field) |
| ob_delit | gemobed.c:323 | delete_at — object.rs:1134 | impl | ja |
| find_pos | gemobed.c:158 | find_pos — object.rs:1173 | impl | ja |
| instr | gemobed.c:228 | instr — object.rs:1211 | impl | ja |
| check | gemobed.c:252 | check — object.rs:1236 | impl | ja (Validate-Sets exakt) |
| scan_to_end | gemobed.c:141 | — | **MISSING** | **nein** (Cursor-Jump) |
| ob_center | gemobed.c:67 | — | N/A (Form-Layer, P4) | — |
| objc_sizes | — | — | N/A (kein AES-Call) | — |

---

## Teil 2 — Findings (nach Severity)

### AUD-P3-001: ob_sst/ob_getsp — INDIRECT-Flag (0x0100) wird nirgends aufgelöst
- **Severity:** HIGH
- **EmuTOS:** gemobjop.c:36-37 (`if (ob_flags & INDIRECT) *pspec = *(LONG *)ob_spec;`), gemobed.c:60-62 (ob_getsp)
- **NewTOS:** object.rs:261-270 (`ObjTree::spec`) — MISSING; kein INDIRECT-Zweig im gesamten `aes`-Crate (grep bestätigt: 0 Treffer)
- **Divergenz (CONFIRMED per Quelle):** Setzt ein Objekt `INDIRECT`, ist `ob_spec` ein Zeiger auf einen LONG, der die *echte* spec enthält. NewTOS liest `ob_spec` immer direkt. Für Box-Typen ⇒ das Zeiger-Bitmuster wird als Colour-Word interpretiert (Zufallsfarben/-thickness); für `*TEXT`/`G_STRING`/`G_ICON` ⇒ die Adresse der LONG-Zelle wird als TEDINFO/String/ICONBLK dereferenziert ⇒ Garbage-Render oder **Wild-Pointer-Deref (Crash)**. Betrifft `just_draw`, `ob_change`, `ob_edit` gleichermaßen.
- **1:1-Fix:** In `ObjTree::spec` (bzw. einem `resolved_spec(obj)`): `let mut s = raw_spec; if flags(obj) & INDIRECT != 0 { s = read_ptr_at(s, abi) }` — einmal an zentraler Stelle, dann von allen Konsumenten nutzen.
- **Runtime-verify:** yes

### AUD-P3-002: just_draw / ob_change — G_USERDEF (Typ 24) + ob_user fehlen komplett
- **Severity:** HIGH
- **EmuTOS:** gemoblib.c:433-448 (ob_user), :792-794 (just_draw G_USERDEF), :1282-1285 (ob_change G_USERDEF)
- **NewTOS:** object.rs:1908-1910 — `_ => {}` (kein G_USERDEF-Arm); change_t:962-979 (kein USERDEF-Sonderfall)
- **Divergenz (CONFIRMED):** Ein `G_USERDEF`-Objekt fällt in den `_`-Default ⇒ **es wird gar nichts gezeichnet** und die User-Draw-Callback-Routine (`ub_code(PARMBLK*)`) wird **nie aufgerufen**. Für native LP64-GEM-Apps mit userdef-Objekten (z. B. custom Slider/Gauges in Dialogen) bleibt die Fläche leer und die vom Callback erwartete Zustandslogik läuft nicht. Kein-Stub-Regel-Verstoß (ignorierter Fall).
- **1:1-Fix:** G_USERDEF-Arm ergänzen, der einen `PARMBLK` (tree, obj, prev/curr-state, x/y/w/h, clip, ub_parm) füllt und `ub_code` über die native Callback-ABI aufruft; in `ob_change` denselben Pfad (redraw=FALSE danach).
- **Runtime-verify:** yes

### AUD-P3-003: objc_change — Clip int_in[2..5] ignoriert; zusätzlich fehlender `curr_state == new_state`-Early-Out
- **Severity:** MEDIUM
- **EmuTOS:** gemsuper.c:228-231 (`gsx_sclip((GRECT*)&OB_XCLIP); ob_change(...)`), gemoblib.c:1263 (`if ((curr_state == new_state) || (spec == -1L)) return;`); OB_XCLIP=int_in[2..5] (aesdefs.h:352-354)
- **NewTOS:** objc_change object.rs:850-861 (liest nur intin 0/6/7, **nie** intin 2..5); change_t:962-979 nutzt `ClipRect::full` (:974) und hat **keinen** State-Gleichheits-Check
- **Divergenz (CONFIRMED):** (a) EmuTOS klippt den Redraw auf das übergebene Rechteck `int_in[2..5]`; NewTOS redrawt immer full-screen — ein Programm, das objc_change mit einem einschränkenden Clip aufruft, sieht den Redraw außerhalb des vorgesehenen Bereichs. (b) EmuTOS macht bei unverändertem State **gar nichts**; NewTOS schreibt State und redrawt trotzdem (Flicker/überflüssiger Draw). Die Soll-Referenz D-052 (docs/api/_discrepancies.md:1486-1489) dokumentiert `int_in[2..5]` als Clip korrekt — **Code weicht von Spec UND Quelle ab**.
- **1:1-Fix:** objc_change: Clip aus intin(2..5) bauen und an `change_t` durchreichen (Full-Clip nur wenn w/h==0); in change_t vor dem Schreiben `if t.state(obj)==newstate || spec==spec_none() { return }`.
- **Runtime-verify:** yes

### AUD-P3-004: gr_gicon — WHITEBAK-Zustand nicht ausgewertet (Maske/Text-BG immer gezeichnet)
- **Severity:** MEDIUM
- **EmuTOS:** gemgraf.c:755-768 (`if (!((state & WHITEBAK) && (bgcol == WHITE))) { gr_gblt(pmask,...); ... }`)
- **NewTOS:** draw_icon object.rs:2179-2180 (Maske+Data **immer** geblittet), :2196-2209 (Label-BG-Rect immer) — kein WHITEBAK-Zweig (grep: WHITEBAK nirgends in object.rs)
- **Divergenz (CONFIRMED):** Bei gesetztem `WHITEBAK` und weißem Icon-Hintergrund überspringt EmuTOS das Masken-Blit und das Text-Hintergrund-Rechteck. NewTOS blittet die Maske immer (bgcol=WHITE ⇒ weißes Rechteck) ⇒ auf gemustertem/farbigem Desktop sichtbarer weißer „Halo"/Kasten hinter Icon und Label.
- **1:1-Fix:** `if !((ob_state & WHITEBAK != 0) && bg == WHITE) { blit_plane(mask...); label_bg_rect }` — Data-Blit + Char + Label bleiben außerhalb der Bedingung.
- **Runtime-verify:** yes

### AUD-P3-005: gr_gicon — Icon-Char-Position ohne ib_xicon/ib_yicon-Offset; Char+Label in falscher Fontgröße
- **Severity:** MEDIUM
- **EmuTOS:** gemoblib.c:784-789 (`ib.ib_xicon += t.g_x; ...`) + gemgraf.c:800-806 (Char bei `ib_xicon+ib_xchar`, `gsx_tblt(SMALL,...)`; Label `gr_gtext(TE_CNTR, SMALL, ...)`)
- **NewTOS:** draw_icon object.rs:2184-2191 (Char bei `ox+ib_xchar`, **ohne** ib_xicon), :2210-2220 (Label via draw_text mit normalen CELL_W/CELL_H)
- **Divergenz (CONFIRMED):** (a) NewTOS addiert ib_xicon/ib_yicon zwar zum Icon-Bild-Rect (:2173-2178), **nicht** aber zur Char-Position ⇒ bei Icons mit Bild-Offset ≠ 0 sitzt der overlay-Char falsch. (b) EmuTOS zeichnet Icon-Char **und** -Label im SMALL-Font; NewTOS nutzt den normalen (IBM) Font ⇒ Label/Char überdimensioniert, andere Zentrierung/Breite.
- **1:1-Fix:** Char bei `ox + ib_xicon + ib_xchar` / `oy + ib_yicon + ib_ychar`; für Char+Label die SMALL-Font-Metrik verwenden (sofern vorhanden, sonst als bewusste HW-absence-Deviation dokumentieren).
- **Runtime-verify:** yes

### AUD-P3-006: just_draw — G_CICON (Typ 33) nicht unterstützt (Colour-Icons unsichtbar)
- **Severity:** MEDIUM
- **EmuTOS:** gemoblib.c:776-791 (G_CICON → `cicon = ((CICONBLK*)spec)->mainlist; ib = *((ICONBLK*)spec); gr_gicon(...)`); Default `CONF_WITH_COLOUR_ICONS=1` (config.h:1266)
- **NewTOS:** object.rs:1908-1910 — kein G_CICON-Konstante/-Arm ⇒ `_ => {}`
- **Divergenz (CONFIRMED):** Ein `G_CICON`-Objekt (CICONBLK, beginnt mit Mono-ICONBLK) wird komplett nicht gezeichnet. EmuTOS zeichnet mindestens die Mono-Version (via ICONBLK-Kopf). Ported Apps/RSCs mit Colour-Icons zeigen leere Icon-Flächen.
- **1:1-Fix:** G_CICON-Arm: ICONBLK-Kopf lesen (wie G_ICON) und mono zeichnen; optional die CICON-Liste. Mindestens der Mono-Fallback ist 1:1 nötig.
- **Runtime-verify:** yes

### AUD-P3-007: just_draw — ob_type nicht mit 0x00ff maskiert (High-Byte bricht Type-Dispatch)
- **Severity:** MEDIUM
- **EmuTOS:** gemobjop.c:40 (`*ptype = objptr->ob_type & 0x00ff;`)
- **NewTOS:** object.rs:1615 (`let ob_type = t.ob_type(obj)` — roh, u16 ungefiltert; auch find_hit nutzt keinen Type)
- **Divergenz (CONFIRMED):** EmuTOS maskiert den Basistyp bewusst mit 0x00ff (High-Byte = ggf. Extended-Type/Flags). NewTOS vergleicht den rohen u16 gegen G_BOX(20)…G_TITLE(32) ⇒ jedes Objekt mit gesetztem High-Byte fällt in `_ => {}` und **wird nicht gezeichnet**.
- **1:1-Fix:** `let ob_type = t.ob_type(obj) & 0x00ff;` an der Dispatch-Stelle.
- **Runtime-verify:** yes (RSC mit extended ob_type nötig)

### AUD-P3-008: objc_edit — EDSTART (kind=0) zeichnet statt No-Op
- **Severity:** MEDIUM
- **EmuTOS:** gemobed.c:342-343 (`if ((kind == EDSTART) || (obj <= 0)) return TRUE;` — ohne jeden Draw, ohne TEDINFO-Zugriff)
- **NewTOS:** objc_edit object.rs:994-1010 (fängt nur `obj <= 0` ab); edit_field_t:1031-1067 (`match kind { ED_INIT, ED_CHAR, _ => {} }`) — für kind=0 läuft der Redraw-Block :1053-1065 und zeichnet Feld **+ Caret** (weil `kind != ED_END`)
- **Divergenz (CONFIRMED):** Ein `objc_edit(..., ED_START)` ist in EmuTOS ein reiner No-Op (Return TRUE, idx unverändert). NewTOS liest die TEDINFO, redrawt das Feld und malt einen Caret — unerwartetes Rendering.
- **1:1-Fix:** `if kind == ED_START(0) || obj <= 0 { intout(1)=intin(2); ret=1; return }` bereits in objc_edit.
- **Runtime-verify:** yes

### AUD-P3-009: edit_char — volles Textfeld: EmuTOS überschreibt letztes Zeichen, NewTOS verwirft die Taste
- **Severity:** MEDIUM
- **EmuTOS:** gemobed.c:405-423 (tmp_back: `if (*idx > te_txtlen-2) { cur_pos--; *idx-=1; }` dann `ins_char(rawstr,*idx,c,te_txtlen)` — ins_char trunkiert bei te_txtlen)
- **NewTOS:** object.rs:1122-1127 (`if check(...) && len < txtlen - 1 { ins_char; idx+=1 }`)
- **Divergenz (CONFIRMED):** Bei vollem Feld (`len == txtlen-1`) tippt der User in EmuTOS über die letzte Position (letztes Zeichen wird ersetzt/nachgeschoben-trunkiert). NewTOS ignoriert den Tastendruck komplett (`len < txtlen-1` false). Sichtbar z. B. bei Eingabe am Längenlimit eines Namensfelds.
- **1:1-Fix:** tmp_back-Logik nachbilden (idx zurücksetzen bei idx>txtlen-2) und `ins_char` mit Truncation bei txtlen statt harter `len < txtlen-1`-Sperre.
- **Runtime-verify:** yes

### AUD-P3-010: edit_char — Template-Skip-Ahead (scan_to_end) bei fehlgeschlagener Validierung fehlt
- **Severity:** MEDIUM
- **EmuTOS:** gemobed.c:141-150 (scan_to_end) + :424-438 (bei `check`-Fehlschlag: Cursor auf nächste passende Position springen, mit Blanks auffüllen)
- **NewTOS:** object.rs:1122-1127 — bei `check`-Fehlschlag passiert **nichts** (Zeichen verworfen); scan_to_end existiert nicht
- **Divergenz (CONFIRMED):** In formatierten Feldern (G_FTEXT mit Template) springt EmuTOS beim Tippen eines Template-Trennzeichens den Cursor zur nächsten Datenposition. NewTOS lässt das Zeichen fallen ⇒ formatierte Eingabe (Datum, IP, etc.) verhält sich anders.
- **1:1-Fix:** scan_to_end portieren und den `else`-Zweig des Printable-Falls (Skip-Ahead + memset-Blanks) nachbauen.
- **Runtime-verify:** yes

### AUD-P3-011: just_draw — Text-Writing-Mode-Bit (gr_crack Bit 7) ignoriert (Text immer transparent)
- **Severity:** MEDIUM
- **EmuTOS:** gemgraf.c:621 (`*pmd = (color & 0x80) ? MD_REPLACE : MD_TRANS`) + gemoblib.c:709 (`gsx_attr(TRUE, tmode, tcol)`)
- **NewTOS:** gr_crack object.rs:1520 liefert `_mode` korrekt, aber alle Konsumenten verwerfen ihn (`(…, _mode)`) und draw_text nutzt hart `WriteMode::Transparent` (:2116)
- **Divergenz (CONFIRMED):** Bei te_color/Box-Colour-Word mit gesetztem Bit 7 (MD_REPLACE) malt EmuTOS den Zeichen-Zellhintergrund (opak); NewTOS zeichnet Text stets transparent. Sichtbar bei REPLACE-Text über gemustertem/farbigem Interieur.
- **1:1-Fix:** `_mode` aus gr_crack an draw_text durchreichen und Replace/Transparent respektieren.
- **Runtime-verify:** yes

### AUD-P3-012: just_draw — CONF_WITH_NICELINES Menü-Separator (disabled all-dashes → Linie) fehlt
- **Severity:** MEDIUM
- **EmuTOS:** gemoblib.c:833-843 (`if (G_STRING && DISABLED && len>3 && is_dashes(...)) gsx_cline(...)`); Default `CONF_WITH_NICELINES=1` (config.h:1312-1313)
- **NewTOS:** G_STRING-Arm object.rs:1763-1792 — keine is_dashes/Line-Ersetzung; zeichnet die Bindestriche wörtlich
- **Divergenz (CONFIRMED):** Ein disabled Menü-Separator aus Bindestrichen wird in EmuTOS als saubere durchgezogene Linie gezeichnet, in NewTOS als Reihe von „-".
- **1:1-Fix:** is_dashes-Check im G_STRING-Zweig; bei Treffer horizontale Linie auf halber Höhe statt draw_text.
- **Runtime-verify:** yes

### AUD-P3-013: objc_draw — Zero-Clip-Konvention (w/h==0 = „kein Clip") invertiert
- **Severity:** MEDIUM
- **EmuTOS:** gemsuper.c:210-211 (`gsx_sclip((GRECT*)&OB_XCLIP)`) + gemgraf.c:95-104 (`if (gl_clip.g_w && gl_clip.g_h) … else vs_clip(FALSE)` — 0-Rechteck = Clipping AUS)
- **NewTOS:** objc_draw object.rs:592 (`ClipRect { xmn:cx, ymn:cy, xmx:cx+cw-1, ymx:cy+ch-1 }`) — bei cw=0 wird `xmx = cx-1 < xmn` ⇒ leerer Clip
- **Divergenz (CONFIRMED):** Ruft ein Programm objc_draw mit w=0/h=0 (dokumentierte „no-clip"-Konvention), zeichnet EmuTOS alles unbeschränkt; NewTOS erzeugt ein leeres Clip-Rechteck ⇒ **es wird gar nichts gezeichnet**.
- **1:1-Fix:** `if cw <= 0 || ch <= 0 { clip = full_screen } else { … }` in objc_draw (und analog überall dort, wo AES-Clip aus int_in gebaut wird).
- **Runtime-verify:** yes

### AUD-P3-014: just_draw — changecol non-3D immer `true` (statt `!xor_is_ok`) + xor_color reduzierte Palette
- **Severity:** LOW
- **EmuTOS:** gemoblib.c:524 (`changecol = !xor_is_ok(obtype, flags, spec)`), :93-102 (xor_color via vdi16comp), :107-148 (xor_is_ok)
- **NewTOS:** object.rs:1656-1666 (`_ => (false, true)`), xor_color:1526-1532 (`WHITE↔BLACK, sonst c^1`)
- **Divergenz (CONFIRMED, für s/w unbeobachtbar):** NewTOS invertiert SELECTED immer inline; EmuTOS wählt für s/w-Objekte (xor_is_ok=TRUE bzw. ws_ncolors≤16) den End-XOR-Pfad. Für die reine s/w/Grau-Palette ist das Pixel-Ergebnis identisch (verifiziert für Button/Box/Text). Für farbige SELECTED-Objekte auf >16-Farb-Displays liefert `c^1` (z. B. Rot(2)→Grün(3)) andere Komplementärfarben als vdi16comp (Rot→LMagenta(15)).
- **1:1-Fix:** `changecol = !xor_is_ok(...)` implementieren und xor_color über die vollständige vdi16comp-Tabelle abbilden.
- **Runtime-verify:** no (nur farbige Objekte auf Deep-Palette)

### AUD-P3-015: just_draw — CHECKED nur im G_STRING-Arm (EmuTOS: alle Typen) + vertikale Position
- **Severity:** LOW
- **EmuTOS:** gemoblib.c:903-908 (CHECKED im generischen `if(state)`-Block für jeden Typ; `gsx_tblt(IBM, t.g_x+2, t.g_y, 1)`)
- **NewTOS:** object.rs:1769-1783 (CHECKED-Glyph nur im G_STRING-Arm; y=`y+(h-CELL_H)/2`)
- **Divergenz (CONFIRMED):** Ein CHECKED G_BOX/G_BUTTON zeigt in EmuTOS ein Häkchen, in NewTOS nicht (nur G_STRING). Für Menüs (G_STRING) identisch; Randfälle abweichend. Zusätzlich minimale y-Differenz (EmuTOS t.g_y-basiert vs. NewTOS vertikal zentriert) bei h≠CELL_H.
- **1:1-Fix:** CHECKED in den generischen Decoration-Block verschieben (alle Typen), y an EmuTOS angleichen.
- **Runtime-verify:** no

### AUD-P3-016: just_draw — CROSSED/DISABLED über vollem Rect statt th-inset Rect
- **Severity:** LOW
- **EmuTOS:** gemoblib.c:889-892 (`if (th>0) gr_inside(&t, th)`) — danach nutzen CROSSED (:910-915) und DISABLED (:917-930) das um th verkleinerte Rect
- **NewTOS:** object.rs:1959-1970 — CROSSED und DISABLED über `(x,y,w,h)` (voll, kein th-Inset)
- **Divergenz (CONFIRMED):** Bei Objekten mit positivem Rahmen (th>0) zeichnet EmuTOS Kreuz/Dither nur über dem Interieur; NewTOS über dem gesamten Rect inkl. Rahmen. 1–th-px sichtbarer Unterschied.
- **1:1-Fix:** CROSSED/DISABLED-Rect bei positivem deco_th um deco_th einrücken.
- **Runtime-verify:** no

### AUD-P3-017: ob_sst — G_TITLE th=1 nicht modelliert (SHADOWED-Title ohne Schatten; Selected-Title 1px-Inset)
- **Severity:** LOW
- **EmuTOS:** gemobjop.c:44-45 (`case G_TITLE: th = 1`) — wirkt in gemoblib.c:889 (`gr_inside(&t,1)`) für SHADOWED/SELECTED-Title
- **NewTOS:** object.rs:1793-1808 (G_TITLE: deco_th bleibt 0; SELECTED = Full-Rect-Fill)
- **Divergenz (CONFIRMED):** Ein SHADOWED G_TITLE bekommt in EmuTOS wegen th=1 einen Schatten, in NewTOS nicht. Der Menü-Title-Highlight ist in EmuTOS um 1px eingerückt (XOR über inset-Rect), in NewTOS Full-Rect-Fill.
- **1:1-Fix:** Für G_TITLE deco_th=1 setzen; Highlight-Fill/Decos um 1px einrücken.
- **Runtime-verify:** no

### AUD-P3-018: objc_add/objc_draw — Return 0 bei Invalid-Args (EmuTOS immer 1)
- **Severity:** LOW
- **EmuTOS:** gemsuper.c:85 (`ret = TRUE;`), :204-206 (OBJC_ADD überschreibt ret nicht ⇒ 1), :210-213 (OBJC_DRAW ⇒ 1)
- **NewTOS:** objc_add object.rs:698-701 (Return 0 bei parent/child<0), objc_draw:583-586 (Return 0 bei null-tree/kein Backend/drawob<0)
- **Divergenz (CONFIRMED):** Bei Valid-Args beide 1. Bei Invalid-Args (parent=-1, null tree, …) gibt EmuTOS 1 zurück (ob_add No-Op / ob_draw crasht auf null), NewTOS gibt defensiv 0 zurück. ABI-Return weicht nur im Fehlerfall ab.
- **1:1-Fix:** Bei Invalid-Args weiterhin `set_ret(1)` (No-Op) für ABI-Treue — dokumentierte defensive Abweichung ansonsten.
- **Runtime-verify:** no

### AUD-P3-019: ob_order — defensives Edge-Handling weicht von EmuTOS ab (Sole-Child / new_pos out-of-range)
- **Severity:** LOW
- **EmuTOS:** gemoblib.c:1201-1246 (kein Bounds-Check der `for`-Schleife :1229-1233; bei leerem Parent nach Delete ⇒ `chg_obj=NIL` ⇒ `tree[-1]`-Zugriff = UB)
- **NewTOS:** object.rs:793-826 (Sole-Child-Zweig :793-800; new_pos-Clamp :810-817)
- **Divergenz (CONFIRMED):** Im gültigen Bereich identisch (verifiziert für new_pos=0/NIL/mid). NewTOS fügt defensive Behandlung hinzu, wo EmuTOS selbst UB/Memory-Corruption produziert (Reorder des einzigen Kindes mit new_pos=NIL ⇒ EmuTOS schreibt `tree[-1]`). Abweichung nur in Fällen, in denen EmuTOS undefiniert ist.
- **1:1-Fix:** Als bewusste Sicherheits-Abweichung dokumentieren (nicht 1:1-„matchen", da EmuTOS dort korrupt ist) — oder exakt EmuTOS-Kontrollfluss nachbilden, wenn strikte Bit-Gleichheit gefordert.
- **Runtime-verify:** no

### AUD-P3-020: gr_gtext/gr_just — Justierungs-Rundung, Overflow-Suppression, Whole-Char-Truncation
- **Severity:** LOW
- **EmuTOS:** gemgraf.c:567-589 (gr_just: `(w+1)/2`, `(h+1)/2`; numchs auf `ptextw/wc` begrenzt) + :493-496 (Text höher als Box ⇒ numchs=0, kein Draw)
- **NewTOS:** draw_text object.rs:2097-2116 (`(w-tw)/2`, `(h-CELL_H)/2`; zeichnet alle Zeichen, Pixel-Clip statt Char-Truncation)
- **Divergenz (CONFIRMED):** (a) Zentrierung: EmuTOS rundet auf ((n+1)/2), NewTOS trunkiert (n/2) ⇒ 1px-Versatz bei ungeraden Margen. (b) Text höher als Box: EmuTOS zeichnet nichts, NewTOS zeichnet (Pixel-geklippt). (c) Überbreiter Text: EmuTOS lässt ganze Zeichen weg, NewTOS klippt mitten im Zeichen.
- **1:1-Fix:** Zentrierung `(margin+1)/2`; Höhen-Check (kein Draw wenn CELL_H > h); ganze Zeichen begrenzen statt Pixel-Clip.
- **Runtime-verify:** no

### AUD-P3-021: edit_char — Arrow-Erkennung nur per Scancode-High-Byte; '@'=leer nicht auf EDINIT/Plain angewandt
- **Severity:** LOW
- **EmuTOS:** gemobed.c:379-404 (`switch(in_char)` vergleicht ganze WORDs: BACKSPACE=0x0e08, DELETE=0x537f, ARROW_LEFT=0x4b00 …), :364 (EDINIT idx via strlen des durch ob_format ge-'@'-leerten rawstr)
- **NewTOS:** object.rs:1083-1097 (Arrows nur über High-Byte `scan==0x4B/0x4D`), :1047 (`ED_INIT => idx = cstr_len_p(text)` ohne '@'-Behandlung)
- **Divergenz (CONFIRMED):** (a) Tasten, die Scancode 0x4B/0x4D mit ASCII≠0 liefern (z. B. SHIFT_ARROW_LEFT=0x4b34 '4'), fügt EmuTOS als '4' ein, NewTOS bewegt den Cursor. (b) te_ptext mit führendem '@' wird von EmuTOS als leer behandelt (idx=0); NewTOS zählt "@..." mit.
- **1:1-Fix:** Arrows/BS/DEL/ESC gegen die vollen Scancode-WORDs matchen; '@'-Leer-Konvention auch für EDINIT-idx und Plain-Text anwenden.
- **Runtime-verify:** no

### AUD-P3-022: just_draw — CONF_WITH_EXTENDED_OBJECTS G_STRING-Unterstreichung (WHITEBAK+0xFF00) fehlt
- **Severity:** LOW
- **EmuTOS:** gemoblib.c:847-860 (`if (G_STRING && WHITEBAK && (state&0xFF00)==0xFF00) { gsx_cline unterstreichen }`); Default `CONF_WITH_EXTENDED_OBJECTS=1` (config.h:1274-1275)
- **NewTOS:** G_STRING-Arm object.rs:1763-1792 — keine Unterstreichungs-Logik
- **Divergenz (CONFIRMED):** MagiC-Style unterstrichene String-Objekte werden in NewTOS ohne Unterstreichung gezeichnet. Nische.
- **1:1-Fix:** Bedingte Unterstreichung (LBLACK/WHITE cline unter dem String) im G_STRING-Zweig ergänzen.
- **Runtime-verify:** no

### AUD-P3-023: objc_edit/objc_change/objc_draw — Clip-State-Semantik (gl_rfull vs. Full; keine Persistenz)
- **Severity:** LOW
- **EmuTOS:** gemsuper.c:224 (objc_edit `gsx_sclip(&gl_rfull)` = Screen ohne Menüleiste); persistentes globales `gl_clip`
- **NewTOS:** edit_field_t object.rs:1054 (`ClipRect::full` = ganzer Screen inkl. Menüleistenzeile); NewTOS hält kein persistentes gl_clip
- **Divergenz (CONFIRMED):** objc_edit klippt in EmuTOS auf gl_rfull (ab gl_hbox), NewTOS auf den ganzen Screen; Edit-Feld liegt i. d. R. unter der Menüleiste ⇒ selten sichtbar. Fehlende gl_clip-Persistenz betrifft nur interleaved Calls, die auf Rest-Clip bauen.
- **1:1-Fix:** objc_edit-Clip auf das „full minus Menübar"-Rect setzen.
- **Runtime-verify:** no

---

## Was ich getrieben habe vs. offen

**Getrieben (statisch, Quelle-gegen-Quelle, Zeile-für-Zeile):**
- Alle 9 Public-Calls (objc_add/delete/draw/find/offset/order/edit/change/sysvar) gegen gemsuper.c-Dispatch + aesdefs.h-Slots verifiziert (Opcodes 40–48, int_in/int_out/addr_in-Mapping, `ret=TRUE`-Default).
- Alle `ob_type`-Render-Arme (G_BOX/IBOX/BUTTON/BOXCHAR/STRING/TITLE/TEXT/BOXTEXT/FTEXT/FBOXTEXT/IMAGE/ICON) gegen just_draw + ob_sst; Colour-Word-Bit-Split (gr_crack) exakt geprüft; signed-thickness/gr_box-Semantik durchgerechnet; 3D-Pfade (FL3DIND/ACT/BAK, add_3d_effect, draw_3d_outline, ADJ3DSTD/OUT) verifiziert.
- Kompletter Edit-Pfad (EDINIT/EDCHAR/EDEND/EDSTART, check/instr/find_pos/scan_to_end/ins_char/ob_delit, alle VALIDATE_*-Sets) gegen gemobed.c; scancode.h-Werte bestätigt.
- Hit-Test-Reihenfolge (find_hit vs. ob_find, DISABLED wird NICHT übersprungen, 3D-Grow, SHADOWED-Ausnahme) durch mehrere überlappende-Geschwister-Fälle als äquivalent nachgewiesen.
- Depth-Semantik draw_tree vs. everyobj (maxdep=0/1/2) durchgerechnet.
- CONF_WITH_*-Gating (3D=1, COLOUR_ICONS=1, EXTENDED_OBJECTS=1, NICELINES=1 im Standard-Build) aus config.h bestätigt.
- Soll-Referenz D-052 gegen Quelle gegengeprüft: **Spec ist korrekt** (dokumentiert Clip int_in[2..5], Colour-Split, DISABLED-Nicht-Skip, kein DRAW3D-Bit) — kein Researcher-Routing nötig; der Code weicht in AUD-P3-003 von der (korrekten) Spec ab.

**Nicht getrieben (offen):**
- Keine Laufzeit-Reproduktion (QEMU/Screendump) — per Auftrag rein statisch (5 parallele Auditoren). Alle mit „Runtime-verify: yes" markierten Findings (001–013) sind aus der Quelle **CONFIRMED**, aber die konkrete Pixel-/Verhaltens-Ausprägung ist noch nicht am Bild reproduziert.
- Ob NewTOS-eigene RSCs tatsächlich INDIRECT/G_USERDEF/G_CICON/extended-ob_type nutzen, konnte ich nicht abschließend klären (die Divergenz besteht unabhängig davon; die Beobachtbarkeit hängt vom Resource-Inhalt ab).

## Out-of-scope Beobachtungen
- `change()` (object.rs:955) nutzt `ObjTree::new`/current_abi — Aufrufer im Form-Layer (P4) müssen für kernel-gebaute LP64-Alerts `change_t`+`lp64` verwenden, sonst Misread unter ILP32-App; Prüfung gehört zu P4.
- gsx_moff/gsx_mon (Maus-Hide) in ob_draw fehlt in NewTOS — architektonische SW-Cursor-Deviation, API-unbeobachtbar (kein Bug).

## Zusammenfassung

**Counts:** BLOCKER 0 · HIGH 2 · MEDIUM 11 · LOW 10 → **23 Findings** über 9 Public-Fns + ~14 interne Helfer (3 komplett MISSING: ob_user/G_USERDEF, G_CICON, scan_to_end).

**Top-3 (schwerste):**
1. **AUD-P3-001 (HIGH):** INDIRECT-Flag wird nie aufgelöst — falscher Deref/Crash bei jedem INDIRECT-Objekt in draw/change/edit.
2. **AUD-P3-002 (HIGH):** G_USERDEF/ob_user fehlt vollständig — userdef-Objekte unsichtbar, Callback nie aufgerufen (Kein-Stub-Verstoß).
3. **AUD-P3-003 (MEDIUM, spec-bestätigt):** objc_change ignoriert den Clip int_in[2..5] und hat keinen `curr==new`-Early-Out — Redraw außerhalb des vorgesehenen Bereichs, Code weicht von der korrekten Spec D-052 ab.
