# VDI-Raster mit dem Bildschirm als Operand (R02–R05): Messdesign, Lauf, Einordnung (2026-09-21)

Auditor-Auswertung der Tickets `TASK/R02.md` … `TASK/R05.md`. Grundlage ist
eine **neue Falltabelle** `tosdump/cases/vdi-raster-screen.tsv` (62 Fälle,
41 Preludes, CRC `9780EC39`), auf `desk-rebuild` @ `c1f42ff` dreimal
gefahren und in `measurements/measurement-2026-09-21-raster-screen/` abgelegt
(`RUNINFO.TXT`, `manifest.sha256`):

| Kürzel | Maschine | Profil | Herkunft |
|---|---|---|---|
| **FALCON-TC** | EmuTOS 1.4 (Release-ROM `etos512de.img`) auf Hatari Falcon030, VIDEL Truecolor (`$FF8266 = 0100`) | 640×480, 16 Planes, 256 Stifte | `emutos-falcon-tc-hatari/release-1.4-rom/RASTER.BIN`; Referenzkopie `tosdump/reference/EMUTOS14-RASTER-screen-falcontc.BIN`. Die gepinnte Quelle (`etos512us.img`) liefert **0 von 62** Fällen anders |
| **NEWTOS** | NewTOS auf QEMU `raspi4b`, headless, `#Z`-Autorun | 640×480, 16 Planes, 256 Stifte | `newtos-qemu-raspi4b/RASTER.BIN`, Kernel `--release` @ `c1f42ff` (mit dem Overlay-Bit-Fix `2d6b3cc`) |
| **Validierung** | dieselbe Falcon-TC-Konfiguration mit der **alten** Tabelle `vdi-raster.tsv` und dem **neuen** Dumper | — | `emutos-falcon-tc-hatari-old-table/RASTER.BIN`: **0 von 59** gegen die echte TT-Referenz `EMUTOS14-RASTER.BIN` (bitgleich), **1 von 59** gegen den NewTOS-QEMU-Dump vom Vormittag (Fall 157, D-VDI-25/R01, wie bekannt) |

