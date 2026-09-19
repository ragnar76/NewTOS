# AES-Handler, die weniger `int_out`-Wörter schreiben als `OUT_LEN` verlangt — Inventar

Datum: 2026-09-19 · Ticket: [`TASK/A27.md`](../../TASK/A27.md) (P2, Bewertung
einer bewussten Abweichung) · Nur Dokumentation und ein Regressionstest, kein
Produktcode. Die Festlegung selbst steht in
[`docs/COMPATIBILITY.md`](../COMPATIBILITY.md), Abschnitt „Sanctioned
deviations", Eintrag *„An AES handler that writes fewer than `OUT_LEN` words
delivers 0 in the unwritten words"*.

Quellenstände: EmuTOS `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (Primärquelle, Track A);
gemlib `sdk/gemlib/gemlib-0.44.0/` (die Bindung, die unsere portierten
Anwendungen benutzen — sie sagt, was eine Anwendung *anfordert*, nicht, was
die AES *liefert*); NewTOS Arbeitsbaum auf `desk-rebuild`, HEAD `dee18d1`.
Alle Zeilenangaben beziehen sich auf diese Stände. Aussagen ohne
`Datei:Zeile` sind als **Schluss** gekennzeichnet.

---

## 1. Der Mechanismus

EmuTOS' Dispatcher `xif` hält `int_out` als **dispatch-lokales, nicht
initialisiertes** Stapelfeld und kopiert danach **unbedingt** `OUT_LEN`
Wörter zurück — ohne Rücksicht darauf, wie viele davon der Handler
beschrieben hat:

```c
/* aes/gemsuper.c:428-448 */
static void xif(AESPB *pcrys_blk)
{
    WORD    control[C_SIZE];
    WORD    int_in[I_SIZE];
    WORD    int_out[O_SIZE];                       /* :432 — auto, uninitialisiert */
    LONG    addr_in[AI_SIZE];
    ...
    RET_CODE = crysbind(OP_CODE, (AESGLOBAL *)pcrys_blk->global, control, int_in, int_out,
                                addr_in);          /* :441-442 — RET_CODE = int_out[0] */

    if (OUT_LEN)
        memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD));   /* :444-445 */
