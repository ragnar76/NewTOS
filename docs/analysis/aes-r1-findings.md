# AES-Runde-1 — Auswertung der beiden Hardware-Dumps und Fix-Vorschlag

Gemessen 2026-08-31. Zwei Läufe derselben 95-Fall-Tabelle
`tosdump/cases/aes-r1.tsv` (CRC `33C13C8F`, Dumper-Build `b59e`):

| Dump | Maschine | `ap_version` | `ap_id` | `ap_planes` | Auflösung |
|---|---|---|---|---|---|
| `tosdump/reference/EMUTOS14-AES.BIN` | Atari TT, EmuTOS **1.4** | `0340` | **0** | 4 | 640×480 |
| `tosdump/reference/NEWTOS-AES.BIN` | Raspberry Pi 4, NewTOS | `0340` | **1** | 16 | 1920×1080 |

Beide Dumps validieren, beide Logs sagen `all 95 cases ran`.
Reproduktion des Vergleichs, aus `tosdump/`:

```
host/build/tosdiff cases/aes-r1.tsv reference/EMUTOS14-AES.BIN reference/NEWTOS-AES.BIN
→ 95 of 95 cases differ          (exit 1)
```

Dieses Dokument ist **reine Analyse**. `tos-api/`, `kernel/` und `tos-types/`
wurden nicht angefasst.

---

## 0. Vorab: ist der Vergleich überhaupt tragfähig?

Drei Dinge mussten zuerst geklärt werden, sonst ist jede Zahl darunter wertlos.

### 0.1 Versions-Schiefstand: geflasht ist 1.4, gepinnt ist 1.4+71

Die Referenz-Quelle in `references/emutos` steht auf `1a7484ab`
(`VERSION_1_4-71-g1a7484ab`), auf dem TT lief das **1.4-Release**. Der
vollständige AES-Delta zwischen beiden:

```
git diff --stat VERSION_1_4 HEAD -- aes/ include/aesdefs.h include/aesext.h \
                                    include/obdefs.h include/sysconf.h
 aes/gemoblib.c    |  2 +-      (len > 3 Guard in just_draw's Separator-Erkennung)
 aes/gemwmlib.c    | 13 +++     (WF_OWNER in wm_get, Commit acb7f3b4, 2025-08-23)
 include/aesdefs.h |  1 +       (#define WF_OWNER 20)
```

Nur **zwei** Änderungen, davon berührt genau eine Runde 1: `WF_OWNER` (Fall 167).
`wm_calc`, `wm_get` sonst, `w_setup`, `get_pwin`, `xif`, `gsx_start`,
`gemgrlib.c`, `gemevlib.c`, `obdefs.h` sind zwischen 1.4 und dem gepinnten Baum
**quellidentisch**. Jede Zitierung unten außer der zu Fall 167 gilt für beide.

Konsequenz, die später bei den Tasks wieder auftaucht: Fall 167
(`wind_get 0 WF_OWNER`) steht heute auf MATCH, weil **beide** FALSE liefern —
EmuTOS 1.4 mangels der Funktion, NewTOS weil Handle 0 kein echtes Fenster ist.
Das ist ein Gleichstand aus zwei verschiedenen Gründen.

### 0.2 Die Box-Metriken sind auf beiden Maschinen identisch

Fall 216 (`graf_handle`, `OUT_LEN 5`) ist **MATCH**:

```
int_out = [1, 8, 16, 19, 19]      auf beiden Maschinen
          ret gl_wchar gl_hchar gl_wbox gl_hbox
```

Das ist der wichtigste Einzelbefund für die Auswertung: `gl_wbox` und `gl_hbox`
sind auf TT und Pi beide 19 (`gl_hbox = gl_hchar + 3`, `gemgraf.c:412-415`).
**Jede** `wind_calc`-Differenz unten ist damit ein Unterschied der Arithmetik,
nicht der Eingangsgrößen.

### 0.3 Die Absenz-Prüfung (Poisoning) ist sauber

Maschinell über beide Dumps geprüft:

- `contrl[0..4]` — beide Maschinen schreiben es **nie** zurück; in allen 95
  Fällen steht exakt das, was die Fall-Tabelle gelegt hat (0 Verstöße).
  Deckt sich mit `xif`, das `control` nur hineinkopiert (`gemsuper.c:435`).
- `int_in[]` — beide Maschinen mutieren keinen Eingabewert (0 Verstöße).
- `addr_in[0..2]` und `addr_out[0]` — in allen 95 Fällen auf beiden Maschinen
  **vollständig vergiftet** (`0xDEADDEAD`). Passt zu `gemsuper.c:445-446`, wo
  `addrout` ausschließlich für `RSRC_GADDR` geschrieben wird.
- Reserved-Bytes des Records — auf beiden Seiten null.

Die einzige Absenz-Verletzung, die das Poisoning findet, ist `int_out` jenseits
von `OUT_LEN` (Defekt D-R1-2).

---

## 1. Vollständige Klassifikation aller 95 Fälle

Jeder Fall ist genau einer Ursache zugeordnet; die Summe ist 95.
`global[]` ist hier ausgeklammert und in Abschnitt 6 behandelt — es unterscheidet
sich in allen 95 Fällen, ist aber innerhalb eines Dumps **konstant** und damit
*ein* Befund, nicht 95.

| Ursache | Klasse | Fälle | Anzahl |
|---|---|---|---|
| `wind_calc` non-3D statt 3D | **Defekt D-R1-1** | 68–79, 83–94, 97–110, 112–118, 120, 121, 132–137 | 53 |
| `int_out` identisch | — | 67, 80, 82, 95, 163–170, 175–178, 193, 216, 228, 229, 240, 242 | 21 |
| `OUT_LEN` ignoriert | **Defekt D-R1-2** | 129, 130, 166, 172, 173, 217, 218, 241 | 8 |
| Handle 0 kein echtes Fenster — Slider-Felder | **Defekt D-R1-3a** | 159, 160, 161, 162 | 4 |
| Handle 0 kein echtes Fenster — `wind_find` off-screen | **Defekt D-R1-3c** | 194, 195, 196, 197 | 4 |
| Auflösung (640×480 vs 1920×1080) | **Profil** | 155, 156, 157 | 3 |
| `OUT_LEN` **und** `wind_calc` | D-R1-2 + D-R1-1 | 131 | 1 |
| Handle 0 kein echtes Fenster — `WF_FXYWH` | **Defekt D-R1-3b** | 158 | 1 |

Drei Ursachen erklären **67 der 74** abweichenden Fälle. Die restlichen sieben
sind: 3 reines Profil, 4 `wind_find` (dritte Facette derselben Ursache wie 3a/3b).

---

## 2. Die Defekte

### D-R1-1 — `wind_calc` rechnet den non-3D-Zweig, das Zielprofil ist 3D

**Fälle:** 53 vollständig + Fall 131 anteilig. Größter Einzelposten der Runde.

**Beweis, und er ist ungewöhnlich sauber.** Ich habe beide Zweige von
`wm_calc` als Modell nachgerechnet und gegen alle 57 `wind_calc`-Fälle mit
`OUT_LEN ≥ 5` gehalten (`gl_wbox = gl_hbox = 19` aus Fall 216, `ADJ3DSTD = 2`
aus `include/obdefs.h:136`, also `adj_wbox = adj_hbox = 23`):

```
EmuTOS-Messung  vs. 3D-Zweig   (gemwmlib.c:1817-1840):  0 Abweichungen / 57
NewTOS-Messung  vs. non-3D     (gemwmlib.c:1841-1853):  0 Abweichungen / 57
```

Das ist keine Näherung — es sind alle 57 Fälle, inklusive der degenerierten
Rechtecke, der negativen Ursprünge, des 32000er-Rechtecks und selbst des
Poison-Falls 137 (`w`/`h` erreichen `wm_calc` als `0xDEAD`; die EmuTOS-Ausgabe
ist `-8555 / -8577` = `-8531 - 1 - 23` bzw. `-8531 - 23 - 23`, exakt der
3D-Zweig auf Müll-Eingaben). NewTOS ist eine **exakte** Reimplementierung des
falschen Zweigs.

**Ursache.**
- EmuTOS: `include/config.h:1259` — `CONF_WITH_3D_OBJECTS` ist per Default **1**;
  die Ausschaltungen bei `:310/:436/:801` gelten für ST/Lisa-artige Profile, nicht
  für das Zielprofil. `wm_calc` `aes/gemwmlib.c:1807-1867`; die 3D-Größen
  `adj_wbox`/`adj_hbox` in `init_3d`, `aes/gemwmlib.c:1310-1325`.
- NewTOS: `tos-api/src/aes/window.rs:917-935` — `borders()`, Doc-Kommentar sagt es
  selbst: *„EmuTOS `wm_calc`, non-3D path"*. `GAD` (`window.rs:131`) = 19 wird
  für beide Achsen benutzt; `calc()` `window.rs:938-952`.

**Die drei Magnituden, einzeln belegt:**

