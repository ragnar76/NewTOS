# V17 — Quittung

**Status: IMPLEMENTIERT · hosttestbelegt · gegengemessen · Abnahme durch den
Tester offen** · 2026-09-14 · Der Zeiger parkt jetzt auf der Mitte des
**Koordinatenbereichs** statt auf der Mitte der **Pixelzahl**. Die Falltabelle
`aes-m07-parameter` hat danach **keinen einzigen `int_out`-Unterschied** mehr
gegen EmuTOS; verdrahtet ist sie trotzdem nicht, und der Grund dafür ist nicht
dieser.

## Quelle

`references/emutos`, Zielprofil Falcon030/TOS4. Die Kette, Zeile für Zeile
selbst nachgeprüft:

```
vdi/vdi_defs.h:94        #define xres  DEV_TAB[0]
vdi/vdi_defs.h:95        #define yres  DEV_TAB[1]
vdi/vdi_control.c:203    DEV_TAB[0] = V_REZ_HZ - 1;     /* update_rez_dependent */
vdi/vdi_control.c:204    DEV_TAB[1] = V_REZ_VT - 1;
vdi/vdi_mouse.c:582      GCURX = xres / 2;              /* vdimouse_init */
vdi/vdi_mouse.c:583      GCURY = yres / 2;
```

`xres` ist also die **letzte adressierbare Koordinate**, nicht die Breite. Bei
640×480 ist `xres` 639 und die Parkposition `639/2 = 319`, `479/2 = 239`.

NewTOS halbierte die Pixelzahl (`tos-api/src/vdi/mouse.rs`, `ensure_init`) →
320/240. Bei jeder **geraden** Auflösung ein Pixel Unterschied, bei jeder
ungeraden keiner — deshalb prüft der neue Test die Formel und nicht die Zahl.

## Test vor Fix — die rote Ausgabe

Neu: `tos-api/tests/vdi_mouse_park.rs`. Zwei Tests, zwei Wege auf dieselbe
Zahl. Damit die rote Ausgabe echt ist, wurde die **bestehende, falsche**
Rechnung zuerst unverändert in eine benannte Funktion `park_position` gehoben
(reiner Auszug, kein Verhaltenswechsel), dann der Test gefahren, dann die
Formel repariert.

```
running 2 tests
test park_position_halves_the_last_coordinate_not_the_pixel_count ... FAILED
test vq_mouse_reports_the_parked_centre_after_open ... FAILED

---- park_position_halves_the_last_coordinate_not_the_pixel_count stdout ----
assertion `left == right` failed: park position at 640x480
  left: (320, 240)
 right: (319, 239)

---- vq_mouse_reports_the_parked_centre_after_open stdout ----
assertion `left == right` failed: vq_mouse position after v_opnwk
  left: (320, 240)
 right: (319, 239)

test result: FAILED. 0 passed; 2 failed
```

Nach dem Fix:

```
running 2 tests
test park_position_halves_the_last_coordinate_not_the_pixel_count ... ok
test vq_mouse_reports_the_parked_centre_after_open ... ok

test result: ok. 2 passed; 0 failed
```

Was die beiden Tests jeweils sind:

* **Der Formeltest** vergleicht `park_position(w, h)` an sechs Geometrien gegen
  eine im Test **ausgeschriebene** Kopie von EmuTOS' Ausdruck (`(w-1)/2`), damit
  der Sollwert nicht aus der geprüften Funktion kommt. Darunter zwei ungerade
  (`641×481`, `1×1`), bei denen beide Lesarten zusammenfallen — sie schlagen an,
  wenn jemand das `-1` an die falsche Stelle setzt, und sie halten die Prüfung
  davon ab, ein Zahlenpaar auswendig zu lernen.
* **Der Endtest** geht durch den echten Trap: `v_opnwk` → `init_wk` →
  `vdimouse_init`, danach Opcode 124 `vq_mouse`, gelesen aus `PTSOUT[0..1]`.
  Er benutzt den vorhandenen 640×480-Speicher-Backend des
  `vdi_reference`-Harness, also dieselbe Geometrie wie die Referenzmessung.

## Der Fix

`tos-api/src/vdi/mouse.rs`:

```rust
pub fn park_position(w: u16, h: u16) -> (i16, i16) {
    // `xres` / `yres` in EmuTOS' sense: the last coordinate, DEV_TAB[0]/[1].
    let xres = w as i16 - 1;
    let yres = h as i16 - 1;
    (xres / 2, yres / 2)
}
```

`ensure_init` ruft sie; sonst ändert sich in der Datei nichts. Die Funktion ist
`pub`, weil der Formeltest eine Integrationsprüfung in einem eigenen Crate ist
und `ensure_init` prozessweit genau **einmal** läuft — zwei Auflösungen in einem
Testprozess sind über `INIT_DONE` nicht erreichbar.

