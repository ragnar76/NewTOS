# QED: ein getipptes Zeichen erscheint nicht auf dem Schirm

> Stand 2026-09-09. Branch `desk-rebuild`. Belege: die drei Läufe in diesem
> Verzeichnis, die Dumps unter `tosdump/reference/`, QEDs Quelle unter
> `sdk/apps/qed/src/`, cflib unter `sdk/cflib/cflib-21/`.
>
> Diese Datei hält den **Befundstand** fest, nicht eine Ursache. Wo eine
> Hypothese widerlegt ist, steht das dabei; wo etwas nicht gemessen ist, steht
> das ebenfalls dabei. Nichts hier ist eine Reparaturanweisung.

---

## 1. Das Symptom

Auf realer Pi 4 tippt der Benutzer in QEDs Editorfenster ein `a`. Das Zeichen
erscheint **nicht**. Der Textcursor rückt trotzdem um **eine Zeichenzelle** vor.

Aufgenommen mit `kernel/src/dbg_qedtrace.rs` (temporärer EL0-Syscall-Tracer,
im Tracker als **F1 „revert before merge"** geführt), drei Läufe:

| Datei | Was sie hält |
|---|---|
| `run1-keystroke-cycle-only.log` | nur der Ereigniszyklus nach dem Tastendruck |
| `run2-four-cycles.log` | derselbe Zyklus plus die drei folgenden |
| `run3-geometry-prologue.log` | zusätzlich die Fenstergeometrie beim Öffnen |

Alle drei zeigen nach `evnt_multi -> 1` (`fired=0x0001` = MU_KEYBD,
`key=0x1e61` = Scancode 0x1e / ASCII 0x61, `ks=0x0`) **ausschließlich**:

```
wind_update, graf_mkstate, wind_get(11), vs_clip(24,61)-(26,78),
    vswr_mode 3, vr_recfl, vswr_mode 2, wind_get(12), wind_update
graf_mouse
wind_update, wind_get(11), vs_clip(32,61)-(34,78),
    vswr_mode 3, vr_recfl, vswr_mode 2, wind_get(12), wind_update
```

Ein 2×17-Rechteck, XOR-invertiert, an alter und dann an neuer Position: der
**Textcursor**. Kein `v_gtext`. Die folgenden Zyklen sind `fired=0x0020`
(MU_TIMER) und wiederholen nur das Cursorblinken — insbesondere kommt **kein**
`WM_REDRAW` (MU_MESAG, 0x0010) hinterher.

---

## 2. Der Pfad, aus QEDs Quelle

Taste bis Pixel. Jede Zeile ist eine Fundstelle, keine Vermutung.

```
evnt_multi
 └ handle_key                                    event.c:288-299
    ├ to_makro(kstate, kreturn)                  :288
    ├ sleep_mouse()                              :290      → graf_mouse
    ├ window = winlist_top()                     :292      QEDs eigene Liste,
    │                                                      KEIN AES-Aufruf
    └ key_window(window, kstate, kreturn)        :298
       └ w->key != NULL && !ICONIFIED && !SHADED window.c:1123
          └ wi_key                               edit.c:913
             ├ clear_info(t_ptr)                 :917
             ├ edit_key(t_ptr, window, ks, kr)   :920  → tasten.c:708
             │   └ Zweig „keine Funktionstaste"  tasten.c:1207-1221
             │      ├ cursor_visible(window,…)   :1210
             │      ├ if (ascii_code)            :1212
             │      │   └ char_insert(t_ptr, c)  :1214  → tasten.c:360
             │      └ else return FALSE          :1219
             └ restore_edit()                    :922   NUR wenn edit_key TRUE
                └ restore_edit                   edit.c:2095
                   ├ if (!chg_anz) return        :2103
                   ├ hide_mouse()                :2105  → graf_mouse
                   ├ set_sliders(…)              :2125,:2132 → wind_set
                   ├ free_for_draw(window)       :2138  → window.c:267
                   ├   ja  → restore_indesk      :2139
                   │         ├ set_clip(TRUE,&w->work)  edit.c:2026
                   │         └ LINE_CHANGE → line_out   :2043
                   │            └ str_out        ausgabe.c:638
                   │               └ out_s       ausgabe.c:443
                   │                  └ v_gtext  ausgabe.c:455
                   └   nein → restore_offdesk    :2141  → edit.c:1959
```

Zwei Tore stehen vor jedem gezeichneten Buchstaben: `edit_key` muss TRUE
liefern, und `chg_anz` muss von `make_chg` (edit.c:1862) gefüllt sein.

---

## 3. Was der Trace davon beweist

**`char_insert` ist gelaufen.** `cursor_visible` (tasten.c:1210) und
`char_insert` (:1214) stehen im **selben** `if`-Zweig derselben Funktion. Der
Cursor wandert im Trace von x=24 auf x=32 — eine Zeichenzelle bei
`font_wcell = 8`. Also hat QED das Zeichen in den Textpuffer eingefügt und
`edit_key` hat TRUE zurückgegeben; `wi_key` (edit.c:920-923) ruft dann
`restore_edit()`.

**Was der Trace nicht beweist.** `restore_indesk` beginnt jeden Fall mit
`set_clip(TRUE, &(window->work))` (edit.c:2026), also ein `vs_clip` auf
(24,61,640,944). Ein solches steht in keinem der drei Läufe — aber
`dbg_qedtrace.rs` **filtert**: das `geometry`-Prädikat
(`dbg_qedtrace.rs:299`) hält nur ausgewählte AES-Aufrufe im Prolog-Puffer, und
`record_vdi` greift nur für GEM-Selektor 115. Aus einem gefilterten Textlog ist
„der Aufruf fehlt" nicht von „der Aufruf wurde nicht geloggt" zu trennen.

**Offen ist damit genau eine Frage:** an welcher der vier Verzweigungen in
`restore_edit` (edit.c:2103 / :2125 / :2138 / :2043) der Weg endet.

---

## 4. Widerlegt: die Geometrie-Hypothese

Der Kopf von `dbg_qedtrace.rs:74-84` nennt als Verdacht, `free_for_draw`
(`window.c:267`) scheitere an der Fenstergeometrie und QED verschiebe die
Ausgabe „auf einen Redraw, der nie kommt".

`run3-geometry-prologue.log` misst das Gegenteil:

```
wind_calc: wtype=0 kind=0x4fff in=(24,60,640,944)  out=(23,18,664,1009)
wind_calc: wtype=1 kind=0x4fff in=(23,19,664,1009) out=(24,61,640,944)
wind_open:  handle=1 xywh=(23,19,664,1009)
wind_get(handle=1, field=11) -> (24,61,640,944)      WF_FIRSTXYWH
wind_get(handle=1, field=12) -> (0,0,0,0)            WF_NEXTXYWH = Listenende
```

`free_for_draw` vergleicht ausschließlich `g_w` und `g_h`:

```c
if (rc_first(w->handle, &w->work, &draw))
    if (w->work.g_w == draw.g_w && w->work.g_h == draw.g_h)
        return TRUE;
```

640 == 640 und 944 == 944. Die Funktion liefert **TRUE**, QED darf direkt
zeichnen. Der Weg über `restore_offdesk` wird nicht genommen, und die
Hypothese trägt nicht.

(Nebenher: der Tracer beschriftet `field=11` als `WF_WORKXYWH`. Feld 11 ist
`WF_FIRSTXYWH`, Feld 4 wäre `WF_WORKXYWH`. Die Zahlen im Log stimmen, die
Beschriftung nicht.)

---

## 5. Die Messlage auf genau diesem Pfad

Gefahren mit `tosdump/host/build/tosdiff` über die vorhandenen Dumps.

### 5.1 Was stimmt

`vs_clip` (129), `vswr_mode` (32) und `vr_recfl` (114) — die drei Aufrufe, mit
denen QED den Cursor zeichnet — stehen in `cases/vdi-all.tsv` und weichen im
Vergleich `EMUTOS14-VDI-all.BIN` gegen `NEWTOS-VDI-all-0906.BIN` **nicht** ab.
Die abweichenden VDI-Fälle sind sämtlich die bekannten: Farbtiefe (33/34/42/72/113),
`v_get_pixel`/OVERLAY_BIT (54/160/161/270/272), `vqin_mode` (77), `vq_chcells`
(173), `v_opnvwk` (274), `vq_extnd` nach `vs_clip` (250/252).

### 5.2 Was abweicht — mit dem *neueren* Pi-Dump

Für die r1-Falltabelle liegen **zwei** NewTOS-Läufe im Baum, beide vom
31.08.2026: `NEWTOS-AES.BIN` (21:26) und `NEWTOS-AES-r2.BIN` (23:53). Maßgeblich
ist der zweite. Gegen `EMUTOS14-AES.BIN` (Atari TT, EmuTOS 1.4) bleiben nach
Abzug der `global[]`-Felder aus `appl_init` **fünf** Fälle:

| Fall | | EmuTOS | NewTOS |
|---|---|---|---|
| 155 | `wind_get(0,WF_WXYWH)` | w=640 h=461 | w=1920 h=1061 |
| 156 | `wind_get(0,WF_CXYWH)` | w=640 h=480 | w=1920 h=1080 |
| 157 | `wind_get(0,WF_PXYWH)` | w=640 h=480 | w=1920 h=1080 |
| 158 | `wind_get(0,WF_FXYWH)` | w=640 h=461 | w=1920 h=1061 |
| 167 | `wind_get(0,WF_OWNER)` | ret=0 | ret=1 |

155–158 sind **Bildschirmmaße**: beide Maschinen ziehen dieselben 19 px
Menüleiste ab (461 = 480−19, 1061 = 1080−19), der Rest ist 640×480 gegen
1920×1080. Bleibt Fall 167, `WF_OWNER` auf Handle 0 — der Fallkommentar der
Tabelle nennt ihn „AES 3.30 and later only". Ob das eine Versionsfrage oder ein
Implementierungsfehler ist, entscheidet dieser Vergleich **nicht**.

Aus `cases/aes-r2-wind.tsv` (78 Fälle, echte Fenster, Pi-Lauf vom 06.09.)
bleiben ebenfalls fünf: `wind_create` der neunte (EmuTOS 9, NewTOS −1);
`WF_CXYWH` eines erzeugten, ungeöffneten Fensters (EmuTOS `gl_rzero`, NewTOS das
create-Rechteck); `WF_OWNER` zweimal (244/247); `WF_PXYWH` nach `wind_open`
(EmuTOS das open-Rechteck 30,40,320,240 — NewTOS das create-Rechteck
10,20,300,200).

### 5.3 Korrektur: was der ältere Dump zeigte und heute nicht mehr gilt

Eine frühere Fassung dieses Abschnitts nannte fünf Abweichungsklassen aus
`NEWTOS-AES.BIN` (dem 21:26-Lauf), darunter eine `wind_calc`-Rahmenmetrik von
4 px (bei MOVER allein 22 px) mit dem Argument, sie sei nicht der Auflösung
geschuldet. **Diese Klassen sind im 23:53-Lauf sämtlich verschwunden:**
`wind_calc`-Metrik, die Desktop-Slider-Abfragen (`ret=0` statt 1), die
y-Koordinate von `WF_FXYWH` (0 statt 19), `wind_find` (0 statt −1) und die
OUT_LEN-Klasse. Von 74 Abweichungen bleiben 5.

Damit ist die Verbesserung **gemessen**, nicht aus Reparaturmeldungen
abgeleitet — und die alte Rahmenmetrik-Differenz taugt nicht als Beleg für das
QED-Symptom. Die vollständigen Ausgaben aller sechs Vergleiche samt der
reproduzierbaren Befehle liegen in `comparison/`.

Der methodische Punkt bleibt trotzdem stehen, in schärferer Form: ein Dump
trägt den Stand seines Laufs. Beide r1-Dumps lagen im selben Verzeichnis, und
der Vergleich mit dem älteren erzeugte fünf Klassen, die es nicht mehr gibt.
**Vor jedem Vergleich ist zu prüfen, welcher Lauf der jüngste für diese
Falltabelle ist**, und ein Befund gilt erst, wenn er gegen den heutigen Code
gegengeprüft wurde. Beispiel für die Gegenprobe: die OUT_LEN-Klasse ist auch im
Code erledigt — `tos-api/src/aes/mod.rs:133` liest `control[OUT_LEN]` und
kopiert nur so viele Wörter.

Der Rust-Dauertest (`cargo run -p xtask -- test`, 427 grün) ersetzt diese
Messung nicht: er fährt laut `tos-api/tests/aes_reference/harness.rs:818-820`
eine **TT-förmige Workstation** (640×480, 4 Planes). Fälle, deren Ergebnis von
der Bildschirmklasse abhängt, kann er strukturell nicht sehen.

### 5.4 Was auf diesem Pfad nie gemessen wurde

- **`WF_FIRSTXYWH` (Feld 11) und `WF_NEXTXYWH` (Feld 12)** — `grep` über
  `cases/aes-r1.tsv` und `cases/aes-r2-wind.tsv`: **0 Treffer**. Das ist die
  Rechteckliste, an der `free_for_draw` entscheidet.
- **`evnt_multi` (25)** — steht in der Never-Liste von
  `docs/MEASUREMENT-COVERAGE.md` und ist mit dem jetzigen Fallformat nicht
  messbar: der Aufruf blockiert und nimmt einen `addr_in[0]` (Message-Puffer),
  für den die Tabelle keine Spalte hat.
- **`graf_mouse` (78)** — ebenfalls nie gemessen.

`docs/MEASUREMENT-COVERAGE.md` ist an einer Stelle veraltet: es führt AES-Runde 2
als „not yet run", die Dumps `EMUTOS14-AES-r2wind.BIN` / `NEWTOS-AES-r2wind.BIN`
existieren und der Lauf ist in `STATUS.md` unter 2026-09-06 protokolliert.

---

## 6. Nebenbefund: NKCC läuft uninitialisiert

Kein Textbefund, aber auf demselben Pfad und belegt.

`edit_key` setzt die Taste mit `gem_to_norm(kstate, kreturn)` um
(tasten.c:715), das ist cflibs `nkccgton.c:30` → `nkc_tos2n`
(`nkccinit.c:142`). `nkc_tos2n` liest die drei Scancode-Tabellen aus
`nkc_keytab`, die `nkc_init()` per `Keytbl(-1,-1,-1)` besorgt
(`nkccinit.c:93-104`).

`nm sdk/cflib/cflib.a`:

```
nkccinit.o:
0000000000000000 T nkc_init
0000000000000000 b nkc_keytab.0     ← BSS: NULL beim Start
0000000000000008 b nkc_keytab.1
0000000000000010 b nkc_keytab.2
```

**`nkc_init` wird im ganzen SDK nirgends aufgerufen** — kein `U nkc_init` in
irgendeinem Objekt, und QED nennt `nkc_*` in keiner Quelldatei. `nkc_tos2n`
dereferenziert damit bei jedem Tastendruck einen Nullzeiger: `ktab[scan]` an
`nkccinit.c:247`, `:300` und `:304`.

Für `a` bleibt das folgenlos: `shift` und `unshift` sind beide NULL, die beiden
Vergleichslesungen treffen dieselbe Adresse, `ktab[scan] != nkc` ist damit immer
falsch, der frühe `return` fällt aus und `ascii` kommt unverändert als 0x61
durch. Zwei Dinge bleiben trotzdem stehen:

1. QED liest bei jedem Tastendruck Adresse 0x1e in EL0 und **faultet nicht** —
   diese Seite ist in QEDs Adressraum lesbar. Das ist eine Aussage über den
   Speicherschutz, nicht über die Textausgabe.
2. Alt- und Ctrl-Kombinationen laufen durch dieselbe Nullzeiger-Arithmetik, und
   dort ist der Ausgang nicht folgenlos (`nkccinit.c:300-313` wählt über
   `ktab[scan]` zwischen zwei Rückgabewegen).

---

## 7. Was als Nächstes zu tun ist

**Auf der QED-Seite** (beantwortet keine Konformitätsfrage, nur die
Verzweigung): die vier Stellen in `restore_edit` haben je einen AES-Aufruf als
Zeugen — `wind_set` bei den Slidern (:2125/:2132), `wind_get(WF_FIRSTXYWH)` in
`free_for_draw`, `vs_clip` auf das Work-Rechteck in `restore_indesk` (:2026),
`graf_mouse` als Klammer (:2105 / :2146). Das vorhandene `--features aes-trace`
(`tos_api::aes::trace`, `tools/aestrace.py`, 112-Byte-Records mit allen 24
int_in/int_out-Wörtern, ohne Filter) zeigt alle vier in einem Lauf.
`dbg_qedtrace.rs` baut dasselbe von Hand nach, filtert dabei und steht ohnehin
als F1 zum Rückbau an.

**Auf der Messseite:** `WF_FIRSTXYWH`/`WF_NEXTXYWH` brauchen keinen Zeiger und
keinen neuen Mechanismus — ein `+100`/`+101`-Prelude und ein `104`-Fall, genau
die Bauart der 78 vorhandenen Fälle. Weil jede Änderung an einer Falltabelle
deren CRC bricht und alle damit genommenen Dumps entwertet, gehört das in eine
eigene Tabelle (Vorschlag `cases/aes-r2b-rlist.tsv`), die beim nächsten
TT-Lauf mitfährt.

**Offen und nicht entschieden:** ob das Symptom je anders war. In den Dokumenten
findet sich kein Beleg, dass ein getipptes Zeichen in QED je erschienen ist;
`STATUS.md` führt den Schritt ausdrücklich als offen („Der QED-Schritt ist
interaktiv (Tippen/Datei öffnen) → Real-HW"). Belegt ist nur, dass QED beim
**WM_REDRAW** Text gezeichnet hat: `docs/analysis/qed-redraw-rootcause.md`
zitiert aus dem Juli-Trace „hunderte `vs_clip`/`vr_recfl`/`v_gtext` — QED
zeichnet seinen Fensterinhalt in die gelieferten Rechtecke".