| Symptom | Δ | EmuTOS 3D | NewTOS non-3D |
|---|---|---|---|
| Titelband (`NAME`/`CLOSER`/`FULLER`), Fälle 68–70, 83–85 | 4 px | `tb += adj_hbox-1` = 22 | `tb += gl_hbox-1` = 18 |
| `MOVER` **allein**, Fälle 71, 86 | 22 px | `TGADGETS` enthält `MOVER` (`gemwmlib.h:16`) → volles Band | `NAME\|CLOSER\|FULLER` → gar kein Band |
| `INFO` allein, Fälle 72, 87 | 1 px | `tb += gl_hbox` = 19 | `tb += gl_hbox-1` = 18 |
| `SIZER` allein, Fälle 73, 88 | 4 px + 18 px | Strip-and-force: `SIZER` fällt aus beiden Leisten, wird dann der **vertikalen** zugeschlagen → nur `rb` wächst | `SIZER` schlägt auf **beide** Leisten durch |

Die vierte Zeile ist die interessanteste: der 3D-Zweig hat eine Konfliktauflösung
(`gemwmlib.c:1823-1834`), die der non-3D-Zweig gar nicht kennt. Sie erklärt die
Fälle 73, 88, 102, 103, 104, 110 zusätzlich zur Banddicke.

**Reichweite in einer echten Anwendung.** `wind_calc` ist die Funktion, mit der
jede GEM-Anwendung ihre Fenstergröße ausrechnet, bevor sie `wind_create`
aufruft — und `WF_WXYWH` liefert bei uns dieselbe Rechnung
(`window.rs:1565`, `calc(WC_WORK, …)`). Eine portierte Anwendung bekommt
also eine Arbeitsfläche, die 4 px zu hoch, 4 px zu breit und bei `MOVER`-Fenstern
22 px zu hoch ist. Für ein Fenster mit Titel *und* Leisten summiert sich das auf
`+4 / +4 / +8` gegenüber TOS4 (Fall 105: EmuTOS `176×104`, NewTOS `180×112`).

**Kopplung, die den Task bestimmt.** `borders()` speist **nur** `wind_calc` und
`WF_WXYWH`. Der gezeichnete Rahmen (`draw_frame`, `window.rs:1011ff.`) rechnet mit
`GAD-1` **eigenständig** und benutzt ebenfalls den non-3D-Gadget-Satz. Wird nur
`borders()` umgestellt, meldet die AES eine um 4 px kleinere Arbeitsfläche, als
sie zeichnet — ein 4-px-Streifen Rahmen läge dann *innerhalb* der gemeldeten
Work-Area. `borders()` und `draw_frame` müssen in **einem** Change-Set wandern.
(Das hatte AUD-P5-013 `docs/analysis/aes-audit/p5-window-graf.md:154-159` bereits
richtig gesehen; die Messung bestätigt die Lesung Zahl für Zahl.)

**Nebenbefund (nur gelesen, nicht gemessen — unsicher.)** EmuTOS ist bei
`SIZER` intern selbst inkonsistent: `wm_calc` (`gemwmlib.c:1823-1839`) zieht bei
`SIZER` allein nur `rb` zusammen, `w_bldactive` (`gemwmlib.c:598-601`) verkleinert
mit `if (havevbar || (kind & SIZER))` / `if (havehbar || (kind & SIZER))`
**beide** Achsen. Für ein `SIZER`-Fenster widersprechen sich also `wind_calc`
und die tatsächliche Work-Area in EmuTOS selbst. Runde 1 kann das nicht
entscheiden (kein `wind_create`). Beim Umsetzen von D-R1-1 gilt: `wind_calc`
folgt `wm_calc`, `draw_frame` folgt `w_bldactive` — nicht beide derselben Formel.

**Was sich nicht ändern darf:** `graf_handle` meldet die **unangepassten**
`gl_wbox`/`gl_hbox` (`gemsuper.c:287-291`), nicht `adj_*`. Fall 216 muss nach dem
Fix weiterhin `[1, 8, 16, 19, 19]` liefern. Ebenso `BAR_H`/`gl_rmenu`, das auf
`gl_hbox` steht (`gemgraf.c:427`).

---

### D-R1-2 — Der Dispatcher ignoriert `OUT_LEN`

**Fälle:** 129, 130, 166, 172, 173, 217, 218, 241 (+ 131 anteilig).

**Messung.** In jedem dieser Fälle stehen bei EmuTOS jenseits von `control[2]`
noch Poison-Wörter, bei NewTOS Werte:

| Fall | `OUT_LEN` | EmuTOS | NewTOS |
|---|---|---|---|
| 129 | 0 | `[dead ×7]` | `[1, 101, 119, 180, 112, dead, dead]` |
| 130 | 1 | `[1, dead ×6]` | `[1, 101, 119, 180, 112, dead, dead]` |
| 217 | 1 | `[1, dead ×6]` | `[1, 8, 16, 19, 19, dead, dead]` |
| 218 | 1 | `[1, dead ×6]` | `[1, 126, 342, 1, 0, dead, dead]` |
| 241 | 0 | `[dead ×7]` | `[-1, dead ×6]` |

**Ursache.**
- EmuTOS `xif`, `aes/gemsuper.c:428-448`: `RET_CODE` ist das **lokale**
  `int_out[0]`; zurückkopiert wird ausschließlich
  `if (OUT_LEN) memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD));`
  (`:443-444`). Bei `OUT_LEN == 0` erreicht **nichts** den Aufrufer, nicht einmal
  der `-1` des unbekannten Opcodes.
- NewTOS: es gibt keinen lokalen Puffer. `Aespb::intout`
  (`tos-api/src/aes/mod.rs:36`) zeigt direkt auf das Array des Aufrufers,
  `set_intout`/`set_ret` (`mod.rs:96-104`) schreiben unbedingt hinein, und alle
  223 Aufrufstellen in `tos-api/src/aes/*.rs` tun das direkt. `OUT_LEN`
  (`C_OUT_LEN`, `mod.rs:54`) ist als Konstante definiert, wird aber nirgends
  gelesen — sie trägt sogar ein `#[allow(dead_code)]`.

**Reichweite.** Zwei Wirkungen, eine davon schlimmer als sie aussieht:

1. Fall 241 zeigt die Vertragsverletzung in Reinform: der `-1` für einen
   unbekannten Opcode wird berechnet und darf den Aufrufer bei `OUT_LEN 0`
   nicht erreichen — bei uns tut er es. `docs/api/aes/_dispatcher.md` §3a hält
   genau das fest.
2. Fall 218 (`graf_mkstate`, `OUT_LEN 1`) zeigt die praktische Gefahr: NewTOS
   schreibt `[126, 342, 1, 0]` — Mausposition und Tastenzustand — in Slots, die
   der Aufrufer nicht deklariert hat. Die Fall-Tabelle hatte `graf_mkstate`
   **bewusst** auf `OUT_LEN 1` gekürzt, damit genau dieser Operator-Zustand nicht
   in den Record gerät (`cases/COVERAGE-AES.md:204`). Die Schutzmaßnahme greift
   nur, wenn die Implementierung `OUT_LEN` respektiert — was die getestete
   Implementierung nicht tut. Für eine echte Anwendung heißt das: wir schreiben in
   Speicher, den ein Aufrufer mit knapp bemessenem `intout`-Array nicht
   bereitgestellt hat. gemlib hält statisch 7 Wörter vor, eine handgeschriebene
   Bindung muss das nicht.

(= AUD-P1-009, `docs/analysis/aes-audit/p1-appl-pd-dispatch.md:139-150`. Der
Audit stufte es als *„Benign, da gemlib `aes_intout` global 7 WORDs groß hält"*
ein. Fall 218 zeigt, dass die Wirkung über „benign" hinausgeht: es ist der
Kanal, über den Operator-Zustand in eine Antwort gerät.)

---

### D-R1-3 — Handle 0 ist bei uns kein echtes Fenster (drei Symptome, eine Ursache)

**Fälle:** 159, 160, 161, 162 (Slider), 158 (`WF_FXYWH`), 194–197 (`wind_find`)
— 9 Fälle.

**Ursache, beide Seiten.**
- EmuTOS: `wm_init` macht das Desktop-Fenster zu einem **vollwertigen** Fenster.
  `aes/gemwmlib.c:1290-1294`:
  ```c
  w_setup(ppd, DESKWH, NONE);                       /* setzt VF_INUSE      */
  w_setsize(WS_CURR, DESKWH, &gl_rscreen);
  w_setsize(WS_PREV, DESKWH, &gl_rscreen);
  w_setsize(WS_FULL, DESKWH, &gl_rfull);            /* ← unter der Menüzeile */
  w_setsize(WS_WORK, DESKWH, &gl_rfull);
  ```
  `w_setup` (`gemwmlib.c:205-218`) setzt dabei auch
  `w_hslide = w_vslide = 0` und `w_hslsiz = w_vslsiz = -1`.
  `get_pwin(0)` (`gemwmlib.c:1391-1404`) liefert daher einen gültigen Deskriptor,
  und `wm_get` beantwortet für Handle 0 alles, was es für jedes Fenster
  beantwortet.
