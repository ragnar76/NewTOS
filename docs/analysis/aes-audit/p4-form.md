# P4 — Form Manager: 1:1-Divergenz-Katalog

Scope: AES Form Manager `tos-api/src/aes/form.rs` gegen EmuTOS `gemfmlib.c`
(fm_do/fm_own/fm_keybd/fm_button/fm_dial/fm_error/fm_show/eralert/find_obj/
fm_inifld), `gemfmalt.c` (fm_alert/fm_parse/fm_strbrk/fm_build), `gemobed.c`
(ob_center/ob_edit), `gemobjop.c` (ob_sst), `gemgrlib.c`
(gr_growbox/shrinkbox/watchbox) — Funktionen form_do, form_dial, form_alert,
form_error, form_center, form_keybd, form_button + Internals. Rein statisch
(kein Build/QEMU, wie beauftragt).

## Teil 1 — Coverage-Tabelle

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `fm_do` | gemfmlib.c:278-353 | `form_do`/`run_form` (form.rs:513,524) | partial | no |
| `fm_own` | gemfmlib.c:82-107 | — | MISSING | no |
| `fm_inifld` | gemfmlib.c:153-160 | (inline in `run_form`, form.rs:532) | partial | no |
| `find_obj` | gemfmlib.c:114-150 | `find_flag`/`next_editable`/`prev_editable`/`find_obj` (form.rs:662,613,834,852) | partial | no |
| `fm_keybd` | gemfmlib.c:163-198 | `form_keybd` (form.rs:924) | impl | ~ (no, 2 Abw.) |
| `fm_button` | gemfmlib.c:201-270 | `form_button` (form.rs:882) | impl | yes |
| `fm_dial` | gemfmlib.c:360-383 | `form_dial` (form.rs:772) | impl | no |
| `fm_show` | gemfmlib.c:389-401 | — | MISSING | no |
| `eralert` | gemfmlib.c:416-433 | — | MISSING | no |
| `fm_error` | gemfmlib.c:436-472 | `form_error` (form.rs:682) | impl | no (invertiert) |
| `fm_alert` | gemfmalt.c:319-396 | `form_alert`/`do_alert` (form.rs:114,135) | impl | no (Neubau) |
| `fm_parse` | gemfmalt.c:157-174 | (inline in `do_alert`, form.rs:142-164) | impl | no |
| `fm_strbrk` | gemfmalt.c:78-134 | `group`/`split_copy` (form.rs:427,456) | impl | ~ (Escaping ja, Limits nein) |
| `fm_build` | gemfmalt.c:188-316 | (inline layout in `do_alert`, form.rs:166-239) | impl | no (eigene Geometrie) |
| `ob_center` (form_center) | gemobed.c:67-106 | `form_center` (form.rs:716) | impl | ~ (yes bis auf Shadow-Typ) |
| `ob_edit`(EDCHAR Cursor) | gemobed.c:335-463 | `edit_field`/`edit_char` (object.rs:1024,1076) | partial | no (Up/Down) |
| `gr_growbox`/`gr_shrinkbox` | gemgrlib.c:309-334 | `graf::animate` (graf.rs:213) | impl | no (kein XOR) |
| `gr_watchbox` | gemgrlib.c:337-355 | `watchbox`/`watchbox_t` (graf.rs:296,305) | impl | yes |

Wiring geprüft: Opcodes 50–56 korrekt im Dispatcher (mod.rs:216-222);
`set_ret`→`int_out[0]`, `set_intout(i)`→`int_out[i]`; Slot-Indizes gegen
`aesdefs.h:305-336` + `gemsuper.c:239-263` verifiziert.

---

## Teil 2 — Findings