## Der Rest der Datei, und wo `xres` sonst noch steht

Auftragspunkt 3 und 4, geprüft und **nicht** angefasst, weil überall richtig:

| Stelle | EmuTOS | NewTOS | Urteil |
|---|---|---|---|
| Klemmung der Zeigerposition | `scrn_clip` klemmt gegen `DEV_TAB[0]`/`[1]` (`vdi/vdi_asm.S:187-223`) | `mouse.rs:202-203` `width()-1` / `height()-1` | gleich |
| `DEV_TAB[0]`/`[1]` selbst | `V_REZ_HZ-1` (`vdi/vdi_control.c:203-204`) | `vdi/workstation.rs:195-196` (`v_opnvwk`) und `:284-285` (`vq_extnd` Modus 0): `xres-1`, wobei die lokale `xres` dort die **Breite** ist | gleich |
| Klemm-Vorgabe der Workstation | `vwk->xmx_clip = xres` (`vdi_control.c:329-330`) | `workstation.rs:184-185` `xres-1` (lokale `xres` = Breite) | gleich |
| `vs_clip` | `min(xres, rect->x2)` (`vdi_control.c:243-244`) | `vdi/attributes.rs:50-51,66-67` — dort heisst die lokale Variable `xres` und **trägt** `width()-1` | gleich |
| `v_rfbox`-Radius | `min(xres>>6, (x2-x1)/2)` (`vdi/vdi_gdp.c:254`) | `vdi/gdp.rs:283` `width()-1 >> 6`, mit Kommentar | gleich |
| `get_pixel_size` | liest `V_REZ_HZ`, **nicht** `xres` (`bios/screen.c:872-876`) | `workstation.rs:86-88,193` bekommt die Breite | gleich |
| Sprite-Beschnitt (`vdi_mouse.c:716,727,907,924`) | `xres+1-MOUSE_WIDTH`, `xres-15` usw. | **keine Entsprechung**: `draw`/`erase` (`mouse.rs:498-560`) beschneiden nicht selbst, sie geben jeden der 256 Bildpunkte einzeln durch `ClipRect::full(w, h)` (`backend.rs:507-514`, inklusive `w-1`) bzw. durch `get_device_pixel`/`put_device_pixel`, die ausserhalb des Schirms nichts tun | gleiches Ergebnis, anderer Bau — kein Off-by-one möglich, weil keine Randarithmetik existiert |