- NewTOS: `tos-api/src/aes/window.rs:147-150` — *„The desktop pseudo-window's AES
  handle … **It has no table slot**"*. `win(handle)` (`window.rs:905-911`) gibt für
  `handle < 1` `None` zurück; ein separater Zweig `None if handle == 0`
  (`window.rs:1633-1639`) beantwortet **vier** Rect-Felder und fällt sonst auf
  `set_ret(0)`.

**3a — Slider-Felder (159, 160, 161, 162).**

| Fall | Feld | EmuTOS | NewTOS |
|---|---|---|---|
| 159 | `WF_HSLIDE` | `[1, 0]` | `[0]` |
| 160 | `WF_VSLIDE` | `[1, 0]` | `[0]` |
| 161 | `WF_HSLSIZ` | `[1, -1]` | `[0]` |
| 162 | `WF_VSLSIZ` | `[1, -1]` | `[0]` |

EmuTOS `wm_get` `gemwmlib.c:1542-1553`. Die Werte `0/0/-1/-1` sind exakt die von
`w_setup` gesetzten Anfangswerte. NewTOS liefert FALSE und schreibt gar nichts.

**3b — `WF_FXYWH` (158).**

```
Fall 158  wind_get(0, WF_FXYWH)
  EmuTOS  [1, 0, 19, 640, 461]   = gl_rfull  (0, gl_hbox, gl_width, gl_height-gl_hbox)
  NewTOS  [1, 0,  0, 1920, 1080] = der ganze Bildschirm
```

`gl_rfull` wird bei `gsx_start` gesetzt, `aes/gemgraf.c:423-424` — **unbedingt**,
unabhängig davon, ob eine Menüzeile installiert ist. NewTOS:
`window.rs:1634` fasst `WF_CXYWH | WF_PXYWH | WF_FXYWH` zusammen und gibt für
alle drei `screen_rect()` zurück. `WF_CXYWH` und `WF_PXYWH` sind damit **richtig**
(EmuTOS: `gl_rscreen`, Fälle 156/157 stimmen bis auf die Auflösung), `WF_FXYWH`
ist falsch. Der richtige Wert steht bereits als `desktop_work()`
(`window.rs:2000-2004`) bereit und ist genau `gl_rfull`, weil `menu::desk_top()`
(`tos-api/src/aes/menu.rs:102-104`) unbedingt `BAR_H` liefert.

**3c — `wind_find` außerhalb des Bildschirms (194–197).**

```
Fälle 194-197  wind_find(-1,-1) / (0,-1) / (-1,0) / (32000,32000)
  EmuTOS  int_out[0] = -1        NewTOS  int_out[0] = 0
```

EmuTOS: `wm_find` ist `ob_find(gl_wtree, 0, 2, x, y)` (`gemwmlib.c:1767-1770`);
`ob_find` startet mit `lastfound = NIL` (`aes/gemoblib.c`, `NIL = -1`,
`include/obdefs.h:21`) und setzt es nur, wenn der Punkt **im** Rechteck des
ROOT-Objekts liegt — und das ROOT trägt das CURR-Rechteck des Desktops, also
`gl_rscreen`. Ein Punkt außerhalb des Bildschirms gehört niemandem.
NewTOS: `find_handle` (`window.rs:2377-2387`) startet mit `let mut best = 0i16`
— der Desktop ist bedingungslose Rückfallantwort, auch für einen Punkt, den kein
Bildschirm enthält. Fall 193 (`0,0`) stimmt auf beiden Seiten mit `0`; die
Fallunterscheidung fehlt nur am Rand.

**Warum eine Ursache.** Alle drei Symptome sind Facetten davon, dass unser
Fenstermodell ein flaches Slot-Array ohne ROOT/Desktop-Eintrag ist, während
EmuTOS' Modell ein Objektbaum ist, dessen ROOT das Desktop-Fenster **ist**.
3c ließe sich zwar separat mit einer Bildschirmgrenze reparieren; 3a und 3b nicht.

**Reichweite in einer echten Anwendung.** Jede GEM-Anwendung, die ihre
Startgeometrie aus `wind_get(0, WF_FXYWH)` holt — das ist der kanonische Weg,
„so groß wie möglich, aber unter der Menüzeile" auszurechnen — bekommt bei uns
19 Pixel zu viel und legt ihr Fenster unter die Menüzeile. `wind_find` mit `-1`
ist die Standard-Abfrage „ist der Mauszeiger überhaupt in einem Fenster"; unsere
`0` beantwortet sie mit „ja, im Desktop".

---

### D-R1-4 — `appl_init` schreibt `AESGLOBAL`-Felder, die EmuTOS nicht anfasst — und zwei Byte darüber hinaus

**Nicht aus dem Diff, sondern aus dem Lesen unseres Schreibers gegen die
EmuTOS-Struktur.** Runde 1 kann es nicht messen, weil `global[]` bewusst *nicht*
vergiftet wird (`core/runner_aes.h:33-41`: es ist lebender Zustand). Ich führe es
hier auf, weil es dieselbe Fehlerklasse ist, die dieses Verfahren finden soll.

EmuTOS' `AESGLOBAL` (`aes/gemrslib.h:15-27`) ist auf m68k **30 Byte = 15 WORDs**:

| Feld | Idx | Byte | von `appl_init` geschrieben? |
|---|---|---|---|
| `ap_version` | 0 | 0 | ja (`gemsuper.c:92`) |
| `ap_count` | 1 | 2 | ja (`:93`) |
| `ap_id` | 2 | 4 | ja (`:94`) |
| `ap_private` | 3 | 6 | ja (`:95-96`) |
| `ap_ptree` | 5 | 10 | **nein** |
| `ap_rscmem` | 7 | 14 | **nein** |
| `ap_rsclen` | 9 | 18 | **nein** |
| `ap_planes` | 10 | 20 | ja (`:97`) |
| `ap_3resv` | 11 | 22 | ja (`:98`) |
| `ap_4resv` | 13 | 26 | **nein** |
| — | 15 | 30 | **existiert nicht** |

EmuTOS lässt 14 der 30 Byte unberührt. NewTOS
(`tos-api/src/aes/appl.rs:132-161` LP64, `:172-196` ILP32) schreibt **alle** 30
Byte und zusätzlich Index 15 (`for i in 13..16 { global.add(i).write(0) }`), also
Byte 30–31.

Der Dumper deklariert sein Array als `static int16_t aes_global[15]`
(`tosdump/core/gem.c:29`) — 30 Byte. Unser `appl_init` schreibt bei **jedem** Lauf
2 Byte über das Ende dieses Arrays hinaus. Der Lauf ist nicht abgestürzt und der
Dump validiert; die Schreibzugriffe landen in dem, was der Linker daneben gelegt
hat. Es ist trotzdem ein echter Out-of-Bounds-Schreibzugriff in den Speicher der
Anwendung, und er passiert bei jeder Anwendung, die die Atari-/EmuTOS-Struktur
statt gemlibs 16-Wort-Array benutzt.

**Unsicherheit, die ich nicht auflösen konnte:** unser Doc-Kommentar
(`appl.rs:31-33`) nennt Index 13/14 `ap_maxchar`/`ap_minchar` (die AES-4.0-Namen)
und leitet aus gemlibs `AES_GLOBMAX = 16` ab, dass das Array 32 Byte groß sei.
Für einen gemlib-Aufrufer stimmt das. Für einen Aufrufer mit der
EmuTOS-/Compendium-Struktur stimmt es nicht. Welche der beiden Größen der
Vertrag ist, ist eine Entscheidung, die ich nicht treffen will — siehe
STOP-Punkte.

---

### D-R1-5 — `ap_id`: die gestartete Anwendung bekommt 1, EmuTOS gibt 0

Siehe Abschnitt 5 (Frage 1). Fällt hier nicht als Fall-Differenz an, weil `ap_id`
in `global[2]` steht und `global[]` in Abschnitt 6 behandelt wird.

### D-R1-6 — `ap_private` kommt für eine gestartete Anwendung aus dem falschen Baum

```
global[3..4]  EmuTOS 4419 (0x1143)     NewTOS 112 (0x0070)
```

- EmuTOS: `ap_private = rs_trees[DESKTOP][ROOT].ob_spec` (`gemsuper.c:95-96`).
  Der Wert steht als Konstante in der eingebauten AES-Resource:
  `aes/gem_rsc.c:551-556`, TREE 2 (`DESKTOP`), ROOT, `ob_spec = 4419L`. Der
  gemessene Wert ist **exakt** dieser eingebaute Default, dieser Lauf wurde also
  nicht durch eine `Q`-Zeile in `EMUDESK.INF` verschoben (`set_aes_background`,
  `geminit.c:635-641`, überschreibt nur das niedrigste Byte).