### AUD-P4-001: form_error — Rückgabe invertiert, Eingabe-Vorzeichen falsch, Strings/Icon falsch, `n>63`-Guard fehlt
- Severity: BLOCKER
- EmuTOS: gemfmlib.c:436-472 (Aufrufkonvention belegt durch gemfslib.c:334 `fm_error(-ret-31)` und desksupp.c:626 `form_error(2)`)
- NewTOS: form.rs:682-703
- Divergenz:
  1. **Return invertiert.** EmuTOS: `ret = fm_show(...); return (ret != 1);` — bei Klick auf Button 1 (das kanonische OK/Cancel) ist Return `FALSE(0)`. NewTOS: `pb.set_ret(do_alert(1, ...))`, und `do_alert` baut genau **einen** `[ OK ]`-Button ⇒ liefert konstant `1`. NewTOS gibt also `1` zurück, wo EmuTOS `0` liefert. (Bei mehr-Button-Alerts liefert NewTOS die rohe 1-basierte Nummer statt des Booleans `ret!=1`.)
  2. **Eingabe-Vorzeichen invertiert.** EmuTOS `n` ist ein *positiver* kleiner DOS-Code (2,3,4,5,8,10,11,15,18 → gemappt auf AL18ERR/AL04ERR/AL05ERR/AL15ERR/AL08ERR; sonst ALXXERR mit `%d`). NewTOS `err_string` (i18n.rs:65-78) matcht nur *negative* TOS-`E*`-Codes (-2,-33,…). Folge: `form_error(2)` (wie es EmuTOS-Desktop aufruft, „file not found") liefert bei NewTOS die generische Meldung „A GEMDOS error occurred." statt „file not found".
  3. **Icon/Strings.** EmuTOS wählt pro Fehlerklasse eine kanonische RSC-Alert (eigenes Icon/Buttons). NewTOS erzwingt fest Icon `[3]` (STOP) + eigenen englischen Text + `[ OK ]`.
  4. **Range-Guard fehlt.** EmuTOS: `if (n > 63) return FALSE;` (keine Alert). NewTOS zeigt für `errnum>63` eine Alert.
- 1:1-Fix: `n>63 ⇒ ret 0, keine Anzeige`; `n` als positiven DOS-Code interpretieren und exakt die EmuTOS-`switch`-Tabelle (2/18/3→AL18ERR, 4→AL04ERR, 5→AL05ERR, 15→AL15ERR, 8/10/11→AL08ERR, default→ALXXERR mit eingesetzter Nummer, Level 1) reproduzieren; Return = `(button != 1)`.
- Runtime-verify: yes

### AUD-P4-002: form_do — `start_fld` (FM_START, int_in[0]) wird komplett ignoriert
- Severity: BLOCKER (dokumentiertes ABI-Argument stillschweigend verworfen — „ignoriert" per Rubrik)
- EmuTOS: gemfmlib.c:278 `fm_do(tree, start_fld)` → :296 `next_obj = fm_inifld(tree, start_fld)`; gemsuper.c:240 `fm_do(FM_FORM, FM_START)`, FM_START = int_in[0]
- NewTOS: form.rs:513-520 (`form_do` liest nur `addrin(0)`, nie `intin(0)`), form.rs:532
- Divergenz: EmuTOS positioniert den Cursor auf `start_fld` (bzw. bei 0 auf das erste editierbare Feld via `find_obj(FORWARD)`). NewTOS liest `int_in[0]` gar nicht und setzt Fokus immer auf `find_flag(EDITABLE)` (erstes editierbares Feld). Ein App-Aufruf `form_do(tree, 5)` (Startfeld 5) fokussiert bei NewTOS das falsche Feld.
- 1:1-Fix: `int_in[0]` als `start_fld` lesen; `fm_inifld`-Logik (start_fld==0 ⇒ `find_obj(0,FORWARD)`, sonst start_fld direkt) übernehmen und als Startfokus verwenden.
- Runtime-verify: yes

### AUD-P4-003: form_alert — Enter liefert Button 1, obwohl `defbut==0` (kein Default)
- Severity: HIGH (falscher Return für legalen Eingang defbut=0)
- EmuTOS: gemfmalt.c:343-347 (DEFAULT-Flag nur wenn `defbut!=0`) + gemfmlib.c:171-195 (fm_keybd RETURN → `find_obj(DEFLT)`; kein DEFAULT gefunden ⇒ `*pnew_obj=0`, `cont=TRUE`, Schleife läuft weiter — Enter tut nichts)
- NewTOS: form.rs:366-372 (`modal_loop`)
- Divergenz: NewTOS `b'\r'|b'\n' => { if defbut>=1 && defbut<=numbut return defbut; return 1; }` — bei `defbut==0` wird `1` zurückgegeben. EmuTOS ignoriert Enter, wenn kein Default gesetzt ist (Nutzer muss klicken). `form_alert(0,"[2][Sure?][Yes|No]")` + Enter ⇒ NewTOS `1` (Yes), EmuTOS wartet.
- 1:1-Fix: Enter nur behandeln, wenn ein DEFAULT-Button existiert (defbut in [1..numbut]); sonst Event verwerfen und weiterloopen.
- Runtime-verify: yes

### AUD-P4-004: find_obj (FORWARD/BACKWARD) — wrap-around + fehlender HIDETREE/DISABLED-Filter
- Severity: HIGH (falsche/fehlerhafte Feldnavigation; navigiert in disabled/hidden Felder)
- EmuTOS: gemfmlib.c:114-150 (`find_obj`: kein Wrap — stoppt bei LASTOB/obj<0 und gibt `start_obj` zurück; überspringt `HIDETREE`+`DISABLED`)
- NewTOS: form.rs:613-630 (`next_editable`), form.rs:834-849 (`prev_editable`) — betrifft `form_keybd` (55) und `form_do`-Tab
- Divergenz: (a) `next/prev_editable` wrappen mit `(i±1) % n` — beim letzten Feld springt NewTOS zum ersten (EmuTOS bleibt stehen). (b) Beide prüfen nur `EDITABLE`, **nicht** `HIDETREE` (ob_flags) und **nicht** `DISABLED` (ob_state) — NewTOS fokussiert versteckte/deaktivierte Editierfelder, die EmuTOS überspringt. (Kontrast: `find_flag` für DEFLT prüft beides korrekt, form.rs:669.)
- 1:1-Fix: `find_obj` 1:1 nachbauen (kein Wrap; bei nichts gefunden `start_obj` zurück; `!(HIDETREE) && !(DISABLED)` als Skip-Bedingung, Flag = EDITABLE).
- Runtime-verify: yes

### AUD-P4-005: form_do — Pfeil-Hoch/Runter navigieren keine Felder (an objc_edit verschluckt)
- Severity: MEDIUM (Multi-Feld-Dialoge)
- EmuTOS: gemfmlib.c:176-182 (fm_keybd: `ARROW_UP`→BACKWARD, `ARROW_DOWN`/`TAB`→FORWARD Feldnavigation)
- NewTOS: form.rs:555-559 (run_form leitet jede Nicht-Enter/Nicht-Tab-Taste an `edit_field(ED_CHAR)`); object.rs:1083-1097 behandelt nur SC_LEFT/SC_RIGHT ⇒ ARROW_UP(0x48)/DOWN(0x50) sind No-Ops
- Divergenz: In NewTOS-`form_do` bewegen Pfeil hoch/runter nichts; nur Tab (vorwärts) navigiert. EmuTOS: hoch = vorheriges Feld, runter/Tab = nächstes Feld.
- 1:1-Fix: In `run_form` vor der `edit_field`-Weiterleitung `fm_keybd`-Navigation aufrufen (RETURN/UP/DOWN/TAB abfangen), nur wenn ein Zeichen übrig bleibt an `edit_field(ED_CHAR)` (wie EmuTOS fm_do:320-325).
- Runtime-verify: yes

### AUD-P4-006: form_do — Doppelklick auf TOUCHEXIT setzt `0x8000`-Bit im Return nicht
- Severity: MEDIUM (falscher Return für Double-Click-TOUCHEXIT-Objekte)
- EmuTOS: gemfmlib.c:216-221 + 267 (`orword=0x8000` bei clks==2, `*pnew_obj = new_obj | orword`) → fm_do:337 propagiert es in den Return
- NewTOS: form.rs:593-596 (`run_form`: nicht-selectable EXIT/TOUCHEXIT ⇒ `press; return hit;` — ohne Klickzahl, ohne 0x8000)
- Divergenz: EmuTOS-`form_do` liefert bei Doppelklick auf ein TOUCHEXIT-Objekt `objindex | 0x8000`. NewTOS liefert nur `objindex`. Apps, die das High-Bit auf Double-Click prüfen, sehen es nie.
- 1:1-Fix: Klickzahl (2) auswerten, bei TOUCHEXIT+Double-Click `0x8000` in den zurückgegebenen Objektindex ORen (fm_button-Semantik übernehmen; idealerweise `run_form` über `form_button`/`fm_button` führen).
- Runtime-verify: yes

### AUD-P4-007: form_alert — DEFAULT-Flag wird nie auf den Default-Button gesetzt (keine Default-Optik)
- Severity: MEDIUM (sichtbare Abweichung)
- EmuTOS: gemfmalt.c:343-347 (`obj = tree+BUTOFF+defbut-1; obj->ob_flags |= DEFAULT;` — nach fm_build)
- NewTOS: form.rs:229-239 (Buttons erhalten `SELECTABLE|EXIT|FL3DACT[|LASTOB]`, nie `DEFAULT`); Default nur als Parameter in `modal_loop`
- Divergenz: In EmuTOS bekommt der Default-Button den dickeren Default-Rahmen (object.rs:1704 rendert DEFAULT). NewTOS setzt das Flag nicht ⇒ der Default-Button ist optisch nicht hervorgehoben.
- 1:1-Fix: Nach dem Tree-Aufbau `tree[first_btn + defbut-1].ob_flags |= DEFAULT` setzen (falls `1<=defbut<=numbut`), vor dem Zeichnen.
- Runtime-verify: yes

### AUD-P4-008: do_alert/fm_strbrk — Zeilen-/Button-Limits weichen ab (6 statt 5 Zeilen, keine Längen-Kappung)
- Severity: MEDIUM (Grammatik-Edge)
- EmuTOS: gemfmalt.c:165,169 + tools/erd.c:1581-1584 (`MAX_LINENUM=5`, `MAX_LINELEN=40`, `MAX_BUTNUM=3`, `MAX_BUTLEN=20`); fm_strbrk kappt zu lange Substrings (:108-115)
- NewTOS: form.rs:108 `MAX_MSG=6`; `split_copy` (form.rs:456-504) hat keine Per-Substring-Längenbegrenzung (nur globaler 512-Byte-Puffer)
- Divergenz: (a) NewTOS erlaubt **6** Meldungszeilen, EmuTOS **5** (die 6. verschluckt EmuTOS). (b) Zeilen >40 Zeichen / Buttons >20 Zeichen werden bei EmuTOS abgeschnitten, bei NewTOS voll übernommen. (`MAX_BTN=3` stimmt.)
- 1:1-Fix: `MAX_MSG=5`; pro Substring bei `MAX_LINELEN`/`MAX_BUTLEN` kappen und Rest fressen wie fm_strbrk.
- Runtime-verify: no

### AUD-P4-009: do_alert — leerer Button-String wird durch „OK" ersetzt (EmuTOS zeigt leeren Button)
- Severity: MEDIUM
- EmuTOS: gemfmalt.c:78-134 (fm_strbrk auf leeren String ⇒ `*pnum=1`, Buttontext `""`); fm_build erzeugt einen (leeren) Button
- NewTOS: form.rs:154-161 (`if numbut==1 && btn[0].1==0 { … "OK" }`)
- Divergenz: `[1][msg][]` bzw. `[1][msg]` ⇒ EmuTOS zeichnet einen leeren Button, NewTOS erfindet „OK". Nicht 1:1 (invented content).
- 1:1-Fix: Keine „OK"-Substitution; leeren Buttontext wie EmuTOS durchreichen.
- Runtime-verify: no

### AUD-P4-010: form_keybd — unbekannte Taste liefert FM_OBJ (int_in[0]) statt FM_INXTOB (int_in[2]) als next-obj
- Severity: MEDIUM (ABI-Edge)
- EmuTOS: gemsuper.c:256-258 (`FM_ONXTOB = FM_INXTOB` vorkopiert, dann `fm_keybd(...,&FM_ONXTOB)`); fm_keybd:185-197 lässt `*pnew_obj` bei direction==-1 unangetastet ⇒ `int_out[1] == int_in[2]`
- NewTOS: form.rs:945,957 (`newobj = obj` = int_in[0]; bei unbekannter Taste bleibt `newobj=obj`, `set_intout(1, newobj)`)
- Divergenz: Bei einer normalen (Nicht-Navigations-)Taste gibt EmuTOS `FM_ONXTOB = FM_INXTOB` (Eingabe-Nextobj, int_in[2]) zurück; NewTOS gibt `FM_OBJ` (int_in[0]) zurück. Unterschiedlich, sobald der App-Aufrufer `int_in[2] != int_in[0]` übergibt. (Der Zeichen-Rückgabewert int_out[2] stimmt.)
- 1:1-Fix: `newobj` mit `int_in[2]` (FM_INXTOB) initialisieren, nicht mit `int_in[0]`.
- Runtime-verify: yes

### AUD-P4-011: fm_own (fm_ownit) fehlt vollständig
- Severity: MEDIUM (überwiegend Multitasking-/Ownership-intern; nicht als unbeobachtbar bewiesen)
- EmuTOS: gemfmlib.c:82-107 (fm_own: `wm_update(BEG/END_UPDATE)`, Menütree-Hold `gl_mntree=NULL`, `get_ctrl`/`get_mown`, `ct_chgown`); von fm_do:287,349 gerufen
- NewTOS: — (weder `form_do` noch `do_alert` sperren den Screen oder halten den Menütree)
- Divergenz: Kein `BEG/END_UPDATE`-Screen-Lock, kein Deaktivieren der Menüleiste während des modalen Dialogs, keine Maus-/Control-Ownership-Übergabe. Bei Single-Tasking ist der Screen-Lock vermutlich unbeobachtbar; das `gl_mntree=NULL` (Menü während Dialog inaktiv) ist am aktuellen Altitude ebenfalls kaum beobachtbar — aber **nicht verifiziert** (Menü-Klick während `form_do` im Multitasking-Ausbau). Default „match it".
- 1:1-Fix: `fm_own(TRUE/FALSE)` mit BEG/END_UPDATE + Menütree-Hold um die modale Schleife legen (form_do und form_alert).
- Runtime-verify: yes (Menü-während-Dialog nur auf realem/Multitasking-Pfad)

### AUD-P4-012: fm_show / eralert fehlen — DOS-Critical-Error-Alertpfad (Laufwerksbuchstabe) nicht abgebildet
- Severity: MEDIUM
- EmuTOS: gemfmlib.c:389-401 (`fm_show`: `rs_str`+`sprintf(alert_str, ad_alert, *pwd)` für Laufwerksbuchstaben), :416-433 (`eralert`: Tabellen `ml_alrt`/`ml_pwlv`, Return `FALSE`=Button1/Cancel, sonst Sanduhr+`TRUE`)
- NewTOS: — (form_error baut eigenen Pfad; kein fm_show/eralert)
- Divergenz: Die kritischen Laufwerks-Alerts (AL00CRT…AL05CRT mit eingesetztem Laufwerksbuchstaben, Retry/Cancel-Semantik) existieren nicht. Betrifft die Disk-Fehler-UX. `eralert` selbst ist an den BIOS-Critical-Vector (Atari/DOS-HW-Pfad) gebunden — der Vektor-Aufruf ist HW-nah, aber die **Alert-Anzeige + Retry/Cancel-Rückgabe** ist Teil des beobachtbaren AES-Verhaltens.
- 1:1-Fix: `fm_show`(Level+optionaler Laufwerksbuchstabe → `fm_alert`) implementieren; `eralert` als Bridge vom Critical-Error-Handler mit den Tabellen ml_alrt/ml_pwlv und Return `(fm_show(...) != 1)`.
- Runtime-verify: yes

### AUD-P4-013: form_dial — FMD_FINISH ignoriert `pt`-Rechteck; GROW/SHRINK-Animation ohne XOR
- Severity: MEDIUM
- EmuTOS: gemfmlib.c:375-379 (`FMD_FINISH`: `w_drawdesk(pt)` + `w_update(DESKWH, pt, DESKWH, FALSE)` — nur `pt`-Bereich); :369-372 gr_growbox/shrinkbox zeichnen XOR-Outline (selbstlöschend)
- NewTOS: form.rs:779 (`FMD_FINISH => window::redraw()` ohne Rect); graf.rs:213-244 (`animate`: BLACK-Outline, dann WHITE-Outline zum „Löschen")
- Divergenz: (a) FMD_FINISH übergibt das `pt`-Rechteck nicht; es wird (vermutlich) der ganze Desktop neugezeichnet statt nur `pt`. (b) `animate` löscht die vorige Outline durch Übermalen mit WHITE statt XOR ⇒ auf gemustertem Hintergrund bleiben transiente weiße Streifen bis FMD_FINISH. (Richtung GROW=little→big, SHRINK=big→little ist korrekt.)
- 1:1-Fix: FMD_FINISH auf das `pt`-Rechteck begrenzt neuzeichnen; Animation als XOR-Outline (selbstrestaurierend), Schrittzahl aus Distanz (gr_stepcalc) statt fix 8.
- Runtime-verify: yes

### AUD-P4-014: do_alert — Alert wird bildschirmmittig zentriert, nicht unter der Menüleiste
- Severity: LOW (kosmetisch/vertikaler Offset)
- EmuTOS: gemobed.c:78 (`yd = gl_hbox + (gl_height - gl_hbox - hd)/2`) via `ob_center` in fm_alert:355
- NewTOS: form.rs:198 (`box_y = (sh - box_h)/2`, volle Bildschirmhöhe)
- Divergenz: NewTOS zentriert vertikal über die volle Höhe; EmuTOS um `gl_hbox` (19 px) nach unten versetzt (unter der Menüleiste). Inkonsistent auch mit dem eigenen `form_center` (form.rs:739, das `bar` korrekt nutzt).
- 1:1-Fix: `box_y = bar + (sh - bar - box_h)/2`.
- Runtime-verify: no

### AUD-P4-015: modal_loop — Ziffern-Shortcut '1'..'9' zur Buttonwahl existiert in EmuTOS nicht
- Severity: LOW (erfundenes Zusatzverhalten)
- EmuTOS: gemfmlib.c:163-198 (fm_keybd behandelt nur RETURN/ENTER/ARROW_UP/TAB/ARROW_DOWN; Ziffern gehen an ob_edit und verpuffen, da alert-Buttons nicht editierbar sind)
- NewTOS: form.rs:373-378 (`b'1'..=b'9' => return n`)
- Divergenz: In NewTOS wählt Drücken von '1'/'2'/'3' den Button; in EmuTOS passiert nichts. Beobachtbar unterschiedlicher Return.
- 1:1-Fix: Ziffern-Zweig entfernen.
- Runtime-verify: yes

### AUD-P4-016: do_alert — Nicht-Ziffer als Icon-Feld ergibt „kein Icon" (EmuTOS zeichnet STOP)
- Severity: LOW (malformed-Input-Edge)
- EmuTOS: gemfmalt.c:163 (`*picnum = alert[1] - '0'` — ungeprüft; nichtnull ⇒ Icon, switch default ⇒ STOPBB, :258-270)
- NewTOS: form.rs:147-151 (nur bei `is_ascii_digit` Icon, sonst 0=kein Icon)
- Divergenz: z. B. `[ ][msg][ok]` ⇒ EmuTOS `' '-'0' = -16` ≠ 0 ⇒ STOP-Icon; NewTOS ⇒ kein Icon. (Für Ziffern 0–9 verhalten sich beide äquivalent: NewTOS clamped 3+ auf STOP, EmuTOS default = STOP.)
- 1:1-Fix: Icon = `s[g1s] - '0'` (ungeclampt) nachbilden; `iconnum!=0` ⇒ Icon, `1→NOTE/2→QUEST/sonst→STOP`.
- Runtime-verify: no

### AUD-P4-017: form_center — SHADOWED-Ausdehnung nur für Box-Typen (EmuTOS ob_sst auch G_BUTTON/G_TEXT/G_TITLE)
- Severity: LOW (Root-Typ-Edge)
- EmuTOS: gemobed.c:96-104 → gemobjop.c:42-66 (`ob_sst` liefert `th` auch für G_TITLE=1, G_TEXT/G_BOXTEXT/G_FTEXT/G_FBOXTEXT via te_thickness, G_BUTTON)
- NewTOS: form.rs:756 (`matches!(otype, G_BOX|G_IBOX|G_BOXCHAR)`)
- Divergenz: Ein SHADOWED-Root, der ein G_BUTTON/G_TEXT/G_TITLE ist, liefert bei NewTOS keine Shadow-Ausdehnung, bei EmuTOS schon. In der Praxis sind SHADOWED-Roots Boxen (OUTLINED/Shadow-Werte im Spec-High-Byte matchen exakt, form.rs:757). Rest von `form_center` ist 1:1 (gl_width/gl_hbox-Offset, OUTLINED ±3 mit Clamp, `ret=1`).
- 1:1-Fix: `th` gemäß ob_sst pro Objekttyp bestimmen (te_thickness für Text-Typen, G_TITLE=1, G_BUTTON `th-1[-EXIT][-DEFAULT]`), nicht nur Box-Spec-Byte.
- Runtime-verify: no

### AUD-P4-018: form_do — kleinere fm_do/fm_button-Lücken (kein Glocke, kein Keyboard-Flush, RBUTTON+EXIT)
- Severity: LOW
- EmuTOS: gemfmlib.c:290 `fq()` (Keyboard-Flush vor Loop); :333 `Bconout(2,0x07)` (Glocke bei Klick ins Leere/NIL); :260-261 (RBUTTON-Objekt, das auch EXIT ist, beendet den Dialog)
- NewTOS: form.rs:524-610 (`run_form`: kein `fq`; Klick ohne Treffer `hit<0` ⇒ still ignoriert, keine Glocke; RBUTTON-Zweig form.rs:583-585 prüft nach `select_radio` kein EXIT ⇒ ein RBUTTON+EXIT verlässt den Dialog nie)
- Divergenz: kein Keyboard-Flush zu Loop-Beginn; keine Glocke bei Klick ins Leere; ein RBUTTON-das-auch-EXIT-ist beendet den Dialog nicht.
- 1:1-Fix: Keyboard-Queue vor der Schleife leeren; bei `hit==NIL` `Bconout(2,0x07)`; nach RBUTTON-Selektion `(state&SELECTED)&&(flags&EXIT)⇒Exit` prüfen.
- Runtime-verify: yes

### AUD-P4-019: Alert-Icon-Bitmaps hartcodiert, nicht gegen gem.rsc (NOTEBB/QUESTBB/STOPBB) verifizierbar
- Severity: LOW (unverifiziert)
- EmuTOS: gemfmalt.c:217,260-272 (`rs_bitblk[NOTEBB/QUESTBB/STOPBB]`, 32×32 aus gem.rsc — Binärressource, kein lesbarer Header in `references/`)
- NewTOS: form.rs:30-64 (`ALERT_ICONS` 3×32 u32, hartcodiert)
- Divergenz: Die drei Icon-Bitmaps sind fest im Code; ein byteweiser Abgleich gegen die generierte gem.rsc war statisch nicht möglich (nur Binär-`.rsc` vorhanden, `mform.rsc`/`mform.def` sind Maus-Cursor-Forms, keine Alert-Icons — die im Task genannte Quelle ist irreführend). Nicht die beobachtbare Rückgabe-Kontrakt betroffen; rein visuell.
- 1:1-Fix: Icons aus der RSC/BITBLK-Quelle ableiten oder gegen ein gerendertes EmuTOS-Referenzbild pixelverifizieren.
- Runtime-verify: yes (Screendump-Vergleich)

---

## Zusammenfassung

**Counts je Severity:** BLOCKER 2 · HIGH 2 · MEDIUM 9 · LOW 6 — gesamt 19 Findings über 7 Public-Fns + Internals.

**Top-3 Blocker/Kritisch:**
1. **AUD-P4-001 (BLOCKER) form_error:** Return invertiert (`1` statt `(ret!=1)`), Eingabe als negativer statt positiver DOS-Code interpretiert, feste Icon/Strings statt EmuTOS-Fehlertabelle, fehlender `n>63`-Guard. Ein konformes Programm (inkl. EmuTOS-Desktop-Muster `form_error(2)`) erhält falschen Text UND falschen Return.
2. **AUD-P4-002 (BLOCKER) form_do:** `start_fld` (FM_START/int_in[0]) wird nie gelesen — falscher Startfokus; dokumentiertes ABI-Argument verworfen.
3. **AUD-P4-003 (HIGH) form_alert:** Enter liefert Button 1, obwohl `defbut==0` (kein Default) — EmuTOS ignoriert Enter dann. Falscher Return für legalen Eingang.

**Was getrieben wurde (statisch, Quelle-gegen-Quelle):** Grammatik-Parsing (Icon/`[..]`-Gruppen/`|`-Split/`||`/`]]`-Escaping/Zeilen-+Button-Limits), form_alert-Return (1-basiert) inkl. Enter/defbut=0 und Ziffern-Pfad, form_error Code→String→Return-Mapping + Range-Guard, fm_do-Eventloop (start_fld, Ownership, Tastatur-Nav vs. Editing, Radio, EXIT/TOUCHEXIT, Double-Click, Glocke), fm_button/fm_keybd 1:1 gegen gemsuper-Bindings (int_in/int_out-Indizes verifiziert), form_center-Zentrierung + OUTLINED/SHADOWED-Extents gegen ob_center/ob_sst, form_dial GROW/SHRINK-Richtung + FINISH-Rect, objc_edit-Cursor-Delegation (Arrow-Handling).

**Was nicht verifiziert werden konnte:** (a) exakte Button-Anzahl/Text der AL\*ERR/AL\*CRT-Alerts (Text steckt in binärer gem.rsc; Return-Divergenz von 001 folgt dennoch zwingend aus der Quelllogik `(ret!=1)`). (b) Icon-Bitmaps byteweise gegen gem.rsc (AUD-P4-019). (c) Alle Runtime-Repro-Items (kein QEMU laut Auftrag) — als `Runtime-verify: yes` markiert. (d) fm_own-Beobachtbarkeit im Multitasking-Ausbau (Menü-während-Dialog).

**Out-of-scope-Beobachtungen (nicht verfolgt):**
- `graf::animate` (graf.rs:213) nutzt WHITE-Übermalen statt XOR ⇒ `graf_movebox/growbox/shrinkbox` (72–74) korrumpieren gemusterten Hintergrund dauerhaft (ohne FMD_FINISH-Repaint). Gehört zu P-Graf.
- `object::next_editable`/`find_flag`-Stride-Logik nutzt `ObjTree::new` (caller-ABI) vs. `ObjTree::lp64` für Kernel-Trees — im form-Scope konsistent, aber die ABI-Breiten-Wahl ist Object-Manager-Kontrakt (P3).

**Cleanup-Bestätigung:** Rein statischer Audit — kein Produkt-Code geschrieben/editiert, keine Temp-Wiring/Selftests angelegt, kein Build/QEMU ausgeführt. Einziges geschriebenes Artefakt ist diese Katalog-Datei.