**Also keine zweite Fundstelle derselben Verwechslung im Quelltext.** Die
Einordnung des Tickets („örtlicher Ausrutscher") stimmt — für den Code.

## Wo sie doch systemisch war: in der Spezifikation

`docs/api/vdi/input/_mouse-packet.md` benutzte `xres` auf **einer Seite in zwei
unvereinbaren Bedeutungen**:

* Zeile 124 (Klemmung): „`GCURX` to `[0, DEV_TAB[0]]` = `[0, xres-1]`" —
  hier ist `xres` die **Breite**;
* Zeile 142 (Startzustand): „`GCURX = xres/2`, `GCURY = yres/2`" — wörtlich aus
  EmuTOS, wo `xres` die **letzte Koordinate** ist;
* Zeile 168 und 176 wieder als Breite.

Wer Zeile 142 mit der auf derselben Seite eingeführten Bedeutung liest, schreibt
`w/2` — genau das, was implementiert war. **Das ist der Ursprung des Defekts,
nicht eine Folge davon.** Beide Seiten sind korrigiert: `_mouse-packet.md` hat
jetzt einen ausdrücklichen Merksatz („`xres`/`yres` are the LAST COORDINATE, not
the width and height") und nennt 319/239; die Klemmzeilen sind auf
`[0, xres]` gestellt (was mit der richtigen Bedeutung dasselbe Intervall ist);
`docs/api/vdi/_state.md:197,205` trug denselben Widerspruch (Zeile 197 klemmte
auf `[0,xres-1]`, Zeile 205 initialisierte auf `xres/2`) und ist gleich
mitgezogen.

## Die Gegenmessung

`tosdump/cases/aes-m07-parameter.tsv` (20 Fälle, CRC `0F46665B`, **unverändert**)
auf QEMU `raspi4b`, headless, 640×480, nach dem Rezept aus
`measurement-2026-09-13-regression/method/`. Artefakte:
`measurement-2026-09-14-v17/`.

Was `tosdiff` gegen `tosdump/reference/EMUTOS14-AES-m07.BIN` an abweichenden
Feldern zählt, vorher gegen nachher:

| Feld | 2026-09-13 | 2026-09-14 (nach dem Fix) |
|---|---|---|
| `global[2]`, `[3]`, `[4]`, `[10]`, `[11]`, `[12]` | je 20 | je 20 |
| `int_out[1]` | **10** | **0** |
| `int_out[2]` | **10** | **0** |

Es bleiben genau die sechs `AESGLOBAL`-Wörter, die in jedem Fall jeder Tabelle
abweichen (`ap_id`, `ap_private`, `ap_planes` und die Wörter daneben). **Kein
einziges `int_out`-Wort unterscheidet sich noch.** Lauf sauber: „all 20 cases
ran", `rc=0`, kein Panic, kein Fault, der Desktop kam dahinter hoch, kein
QEMU-Prozess blieb stehen.

## Bildkontrolle

Zwei Screendumps, dieselben zwei Anwendungen, die `measurement-2026-09-14-a35/`
aufgenommen hat, damit die Bilder vergleichbar sind — QED und TeraDesk, je über
`#Z`-Autorun, QMP-Screendump nach 30 s. Verglichen wurde **Bildpunkt für
Bildpunkt über alle 640×480**, nicht nach Augenschein:

```
qed        40 Bildpunkte anders, Rahmen x 319..329, y 239..255
teradesk   54 Bildpunkte anders, Rahmen x 319..330, y 239..255
```

Jeder abweichende Punkt liegt im 16×16-Kasten des Pfeils. Der Pfeil ist **um
genau einen Punkt nach links und einen nach oben** gerückt; seine Form ist
unverändert (die Zeichenabbildung der 22×21-Nachbarschaft beider Bilder ist
ausser der Verschiebung identisch). **Sonst kein einziger Punkt** — nicht in der
Menüleiste, nicht in den Symbolen, nicht in den Beschriftungen, nicht im
Hintergrundraster.

## Die Tabelle ist trotzdem nicht verdrahtet — und warum nicht

Erzeugt (`mkrusttests`), eingehängt, gefahren. Ergebnis:

* **allein: 20 von 20 grün.**
  `cargo test -p tos-api --test aes_reference cases_m07_parameter`
  → `test result: ok. 20 passed; 0 failed`
* **im gemeinsamen Testbinär: 19 von 20.** Rot bleibt Fall 69 —
  `int_out[0]` 48 statt 32, also `MU_MESAG` **und** `MU_TIMER` statt nur dem
  Timer.

Das ist M08s Befund **B-2** und nicht dieser hier. Nachgeprüft statt vermutet,
und die Quelle ist eingegrenzt:

| zusammen gefahren mit | Fall 69 |
|---|---|
| `cases_r1` | grün |
| `cases_r2_wind` | grün |
| `cases_m05_addrin` | **rot** |
| `cases_m06_sysvar` | grün |

`aes-m05-addrin.tsv` öffnet Fenster (`+100`/`+101` vor seinen `wind_get`-Fällen).
Sein deklarierter Reset ist `wind_new`, und `wm_new`
(`references/emutos/aes/gemwmlib.c:1874-1892`) löscht Fenster und räumt
`wind_update`-Sperren ab — **die Nachrichtenschlange rührt es nicht an**. Das
liegengebliebene `WM_REDRAW` lässt Fall 69s `evnt_multi(MU_MESAG|MU_TIMER)`
`MU_MESAG` melden. Dass es **nicht** am Produkt liegt, steht in der Messung: die
frische QEMU-Aufnahme meldet für Fall 69 `int_out[0]` übereinstimmend mit EmuTOS
(der Diff führt für Fall 69 nur `global[]` und — vor dem Fix — die Mausposition).

Gemäss Auftrag („falls noch Fälle rot bleiben: nicht verdrahten") ist die
Tabelle wieder ausgebaut und die erzeugte Datei entfernt, statt eine rote Suite
zu hinterlassen. **Der Weg ist billig und bekannt:** derselbe Kehraus, den
`sweep_3d_look` für die 3D-Variablen macht, nur für die Nachrichtenschlange —
und die AES kann das über ihre eigene API, weil genau der Aufruf aus Fall 69
(`evnt_multi(MU_MESAG|MU_TIMER)` mit Zählerstand 0) je eine Nachricht abholt und
sofort zurückkehrt. Das gehört in M08s offenen Punkt, nicht in V17. Der
Modulkopf von `tos-api/tests/aes_reference.rs` trägt den Befund jetzt in dieser
Form (vorher nannte er die Mausposition als ersten von zwei Gründen).

## Die Einordnungen, die mitkorrigiert werden mussten

„Eine Begründung steht selten nur einmal." Gesucht wurde nach der Kernaussage,
nicht nach dem Ticketnamen.

| Fundstelle | Stand | Getan |
|---|---|---|
| `tos-api/tests/aes_reference.rs` (Modulkopf) | nannte die Mausposition als Grund fürs Nichtverdrahten | neu geschrieben: Grund ist jetzt allein Fall 69, mit der Eingrenzung oben |
| `TASK/M07-STATUS-DONE.MD:69` | „die Mitte eines 640×480-Schirms, um eins versetzt" — liest sich als Geräteunterschied | datierter Nachtrag angehängt, der die Einordnung widerruft |
| `docs/api/vdi/input/_mouse-packet.md`, `docs/api/vdi/_state.md` | zweideutiges `xres` (der Ursprung) | korrigiert, siehe oben |
| **`tosdump/cases/aes-m07-parameter.tsv:39-44`** | „WHAT THESE CASES WILL DIFFER IN, **by construction and not by defect**: int_out[1..4] are the machine's own mouse position … have no reason to agree between a TT and a Pi" | **nicht korrigierbar.** Die Tabellen-CRC läuft über die ganze Datei einschliesslich Kommentaren (`tosdump/core/cases.c:234`), und `tosdiff` wie `mkrusttests` weisen jeden Dump mit abweichender CRC ab (`tosdiff.c:175,294-305`, `mkrusttests.c:520-524`). Eine Textänderung entwertete beide Messungen vom 2026-09-13. **Gemeldet, nicht geändert.** |
| `measurement-2026-09-13-m07-parameter/RUNINFO.TXT:142-143` | „the centre of a 640x480 screen read off by one against the other. Not a finding about evnt_multi." | **nicht geändert**: die Datei sagt in ihrer zweiten Zeile „MEASUREMENT ONLY: this directory states what ran and what came back". Ein Messprotokoll rückwirkend umzuschreiben wäre schlimmer als der falsche Satz. Der neue Lauf widerspricht ihm mit Zahlen. |
| `docs/STATUS.md` M07-Eintrag (2026-09-13) | dieselbe Formulierung | datiertes Protokoll, nicht rückwirkend geändert; der neue Eintrag vom 2026-09-14 nennt und widerruft sie ausdrücklich |
| `tosdump/cases/COVERAGE.md:84,122` | `vq_mouse` (124) „excluded — live pointer position; differs between two runs on one machine" | **Grenzfall, nur gemeldet.** Auf einer Maschine mit Bediener stimmt die Begründung; in einem unbeaufsichtigten Lauf ist `vq_mouse` genau die Parkposition und damit ein Sollwert. Ein Fall dafür hiesse eine neue VDI-Tabelle mit eigener CRC — Sache des Leads, nicht ungefragt. |

## Belege

* `cargo run -p xtask -- test`: **540 grün / 0 rot / 1 ignoriert**
  (Sollstand 538 plus meine zwei neuen Fälle).
* Beide Boards **frisch** gebaut, `--release`, je vorher
  `touch kernel/src/lib.rs tos-api/src/lib.rs`, damit das geteilte
  Zielverzeichnis nicht „up to date" melden kann. Dass es echte Bauten waren,
  belegen die **unterschiedlichen** ELF-Prüfsummen:
  * `qemu-virt` `a866fabdb11efaa5f30f4cfb7f5aae90a05cc83eec7e6259dc1d67ded6afd658`
  * `rpi4` `be46c44d42b8800e1c3a415f8dd39ad7c48de39e242c2eb5e5208f48fbb7547e`
  * `kernel8.img` `5fd8c0286fba2c55667cda7bd40ac93cb2e3be53d26c2dcb2d1009d43519a247`
* Warnungen, gegen die genannte Grundlinie: rustc `tos-api` **64**, `kernel`
  **94**; clippy `tos-api` **81**, `kernel` **125** — je für beide Boards
  gemessen, **Delta 0**.
* Gegenmessung und Bildkontrolle: `measurement-2026-09-14-v17/RUNINFO.TXT`.

## Was ich nicht angefasst habe

* `tosdump/cases/*.tsv` — keine Zeile, kein Kommentar. Jede CRC unverändert.
* `tosdump/core/`, `tosdump/host/` — nur gebaut (`make all`, Exit 0), nicht
  geändert.
* Fall 69 / M08 B-2 — ausdrücklich fremd, offen gelassen, diagnostiziert.
* Alles andere in `tos-api/src/vdi/` — geprüft (Tabelle oben), nichts zu tun.

## Offen

1. **Abnahme durch den Tester**: Boot beider Boards, `svc`-Selbsttests,
   Screendumps, `fsck_msdos`. Was diese Änderung berührt, steht in der
   Übergabe des Entwicklerberichts.
2. **Echte Pi 4 bei 1920×1080**: dieselbe Formel gibt dort 959/539 statt
   960/540. Nicht gefahren — keine Hardware-Aktionen.
3. **`aes-m07-parameter` verdrahten**, sobald M08 B-2 geschlossen ist. Der
   Emitterbefehl steht im Kopf jeder erzeugten Falldatei; die Tabelle ist
   allein grün.