- NewTOS: `desktop_bg_obspec()` (`window.rs:496-510`) liest den per
  `WF_NEWDESK` registrierten Hintergrundbaum; ist keiner registriert, liefert es
  `FALLBACK_ROOT_OBSPEC = 0x0070` (`window.rs:446`). Der gemessene Wert **ist**
  dieser Fallback.

**Und das ist der eigentliche Befund.** Der Doc-Kommentar bei `window.rs:479-482`
begründet die Abweichung damit, echte GEM-Anwendungen riefen `appl_init` auf,
*nachdem* der Desktop steht, und läsen deshalb den lebenden Wert. Die Messung
widerlegt diese Prämisse: der Startpfad hebt die `WF_NEWDESK`-Registrierung vor
dem Start wieder auf (`window.rs:520-524`, zusammen mit `save_windows`), sodass
eine gestartete Anwendung **immer** den weißen Fallback sieht und **nie** das
Desktop-Muster. Für den einzigen Fall, der zählt, liefert die dokumentierte
Abweichung nicht das, was sie zu liefern behauptet.

---

## 3. Profil-Unterschiede (kein Defekt)

| Was | EmuTOS TT | NewTOS Pi | Warum kein Defekt |
|---|---|---|---|
| Fälle 155/156/157 (`WF_WXYWH`/`CXYWH`/`PXYWH` auf Handle 0) | `640×480`-abgeleitet | `1920×1080`-abgeleitet | Beide berechnen dieselbe Beziehung: CURR/PREV = ganzer Bildschirm, WORK = Bildschirm minus `gl_hbox` (auf beiden `y = 19`, weil `gl_hbox` auf beiden 19 ist). Nur die Bildschirmgröße unterscheidet sich. |
| `ap_planes` (`global[10]`) 4 vs 16 | 4 Planes | 16 (Falcon-Truecolor) | `gl_nplanes = gsx_nplanes() = INQ_TAB[4]` (`gemsuper.c:97`); 16 ist der korrekte Wert für 16-Bit-Truecolor, nicht `log2(Farben)`. |
| `ap_3resv` (`global[11..12]`) `0x0000BE04` vs `0x001180B0` | `&D` | Adresse der pd-Tabelle | Eine maschineninterne Adresse. Zwischen zwei Maschinen niemals vergleichbar. |
| Wortreihenfolge der LONG-Felder in `global[]` | m68k: High-Word zuerst | AArch64-LE: Low-Word zuerst | Ein LONG, das in einem WORD-Array liegt, hat auf beiden Maschinen eine andere Wort-Reihenfolge. Siehe Abschnitt 6. |

Die Auflösung, `ap_planes` und die LONG-Wortbreite/-reihenfolge sind **kein**
Defekt — hier stimme ich mit der Lesart des Leads überein.

---

## 4. Defekte in der Messung, die durchgekommen sind

Drei wurden vor dem Lauf verworfen (`cases/COVERAGE-AES.md:199-216`). Es sind
weitere durchgekommen.