**Kurzantwort.** Die vier Bildschirmpfade sind jetzt gemessen, nicht
gefolgert. NewTOS stimmt mit EmuTOS auf dem Falcon in **61 von 62** Fällen
Wort für Wort überein — 512 Wörter je Puffer, beide Puffer, beide MFDBs, alle
Arrays. Darunter: der gelöschte Schirm liest sich als `0xFFFF` in den
App-Puffer (OVL-001-Folge (2), **geschlossen**), Stift 0 als `0xFFDF`, jeder
Stift als sein 5-Bit-Wort, alle acht Überlappungsrichtungen (Kopierrichtung
nach Adressvergleich bei EmuTOS gegen Zeilen/Spalten-Entscheidung bei
NewTOS: dasselbe Bild), Clipping auf dem Schirm, alle vier `vrt_cpyfm`-Modi.
Der eine Unterschied ist **vor dem Lauf vorhergesagt** worden und ist ein
**Defekt**: `vrt_cpyfm` mit gesetztem `PAT_FLAG` (`INTIN[0] = 0x11`) auf dem
16-Bit-Pfad — EmuTOS zeichnet nichts, NewTOS zeichnet (D-VDI-23, bisher als
„unmeasured twin" offen). Roter Hosttest liegt vor.

Ein unabhängiges Modell der EmuTOS-Regeln (`method/oracle.py`, ohne Blick auf
eine Implementierung geschrieben) stimmt mit dem Falcon-Dump in **62 von 62**
Fällen überein — die Tabelle enthält keinen Tabellenfehler, und die
Erwartungen sind die aus der Quelle.

---

## 1. Messdesign

### 1.1 Was der Record tragen kann (die M09-Frage)

Der Raster-Record (Area 6, 3696 Bytes, `tosdump/core/tosdump.h`) trägt
**beide Pixelpuffer vollständig** — `buf_a[512]` und `buf_b[512]` Wörter —
plus beide MFDBs und alle fünf Arrays nach dem Aufruf. M09 betrifft den
AES-Record (120 Bytes, keine Pufferinhalte); für den Raster-Record **muss M09
nichts nachrüsten**. Ein Screen→Memory-Fall (R02) landet damit direkt im
Record (der Ziel-Puffer B ist das Ergebnis). Was der Record nicht tragen kann,
ist der Bildschirm selbst — dafür ist die Beobachtung entworfen:

### 1.2 Das Runner-Delta (`tosdump/core`, klar begrenzt, alte Tabelle unverändert)

Drei Zusätze zur Raster-Tabellenform, alle in `core/cases.h` unter
„THE SCREEN" dokumentiert und vom Loader bewacht; eine Tabelle ohne sie —
`vdi-raster.tsv` — parst und läuft byte-identisch wie zuvor (Validierung
oben, `make -C tosdump/host test` grün):

1. **`!screen w,h,planes`** — einmal, vor dem ersten Fall. Der Runner
   verweigert den ganzen Lauf, wenn die geöffnete Workstation kleiner ist
   (`DEV_TAB[0..1]`, `vdi_control.c:203-204`) oder nicht genau so tief
   (`INQ_TAB[4]` via `vq_extnd(1)`, `:210`). Beide Läufe loggen „table
   declares screen 640x480 at 16 planes; workstation reports 640x480 at 16
   planes". Eine Pi bei 1920×1080 wäre zulässig (≥), die Rechtecke liegen
   alle innerhalb 640×480.
2. **`S`** als MFDB-Spalte — `fd_addr = NULL`, alle anderen Felder 0 (das
   klassische Screen-MFDB; EmuTOS' `setup_info` liest dann `v_bas_ad`,
   `v_planes`, `v_lin_wr`, `vdi_raster.c:925-929,940-944`). Der Rechteck-Guard
   prüft gegen die deklarierte Ausdehnung; `vr_trnfm` darf `S` nicht nennen
   (kein NULL-Check in `vdi_vr_trnfm`, `:295-297`).
3. **Preludes** (`+`-Zeilen, wie in der VDI-Tabelle, nie aufgezeichnet) mit
   Whitelist: 109/121 (mit MFDBs, derselbe Guard), 25 `vsf_color`, 114
   `vr_recfl`, 129 `vs_clip`. Sie legen das bekannte Bild.
4. **Readback**: Füllspalte `seed_a,seed_b,rx,ry,rw,rh` — nach dem gemessenen
   Aufruf liest der Runner das Rechteck Pixel für Pixel mit `v_get_pixel`
   (eigene Arrays) und legt `INTOUT[1]` — auf einem Truecolor-Schirm das
   16-Bit-Wort selbst, `vdi_fill.c:671-681,1095-1110` — in Puffer B bei
   `row*32+col`, dem Layout einer 32×16-Geräteform. Ein Readback vergleicht
   sich damit genau wie ein Speicherziel. Guard: ≤ 32×16, im Schirm, der Fall
   nennt `S`, der Fall nennt nicht B.

**Wiederherstellung (R03):** der Reset vor jedem Fall (`td_reset_vdi`,
`v_clrwk` + Attribute + `vs_clip` aus) ist der definierte Anfangszustand und
zugleich die Rückstellung des vorigen Falls; es gibt keine bb_save-artige
Rückkopie. Headless ist der Endzustand irrelevant.

**Mauszeiger:** `v_clrwk` löscht per `memset` auch das Sprite; headless
bewegt sich nichts, und alle Rechtecke liegen bei (100,60) bzw. (0,0), weit
vom Bildmittelpunkt.

Host-Tests für das Delta: `tosdump/tests/test_cases_rast.c`
(`test_screen_rules`: Direktive, `S`, Whitelist, Readback-Guard) und
`tosdump/tests/test_runner_rast.c` (Preludes laufen zwischen Reset und
Aufruf, 12 `v_get_pixel`-Aufrufe mit den richtigen Koordinaten, Readback
landet bei `row*32+col`, Screen-MFDB als acht Nullen aufgezeichnet).

### 1.3 Die erwarteten Wörter (alle aus EmuTOS)

| Was | Wort | Herleitung |
|---|---|---|
| gelöschter Schirm | `0xFFFF` | `v_clrwk` memset `0xff` (`vdi_control.c:610-618`) |
| Stift 0 (weiß) | `0xFFDF` | `set_color16`: `(r<<11)\|(g<<6)\|b`, 5-Bit-Kanonen, Bit 5 nie gesetzt (`vdi_col.c:588-613`) |
| Stift 1 / 2 / 3 / 4 / 6 | `0x0000` / `0xF800` / `0x07C0` / `0x001F` / `0xFFC0` | `videl_palette1` (`vdi_col.c:130-136`) |
| Stift 8 (733,733,733) / 9 (533,…) | `0xBDD7` / `0x8C51` | `vdi2fivebits = (col*31+500)/1000` → 23 / 17 |
| Stift 16 / 255 | `0xFFDF` / `0x0000` | `videl_palette2[0]` / `[239]` (`vdi_col.c:137-199`), `MAP_COL` gefaltet |
| ungültiger Stift (999, −1) | Stift 1 | `validate_color_index` (`vdi_control.c:224-230`) |

---

## 2. Die Tabelle je Ticket

Testnamen: `tos-api/tests/vdi_raster_reference/cases_screen.rs`, Binär
`vdi_reference_raster_screen` (generiert aus der Falcon-Referenz; 61 grün,
1 rot). Fall-ID = physische Zeile in `vdi-raster-screen.tsv`.

### R02 — `vro_cpyfm` Screen → Memory (Section E, 15 Fälle; 15 gleich)

| Fall | Aufbau | EmuTOS-Regel | Falcon-TC = NewTOS | Klasse |
|---|---|---|---|---|
| 69 | gelöschter Schirm (100,60)–(131,75) → B, op 3 | `setup_info` Screen-Quelle, `vro_cpyfm16` S_ONLY (`vdi_raster.c:925-929,1048-1064`) | 512 × `0xFFFF` | **gleich — OVL-001 Folge (2) geschlossen** |
| 70 | dito am Ursprung (0,0) | dito | 512 × `0xFFFF` | gleich |
| 72 / 73 / 74 / 75 | ops 6 XOR / 1 AND / 4 NOTS_AND_D / 10 NOT_D gegen die Füllung | Logik-Tabelle auf dem `UWORD` (`:1008-1220`) | `~fill` / `fill` / `0x0000` / `~fill` | gleich (Op im 16-Bit-Raum, Bit 5 inklusive) |
| 80 | Prelude `vsf_color 2`, `vr_recfl` ganzes Rechteck → B | Stiftwort | 512 × `0xF800` | gleich |
| 83 | Prelude Stift 3 auf 16×8 innen | Rechteckgeometrie | Zeilen 4–11, Spalten 8–23 `0x07C0`, Rest `0xFFFF` | gleich |
| 87 | Prelude Stift 0 ganzes Rechteck | Stift-0-Wort ≠ gelöschtes Wort | 512 × `0xFFDF` | gleich (Overlay-Bit unterscheidet 69 von 87) |
| 92 | Stift-4-Block 16×8 → B-Anker (8,4) | `d_xmin/d_ymin`, `d_nxln = fd_wdwidth*32` (`:938`) | `0x001F` in Zeilen 4–11/Sp. 8–23, sonst Füllung | gleich |
| 98 | Stifte 8 / 9 je Hälfte | 5-Bit-Quantisierung | `0xBDD7` / `0x8C51` | gleich |
| 100 | Ziel 4 Planes | `s_nxwd != d_nxwd` → return (`:1360`) | B unverändert | gleich |
| 101 | `0x13` (PAT_FLAG\|3) | `vro_cpyfm16` liest INTIN[0] ungestrippt, kein Arm (`:981`) | B unverändert | gleich (D-VDI-23, vro-Hälfte) |
| 104 | Quellecken vertauscht | `arb_corner` auf lokaler Kopie (`vdi_asm.S:79-81`) | 512 × `0xF800`, PTSIN unverändert | gleich (D-VDI-24) |
| 107 | XOR roter Schirm in die Füllung | | `fill ^ 0xF800` | gleich |

### R03 — `vro_cpyfm` Memory → Screen, Readback (Section F, 15 Fälle; 15 gleich)

| Fall | Aufbau | EmuTOS-Regel | Falcon-TC = NewTOS (Readback) | Klasse |
|---|---|---|---|---|
| 118 | A (32×16) → (100,60), op 3 | `d_form = v_bas_ad`, `dont_clip` (`:940-944,882-900`) | `fill(31)` 512 Wörter | gleich |
| 119 | 16×8 aus (8,4) → (101,61), Fenster 18×10 | Quell-Offset, Zielanker | Zeilen 1–8/Sp. 1–16 aus `fill[(4+r)*32+8+c]`, Rand `0xFFFF` | gleich (kein Überzeichnen) |
| 120 / 121 / 122 / 123 / 124 | ops 6 / 1 / 4 / 10 / 15 auf `0xFFFF` | Logik-Tabelle | `~fill` / `fill` / `~fill` / `0x0000` / `0xFFFF` | gleich |
| 125 | `0x13` | kein Arm | `0xFFFF` (nichts gezeichnet) | gleich |
| 126 | Quelle 8 Planes | `s_nxwd 16 != 32` | `0xFFFF` | gleich |
| 130 | Prelude `vs_clip 1 (104,62)-(127,73)` | `do_clip`: `s_xmin` wandert mit `d_xmin` (`:809-877`) | Füllung nur innen, mit Quell-Offset; außen `0xFFFF` | gleich |
| 132 | Clip endet (115,67) | `d_xmax = min(clip)` | Sp. 0–15/Zeilen 0–7 | gleich |
| 133 | Zielecken vertauscht | `arb_corner` | landet bei (100,60) | gleich |
| 136 / 139 | XOR auf Rot / D_ONLY auf Grün | | `fill ^ 0xF800` / `0x07C0` | gleich |
| 140 | Quelle (5,3), nicht wortausgerichtet | 16-Bit-Pfad ist pixelweise | `fill[(3+r)*32+5+c]` | gleich |

### R04 — `vro_cpyfm` Screen → Screen, überlappend (Section G, 15 Fälle; 15 gleich)

Prelude legt je Fall einen 16×8-Block der Füllung an die Quelle; der Fall
verschiebt ihn um (8,4), (8,0), (0,4) in jede Richtung. EmuTOS wählt die
Richtung durch **Adressvergleich** (`src < dst`, `vro_cpyfm16` `:995`:
rückwärts ab `s_xmax/s_ymax`, `d_xmax/d_ymax`, `:996-1001`); NewTOS getrennt
nach Zeilen und Spalten (`kernel/src/vdi_backend.rs`, `copy_rect`:
`down = dy > sy`, `right = dx > sx`). Auf einem Schirm liefern beide Regeln
dasselbe Bild — jetzt gemessen, nicht argumentiert.

| Fall | Richtung | EmuTOS-Pfad | Ergebnis | Klasse |
|---|---|---|---|---|
| 158 / 160 | (+8,+4) / (−8,−4) | rückwärts / vorwärts | Block korrekt verschoben, Rest Prelude-Bild | gleich |
| 162 / 164 | (+8,0) / (−8,0), gleiche Zeilen | rückwärts (Spaltenordnung entscheidet) / vorwärts | gleich | gleich |
| 166 / 168 | (0,+4) / (0,−4), gleiche Spalten | rückwärts (Zeilenordnung) / vorwärts | gleich | gleich |
| 170 / 172 | (−8,+4) / (+8,−4) | rückwärts / vorwärts — NewTOS' Spaltenrichtung ist hier die andere, Zeilen verschieden → unbeobachtbar | gleich | gleich (Argument durch Messung bestätigt) |
| 176 / 178 | XOR (+8,+4) / (−8,−4) | in-place, Reihenfolge sichtbar bei falscher Richtung | gleich | gleich |
| 180 / 182 | auf sich selbst, op 3 / op 6 | `src == dst` → vorwärts | unverändert / `0x0000` über dem Block | gleich |
| 185 | (+8,+4) mit `vs_clip` | `do_clip` auf Screen-Ziel | nur innerhalb des Clips | gleich |
| 187 | `0x13` | kein Arm | nichts kopiert | gleich |
| 189 | Block nach (0,0), Fenster 18×10 | | Block bei (0,0), Rand `0xFFFF` | gleich |

### R05 — `vrt_cpyfm` Mono → Screen (Section H, 17 Fälle; 16 gleich, **1 Defekt**)

Quelle: Puffer A als 1-Plane-Form 32×16 (zwei Wörter je Zeile, MSB zuerst,
die Bits der Füllung). EmuTOS: `s_nxwd == 2` (`:1384`), `fg/bg =
MAP_COL[validate_color_index(INTIN[1..2])]` (`:1390-1395`), `plane_ct 16 →
vrt_cpyfm16` (`:1432-1437`), `fgcol = palette[fg]` (`:1234-1236`).

| Fall | Aufbau | EmuTOS-Regel | Falcon-TC | NewTOS | Klasse |
|---|---|---|---|---|---|
| 201 | MD_REPLACE 1/0 | `:1304-1322` | set `0x0000`, clear `0xFFDF` | gleich | gleich |
| 202 | MD_TRANS 1 | `:1287-1303` | set `0x0000`, clear bleibt `0xFFFF` | gleich | gleich |
| 203 | MD_XOR auf gelöscht | `*q = ~*q` (`:1270-1286`) | set `0x0000` | gleich | gleich |
| 204 | MD_ERASE bg 0 | `:1253-1269` | clear `0xFFDF`, set bleibt `0xFFFF` | gleich | gleich |
| 205 / 206 / 207 | REPLACE 2/4, TRANS 3, ERASE bg 6 | Stiftwörter | `0xF800`/`0x001F`, `0x07C0`, `0xFFC0` | gleich | gleich |
| 210 | XOR auf Rot | | set `0x07FF`, clear `0xF800` | gleich | gleich |
| 211 | Stifte 999 / −1 | `validate_color_index → 1` | alles `0x0000` | gleich | gleich |
| 212 | Stifte 255 / 16 | `videl_palette2`, `MAP_COL[255]=15` | set `0x0000`, clear `0xFFDF` | gleich | gleich (QEMU); Host-Double musste dafür `videl_palette2` bekommen, s. § 4 |
| 213 | Quelle (5,3), Fenster 18×10 | `src_off`, `bit_mask` (`:1242-1243`) | Bits `(3+r, 5+c)`, Rand `0xFFFF` | gleich | gleich |
| **215** | **`0x11` = PAT_FLAG\|MD_REPLACE** | `vrt_cpyfm16` liest INTIN[0] ungestrippt, `default: return` (`:1232,1323-1324`) | **512 × `0xFFFF` — nichts gezeichnet** | **REPLACE-Expansion `0x0000`/`0xFFDF`** | **DEFEKT, CONFIRMED** (§ 3) |
| 216 | Schirm als Mono-Quelle → B | `s_nxwd 32 != 2` (`:1384`) | B unverändert | gleich | gleich |
| 218 | REPLACE mit `vs_clip` | `do_clip` | nur innen | gleich | gleich |
| 219 | REPLACE am Ursprung | | | gleich | gleich |
| 222 / 223 | REPLACE 2/4 / XOR in **16-Plane-Speicher** | `vrt_cpyfm16` auf Speicherziel — auf dem TT nicht messbar (`vwk->ext` NULL, `vdi_control.c:450-452`) | set/clear Stiftwörter; set `~fill` | gleich | gleich (AUD-V5-005 `mono_to_mem` erstmals gegen EmuTOS gemessen) |

---

## 3. Der Defekt: `vrt_cpyfm` mit `PAT_FLAG` auf dem 16-Bit-Pfad zeichnet (CONFIRMED)

**EmuTOS** (`vdi/vdi_raster.c`): `cpy_raster` strippt `PAT_FLAG` in sein
lokales `mode` (`:1347`) und ruft für `plane_ct > 8` `vrt_cpyfm16(info)`
ohne es (`:1432-1437`); `vrt_cpyfm16` liest `mode = INTIN[0]` neu (`:1232`),
`0x11` trifft keinen der vier Arme, `default: return` (`:1323-1324`). Die
Zielfläche bleibt unberührt.

**NewTOS** (`tos-api/src/vdi/raster.rs`, `vrt_cpyfm`, Zeilen 292-303):
`let mode = match raw & !0x10 { 1 => Replace, … }` — das Bit wird gestrippt
und es wird gezeichnet. Der Zwilling `vro_cpyfm` (`:440-442`) hat den
richtigen Frühausstieg (`if dp > PALETTE_PLANES_MAX && raw_mode & 0x10 != 0
{ return; }`); `vrt_cpyfm` hat ihn nicht.

**Gemessen:** Fall 215, `INTIN = 0x11,1,0`, 32×16 Mono → Schirm (100,60),
Readback: Falcon-TC (beide ROMs) 512 × `0xFFFF`; NewTOS QEMU 275 × `0x0000`
+ 237 × `0xFFDF` (`DIFF-falcontc-rel-vs-newtos-qemu.txt`).

**Beobachtbares Fehlerbild:** ein Programm, das `vrt_cpyfm(handle, MD_REPLACE
| PAT_FLAG, …)` aufruft (etwa weil es den Modus mit einem Muster-Flag aus dem
Line-A-Kontext durchreicht), sieht auf einem Falcon/TOS4-Truecolor-Schirm
nichts, auf NewTOS ein Icon. Kein bekanntes Programm tut das; die Schwere ist
niedrig. Es ist trotzdem ein Vertragsunterschied durch die dokumentierte API
und genau die Regel, die für `vro_cpyfm` bereits als Vertrag reproduziert ist.

**Spec-Status:** `docs/api/vdi/raster/_raster.md` (Absatz „Unmeasured twin",
Edge Case 10) und `docs/api/vdi/_discrepancies.md` D-VDI-23 („Open half …
not measurable on the user's TT … NewTOS' `vrt_cpyfm` therefore still
strips") sind **überholt**: die Regel selbst steht dort richtig, die
Messlücke ist geschlossen — an den Researcher: „gemessen auf Hatari
Falcon030 Truecolor, `EMUTOS14-RASTER-screen-falcontc.BIN` Fall 215, nichts
gezeichnet; NewTOS zeichnet (rot: `case_0215_…`)".

**Roter Test:** `cargo test -p tos-api --test vdi_reference_raster_screen`
→ `cases_screen::case_0215_op121_h12_pat_flag_md_replace_on_the_16_bit_path_emutos_draws_nothin`
(512 Wörter gegen die Falcon-Referenz). Eigenes Binär, alphabetisch als
letztes der `vdi_*`-Binäre, damit die fail-fast-Suite die übrigen vorher
durchläuft.

**Für den Entwickler:** in `vrt_cpyfm` vor dem Modus-Match dieselbe Sperre
wie in `vro_cpyfm` — Ziel-Planes `> PALETTE_PLANES_MAX` und `raw & 0x10 != 0`
→ return (kein Backend-Aufruf, kein Speicherziel). Abnahme: das rote Binär
wird 62/62 grün, und derselbe QEMU-Lauf ergibt **0 von 62** gegen
`EMUTOS14-RASTER-screen-falcontc.BIN`.

---

## 4. Was gefahren wurde, was nicht

**Gefahren:** Runner-Validierung (alte Tabelle, Falcon-TC, neuer Dumper: 0/59
gegen die echte TT-Referenz); die neue Tabelle auf zwei EmuTOS-ROMs (0/62
zwischen den ROMs); Modellprüfung der Falcon-Referenz gegen die
EmuTOS-Regeln (0/62); NewTOS in QEMU raspi4b (1/62); `tosdiff --validate`
für beide; die tosdump-Host-Suite; `cargo test -p tos-api --no-fail-fast`
(885 grün / 1 rot, nur Fall 215). Adversarial: alle 16 Logik-Ops auf
Screen↔Memory, Overlay-Bit-Unterscheidung, ungültige Stifte, erweiterte
Palette, PAT_FLAG auf allen drei Screen-Pfaden, vertauschte Ecken, nicht
wortausgerichtete Quellen, Planes-Mismatch beider Seiten, Schirm als
Mono-Quelle, Clipping auf allen drei Zielpfaden, acht Überlappungsrichtungen
plus XOR-Überlappung, Kopie auf sich selbst.

**Nicht gefahren / offen:**

* **Echte Pi 4 (1920×1080):** keine Hardware-Aktion. Die Tabelle ist dort
  lauffähig (`!screen` verlangt ≥ 640×480 und genau 16 Planes;
  `DUMPRAST.NAP` + `RASTER.TSV` + `#Z`-INF in die Wurzel von C:). Erwartung:
  identisch zum QEMU-Dump (kein Fall hängt von der Schirmbreite ab). Das ist
  die verbleibende Hardwareabnahme jedes der vier Tickets.
* **Kernel-Backend auf dem Host:** `copy_rect` (R04) und `blit_mono` (R05)
  laufen im Hosttest im Double (`tos-api/tests/vdi_reference/harness.rs`, das
  jetzt Op und Modus ehrt), nicht im Kernel. Für diese zwei Arme ist der
  QEMU-Lauf das Zeugnis; der Hosttest sichert die `tos-api`-Schicht
  (Dispatch, Modus-/Stiftprüfung, Rechtecke, Clip-Weitergabe).
* **Ill-formed Aufrufe über den Schirmrand** (Ziel ohne Clip teils außerhalb,
  Quelle außerhalb): der Guard verweigert sie, weil EmuTOS dort über das
  Zeilenende in die nächste Zeile bzw. am unteren Rand über das VRAM hinaus
  schreibt. NewTOS klemmt stattdessen auf den Schirm (`mem_to_screen`,
  `copy_rect`). Für wohlgeformte Programme unbeobachtbar; für kaputte
  Programme „NewTOS schreibt nicht in fremden Speicher" — Abweichung
  benannt, nicht gemessen, nicht als Defekt gewertet.
* **D-VDI-25 / R01** bleibt zurückgestellt; die Screen-Pfade sind jetzt
  gemessen (die in D-VDI-25 verlangte Reihenfolge „erst die Screen-Pfade
  messen" ist erfüllt), die Entscheidung liegt beim Lead.
* **OVL-001 Folge (3)** (`v_contourfill` Seed-Modus auf dem gelöschten
  Grund) ist keine Rasterfunktion und nicht Teil von R02–R05; eine kleine
  VDI-Tabelle (`+103` mit `INTIN[0] < 0` als Prelude, `v_get_pixel` als Fall)
  wäre mit derselben Methode billig zu haben.

**Harness-Befund (kein NewTOS-Defekt):** das Backend-Double säte nur die
Stifte 0–15; Fall 212 (Stift 16/255) war deshalb im ersten Hostlauf rot,
obwohl QEMU stimmte. Das Double bekam `videl_palette2` aus der gepinnten
Quelle transkribiert (`VIDEL_PALETTE2`, 240 Tripel) und ehrt in `copy_rect`
Op und in `blit_mono` Modus — ohne das hätte die generierte Suite für die
Sections G/H das Double statt NewTOS geprüft.

---

## 5. Außerhalb des Auftrags beobachtet, nicht verfolgt

* `docs/api/vdi/raster/_raster.md` und `_discrepancies.md` D-VDI-23 tragen
  noch den Stand „vrt-Hälfte nicht messbar" (§ 3) — Researcher.
* `tosdump/cases/vdi-raster.tsv` Kopfkommentar („There is no screen MFDB
  here on purpose: the two machines' screens differ") und `core/cases.h`
  Regel 2 der alten Guard-Liste sind durch den Falcon-TC-Stand historisch;
  die Tabelle bleibt eingefroren, der neue Abschnitt in `cases.h` sagt es.
* `tosdump/deploy-tt/README.TXT` / `deploy-pi4/README.TXT` kennen die neue
  Tabelle nicht (gitignoriert, nur Staging).
* `TD_RAST_MAX_CASES = 64` begrenzt eine Tabelle auf 64 Fälle; die neue
  nutzt 62. Eine Erweiterung wäre kein Formatwechsel, nur Kapazität.

---

## 6. Quittungssätze für die Tickets

Für **R02, R03, R04** ist derselbe Satz belegt:

> Messdesign umgesetzt als neue Falltabelle `tosdump/cases/vdi-raster-screen.tsv`
> (CRC `9780EC39`, Section E/F/G) mit einem begrenzten Runner-Delta
> (`!screen`, `S`, Preludes, Readback per `v_get_pixel`; `core/cases.h`
> „THE SCREEN", Host-Suite grün, alte Tabelle byte-identisch validiert). Der
> Screen-Pfad ist tatsächlich gelesen: EmuTOS 1.4 auf Hatari Falcon030
> Truecolor (beide ROMs, 0 Unterschiede) und NewTOS in QEMU raspi4b liefern
> in allen 15 Fällen der Section dieselben 512 Wörter je Puffer
> (`measurements/measurement-2026-09-21-raster-screen/DIFF-falcontc-rel-vs-newtos-qemu.txt`);
> die Erwartungen sind durch ein unabhängiges Modell der EmuTOS-Regeln
> bestätigt (`ORACLE-model-vs-falcontc-rel.txt`, 0/62). Hosttests:
> `tos-api/tests/vdi_raster_reference/cases_screen.rs` (Binär
> `vdi_reference_raster_screen`), Referenz
> `tosdump/reference/EMUTOS14-RASTER-screen-falcontc.BIN`. Offen: der
> Hardwarelauf auf der Pi 4 (Tabelle dort lauffähig, Erwartung identisch).

Für **R05**:

> Wie oben, Section H (17 Fälle): 16 gleich, darunter alle vier `MD_*`-Modi,
> ungültige und erweiterte Stifte und das 16-Plane-Speicherziel; **ein
> bestätigter Defekt** (Fall 215, `vrt_cpyfm` mit `PAT_FLAG` auf dem
> 16-Bit-Pfad zeichnet, EmuTOS nicht — D-VDI-23, bisher „unmeasured twin"),
> roter Hosttest `case_0215_op121_h12_…`, Reparaturort
> `tos-api/src/vdi/raster.rs::vrt_cpyfm` (Frühausstieg wie in `vro_cpyfm`),
> Spec-Nachtrag an `_raster.md`/`_discrepancies.md`. Offen: Fix, Gegenmessung
> (0/62 erwartet), Pi-4-Lauf.

Für **V11/V12, Folge (2)** (`docs/analysis/vdi-v-series-classification-2026-09-21.md` § 5):

> `vro_cpyfm` Screen→Memory eines gelöschten Bereichs schreibt auf NewTOS
> @ `c1f42ff` `0xFFFF`-Wörter in die App-MFDB, wie auf dem Falcon (Fälle 69/70,
> 512/512 Wörter gleich); Stift 0 daneben `0xFFDF` (Fall 87). Folge (2) ist
> geschlossen; Folge (3) (`v_contourfill`) bleibt ungemessen.