```

Folgen, alle drei aus diesen Zeilen:

1. Innerhalb von `OUT_LEN` behält der Aufrufer **nie** seinen eigenen Wert —
   jedes der `OUT_LEN` Wörter wird überschrieben (`:445`).
2. Ein Wort, das der Handler **nicht** beschrieben hat, trägt den Inhalt, den
   der Supervisorstapel an dieser Stelle gerade hatte — in der Praxis den Rest
   des vorigen AES-Aufrufs desselben Prozesses (siehe § 4, die Werte sind
   wiedererkennbar). Das ist **kein Soll**: `auto`-Speicher, den EmuTOS
   nirgends initialisiert.
3. `RET_CODE` ist immer beschrieben, weil `crysbind` mit `ret = TRUE`
   startet (`gemsuper.c:85`) und jedes Ergebnis nach `int_out[0]` geht (`:441`).

NewTOS hält dieselben vier Felder dispatch-lokal, **nullt** sie aber
(`tos-api/src/aes/mod.rs:400-402`) und kopiert wie `xif` genau
`min(OUT_LEN, O_SIZE)` Wörter zurück (`mod.rs:215-222`,
`copy_intout_back`). Ein Handler, der weniger schreibt, liefert also bei uns
**0**, bei EmuTOS Stapelinhalt. Genau diese Differenz ist Gegenstand des
Tickets; sie wird **nicht** beseitigt, sondern belegt und festgelegt.

---

## 2. Methode

Drei Sweeps mit `/usr/bin/grep`, kein Raten:

* **Was Anwendungen anfordern:** `AES_PARAMS(opcode, nintin, nintout, …)` in
  allen `sdk/gemlib/gemlib-0.44.0/a_*.c` (die `x_*`/`pdlg_*`-Bindungen sind
  MagiC/XaAES-Erweiterungen mit Opcodes, die unser Dispatcher nicht kennt;
  sie fallen auf den `default`-Arm, § 3 letzte Zeile). Bindungen ohne das
  Makro (`a_wind_get.c`, `a_wind_get_int.c`, `a_wind_get_ptr.c`) legen
  `aes_control[2]` von Hand: `a_wind_get.c:351` = **5**.
* **Was EmuTOS schreibt:** `crysbind` (`aes/gemsuper.c:77-421`) mit den
  `int_out`-Makros aus `include/aesdefs.h` (`RET_CODE` :140, `EV_MX..EV_BRET`
  :194-199, `FM_XC..FM_HC` :324-327, `FM_ONXTOB/FM_OCHAR` :333-334,
  `OB_XOFF/OB_YOFF` :367-368, `OB_ODX` :374, `OB_O1/OB_O2` :383-384,
  `GR_O1/GR_O2` :399-400, `GR_WCHAR..GR_HBOX` :413-416, `GR_MX..GR_KSTATE`
  :418-421, `FS_BUTTON` :465, `WM_WCOX..WM_WCOH` :489-492, `WM_OX..WM_OH`
  :509-512), und für jeden Handler mit Ausgabezeiger der Handler selbst
  (`wm_get`, `ev_multi`, `ob_sysvar`, `fs_input`, `w_owns`, …).
* **Was NewTOS schreibt:** alle `set_intout(i, …)` / `set_ret(…)` in
  `tos-api/src/aes/*.rs`, je Handler und je Pfad.

Gezählt wird die Zahl der beschriebenen Wörter **einschließlich**
`int_out[0]`. „Unterschreibend" heißt: geschriebene Wörter < `OUT_LEN` der
gemlib-Bindung, auf mindestens einem erreichbaren Pfad.

---

## 3. Das Inventar

Alle 79 Opcodes, die `crysbind` in `tos-api/src/aes/mod.rs:540-626` kennt.
Opcodes, deren gemlib-Bindung `OUT_LEN 1` deklariert und deren Handler nur
`RET_CODE` schreibt, sind in einer Sammelzeile zusammengefasst — dort kann
nichts unterschreiben.

| Opcode | Name | `OUT_LEN` (gemlib) | geschrieben, EmuTOS | geschrieben, NewTOS | unterschreibend |
|---:|---|---:|---|---|---|
| 10–15, 17, 19 | `appl_init/read/write/find/tplay/trecord/yield/exit` | 1 | 1 (`RET_CODE`; `appl_init` schreibt zusätzlich `global[]`, nicht `int_out`) | 1 | nein |
| 20 | `evnt_keybd` | 1 | 1 | 1 | nein |
| 21 | `evnt_button` | 5 | 5 — `ev_rets` `rets[0..3]` (`gemevlib.c:42-57`, aus `ev_button` `:91`) | 5 (`event.rs` `fill_rets`) | nein |
| 22 | `evnt_mouse` | 5 | 5 (`ev_mouse` `:126-127`) | 5 | nein |
| 23, 24 | `evnt_mesag`, `evnt_timer` | 1 | 1 | 1 | nein |
| 25 | `evnt_multi` | **7** | **5 oder 6 oder 7**: `[1..4]` immer (`ev_rets` `:268`); `[5]` (`EV_KRET`) **nur** wenn `MU_KEYBD` feuert (`:179`, `:275`); `[6]` (`EV_BRET`) **nur** wenn `MU_BUTTON` feuert (`:193`, `:200`, `:277`) | 7 — `[5]`/`[6]` immer, 0 wenn nicht gefeuert (`event.rs:644-645`, Init `:445-446`) | **EmuTOS ja** (bedingt), NewTOS nein — Ergebnis beim Aufrufer identisch (0) |
| 26 | `evnt_dclick` | 1 | 1 | 1 | nein |
| 30–39 | `menu_*` | 1 | 1 | 1 | nein |
| 40–43, 45, 47 | `objc_add/delete/draw/find/order/change` | 1 | 1 | 1 | nein |
| 44 | `objc_offset` | 3 | 3 (`ob_offset` `gemoblib.c:289-293`, schreibt beide immer) | 3; auf dem NewTOS-eigenen Schutzpfad (Baum NULL / `obj < 0`, `object.rs:1110-1112`) nur 1 | nein (Schutzpfad: NewTOS-Ergänzung, EmuTOS würde durch NULL schreiben) |
| 46 | `objc_edit` | 2 | 2 — `crysbind` kopiert `OB_ODX = OB_IDX` **vor** dem Aufruf (`gemsuper.c:225`) | 2 (`object.rs:1478-1484`, beide Pfade) | nein — aber die r4-Tabelle deklariert **3** und misst dort Stapel, § 4 |
| 48 | `objc_sysvar` | **3** | **SET (`mode != 0`): 1** (`ob_sysvar` `gemoblib.c:1355-1390` schreibt `out1/out2` nie); GET `LK3DIND/LK3DACT/AD3DVALUE`: 3; GET `INDBUTCOL/ACTBUTCOL/BACKGRCOL`: **2** (`:1405-1411`, nur `*out1`); GET unbekanntes `which`: **1** (`:1418`) | SET: 1 (`object.rs:1332-1370`); GET: **immer 3** — `[2]` = 0 bei den Einwort-Selektoren, `[1..2]` = 0 bei unbekanntem `which` (`:1372-1380`) | **ja**, SET-Modus auf beiden Seiten (`[1..2]`); GET: EmuTOS ja, NewTOS nein (0 explizit) |
| 50 | `form_do` | 1 (`a_form_do.c`); gemlib-`form_xdo` (MagiC): **2** | 1 | 1 | **ja** unter `form_xdo` (`x_form_xdo.c:34`, liest `[1]` als `lastcrsr`) — beide Seiten |
| 51–53 | `form_dial/alert/error` | 1 | 1 | 1 | nein |
| 54 | `form_center` | 5 | 5 (`ob_center` `gemobed.c:105`, `r_set` schreibt vier) | 5; Schutzpfad Baum NULL: 1 (`form.rs:970-976`) | nein |
| 55 | `form_keybd` | 3 | 3 — `FM_OCHAR = FM_ICHAR; FM_ONXTOB = FM_INXTOB` vor dem Aufruf (`gemsuper.c:256-257`) | 3 | nein |
| 56 | `form_button` | 2 | 2 (`fm_button` `gemfmlib.c:267`, kein früher Ausstieg) | 2 | nein |
| 70 | `graf_rubbox` | 3 | 3 (`gr_rubwind` `gemgrlib.c:217-218`) | 3 | nein |
| 71 | `graf_dragbox` | 3 | 3 (`:265-266`) | 3 | nein |
| 72–76 | `graf_mbox/growbox/shrinkbox/watchbox/slidebox` | 1 | 1 | 1 | nein |
| 77 | `graf_handle` | 5 (`a_graf_handle.c`); gemlib-`graf_xhandle`: **6** | 5 (`gemsuper.c:288-292`) | 5 (`graf.rs:563-573`) | **ja** unter `graf_xhandle` (`a_graf_xhandle.c:46`, liest `[5]` als `device`) — beide Seiten; **gemessen** Fall 283, § 4 |
| 78 | `graf_mouse` | 1 | 1 | 1 | nein |
| 79 | `graf_mkstate` | 5 | 5 (`gr_mkstate` `:460-463`) | 5 | nein |
| 80–82 | `scrp_*` | 1 | 1 | 1 | nein |
| 90, 91 | `fsel_input`, `fsel_exinput` | 2 | 2 auf dem Erfolgspfad (`fs_input` `gemfslib.c:922`); **1** auf den frühen `return FALSE` (`pipath == NULL` `:630-631`, kein Speicher `:658-661`) | 2 auf jedem Pfad (`fsel.rs:557-570`, `[1]` = 0 explizit) | **EmuTOS ja** (Fehlerpfade), NewTOS nein — Ergebnis 0 identisch |
| 100 | `wind_create` | 1 (`a_wind_create.c`); gemlib-`wind_xcreate` (XaAES): **5** | 1 | 1 | **ja** unter `wind_xcreate` (`a_wind_xcreate.c:79`, liest `[1..4]`) — beide Seiten |
| 101–103 | `wind_open/close/delete` | 1 | 1 | 1 | nein |
| 104 | `wind_get` — **je Feld**, `OUT_LEN 5` (`a_wind_get.c:351`; `a_wind_sget.c`: 1) | 5 | | | |
| | `WF_WXYWH/CXYWH/PXYWH/FXYWH` (4–7) | 5 | 5 (`w_getsize` `gemwmlib.c:1595-1596`) | 5 | nein |
| | `WF_HSLIDE/VSLIDE/HSLSIZ/VSLSIZ` (8/9/15/16) | 5 | **2** (`poutwds[0]` `:1542-1551`) | 2 (`scalar_out` `window.rs:2614-2617`) | **ja**, `[2..4]` — beide Seiten; **gemessen** Fälle 415/418/433/436, § 4 |
| | `WF_TOP` (10) | 5 | **2** (`:1553-1554`) | 2 (`:2676-2679`) | **ja**, `[2..4]` — beide Seiten; gemessen Fall 421 |
| | `WF_FIRSTXYWH/NEXTXYWH` (11/12), Stück gefunden | 5 | 5 (`w_owns` `:1234`, `rc_copy` + `rc_intersect`) | 5 (`Owns::Visible`, `:2648-2653`) | nein |
| | `WF_FIRSTXYWH/NEXTXYWH`, Listenende **P2** (Schleife betreten, kein Schnitt) | 5 | 5 (`rc_intersect` schreibt vier auch bei FALSE, `util/rectfunc.c:72-75`; dann `:1240`) | 5 (`Owns::Exhausted`, `:2654-2659`) | nein |
| | `WF_FIRSTXYWH/NEXTXYWH`, Listenende **P1** (Schleife nie betreten) | 5 | **3** — `RET_CODE` und `g_w = g_h = 0` (`:1240`, die einzige Anweisung nach der Schleife) | 3 (`Owns::Empty`, `:2664-2667`) | **ja**, `[1..2]` — beide Seiten; **gemessen** r2b 127/133/204 (A15), r4 452, § 4 |
| | `WF_SCREEN` (17) | 5 | 5 (`gsx_mret` `:1564`, zwei LONGs) | **1** — FALSE, nichts geschrieben (`:2713-2716`) | eigene, bereits benannte Abweichung (A54, `COMPATIBILITY.md`) — nicht Gegenstand von A27 |
| | `WF_COLOR` (18) | 5 | **3** — `RET_CODE`, `poutwds[1..2]` = `int_out[2..3]` (`:1569-1570`); **`int_out[1]` und `[4]` nie** | 3 (`:2751-2757`, `set_intout(2, …)`, `set_intout(3, …)`) | **ja**, `[1]` und `[4]` — beide Seiten |
| | `WF_DCOLOR` (19) | 5 | **3** — `poutwds[1..2]` (`:1574-1575`) | 3 (`:2722-2727`) | **ja**, `[1]` und `[4]` — beide Seiten |
| | `WF_OWNER` (20) | 5 | 5 (`:1580-1588`) | 5 (`:2761-2770`) | nein |
| | ungültiges Handle (außer 10/17/19) oder Feld ohne Arm (`WF_KIND` 1, `WF_NAME` 2, `WF_INFO` 3, `WF_NEWDESK` 14, 0, 13, ≥ 21) | 5 | **1** — `return FALSE` vor dem Schalter (`:1521-1523`) bzw. `default:` (`:1591-1592`) | 1 (`:2636-2638`, `:2792-2798`) | **ja**, `[1..4]` — beide Seiten; gemessen Fälle 336/397/400/427/430/445/448 |
| 105 | `wind_set` | 1 (alle `a_wind_set*.c`); gemlib-`wind_xset_grect` (XaAES): **5** | 1 | 1 | **ja** unter `wind_xset_grect` (`a_wind_xset_grect.c:27`, liest `[1..4]`) — beide Seiten |
| 106, 107 | `wind_find`, `wind_update` | 1 | 1 | 1 | nein |
| 108 | `wind_calc` | 5 | 5 (`wm_calc` `:1807-1859`, vier Ausgabezeiger, kein früher Ausstieg) | 5 (`window.rs:3130-3134`) | nein |
| 109 | `wind_new` | **0** (`a_wind_new.c:25`) | 1 (`ret = TRUE`) — wird nie kopiert | 1 | nein (nichts wird kopiert) |
| 110–114 | `rsrc_*` | 1 | 1 (`rsrc_gaddr`: Adresse über `addrout[0]`, `gemsuper.c:446-447`) | 1 | nein |
| 120–127 | `shel_*` | 1 | 1 | 1 | nein |
| sonst | `default` → `-1` (`gemsuper.c:406-417`; `mod.rs:633`) | was die fremde Bindung deklariert: `appl_getinfo` 5, `appl_search` 3, `appl_getcicon` 3, `graf_multirubber` 3, `fslx_*`, `pdlg_*`, `x_*` … | 1 (`ret = -1`) | 1 | **ja**, sobald die Bindung `OUT_LEN > 1` deklariert — beide Seiten. gemlib's `appl_getinfo` (`a_appl_getinfo.c:362-370`) löst den Trap bei `ap_version < 0x400` ohne `?AGI` gar nicht aus; andere Bindungen (`appl_search` 18) schon |

### Zusammenfassung

* **Auf beiden Seiten unterschreibend, unter der gemlib-Standardbindung
  (der Kern des Tickets) — 2 Opcodes plus der `default`-Arm:** `wind_get`
  (104) in sechs Feldklassen (`WF_HSLIDE/VSLIDE/HSLSIZ/VSLSIZ`, `WF_TOP`,
  `WF_COLOR`, `WF_DCOLOR`, Listenende P1, Absage), `objc_sysvar` (48) im
  SET-Modus (gemlib deklariert 3 in beiden Modi, `a_objc_sysvar.c:71`, und
  liest `[1..2]` immer, `:83-87`), und der `default`-Arm für jeden unbekannten
  Opcode, dessen Bindung `OUT_LEN > 1` deklariert.
* **Auf beiden Seiten unterschreibend, nur unter Erweiterungsbindungen — 4
  Opcodes:** `graf_handle` (77) unter `graf_xhandle` (`OUT_LEN 6`),
  `wind_create` (100) unter `wind_xcreate` (5), `wind_set` (105) unter
  `wind_xset_grect` (5), `form_do` (50) unter `form_xdo` (2). Unter ihrer
  Standardbindung schreiben alle vier voll.
* **Nur bei EmuTOS unterschreibend, NewTOS schreibt das Wort explizit als 0
  — 3 Opcodes:** `evnt_multi` (25) `[5]`/`[6]` ohne gefeuertes
  `MU_KEYBD`/`MU_BUTTON`, `fsel_input`/`fsel_exinput` (90/91) `[1]` auf den
  frühen Fehlerpfaden; dazu `objc_sysvar` GET mit Einwort- oder unbekanntem
  Selektor (oben schon gezählt). Beim Aufrufer kommt auf NewTOS in beiden
  Varianten 0 an — ob aus dem genullten Puffer oder aus dem expliziten
  `set_intout(…, 0)` ist von außen nicht unterscheidbar. Die Festlegung in
  `COMPATIBILITY.md` deckt beide.
* **Vollständig schreibend, kein Befund:** die übrigen **70** Opcodes
  (79 − 2 − 4 − 3).
* **Nicht Gegenstand:** `WF_SCREEN` (A54, eigene Abweichung); die
  NewTOS-eigenen Schutzpfade bei `objc_offset`/`form_center` (EmuTOS hat dort
  keinen Pfad, sondern einen NULL-Zugriff).

**RET_CODE**: Jeder NewTOS-Handler setzt `int_out[0]` auf jedem
Rückkehrpfad — per `grep set_ret` je Handler geprüft, nicht Zeile für Zeile
bewiesen. EmuTOS' Vorbelegung `ret = TRUE` (`gemsuper.c:85`) hat bei uns keine
Entsprechung; ein Handler, der `set_ret` vergäße, lieferte 0 statt 1. Kein
Befund, aber der Punkt, an dem eine spätere Regression zuerst auffiele.

---

## 4. Messbelege — der Stapelinhalt ist wiedererkennbar

Die Rohwörter unten sind mit einem eigenen Recordleser direkt aus den BIN
gelesen (Layout `tosdump/core/tosdump.h:114-125`, `int_out` bei Offset 76,
big-endian), nicht aus den Diff-Dateien übernommen. Poison `0xDEAD` steht
dort, wo `OUT_LEN` endet — auf beiden Maschinen identisch, was die
Kopiergrenze selbst bestätigt.

**`measurement-2026-09-12-aes-r4/` (Tabelle `aes-r4-subfunctions.tsv`, CRC
`227422B2`; EmuTOS 1.4 auf dem TT via Hatari gegen NewTOS QEMU raspi4b):**

| Fall | Aufruf | EmuTOS `int_out[0..OUT_LEN)` | NewTOS | woher der EmuTOS-Rest stammt |
|---:|---|---|---|---|
| 283 | `graf_handle`, `OUT_LEN 7` | `1, 8, 16, 19, 19, `**`0, 1`** | `1, 8, 16, 19, 19, `**`0, 0`** | Stapel; `[0..4]` stimmen überein |
| 415 / 418 | `wind_get(1, WF_HSLIDE/VSLIDE)`, `OUT_LEN 5` | `1, 0, `**`20, 300, 200`** | `1, 0, `**`0, 0, 0`** | das `WF_FXYWH`-Rechteck des Falls davor (Fall 412) |
| 421 | `wind_get(1, WF_TOP)` | `1, 1, `**`20, 300, 200`** | `1, 1, `**`0, 0, 0`** | dito |
| 433 / 436 | `wind_get(1, WF_HSLSIZ/VSLSIZ)` | `1, -1, `**`63, 296, 194`** | `1, -1, `**`0, 0, 0`** | das `WF_FIRSTXYWH`-Stück `31, 63, 296, 194` (Fall 424) |
| 452 | `wind_get(1, WF_NEXTXYWH)` am Ende einer Einstück-Liste (P1) | `1, `**`31, 63`**`, 0, 0` | `1, `**`0, 0`**`, 0, 0` | dasselbe Stück — `g_x`/`g_y` sind der Rest von Fall 451 |
| 336 / 397 / 400 / 445 / 448 | `wind_get` mit Absage (`FALSE`) | `0, `**`0, 0x0218, 0, 0xA000`** bzw. **`0x0100, 0x0218, 0, 0xA000`** | `0, 0, 0, 0, 0` | die `WF_SCREEN`-Antwort `0x01000218` / `40960` (Fälle 331/439) |
| 568–574 | `objc_edit`, `OUT_LEN 3` (Tabelle über gemlib's 2 hinaus) | `1, 7, `**`63`** | `1, 7, `**`0`** | wieder die 63 aus dem Rechteck |

**`measurement-2026-09-09/run3-pi4-rlist/` gegen `tosdump/reference/EMUTOS14-AES-r2brlist.BIN`**
(Tabelle `aes-r2b-rlist.tsv`, CRC `B4CC4A5A`, echter TT): die drei
Listenende-Fälle 127, 133, 204 liefern `1, 31, 63, 0, 0` bzw. `1, 0, 19, 0, 0`
— zwei verschiedene Wertepaare aus einem Poison-Muster in einem Lauf, auf der
Pi `1, 0, 0, 0, 0`. Vollständig hergeleitet in
[`TASK/A15-STATUS-UNFINISHED.MD`](../../TASK/A15-STATUS-UNFINISHED.MD), als
Erratum zum Tabellenkopf in
[`tosdump/cases/ERRATA.md`](../../tosdump/cases/ERRATA.md).

**Was die Tabellen der Runden 1, 2, m05, m06, m07 zeigen:** dort ist
`OUT_LEN` je Fall auf die tatsächlich geschriebenen Wörter zugeschnitten
(`WF_HSLIDE` mit `OUT_LEN 2`, `objc_sysvar` SET mit 1, GET-Einwort mit 2,
`evnt_multi` mit 5) — die Fälle 159–163, 173, 217 (r1), 90–98, 107–118 (m06),
63–72 (m07) stimmen Wort für Wort überein. Der Befund tritt also **genau
dort** auf, wo eine Tabelle mehr Wörter anfordert, als der Handler schreibt,
und die r4-Einordnung hat diese 23 Fälle bereits als „Schublade S3-a — kein
Soll" geführt
([`aes-r4-subfunktionen-einordnung-2026-09-12.md`](aes-r4-subfunktionen-einordnung-2026-09-12.md),
§ 4). Dieses Inventar liefert die Regel dahinter, je Opcode.

---

## 5. Was die Bindung selbst erwartet

gemlib **nullt** vor dem Trap genau die Wörter vor, von denen es weiß, dass
eine AES sie womöglich nicht beschreibt:

* `a_wind_get.c:355-358`: *„this line is required for WF_FIRSTXYWH and
  WF_NEXTXYWH because lot of programmers doesn't verify the return value and
  espect W or H will be 0"* — `aes_intout[3] = aes_intout[4] = 0;`
* `a_graf_xhandle.c:48-49`: *„initialise aes_intout[5] for AES that doesn't
  support this extension"* — `aes_intout[5] = 0;`

Unter EmuTOS ist diese Vorbelegung **wirkungslos**: `xif` überschreibt alle
`OUT_LEN` Wörter aus seinem Stapelfeld (`gemsuper.c:445`), die vorgelegte 0
kommt nie beim Aufrufer an — was ankommt, ist Stapel, der in Fall 283 an
`[5]` zufällig 0 und an `[6]` 1 war, und bei den Fällen 415–436 die
Rechteckwörter des vorigen Aufrufs. NewTOS liefert dort, was die Bindung
vorlegen wollte. Die 0 ist also nicht
nur die schutzseitig richtige Wahl, sondern die, die die Client-Bibliothek
für den Fall „Wort nicht beschrieben" selbst vorsieht.

Wie konforme Anwendungen die bedingten Wörter lesen, zeigt QED:
`if (event & MU_KEYBD)` vor jedem Zugriff auf `kreturn`
(`sdk/apps/qed/src/aktion.c:148`); TeraDesk ebenso
(`sdk/apps/teradesk/library/xdialog/xdevent.c:179`). Ein Programm, das das
Wort ohne diese Prüfung liest, ist auf EmuTOS bereits von der
Aufrufhistorie abhängig.

---

## 6. Folgerung

Die Abweichung ist real, eng umrissen (§ 3), nur in absichtlich
überdeklarierten Messfällen sichtbar (§ 4), und die Richtung „0" ist die,
die die Bindung selbst wählt (§ 5). Sie wird als benannte Abweichung in
[`docs/COMPATIBILITY.md`](../COMPATIBILITY.md) geführt; der Kommentar an
`tos-api/src/aes/mod.rs:400-402` verweist dorthin. Die Spec-Aussage D-095
(„`int_out[1]` … reads back unchanged") in
[`docs/api/aes/window-graphics.md`](../api/aes/window-graphics.md) war für
beide Seiten falsch — innerhalb `OUT_LEN` behält der Aufrufer nie seinen Wert
— und ist korrigiert; ebenso die A18-Formulierung zum Listenende P1 („the
caller's buffer content unchanged"), die derselben Fehlannahme aufsaß.
Regressionstest: `tos-api/tests/aes_intout_underwrite.rs`.

Nicht Teil dieses Tickets, festgehalten: (a) `objc_sysvar` GET mit
Einwort-Selektor schreibt bei uns `[2]` = 0 **explizit**, EmuTOS lässt es
stehen — beim Aufrufer identisch, aber ein zweiter Ort, an dem die Regel
„0" entsteht (siehe A15-Kompensationsprüfung); (b) dieselbe Klasse für
`evnt_multi` `[5]`/`[6]` und `fsel_*` `[1]`; (c) die Fälle in
`aes-r4-subfunctions.tsv`, die mehr `OUT_LEN` deklarieren als gemlib
(568–574 mit 3 statt 2, 283 mit 7), bleiben als Messbeleg stehen — ein
Vergleichsharness, das r4 einmal nachspielt, muss die Wörter aus § 3 selbst
benennen, wie es `tos-api/tests/aes_reference/harness.rs` für `WF_OWNER`
bereits tut.