**M1 — Fall 216 vergleicht einen Handle byte-genau.** `int_out[0]` von
`graf_handle` ist der physische VDI-Workstation-Handle. Die Fall-Tabelle sagt es
selbst (`aes-r1.tsv:205-207`: *„like every handle, may legitimately differ
between machines (spec section 5)"*) und vergleicht ihn trotzdem exakt, weil der
AES-Bereich laut `docs/TOS-REFERENCE-DUMPER.md` §6 keinen Ausnahme-Mechanismus
hat. Er steht heute nur deshalb auf MATCH, weil beide Maschinen zufällig `1`
liefern. Ändert eine Seite ihre Workstation-Reihenfolge, meldet der Fall einen
Defekt, der keiner ist. Fall 216 ist aber gleichzeitig der wichtigste Fall der
Runde (Abschnitt 0.2) — er darf nicht einfach entfallen, er braucht eine
Feldbehandlung.

**M2 — Fall 163 (`WF_TOP`) misst, welche Fenster der Operator offen hatte.**
`w_top()` (`gemwmlib.c`) liefert `gl_wtop`, sonst `DESKWH`. Auf dem TT stand
`gl_wtop` auf `NIL`, also kam `0` heraus — das heißt, der Operator hatte kein
Desktop-Fenster offen. Hätte er eines offen gehabt, wäre `1` oder `2`
herausgekommen und der Fall hätte einen Defekt gemeldet, der keiner ist. Die
`COVERAGE`-Begründung für den `wind_find`-Block (*„Only points whose answer
cannot depend on which windows happen to be open are here"*) gilt für `WF_TOP`
gerade nicht.
Dazu kommt: EmuTOS' `sh_ldapp` (`aes/gemshlib.c:622-634`) schließt vor
`dos_exec` **keine** Fenster — die Fenster des Desktops bleiben in der Liste,
während die Anwendung läuft. NewTOS räumt sie ab (`save_windows`,
`window.rs:525ff.`). Fall 163 kann diesen strukturellen Unterschied nicht sehen;
er stand nur zufällig auf Gleichstand.

**M3 — Die Fälle 155–158 können byte-genau niemals grün werden.** Sie liefern
absolute Bildschirmkoordinaten. Selbst nach dem Fix von D-R1-3b liefert Fall 158
`(0, 19, 1920, 1061)` gegen `(0, 19, 640, 461)`. Vier Fälle sind dauerhaft rot,
obwohl drei davon kein Defekt sind und der vierte nach dem Fix korrekt ist. Das
ist die schärfste Stelle: **die Erfolgsbedingung für D-R1-3b lässt sich mit dem
heutigen Werkzeug nicht als „Fall 158 wird grün" formulieren.**

**M4 — `global[]` erzeugt 95 rote Fälle aus einem einzigen Wert.** In beiden
Dumps ist `global[]` über alle 95 Records **identisch** (maschinell geprüft:
je genau eine verschiedene Belegung pro Dump). Es wird von `appl_init` einmal
gefüllt, und kein Runde-1-Opcode verändert es. Der Report zeigt denselben Befund
95-mal und begräbt darunter alle echten Unterschiede — Abschnitt 6.

**M5 — Der Schutz von Fall 218 setzt genau das voraus, was geprüft wird.**
Der Fall wurde auf `OUT_LEN 1` gekürzt, damit Operator-Zustand nicht in den
Record gerät. Weil D-R1-2 existiert, ist er trotzdem hineingeraten. Kein Fehler
der Fall-Tabelle, aber der Schutz ist nicht selbsttragend und sollte im
Vergleicher (nicht in der Tabelle) durchgesetzt werden.

**Fälle, die aus dem falschen Grund grün sind.** Kein Messdefekt, aber sie werden
beim Fixen umkippen und dürfen dem Tester nicht als Regression erscheinen:

| Fall | steht auf MATCH weil | kippt bei |
|---|---|---|
| 164 (`WF_KIND`) | Handle 0 hat keinen Slot → unser `WF_KIND`-Zweig wird nie erreicht. Für ein *echtes* Fenster liefern wir `kind`+TRUE, EmuTOS FALSE (AUD-P5-014, bewusste Abweichung D-053) | D-R1-3 |
| 167 (`WF_OWNER`) | EmuTOS **1.4** kennt die Funktion nicht (Abschnitt 0.1) und wir erreichen unseren Zweig nicht | D-R1-3 |

---

## 5. Frage 1 — Was bestimmt in EmuTOS die pid der gestarteten Anwendung?

**Antwort: die gestartete Anwendung bekommt gar keine eigene pid. Sie läuft auf
dem AESPD der Shell, und der hat pid 0.** Die Spec-Aussage ist falsch, die
Diagnose des Audits war richtig, und der Fix ist nie gelandet.

**Die Kette, jede Stufe zitiert.**

1. `appl_init` setzt `ap_id = rlr->p_pid` — `aes/gemsuper.c:94`. `rlr` ist der
   *laufende* AESPD, nicht ein für die Anwendung angelegter.
2. Die pid-Vergabe: `geminit.c:871` gibt in der Init-Schleife jedem Deskriptor
   `p_pid = i`; danach `curpid = 0; rlr->p_pid = curpid++;` (`geminit.c:875-876`)
   — der Ausgangsprozess ist **pid 0**, `curpid` steht auf 1.
3. `gl_mowner = ctl_pd = iprocess("SCRENMGR", ctlmgr)` (`geminit.c:888`) →
   `pstart` → `getpd` (`gempd.c:60-70`, `p->p_pid = curpid++`) → **pid 1**.
   Der Kommentar bei `geminit.c:885-887` sagt es wörtlich: *„it has a pid == 1"*.
   Accessories bekommen danach 2, 3, …
4. Der Shell-Ablauf `run_accs_and_desktop` → `sh_main` → `sh_ldapp` läuft auf dem
   Ausgangsprozess: `aes/gemstart.S:139-142` (`_accdesk_start`) setzt
   ausdrücklich *„use AES process 0 stack"*.
5. **Und das ist der Kern:** `sh_ldapp` startet eine normale Anwendung so
   (`aes/gemshlib.c:622-634`):
   ```c
   sh_show(fname);
   p_nameit(rlr, fname);            /* den EIGENEN AESPD umbenennen */
   p_setappdir(rlr, D.s_cmd);
   rlr->p_flags = 0;
   ret = dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn);
   ```
   Kein `pstart`, kein `getpd`, kein neuer AESPD. Die Anwendung erbt den AESPD
   der Shell, samt pid, und der wird lediglich auf ihren Dateinamen umbenannt.
   Genauso beim ROM-Desktop (`gemshlib.c:600-611`).
6. Struktureller Gegenbeweis, falls 1–5 nicht reichen: es gibt in EmuTOS
   überhaupt keinen Deskriptor für eine Anwendung.
   `NUM_PDS = (NUM_ACCS + 2)` — `aes/struct.h:36`, Kommentar
   *„acc's + ctrlpd + dos appl."* —, und `totpds = num_accs + 2`
   (`geminit.c:819`). Zwei Systemprozesse plus `NUM_ACCS` Accessories, sonst
   nichts. Der „dos appl." *ist* Prozess 0.

**Die Messung bestätigt es:** `EMUTOS14-AES.LOG` sagt `appl_init id 0` /
`ap_id 0`, und `global[2] = 0` in allen 95 Records.

**Damit ist die Spec falsch.** `docs/api/aes/_state.md:62` (*„Small integers;
**0 and 1 are system**, DAs/apps are **2+**"*), `:31-33` und `:283` behaupten
das Gegenteil. Richtig ist: **pid 0 = Shell/Desktop *und* die von ihr gestartete
Anwendung; pid 1 = SCRENMGR; pid 2+ = Desk-Accessories.** Das ist ein
Spec-Fehler, der über die Formulierung hinausgeht — er ordnet der Anwendung
eine eigene Prozessidentität zu, die es nicht gibt.

**Zur Sorge des Leads, der P1-004-Fix habe von 1:1 weggeführt: nein.**

- Der Audit hat es **richtig** diagnostiziert. `AUD-P1-004`
  (`docs/analysis/aes-audit/p1-appl-pd-dispatch.md:77-92`) schreibt wörtlich
  *„EmuTOS `gemsuper.c:94` `ap_id = rlr->p_pid`; `geminit.c:875-877` setzt
  Prozess 0 (die »dos appl« = Desktop/Shell/gestartete App) auf `p_pid = 0`"*
  und als 1:1-Fix *„pid-Vergabe an EmuTOS ausrichten (Primär-Prozess = 0)"*.
  Nur der Verweis auf `_state.md` in derselben Zeile übernimmt die falsche
  Spec-Aussage; die Analyse selbst ist korrekt.
- Der **pid-Teil ist nie umgesetzt worden.** `00-MASTER.md:30` listet P1-004 zur
  Zusammenführung in WP1, aber der WP1-Commit `8d77372` ist ausschließlich die
  Queue-Vereinheitlichung (P2-001..003). `current_pid()`
  (`tos-api/src/aes/appl.rs:203-211`) hat seit dem Ursprungs-Commit `b963ac9`
  dieselbe Semantik: `slot_for(bp) + 1`, bzw. `1` ohne BasePage. Beide Zweige
  ergeben für die gemessene Anwendung `1` — genau der gemessene Wert. Es gibt
  also nichts rückgängig zu machen; die Aufgabe steht noch aus.
- Die zweite Hälfte von P1-004 (Kollision Desktop ↔ erste EL0-App, beide auf
  pid 1) besteht damit ebenfalls unverändert: `register_process`
  (`tos-api/src/aes/mod.rs:284-288`) vergibt `slot + 1`, `current_pid` ohne
  BasePage vergibt `1`. Runde 1 kann das nicht sehen (nur ein Prozess gemessen);
  der Befund steht rein auf der Quelle.

---

## 6. Frage 2 — Was steckt in der `global[]`-Differenz?

`global[]` unterscheidet sich in allen 95 Fällen, aber es ist **ein** Befund:
innerhalb jedes Dumps ist die Belegung über alle 95 Records identisch (maschinell
geprüft). Zerlegt:

```
Idx    Feld        Breite   EmuTOS         NewTOS         Urteil
0      ap_version  WORD     0x0340         0x0340         gleich
1      ap_count    WORD     1              1              gleich
2      ap_id       WORD     0              1              DEFEKT  (D-R1-5)
3..4   ap_private  LONG     0x1143 (4419)  0x0070 (112)   DEFEKT  (D-R1-6)
5..6   ap_ptree    LONG     0              0              gleich
7..8   ap_rscmem   LONG     0              0              gleich
9      ap_rsclen   WORD     0              0              gleich
10     ap_planes   WORD     4              16             PROFIL  (Truecolor)
11..12 ap_3resv    LONG     0x0000BE04     0x001180B0     UNVERGLEICHBAR (Adresse)
13..14 ap_4resv    LONG     0              0              gleich
```

**Wie viel davon ist LP64-vs-m68k-Layout?** Der Breitenunterschied selbst
**nicht** — beide Seiten liefern hier dieselbe Feldbelegung an denselben
Byte-Offsets, weil unser Schreiber (`appl.rs:132-161`) gemlibs Offsets
6/10/14/22 exakt trifft. Was der ABI-Unterschied erzeugt, ist die
**Wortreihenfolge innerhalb eines LONG**: m68k legt ein LONG mit dem High-Word
zuerst ab, AArch64-LE mit dem Low-Word zuerst. Der Record serialisiert
`global[]` wortweise. Also gilt für **jedes LONG-Feld mit einem Wert ≠ 0**, dass
`global[n]` und `global[n+1]` gegenüber dem m68k-Dump vertauscht sind:

```
ap_3resv:  EmuTOS  global[11]=0x0000  global[12]=0xBE04   → LONG 0x0000BE04
           NewTOS  global[11]=0x80B0  global[12]=0x0011   → LONG 0x001180B0
```

Von den vier LONG-Feldern ist das bei zwei sichtbar (`ap_private`, `ap_3resv`);
bei `ap_ptree`/`ap_rscmem`/`ap_4resv` fällt es nicht auf, weil sie null sind.
Für eine Anwendung ist die Reihenfolge korrekt: ein nativer LP64-gemlib-Leser
macht `*(long*)&aes_global[3]` und liest genau diese Bytes wieder richtig ein.

**Ist der Vergleich strukturell sinnlos, wie er heute steht? Ja — teilweise.**

- Wortweise verglichen sind `global[3..8]`, `global[11..14]` **bedeutungslos**:
  sie sind Hälften von LONGs, deren Wortreihenfolge per ABI verschieden ist.
- `global[10]` (`ap_planes`) und `global[11..12]` (`ap_3resv`) garantieren, dass
  `global[]` zwischen diesen beiden Maschinen **niemals** gleich wird. Solange
  es je Record verglichen wird, sind alle 95 Fälle dauerhaft rot.
- Verwertbar ist genau das, was direkt WORD-typisiert ist: `ap_version`,
  `ap_count`, `ap_id`, `ap_rsclen`, `ap_planes`. Genau dort steckt auch der
  einzige mit dem heutigen Werkzeug sichtbare Defekt (`ap_id`).

**Versteckt sich ein echter Defekt hinter dem Rauschen? Ja, zwei:**
`ap_id` (D-R1-5) und `ap_private` (D-R1-6). Beide waren nur zu finden, weil ich
die LONGs von Hand in der jeweils richtigen Wortreihenfolge zusammengesetzt habe
— der Report zeigt sie als sechs unzusammenhängende Wort-Differenzen.

**Was den Vergleich sinnvoll machen würde** (Task T-R1-7, Werkzeug):

1. `global[]` **einmal pro Dump** vergleichen und melden, nicht 95-mal. Es ist
   per Konstruktion über alle Records konstant, solange kein Fall es verändert
   (`rsrc_load`/`rsrc_free` täten es — Runde 2). Der Vergleicher kann das prüfen
   statt annehmen: „konstant über alle Records?" ist eine Zeile.
2. `global[]` als **`AESGLOBAL` mit benannten Feldern** vergleichen, nicht als
   15 Wörter: `ap_private` als LONG in der Wortreihenfolge der jeweiligen
   Maschine, `ap_version`/`ap_count`/`ap_id`/`ap_rsclen`/`ap_planes` als WORD.
   Die Byte-Reihenfolge des Records ist ohnehin schon normiert; die
   **Wort**-Reihenfolge innerhalb eines LONGs ist es nicht, und genau das fehlt.
   Die Information dafür ist vorhanden: der Dump-Header trägt keine
   Endianness-Kennung, aber der Bereich und die ABI der Maschine sind bekannt —
   sauberer wäre ein Header-Flag. Das berührt das Record-Format, das ich nicht
   ändern darf; es ist eine Entscheidung des Leads (STOP-Punkt).
3. Eine Feldklasse für `ap_3resv` (Adresse → nicht vergleichen) und `ap_planes`
   (maschinenabhängig → melden, nicht als Fehler zählen).

---

## 7. Der Vorschlag, als Tasks

Vorab, weil es die Reihenfolge bestimmt: **T-R1-1 ist die einzige Aufgabe, die
unter anderen Symptomen sitzt.** Sie ändert nicht *was* eine Funktion antwortet,
sondern *wie viel davon* den Aufrufer erreicht — und kontaminiert damit die
Lesbarkeit von fünf Fällen (166, 172, 173, 217, 218), die eigentlich etwas
anderes messen. Alles andere ist voneinander unabhängig.

### T-R1-1 — Dispatcher: `OUT_LEN` respektieren  · zuerst · keine Abhängigkeit

**Was.** `xif`s Kopie-zurück-Regel nachbauen: `int_out` wird in einem lokalen
Puffer aufgebaut und am Ende von `dispatch` mit genau `control[2]` Wörtern in das
Array des Aufrufers kopiert; bei `OUT_LEN == 0` gar nicht.

**Dateien.** `tos-api/src/aes/mod.rs` — und **nur** dort: `Aespb.intout` zeigt auf
einen lokalen `[i16; O_SIZE=7]` (`include/aesdefs.h:123`), `dispatch` kopiert
zurück. Die 223 `set_intout`/`set_ret`-Aufrufstellen in den elf Manager-Modulen
bleiben unangetastet. Kein Kernel-Code betroffen (geprüft: `kernel/` ruft keine
`fn(&Aespb)`-Handler direkt, nur Hilfsfunktionen).

**Zwei Fallstricke, die beim Umsetzen beißen werden.**
1. `dispatch` hat für die Opcodes 20–25 frühe `return`s
   (`mod.rs:176-181`, die `evnt_*`-Waits, die einen `Switch` propagieren). Ein
   Copy-back am Funktionsende wird von ihnen übersprungen. Der Rückkopier-Pfad
   muss vor **jedem** Verlassen laufen.
2. EmuTOS klemmt `OUT_LEN` **nicht** auf `O_SIZE` (`gemsuper.c:443-444`); ein
   `OUT_LEN > 7` läse dort über den lokalen Puffer hinaus. In Rust wäre das ein
   Out-of-Bounds-Lesen. → **STOP-Punkt für den Lead** (siehe Abschnitt 8, S3).

**Erfolgsbedingung.** `host/build/tosdiff cases/aes-r1.tsv …` meldet für die
Fälle **129, 130, 166, 172, 173, 217, 218, 241** keine `int_out`-Differenz mehr
(8 Fälle von abweichend auf übereinstimmend). Fall 131 verliert seine
`OUT_LEN`-Komponente, bleibt aber bis T-R1-2 wegen `int_out[2]` abweichend.
Kein anderer Fall darf sich ändern.

---

### T-R1-2 — `wind_calc` + `draw_frame` auf den 3D-Zweig  · unabhängig von T-R1-1

**Was.** `borders()` auf `wm_calc`s 3D-Arithmetik umstellen (`adj_wbox`/
`adj_hbox` = `gl_wbox`/`gl_hbox` + 2·`ADJ3DSTD`, `TGADGETS` statt
`NAME|CLOSER|FULLER`, `INFO` mit vollem `gl_hbox`, Strip-and-force für `SIZER`)
**und im selben Change-Set** `draw_frame` auf die zugehörige `w_bldactive`-3D-
Geometrie. Dabei `gl_wbox` und `gl_hbox` als getrennte Größen einführen statt des
einen `GAD` — auf beiden gemessenen Maschinen sind sie gleich (19), in
`gsx_start` (`gemgraf.c:412-415`) sind sie es nicht zwangsläufig.

**Dateien.** `tos-api/src/aes/window.rs` (`GAD`/`borders`/`calc`/`draw_frame`
und die Slider-/Gadget-Zeichenpfade, die heute `GAD` benutzen).
`tos-api/src/aes/graf.rs:454-463` **nicht** ändern — `graf_handle` meldet die
unangepassten Werte. `tos-api/src/aes/menu.rs` (`BAR_H`) **nicht** ändern —
`gl_rmenu` steht auf `gl_hbox`.

**Blast radius außerhalb von `tos-api`.** Der Desktop ruft `wind_calc` über den
AES-Trap (`kernel/src/desk/mod.rs:425-433`) und cached das Ergebnis in
`full_work_w` (`kernel/src/desk/global.rs:233-236`), zieht die neue Zahl also von
selbst. Der Arena-Deckel `MAX_WIN_ITEMS = 350` bleibt ausreichend: die
Arbeitsfläche wird durch 3D **kleiner** (bei 1920×1080 ca. 1900×1005 → 1896×996),
also passen weniger Zellen hinein, nicht mehr. Der Doc-Kommentar
`kernel/src/desk/global.rs:55-59` nennt die alten Zahlen und wird stale.

**Erfolgsbedingung.** Die **53** Fälle 68–79, 83–94, 97–110, 112–118, 120, 121,
132–137 gehen in `int_out` von abweichend auf übereinstimmend; Fall 131
zusätzlich (zusammen mit T-R1-1). Fall **216 muss auf MATCH bleiben**
(`[1, 8, 16, 19, 19]`) — das ist die Kontrollbedingung dafür, dass die
3D-Anpassung intern geblieben ist. Fälle 67, 80, 82, 95 (Kind 0 / `HOTCLOSE`)
müssen ebenfalls auf MATCH bleiben.

**Zusätzlich, nicht per Dump prüfbar:** die Fensterrahmen des Desktops werden
sichtbar anders. Braucht einen QMP-Screendump-Vergleich beim Tester.

---

### T-R1-3 — Handle 0 als echtes Fenster  · unabhängig · nach T-R1-1 einplanen

**Was.** Den Desktop-Slot einführen, wie `wm_init` ihn anlegt: Slot für Handle 0
mit `VF_INUSE`, `kind = NONE`, `CURR = PREV = gl_rscreen`,
`FULL = WORK = gl_rfull`, `hslide = vslide = 0`, `hslsiz = vslsiz = -1`
(`gemwmlib.c:1290-1294` + `:205-218`). Damit fallen 3a und 3b von selbst weg.
3c zusätzlich: `find_handle` startet mit `-1` und liefert 0 nur, wenn der Punkt
im CURR-Rechteck des Desktop-Fensters liegt.

**Dateien.** `tos-api/src/aes/window.rs`: `MAX_WINDOWS`/`win()`/`slots()`
(Handle→Slot-Abbildung um Handle 0 erweitern), der `None if handle == 0`-Zweig in
`wind_get` (`:1633-1639`) entfällt, `find_handle` (`:2377-2387`).

**Warum ein Task statt drei.** 3a und 3b brauchen beide den Desktop-Slot; 3c
wäre separat machbar, gehört aber zu derselben Ursache und zum selben Test.
Wenn feinere Granularität gewünscht ist, ist 3c abtrennbar — 3a und 3b nicht.

**Erfolgsbedingung, und hier wird es unangenehm.**
- **Sauber prüfbar:** Fälle 159, 160, 161, 162 und 194, 195, 196, 197 gehen von
  abweichend auf übereinstimmend (8 Fälle).
- **Nicht mit dem heutigen Werkzeug prüfbar:** Fall 158 (`WF_FXYWH`). Nach dem Fix
  liefert NewTOS `(0, 19, 1920, 1061)`, EmuTOS `(0, 19, 640, 461)` — der Fall
  bleibt rot, obwohl er korrekt ist. Die prüfbare Aussage ist eine *Beziehung*:
  `WF_FXYWH.y == gl_hbox` und `WF_FXYWH.h == WF_CXYWH.h - gl_hbox`, beides
  fallübergreifend (216 liefert `gl_hbox`, 156 liefert `WF_CXYWH`). Der
  Vergleicher kann solche Invarianten heute nicht ausdrücken. → **T-R1-7 nötig,
  sonst nur manuell nachweisbar.**
- **Zwei Fälle kippen von grün auf rot, und das ist erwartet, keine Regression:**
  Fall 164 (`WF_KIND` — legt AUD-P5-014 / die bewusste Abweichung D-053 offen)
  und Fall 167 (`WF_OWNER` — legt den 1.4-Versionsschiefstand offen,
  Abschnitt 0.1). Beide müssen vor dem Tester-Lauf als erwartet angemeldet
  werden, sonst meldet er zwei Regressionen. Für 164 braucht der Lead eine
  Entscheidung (D-053 halten oder auf EmuTOS-FALSE zurück), für 167 ebenfalls
  (auf einem Post-1.4-EmuTOS neu dumpen oder 167 als bekannten Schiefstand
  führen).

---

### T-R1-4 — `appl_init` schreibt nur die Felder, die EmuTOS schreibt  · unabhängig · klein

**Was.** `write_lp64`/`write_ilp32` auf die sechs Felder aus `gemsuper.c:92-98`
reduzieren: `ap_version`, `ap_count`, `ap_id`, `ap_private`, `ap_planes`,
`ap_3resv`. `ap_ptree`/`ap_rscmem`/`ap_rsclen`/`ap_4resv` bleiben unberührt,
Index 15 wird gar nicht mehr angefasst — damit ist auch der 2-Byte-Überlauf über
`AESGLOBAL` hinaus weg.

**Dateien.** `tos-api/src/aes/appl.rs:132-196` (nur diese zwei Funktionen und der
Modul-Kommentar `:10-63`).

**Abhängigkeit.** Braucht die Antwort auf STOP-Punkt S1 (ist der Vertrag
EmuTOS' 30-Byte-`AESGLOBAL` oder gemlibs 32-Byte-`aes_global[16]`?).

**Erfolgsbedingung.** Kein Fall der Runde 1 ändert sich (alle betroffenen Slots
sind auf beiden Seiten null). Nachweisbar wird es erst mit einem Runde-2-Fall,
der `global[]` vor `appl_init` vergiftet — der Dumper vergiftet `global[]`
bewusst nicht (`core/runner_aes.h:33-41`), also braucht das einen dedizierten
Prelude-Fall. **Task ohne heute prüfbare Erfolgsbedingung** — bis dahin ist der
Nachweis das Code-Review gegen `gemsuper.c:92-98` plus ein Unit-Test, der die
30 Byte vorher/nachher vergleicht.

---

### T-R1-5 — `ap_id`: Primärprozess auf 0  · **braucht zuerst eine Entscheidung**

**Was.** `current_pid()` so umstellen, dass Shell/Desktop und die von ihr
gestartete Anwendung pid **0** melden, SCRENMGR-Äquivalent 1, DAs 2+.
Gleichzeitig die noch offene zweite Hälfte von AUD-P1-004 (Kollision auf pid 1).

**Dateien.** `tos-api/src/aes/appl.rs:203-211` (`current_pid`),
`tos-api/src/aes/mod.rs:280-289` (`register_process`), `tos-api/src/aes/pd.rs`
(Deskriptortabelle, `open`/`close`), `kernel/src/launch.rs:89`.
Zusätzlich `docs/api/aes/_state.md:31-33, 62, 283` — die Spec ist falsch und muss
**vor** dem Code korrigiert werden (Regel 5: Spec vor Code).

**Warum es eine Entscheidung braucht.** EmuTOS' Modell ist, dass die Anwendung den
AESPD der Shell *übernimmt* — dieselbe pid, dieselben Fenster, dieselbe
Nachrichtenschlange. NewTOS' Startpfad ist genau umgekehrt gebaut:
`save_windows`/`restore_windows` (`window.rs:525ff.`) setzt voraus, dass Desktop
und Anwendung getrennte Fensterwelten haben, und `ADR` „Single-tasking: app owns
full screen" hat das bewusst so entschieden. `ap_id = 0` für beide ist mit
getrennten Fensterwelten vereinbar; „die Anwendung *ist* der Desktop-Prozess"
ist es nicht. Wie weit die 1:1-Übernahme gehen soll, kann ich nicht entscheiden.
→ **STOP-Punkt S2.**

**Erfolgsbedingung.** `global[2] == 0` im NewTOS-Dump — mit T-R1-7 (`global[]`
einmal pro Dump, benannte Felder) direkt ablesbar, ohne T-R1-7 nur von Hand aus
dem Record. Zusätzlich: Log-Zeile `appl_init id 0` im NewTOS-Lauf.

---

### T-R1-6 — `ap_private` aus der richtigen Quelle  · **braucht zuerst eine Entscheidung**

**Was.** EmuTOS liefert `rs_trees[DESKTOP][ROOT].ob_spec` aus seiner **eingebauten
AES-Resource** (`gem_rsc.c:551-556`, konstant 4419), nicht aus dem per
`WF_NEWDESK` registrierten Desktop-Baum. Unsere AES hat keine eingebaute
DESKTOP-Resource. Entweder wir bekommen eine (dann ist der Wert auch beim Start
einer Anwendung stabil), oder wir dokumentieren die Abweichung neu — die
bisherige Begründung (`window.rs:479-482`) ist durch die Messung widerlegt.
→ **STOP-Punkt S4.**

**Dateien.** `tos-api/src/aes/window.rs:441-510`, `tos-api/src/aes/appl.rs`
(nur die Quelle des Feldes).

**Erfolgsbedingung.** `global[3..4]` als LONG gelesen == 4419 im NewTOS-Dump,
falls die eingebaute Resource kommt. Braucht T-R1-7 für die Wortreihenfolge.

---

### T-R1-7 — Werkzeug: `global[]` und maschinenabhängige Felder vergleichbar machen · **kein Produktcode** · unabhängig, aber Voraussetzung für die Nachweise von T-R1-3b/5/6

**Was.**
1. `global[]` einmal pro Dump vergleichen statt 95-mal, mit vorheriger Prüfung
   „über alle Records konstant?".
2. `global[]` als `AESGLOBAL` mit **benannten** Feldern vergleichen, LONG-Felder
   als LONG in der Wortreihenfolge der jeweiligen Maschine.
3. Feldklassen für den AES-Bereich: `ap_3resv` und `graf_handle`s
   `int_out[0]` (Fall 216) als „nicht vergleichen", `ap_planes` und die
   Auflösungs-Rects als „melden, nicht als Fehler zählen".
4. Fallübergreifende Invarianten (`WF_FXYWH.y == graf_handle.gl_hbox`), damit
   T-R1-3b überhaupt eine Erfolgsbedingung bekommen kann.

**Dateien.** `tosdump/host/tosdiff.c`, `tosdump/host/casefile.c` (Feldtabelle),
`tosdump/host/Makefile`/Tests. **Nicht** die Dumps, nicht `aes-r1.tsv`, nicht
`core/record.*` (Record-Format).

**Der Konflikt, den der Lead entscheiden muss.**
`docs/TOS-REFERENCE-DUMPER.md` §6 sagt ausdrücklich: *„Comparison is exact for
every byte of these two areas. No exception mechanism — if you find yourself
wanting one, the case is wrong, not the comparator."* Für den VDI trägt diese
Prämisse. Für die AES trägt sie **nicht**: `ap_planes`, `ap_3resv`, die
`wind_get`-Rects und `graf_handle`s Handle sind maschinenabhängig, und `global[]`
kommt gar nicht aus einem Fall, sondern aus dem Record-Format — die Regel „dann
ist der Fall falsch" hat dort nichts, woran sie greifen könnte. Es sind zwei
Wege denkbar:
- **(a) Fälle bereinigen, Vergleicher unangetastet:** 155–158, 163, 216 aus
  Runde 2 verbannen und durch maschinenunabhängige Formulierungen ersetzen. Löst
  `global[]` **nicht**, und kostet Fall 216 — den Fall, der diese ganze Auswertung
  erst tragfähig gemacht hat.
- **(b) minimale Feldklassen im Vergleicher**, wie oben. Widerspricht §6 im
  Wortlaut und braucht deshalb die ausdrückliche Zustimmung des Leads plus eine
  Korrektur von §6.
Meine Empfehlung ist (b), beschränkt auf den AES-Bereich, und §6 entsprechend
präzisieren („der VDI-Bereich ist byte-exakt; der AES-Bereich enthält
maschinenabhängige Felder und braucht eine Feldklasse"). Ich habe das **nicht**
umgesetzt. → **STOP-Punkt S5.**

---

### T-R1-8 — Runde-2-Fälle, die diese Runde als fehlend nachgewiesen hat · **kein Produktcode**

Nicht als Fix, sondern als Lücke: folgende Aussagen konnte Runde 1 nicht treffen.

| Was | Warum Runde 1 es nicht kann | Was Runde 2 braucht |
|---|---|---|
| T-R1-4 (`appl_init`-Schreibmenge) | `global[]` wird bewusst nicht vergiftet | Prelude-Fall, der `global[]` vor `appl_init` vergiftet |
| AUD-P5-014 (`WF_KIND` auf einem **echten** Fenster) | Runde 1 hat kein `wind_create` | `wind_create`-Prelude + `wind_get(h, WF_KIND)` |
| AUD-P5-012 (`WF_SCREEN`-Puffer) | Adresse ist unvergleichbar; mit `OUT_LEN 1` unsichtbar | grundsätzlich nicht byte-exakt vergleichbar — nur Länge, und die ist konfigurationsabhängig |
| P-AES-3 (Gadget-Index-Clamp) | war schon vor dem Lauf als „Runde 2" markiert | `WF_COLOR`/`WF_DCOLOR` mit `IN_LEN 3` |
| `wm_calc` vs. `w_bldactive` bei `SIZER` | kein `wind_create` | `wind_create(SIZER)` + `WF_WXYWH` gegen `wind_calc` |
| `IN_LEN > I_SIZE` mit einem Handler, der die oberen Slots liest | kein Handler in Runde 1 tut das | eigener Fall |

---

### Parallelisierbarkeit

- **Seriell erzwungen:** nichts. Es gibt keine echte Code-Abhängigkeit zwischen
  den Tasks.
- **Empfohlene Reihenfolge:** T-R1-1 **zuerst** — es ist die einzige Änderung, die
  beeinflusst, was die anderen Fälle überhaupt zeigen, und sie fasst den
  Dispatcher an, durch den alles andere läuft. Danach ist die Reihenfolge frei.
- **Echt parallel möglich:** T-R1-2 (`window.rs` Geometrie + Zeichnen),
  T-R1-4 (`appl.rs` Schreibmenge) und T-R1-7 (`tosdump/host/`) berühren
  disjunkte Dateien.
- **Nicht parallel zu T-R1-2:** T-R1-3 — beide fassen `window.rs` an, T-R1-3
  zusätzlich das Fenster-Slot-Modell, auf dem T-R1-2s `draw_frame` sitzt.
  Nacheinander, T-R1-2 zuerst (kleinere Fläche im Slot-Modell).
- **Blockiert bis zur Entscheidung:** T-R1-5 (S2), T-R1-6 (S4), T-R1-7 (S5),
  T-R1-4 (S1).

---

## 8. STOP-Punkte — Entscheidungen, die ich nicht treffen will

**S1 — Wie groß ist `AESGLOBAL` vertraglich?** EmuTOS' Struktur
(`gemrslib.h:15-27`) ist 30 Byte; gemlibs `short aes_global[AES_GLOBMAX=16]` ist
32. Unser Schreiber setzt 32 voraus und schreibt bei einem 30-Byte-Aufrufer über
das Ende. Betrifft T-R1-4.

**S2 — Wie weit geht „die Anwendung erbt den AESPD der Shell"?** `ap_id = 0` ist
belegt und unstrittig. Ob NewTOS darüber hinaus EmuTOS' Modell übernimmt (die
Anwendung *ist* der Desktop-Prozess, erbt dessen Fenster und Schlange) steht im
Widerspruch zum bewussten Single-Tasking-Handoff mit
`save_windows`/`restore_windows`. Betrifft T-R1-5.

**S3 — `OUT_LEN > O_SIZE`: EmuTOS' unbegrenzte Kopie nachbauen oder klemmen?**
EmuTOS kopiert `OUT_LEN` Wörter aus einem 7-Wort-Stack-Puffer, ohne zu prüfen.
Das 1:1 nachzubauen wäre in Rust ein Out-of-Bounds-Lesen. Klemmen auf 7 ist die
einzige speichersichere Option und eine bewusste Abweichung. Nicht messbar
(Runde 1 hat maximal `OUT_LEN 5`, und ein Fall mit `OUT_LEN > 7` verglich
Stack-Müll, was die Tabellenregel ausdrücklich verbietet). Betrifft T-R1-1.

**S4 — Bekommt unsere AES eine eingebaute DESKTOP-Resource?** Nur damit hat
`ap_private` eine EmuTOS-treue Quelle. Betrifft T-R1-6.

**S5 — Darf der AES-Bereich Feldklassen bekommen?** Widerspricht
`docs/TOS-REFERENCE-DUMPER.md` §6 im Wortlaut, ist aber die einzige Art, wie
`global[]` und die maschinenabhängigen Felder jemals eine Erfolgsbedingung
bekommen. Betrifft T-R1-7 und damit die Nachweise von T-R1-3b, T-R1-5, T-R1-6.

**S6 — Fall 167 / der 1.4-Schiefstand.** Nach T-R1-3 weicht Fall 167 ab, weil das
geflashte EmuTOS 1.4 `WF_OWNER` nicht kennt, der gepinnte Referenzbaum aber
schon — und wir haben gegen den gepinnten Baum implementiert. Entweder auf einem
Post-1.4-EmuTOS neu dumpen oder 167 als bekannten Schiefstand führen.

---

## 9. Abgleich mit der Vorab-Lesart des Leads

| | Urteil | Anmerkung |
|---|---|---|
| **F1** `OUT_LEN` ignoriert, Fälle 129/130/131, 217, 218, 241, eine Ursache im Trap-Marshalling | **bestätigt** | Vollständig ist die Liste **129, 130, 131, 166, 172, 173, 217, 218, 241** — 166, 172 und 173 fehlten. „Eine Ursache im Marshalling" stimmt exakt: es gibt bei uns keinen lokalen Puffer, `set_intout` schreibt direkt in das Array des Aufrufers. |
| **F2** `wind_calc` non-3D; 56 von 60 Fällen; drei Magnituden 4 px / 22 px / 1 px mit den drei Quellstellen | **bestätigt, mit einer Korrektur und einer Ergänzung** | Die drei Magnituden und ihre Quellstellen sind Zeile für Zeile richtig. Die **Zählung** ist 53 vollständig + 1 anteilig (131) von 61 `wind_calc`-Fällen; 67, 80, 82, 95 stimmen bereits überein (Kind 0 und `HOTCLOSE` fügen in beiden Zweigen nichts hinzu). Ergänzung: es gibt eine **vierte** Magnitude, die Strip-and-force-Konfliktauflösung für `SIZER` (`gemwmlib.c:1823-1834`), die im non-3D-Zweig gar nicht existiert — sie erklärt die Fälle 73, 88, 102, 103, 104, 110 zusätzlich. |
| **F3** `wind_get(0)` beantwortet `WF_HSLIDE`/`VSLIDE`/`HSLSIZ`/`VSLSIZ` nicht; EmuTOS 0/0/−1/−1 mit Rückgabe 1 | **bestätigt** | Werte und Rückgabecode exakt. |
| **F4** `WF_FXYWH` liefert den ganzen Bildschirm, `WF_WXYWH` zieht die Menüzeile korrekt ab | **bestätigt** | |
| **F5** `wind_find` off-screen liefert 0 statt −1 | **bestätigt** | |
| **F3/F4/F5 als drei getrennte Befunde** | **widerspreche ich** | Es ist **eine** Ursache: Handle 0 ist bei uns kein Tabelleneintrag (`window.rs:147-150`), während EmuTOS ihn in `wm_init` als vollwertiges Fenster anlegt (`gemwmlib.c:1290-1294`). Genau das ändert den Task-Zuschnitt: ein Work Package statt drei, und die drei Symptome verschwinden zusammen. |
| Auflösung, `ap_planes`, `global[]`-Zeigerbreite als Profil, nicht als Defekt | **bestätigt, aber unvollständig** | Richtig für die Auflösung und `ap_planes`. Bei `global[]` ist es nicht die *Zeigerbreite* — die Offsets stimmen auf beiden Seiten — sondern die **Wortreihenfolge innerhalb eines LONG**. Und hinter diesem Rauschen stecken **zwei echte Defekte**: `ap_id` (0 vs 1) und `ap_private` (4419 vs 112). |

**Was in der Vorab-Lesart fehlte:**
- `ap_private` = 112 statt 4419 (D-R1-6) — und dass die dokumentierte Begründung
  für diese Abweichung durch die Messung widerlegt ist.
- `appl_init` schreibt Felder, die EmuTOS nicht anfasst, und zwei Byte über
  `AESGLOBAL` hinaus (D-R1-4).
- Der Versionsschiefstand 1.4 vs. `1a7484ab` und seine Folge für Fall 167.
- Dass Fall 216 (identische Box-Metriken) die Voraussetzung dafür ist, die
  `wind_calc`-Differenz überhaupt als Arithmetik-Differenz lesen zu dürfen.
- Vier Messdefekte (M1 Fall 216 als Handle, M2 Fall 163 als Operator-Zustand,
  M3 Fälle 155–158 dauerhaft rot, M4 `global[]` 95-fach) und zwei Fälle, die aus
  dem falschen Grund grün sind (164, 167).
- Dass die Erfolgsbedingung für den `WF_FXYWH`-Fix mit dem heutigen Werkzeug
  **nicht** formulierbar ist.

---

## 10. Was ich nicht bestimmen konnte

- **Ob die `EMUDESK.INF` des TT eine `Q`-Zeile hatte.** Der gemessene
  `ap_private` = 4419 ist exakt der eingebaute Default aus `gem_rsc.c:555`, also
  war dieser Lauf nicht verschoben. Ob ein anderer TT-Lauf einen anderen Wert
  liefern würde, kann ich von hier nicht sagen — das Feld ist grundsätzlich
  operator-verschiebbar (unterstes Byte).
- **Die EmuTOS-interne `SIZER`-Inkonsistenz** zwischen `wm_calc` und
  `w_bldactive` ist nur **gelesen**, nicht gemessen (Runde 1 hat kein
  `wind_create`). Ich führe sie als unsicher.
- **Ob die pid-Kollision Desktop ↔ EL0-App** (zweite Hälfte von AUD-P1-004)
  heute tatsächlich zuschlägt: die Quelle sagt ja (`current_pid` liefert in
  beiden Zweigen 1), gemessen ist nur ein einziger Prozess.
- **Welche der beiden `AESGLOBAL`-Größen** (30 oder 32 Byte) der Vertrag ist —
  siehe S1.
