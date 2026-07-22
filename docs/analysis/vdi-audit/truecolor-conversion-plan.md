# VDI Truecolor-Konversionsplan (16-Farb-Palette → Falcon-Truecolor)

> Researcher, 2026-07-22. Reine Analyse/Planung, kein Produkt-Code. Auslöser:
> User-Entscheidung Memory `vdi-truecolor-workstation` — Ziel-Profil
> Falcon030/TOS4, `CONF_WITH_VDI_16BIT=1`. Dieser Plan MUSS vor den restlichen
> VDI-Render-Fixes (V2/V3/V4, siehe `00-MASTER.md`) umgesetzt werden, weil er
> das darunterliegende Farbmodell verändert.
>
> Zitierkonvention: jede Tatsachenbehauptung trägt `datei:zeile` aus
> `references/emutos/`. Wo ich auf Basis mehrerer Zitate schließe statt
> wörtlich zu zitieren, ist das als "Ableitung" markiert.

## 0. Zusammenfassung & Empfehlung

**EmuTOS kennt genau ein Truecolor-VDI: 16-bit RGB565-artig, gated durch
`v_planes > 8`** (`vdi/vdi_defs.h:22` `#define TRUECOLOR_MODE (v_planes > 8)`).
Es gibt in EmuTOS **keinen** 24- oder 32-bit-VDI-Pfad — der Truecolor-Zweig ist
überall an `v_planes > 8` gebunden und der einzige konkrete Wert, den ein
Falcon je für `v_planes` liefert, ist 16 (`bios/videl.c:320-341`
`get_videl_bpp()`: `SPS_HICOLOR → bits_per_pixel = 16`, aufgerufen aus
`videl_get_current_mode_info` → `*planes`, `bios/videl.c:969-974`). Das ist
**keine Ratensache** — es gibt schlicht keine andere Referenz. Empfehlung:
**16-bit RGB565 auf dem Draht (Pel-Modell, Attribute, App-sichtbare
Memory-MFDBs), Backend expandiert nur an der physischen Bildschirmgrenze auf
32-bpp XRGB8888.**

Zwei Teil-Entscheidungen darunter sind nicht gleich "hart erzwungen":

1. **Frei/intern (kein 1:1-Zwang):** Der physische Pi4-Framebuffer bleibt
   32-bpp XRGB8888. EmuTOS schreibt auf dem Falcon direkt 16-bit-Pixel in
   VRAM; unser Backend darf stattdessen bei jedem Screen-Write von der
   16-bit-RGB565-Farbe auf 32-bit XRGB8888 expandieren — das ist ein
   Interna-Detail nach `docs/COMPATIBILITY.md` (Beobachtungspunkt ist die
   VDI-Semantik, nicht die physische VRAM-Byte-Breite).
2. **Erzwungen (Contract, kein Spielraum):** Das **App-sichtbare
   Memory-MFDB-Wireformat** (`fd_stand=0`, `fd_nplanes=16`) muss echtes
   **16 bit/Pixel gepackt** sein, nicht unser heutiges 32-bpp-chunky-Modell
   (`tos-api/src/vdi/raster.rs:21-36`, `CHUNKY_PLANES=32`). Ein Falcon-Truecolor-
   Programm, das sich selbst eine MFDB baut und direkt RGB565-Pixel hineinpokt
   (Bildbetrachter, Sprite-Editor), erwartet exakt dieses Byte-Layout, weil
   `vro_cpyfm16`/`vrt_cpyfm16` roh auf `UWORD*` arbeiten
   (`vdi/vdi_raster.c:975-1221`, `:1226-1326`) — das ist beobachtbar, kein
   Implementierungsdetail. Das ist die **größte strukturelle Änderung** dieses
   Pakets (siehe §3.4, §6) und braucht ausdrückliches Lead-Go, weil sie
   bestehende, bereits committete Annahmen aus dem V5-Audit invalidiert.

Eine dritte, bisher nicht im Auftrag genannte, aber aus dem Quellstudium
zwingend folgende Struktur-Änderung: **die Farbtabelle wird pro Workstation
(`Vwk`/`VwkExt`), nicht mehr ein einziges Backend-globales Array** (siehe §3.5).
Das ist ebenfalls Contract, kein Stil.

---

## 1. Ziel-Modus aus EmuTOS (mit Beleg)

### 1.1 Gating

- `include/config.h:1628-1629` — `CONF_WITH_VDI_16BIT` default `1` im
  generischen (Atari-Hardware-)Zweig.
- `include/config.h:2198-2201` — Hard-Error `CONF_WITH_VDI_16BIT requires
  CONF_WITH_VIDEL`.
- `include/config.h:1092-1093` — `CONF_WITH_VIDEL` default `1` (Atari-Hardware).
- `include/config.h:803-804` — für **Nicht-Atari**-Ziele (Lisa, Amiga, …) ist
  `CONF_WITH_VDI_16BIT` explizit `0` ("Like ST, not Falcon") — bestätigt, dass
  unser Falcon030/TOS4-Zielprofil (`CONF_ATARI_HARDWARE=1`, kein
  `MACHINE_*`-Override) im "echten" Zweig landet und `CONF_WITH_VDI_16BIT=1`
  bekommt, exakt wie in der User-Entscheidung angenommen.
- `vdi/vdi_defs.h:22` — `#define TRUECOLOR_MODE (v_planes > 8)`. Truecolor ist
  also rein plane-count-gated, nicht ein separates Flag.
- `bios/videl.c:320-341` (`get_videl_bpp`) — `SPS_HICOLOR → bits_per_pixel =
  16`; das ist der einzige `v_planes`-Wert `> 8`, den der Falcon-Pfad je
  liefert (die anderen Zweige sind 1/4/8). ⇒ **Truecolor = 16 bpp, immer.**

### 1.2 Bit-Layout — zwei verschiedene Farb-"Ebenen" in EmuTOS' Truecolor

EmuTOS hat unter Truecolor **zwei getrennte Farbrepräsentationen**, die leicht
verwechselt werden — das ist der Kern dessen, was Punkt 3 der Aufgabe ("fließen
Farben künftig direkt als RGB?") korrigiert:

**(a) Rohe Pixel-Daten in Bitmaps (MFDBs) — echtes 16-bit RGB565-chunky.**
`vro_cpyfm16` (`vdi/vdi_raster.c:975-1221`) und `vr_trnfm16`
(`vdi/vdi_raster.c:190-256`) arbeiten strikt auf `UWORD*`/`WORD*`-Pixelwörtern,
ohne jede Palette — der 16-Bit-Logikoperator-Tisch (`BM_S_AND_D` … `BM_ALL_BLACK`,
identisch zur planaren Tabelle) wirkt **direkt auf das Pixelwort**
(`vdi_raster.c:1008-1220`). Das ist der Pfad, den ein Programm nimmt, das
selbst ein Bild in eine MFDB schreibt und blittet.

**(b) Die VDI-"Stift"-Attribute (Linie/Fill/Text/Marker) bleiben ein
PEL-Index-Modell — nur die Auflösung ändert sich.** Das ist die zentrale,
nicht-offensichtliche Erkenntnis:

- `init_wk` setzt `vwk->line_color = MAP_COL[l]`, `mark_color`, `text_color`,
  `fill_color` genauso wie im Nicht-Truecolor-Fall (`vdi/vdi_control.c:283-298,
  313-314`) — **unverändert durch Truecolor**.
- `vdi_vsl_color` (`vdi/vdi_line.c:143-150`): `vwk->line_color =
  MAP_COL[lc]` — dieselbe Zeile für Truecolor wie für Palette-Modi.
- `MAP_COL`/`REV_MAP_COL` werden **immer** aufgebaut, unabhängig von
  Truecolor: `init_colors()` (`vdi/vdi_col.c:707-758`) füllt
  `MAP_COL[MAXCOLOURS]` aus `MAP_COL_ROM = {0,15,1,2,4,6,3,5,7,8,9,10,12,14,
  11,13}` (`vdi_col.c:28-29`) plus `MAP_COL[1] = numcolors - 1`
  (`vdi_col.c:737`) — **das ist keine Identität**, und es hängt dynamisch von
  `numcolors` ab.
- Die "Hardware-Register"-Zahl, die `MAP_COL[colnum]` liefert, ist unter
  Truecolor **kein DAC-Register mehr**, sondern ein Index in eine
  **pro-Workstation** softwareseitige Farbtabelle:
  `vdi/vdi_defs.h:153-158`:
  ```c
  typedef struct {
      UWORD palette[256];    /* pseudo-palette with pixel value RRRRRGGGGG0BBBBB */
      WORD req_col[256][3];  /* requested colour */
  } VwkExt;
  ```
  (Kommentar ist wörtliches Zitat — bestätigt das exakte Bit-Layout:
  **5 Bit R @ Bit 15-11, 5 Bit G @ Bit 10-6, Bit 5 immer 0, 5 Bit B @ Bit
  4-0** — das ist **RGB555, in ein 16-Bit-RGB565-Slot gepackt mit stets
  gelöschtem grünem LSB**, nicht "echtes" 5:6:5. `vdi_col.c:599`
  ("Note: as in TOS4, the VDI never sets the least-significant bit of
  green") bestätigt das als Absicht, nicht Zufall.
- Jeder Zeichenpfad (Linie, Fill, Text, Maus, Raster-Blit, Konsole) löst den
  gespeicherten PEL-Wert **erst am Schreibpunkt** über `CUR_WORK->ext->palette`
  auf: `vdi_line.c:459` (`swblit_rect_common16`), `vdi_line.c:1641`,
  `vdi_line.c:2102`, `vdi_textblit.c:764`, `:1042`, `vdi_raster.c:1234`
  (`vrt_cpyfm16`: `fgcol = palette[info->fg_col]`), `vdi_mouse.c:752`,
  `bios/conout.c:164-165,565`.

⇒ **`MAP_COL` entfällt bei Truecolor NICHT** (Gegenteil der in der Aufgabe
formulierten Vermutung) — es bleibt exakt der gleiche Mechanismus, nur zeigt
`MAP_COL[colnum]` jetzt in eine **Software**-Tabelle statt in ein
Hardware-DAC-Register. Farben fließen weiterhin als **PEL 0..255** durch
Linien/Fill/Text/Cursor-Primitiven — nur Roh-Bitmap-Daten (MFDB-Pixel) sind
direktes RGB.

### 1.3 Gültiger Pen-Bereich unter Truecolor: 0..255, nicht 0..15

`numcolors` ist als `#define numcolors DEV_TAB[13]` (`vdi_defs.h:98`)
definiert. `update_rez_dependent()` setzt `DEV_TAB[13] = (v_planes<8) ?
(1<<v_planes) : 256` (`vdi_control.c:206`) — für `v_planes==16` also **256**,
nicht 65536 und nicht 16. Der Grund: `EXTENDED_PALETTE = (CONF_WITH_VIDEL ||
CONF_WITH_TT_SHIFTER)` (`vdi_defs.h:20`) ist für unser Profil wahr, also ist
`MAXCOLOURS=256` (`vdi_col.c:18-22`) und `REQ_COL[16]`/`req_col2[240]`
(`vdi_defs.h:250-251`) sind beide aktiv — der VDI-"Pen"-Raum ist **immer**
0..255 sobald `CONF_WITH_VIDEL` an ist, Truecolor oder nicht. Alle
Bereichsprüfungen (`vs_color16` `vdi_col.c:628`, `vdi_vq_color`
`vdi_col.c:854`, `validate_color_index` `vdi_control.c:224-230`) nutzen
`numcolors` = 256 für unser Zielprofil.

### 1.4 Empfehlung im Detail

| Ebene | EmuTOS-Referenz | NewTOS-Ziel |
|---|---|---|
| Attribut-Farbe (Linie/Fill/Text/Marker/Cursor) | PEL 0..255 über `MAP_COL`/pro-Vwk-`palette[256]` (§1.2b) | **PEL-Modell beibehalten**, `Pel = u8` reicht (0..255 passt), aber Bereich/Range und Tabellengröße auf 256 heben |
| Roh-Pixel in Memory-MFDB (`fd_nplanes=16`) | 16-bit RGB565-chunky, kein Palette-Lookup (§1.2a) | **Wireformat auf 16 bit/Pixel packed umstellen** — Contract, erzwungen (§3.4) |
| Physischer Bildschirmspeicher (Pi4) | n/a (Falcon-VRAM ist 16-bit) | **32-bpp XRGB8888 beibehalten** (frei/intern), Backend expandiert 16→32 exakt am Schreibpunkt |
| Farbtabellen-Ort | pro-`Vwk` (`vwk->ext->palette[256]`) | **von Backend-global auf pro-`Vwk` verschieben** (§3.5) — Contract |

---

## 2. Exakte Workstation-Werte für Truecolor

Alle Werte aus `update_rez_dependent()` (`vdi/vdi_control.c:193-214`), von
`vdi_v_opnwk` nach dem Neuladen der ROM-Templates aufgerufen
(`vdi_control.c:533-539`):

| Feld | Formel | EmuTOS file:line | Wert für `v_planes=16` |
|---|---|---|---|
| `DEV_TAB[13]` (Farbzahl) | `(v_planes<8) ? (1<<v_planes) : 256` | `vdi_control.c:206` | **256** |
| `DEV_TAB[35]` (Color-Capability) | `(v_planes==1) ? 0 : 1` | `vdi_control.c:207` | 1 |
| `DEV_TAB[39]` (Palette-Size, `get_palette()`) | s.u. | `vdi_control.c:208`, `bios/screen.c:831-869` | **0** |
| `INQ_TAB[4]` (Planes) | `= v_planes` | `vdi_control.c:210` | **16** |
| `INQ_TAB[5]` (Video-LUT vorhanden) | `(v_planes==16 \|\| monitor==MON_MONO) ? 0 : 1` | `vdi_control.c:211-213` | **0** |
| `DEV_TAB[3]/[4]` (Pixelgröße) | `get_std_pixel_size` (VIDEL/TT-Pfad) | `bios/screen.c:872-876,895-901` | 278/278 (unverändert, planes-unabhängig, bereits korrekt in V1 gefixt) |

**`DEV_TAB[39]` im Detail** (`get_palette()`, `bios/screen.c:831-869`):
```c
if (has_videl) {
    WORD mode = vsetmode(-1);
    if ((mode&VIDEL_COMPAT) || ((mode&VIDEL_BPPMASK) == VIDEL_4BPP))
        return 4096;
    return 0;
}
```
`VIDEL_BPPMASK=0x0007`, `VIDEL_4BPP=2`, `VIDEL_TRUECOLOR=4` ("65536 colours",
`include/biosdefs.h:153-158`). Ein echter (nicht ST-kompatibler) Truecolor-
Modus hat `mode&VIDEL_COMPAT==0` und `bpp==VIDEL_TRUECOLOR≠VIDEL_4BPP` ⇒
**`get_palette()` liefert 0**, also `DEV_TAB[39]=0`.

**Wichtig — das ist bereits die ZWEITE Korrektur dieses Feldes:** Der aktuelle
NewTOS-Code (`tos-api/src/vdi/workstation.rs:23-29,185`) trägt
`PALETTE_PRECISION=4096` mit dem Kommentar "get_palette() … EmuTOS returns
4096 for any STe/TT-style colour shifter" — das war für das **vorherige**
16-Farben-VIDEL-4bpp-Ziel korrekt (`(mode&VIDEL_BPPMASK)==VIDEL_4BPP` trifft
zu). Für das **neue** Truecolor-Ziel ist der Wert **0**, nicht 4096.
Der Spec-Doc-Eintrag `docs/api/vdi/workstation/01-v_opnwk.md:221` ("`[39]=16`")
war schon vor diesem Wechsel falsch (SPEC-V1-A, siehe `v1-control-workstation.md:157`)
— **beide Korrekturen sollten in einem Schritt zusammengeführt werden**
(direkt auf den Truecolor-Zielwert 0 korrigieren, nicht erst auf 4096 und
dann nochmal auf 0).

**`vdi_vq_extnd` (Opcode 102)** kopiert nur die bereits gepatchten
`DEV_TAB`/`INQ_TAB`-Globals in `INTOUT` (`vdi/vdi_control.c:619-651|` genauer
`:626-654`) — keine eigene Truecolor-Logik. Sobald `update_rez_dependent()`
korrekt ist, ist `vq_extnd` es automatisch mit.

---

## 3. Jede betroffene Stelle in unserem Code

### 3.1 `tos-api/src/vdi/workstation.rs` (`DEV_TAB`/`INQ_TAB`)

- `workstation.rs:23-29` `PALETTE_PRECISION: i16 = 4096` → muss **0** werden
  für Truecolor (§2).
- `workstation.rs:129,183` `let ncol = backend.num_colors(); dev[13] = ncol
  as i16;` — funktioniert automatisch, **wenn** `num_colors()` künftig 256
  liefert (§3.6).
- `workstation.rs:184` `dev[35] = if ncol <= 2 { 0 } else { 1 }` — Formel ist
  eigentlich `v_planes==1` (`vdi_control.c:207`), nicht `ncol<=2`; für unser
  Profil (immer `v_planes=16`) liefert sie zufällig denselben Wert (1), aber
  die Formel ist konzeptionell falsch und sollte auf eine echte
  `backend.planes()`-Abfrage umgestellt werden (Hygiene, kein Bug im
  Truecolor-Zielprofil selbst).
- `workstation.rs:233-234` — dieselbe `dev[13]`/`dev[35]`-Logik nochmal in
  `vq_extnd`, dieselbe Korrektur nötig.
- **`workstation.rs:245` `inq[4] = ncol.trailing_zeros() as i16; // number of
  planes = log2(colours)`** — **das ist die zentrale, jetzt sichtbar
  werdende Verwechslung** aus §1: "Planes" und "log2(Farbzahl)" fallen nur
  im palettenbasierten Modell zusammen (16 Farben = 4 Planes). Unter
  Truecolor ist `ncol=256` (log2=8) aber `v_planes=16` — **`trailing_zeros`
  liefert 8 statt der geforderten 16** (`vdi_control.c:210`). Das
  `RasterBackend`-Trait braucht eine **eigene `planes()`-Methode**,
  entkoppelt von `num_colors()` (§3.6).
- `workstation.rs:246` `inq[5] = if ncol > 2 { 1 } else { 0 }` — muss für
  Truecolor **0** werden (`v_planes==16 → INQ_TAB[5]=0`,
  `vdi_control.c:211-213`), nicht von `ncol` abhängen.
- `workstation.rs:139,147,149,160` `validate_color_index(pb.intin(n), ncol)`
  — automatisch korrekt sobald `ncol=256`.

### 3.2 `tos-api/src/vdi/inquiry.rs`

- Modul-Doku `inquiry.rs:5-7`: *"Colours round-trip through
  `MAP_COL`/`REV_MAP_COL`; NewTOS stores PEL directly in the `Vwk` and uses an
  identity 16-entry map"* — das ist laut §1.2 **so für EmuTOS selbst nicht
  richtig**: `MAP_COL_ROM` ist keine Identität (`vdi_col.c:28-29`), und
  `vdi_vql_attributes` liest `REV_MAP_COL[vwk->line_color]` zurück
  (`vdi_line.c:158`, gesehen im Rahmen dieser Untersuchung) statt den
  gespeicherten Pel direkt zu echoen. **`vql/vqm/vqf/vqt_attributes`
  (`inquiry.rs:20-67`) müssten bei einem 1:1-`MAP_COL` `REV_MAP_COL[...]`
  statt `vwk.line_color as i16` roh ausgeben.** Das ist ein **bereits
  bestehender Gap im 16-Farb-Modell**, den der Truecolor-Umbau zwangsläufig
  aufdeckt, weil `MAP_COL` jetzt (§3.5) real permutiert statt Identität ist —
  siehe §4 (Interaktion mit V5-008).
- `inquiry.rs:75-107` `vq_color` — `ncol`-Parameter automatisch korrekt bei
  256; **die "actual"-Quantisierung (`flag≠0`) muss aber auf die
  Truecolor-Formel umgestellt werden** (§4, V3-010).

### 3.3 `tos-api/src/vdi/req_col.rs` + `tos-api/src/vdi/attributes.rs`
(Farb-Setter)

- `req_col.rs:23-26` `const REQ_COL_COUNT: usize = 16` — Modell ist
  **device-global**, per Modul-Doku bewusst so für den **Nicht-Truecolor**-
  Pfad (`vdi_col.c:690,821`, wo `REQ_COL`/`req_col2` tatsächlich
  geräteglobal sind). Für Truecolor ist das **kein** passendes Modell mehr
  (§1.2b/§3.5): dort ist `req_col`/`palette` **pro-`Vwk`**
  (`vwk->ext->req_col[256][3]`, `vdi_defs.h:157`). `req_col.rs` bleibt für den
  Nicht-Truecolor-Fall richtig (bleibt unverändert), **zusätzlich** wird für
  Truecolor eine parallele **pro-Workstation**-Struktur gebraucht (§3.5,
  §3.6) — keine Erweiterung von `req_col.rs` selbst.
- `attributes.rs:139-140,226-267` (`vsm_color`, `vs_color`, `vsl_color`,
  `vsf_color`, `vst_color`) — `validate_color_index(pb.intin(0), ncol)`
  automatisch korrekt bei `ncol=256`; **aber `vs_color` (Opcode 14,
  `attributes.rs:226`, dispatcht `mod.rs:263`) muss unter Truecolor komplett
  anders arbeiten**: EmuTOS verzweigt in `vdi_vs_color`
  (`vdi_col.c:656-667`) nach `vs_color16` (`vdi_col.c:620-649`) →
  `set_color16` (`vdi_col.c:601-614`, 5-Bit-Quantisierung + `MAP_COL`-Indirektion
  in `vwk->ext->palette`) statt in die DAC-Programmierung
  (`set_color`/`Setcolor`/`VsetRGB`, `vdi_col.c:526-583`). Unser
  `attributes::vs_color` ruft heute `backend.set_color(...)`
  (hardware-CLUT-Modell) — für Truecolor braucht es stattdessen den
  Truecolor-Zweig: 5-Bit-Quantisierung + Schreiben in **die aufrufende
  `Vwk`s eigene** Farbtabelle, nicht das Backend.
- **`attributes.rs:308-327` (`vsf_udpat`, Opcode 112)** — `let planes =
  ncol.trailing_zeros() as i16; // INQ_TAB[4]` (`:317`) — **dieselbe
  Verwechslung wie workstation.rs:245**, hier zur Validierung der
  Multi-Plane-Pattern-Wortzahl (`count == planes*16`). Für Truecolor muss
  das echte `v_planes=16` sein (`64*16=1024` Worte für ein Vollfarb-Muster,
  nicht `8*16=128`). Braucht dieselbe `planes()`-Entkopplung wie
  workstation.rs. (Kreuzbezug: V2-009, §4.)

### 3.4 `tos-api/src/vdi/backend.rs` + `kernel/src/vdi_backend.rs` (Pel/Palette/Memory-Modell) — **größte strukturelle Änderung**

- `kernel/src/vdi_backend.rs:21` `const NUM_COLORS: u16 = 16;` → **256**.
- `kernel/src/vdi_backend.rs:26-43` `DEFAULT_PALETTE`/`PALETTE`/`REQ_COL`
  (`:49,54`) — feste 16-Einträge-Arrays, **backend-global** (eine Instanz
  für den ganzen Bildschirm). Muss auf 256 Einträge wachsen **und** — per
  §1.2b/§3.5 — konzeptionell **pro-Workstation** werden statt
  Backend-Singleton (die Größenänderung allein reicht nicht).
- `kernel/src/vdi_backend.rs:107-110` `pel_to_rgb` — die 8-Bit-RGB→32bpp-
  Auflösung; wird zur Truecolor-Auflösung `RGB565(5:5:5+0) → XRGB8888`
  (Rundungs-/Quantisierungslogik wie `vdi2fivebits`/`fivebits2vdi`,
  `vdi_col.c:588-591,804-807`), nicht mehr "welche der 16 GEM-Standardfarben".
- **`tos-api/src/vdi/raster.rs:21-36` (Moduldoku) + `:47-53`
  (`CHUNKY_PLANES: i16 = 32`) + `:55-171` (`Mfdb`/`get_mem`/`put_mem`, alles
  `u32`)** — das App-sichtbare Memory-MFDB-Format ist heute **32-bpp
  XRGB8888 chunky**. Für Truecolor-Konformität (§0 Punkt (b)) muss das
  **16-bit RGB565-chunky** werden: `get_mem`/`put_mem` auf `u16` umstellen,
  `mem_stride_px`/`mem_wdwidth`/`mem_capacity` entsprechend, `CHUNKY_PLANES`
  von 32 auf **16**. Das betrifft:
  - `raster.rs:229-297` `vrt_cpyfm` (Memory-Zweig `:287-296`,
    `mono_to_mem` `:307-356`) — `fg_px`/`bg_px` würden dann 16-bit-Werte
    (`backend.pel_to_device(fg)` müsste ein `u16` statt `u32` liefern, oder
    ein zweites `pel_to_device16`).
  - `raster.rs:364-410` `vro_cpyfm` (Plane-Match-Gate `:382-396`,
    `CHUNKY_PLANES`-Vergleich) — Gate-**Prinzip** bleibt richtig
    (Bildschirm muss als Memory-kompatible Tiefe auflösen), der
    **Wert** ändert sich von 32 auf 16 für Truecolor-MFDBs.
  - `raster.rs:412-521` `screen_to_mem`/`mem_to_screen`/`mem_to_mem`
    (nutzen `get_device_pixel`/`put_device_pixel`, aktuell `u32`) — die
    Screen-Seite bleibt 32-bpp (physischer FB, frei), die
    **Memory**-Seite wird 16-bit; die Konvertierung 16↔32 muss an genau
    dieser Grenze passieren (in `screen_to_mem`/`mem_to_screen`, nicht in
    `Mfdb` selbst).
  - `raster.rs:483-521` `bb_save_words`/`bb_save`/`bb_restore` (AES'
    eigener Backing-Store) — **Ableitung, nicht Zitat direkt für uns, aber
    EmuTOS-belegt:** `aes/gemgsxif.c:334` `memsize(sw/16,sh,
    gl_tmp.fd_nplanes)` und `:525` `pfd->fd_nplanes = gl_nplanes` (=
    `v_planes`, hier 16) zeigen, dass **auch AES' eigener bb_save-Puffer**
    unter Truecolor mit 16 bit/Pixel arbeitet, nicht mit 32. Da
    `vro_cpyfm`/`vrt_cpyfm` denselben Code für App- und AES-Puffer
    durchlaufen, ist ein **einheitliches** 16-bit-Memory-MFDB-Format (statt
    zweier paralleler Formate) sowohl der einfachere **als auch** der
    EmuTOS-treuere Weg.
  - **`raster.rs:523-543` — der bestehende STOP-Block für `vr_trnfm`
    Truecolor (AUD-V5-002/003)** wird durch diesen Plan **aufgelöst**: die
    "erfundene" 32-bpp-Zieltiefe entfällt, weil das Memory-Format jetzt
    16-bit ist — `vr_trnfm16` (`vdi_raster.c:190-256`) kann dann **wörtlich**
    portiert werden (Bit-für-Bit-Transpose auf `WORD*`, kein Format-Sprung
    mehr nötig). Das ist der wichtigste Gewinn dieses Plans für V5.
- `backend.rs:19-22` (`Pel = u8`) — bleibt unverändert ausreichend (0..255
  passt in `u8`).
- `backend.rs:176-186` `pel_to_device(&self, pel: Pel) -> u32` — Doku zitiert
  bereits korrekt `vrt_cpyfm16` (`vdi_raster.c:1234-1236`); die
  **Rückgabetiefe** muss aber für ein 16-bit-Memory-MFDB auf `u16`
  umgestellt werden (oder eine zusätzliche `pel_to_device16`-Methode neben
  der bestehenden 32-bit-Screen-Variante).

### 3.5 Farbtabelle: Backend-global → pro-`Vwk` (neuer Struktur-Bedarf)

Nicht explizit in der Aufgabe genannt, aber zwingend aus §1.2b: EmuTOS
allokiert `vwk->ext` **pro Workstation** — physisch als `static VwkExt
phys_work_ext` (`vdi_control.c:51`, zugewiesen `vdi_control.c:554-558`),
virtuell als Anhängsel hinter dem `Mxalloc`-Block
(`vdi_control.c:438-452`: `size += sizeof(VwkExt)`, `vwk->ext = (VwkExt*)(vwk+1)`).
`init_wk` seedet jede neue `VwkExt` unabhängig aus dem geräteglobalen
Default (`vdi_control.c:354-363`, kopiert `REQ_COL`/`req_col2` →
`vwk->ext->req_col`, dann `set_color16` für alle 255 Einträge). Danach
schreibt `vs_color16` **nur noch in die eigene** `vwk->ext` — zwei offene
virtuelle Workstations können unter Truecolor also **unterschiedliche**
Farben auf demselben Pen-Index tragen; das ist ein testbares,
EmuTOS-dokumentiertes Verhalten.

**Betroffen:** `tos-api/src/vdi/vwk.rs` (`Vwk`-Struct, `:50-95`) braucht ein
neues Feld (z. B. `ext: Option<&'static VwkExtCell>` oder eine
eingebettete `[u16; 256]`+`[[i16;3]; 256]`), analog zu EmuTOS' `vwk->ext`.
`kernel/src/vdi_backend.rs`s heutiges Backend-Singleton-`PALETTE` bleibt
**nur** für den (Nicht-Truecolor-)Hardware-CLUT-Fall passend; für Truecolor
zieht die Farbtabelle in `tos-api`s `Vwk`-Schicht um. Das ist eine
Verantwortungs-Verschiebung von `kernel/` nach `tos-api/`, kein reiner
Wertewechsel.

### 3.6 `RasterBackend`-Trait: `planes()` von `num_colors()` entkoppeln

Zusammenfassung der in §3.1/§3.3 mehrfach aufgetauchten Konflation: unter
Truecolor gilt **`v_planes` (16) ≠ log2(`numcolors`=256) (8)**. Das
`RasterBackend`-Trait (`backend.rs:79-215`) braucht eine eigene Methode
(z. B. `fn planes(&self) -> u16`), die `INQ_TAB[4]`
(`vdi_control.c:210`), `DEV_TAB[35]` (`vdi_control.c:207`, eigentlich
`v_planes==1`) und die `vsf_udpat`-Multi-Plane-Wortzahl (`vdi_fill.c:149-166`,
unser `attributes.rs:317`) korrekt speist, statt sie aus `num_colors()`
herzuleiten.

---

## 4. Interaktion mit den offenen VDI-Findings (V2/V3/V4/V5/V6)

### 4.1 Reihenfolge: Truecolor MUSS vor V2/V3/V4 UND faktisch vor dem Rest von V5 kommen

`00-MASTER.md` sortiert `VW1 = V5` (Raster/Backend) als ersten offenen
Fix-Block; **V5-002/003 SIND bereits die Truecolor-Lücke** (fehlender
`vr_trnfm16`-Zweig, planar-vs-chunky-Pipeline-Bruch,
`v5-raster-blit.md:40-50`). Dieser Plan liefert die fehlende
Referenz/Entscheidung für genau diese zwei Findings. Empfehlung: **VW1 (V5)
wird faktisch zum "Truecolor-Fundament"-Paket** — die in §3.4/§3.5/§3.6
beschriebenen Struktur-Änderungen laufen **innerhalb** von VW1, bevor V5-001/
004/005 (orthogonale Bugs, §5) und alle späteren Pakete (VW2 erneut, VW3-VW6)
angefasst werden. **V1 (Workstation, bereits 🟩 committed in `727b33c`) muss
neu geöffnet werden** — die dort committeten Werte (`DEV_TAB[13]=16`,
`DEV_TAB[39]=4096`, `INQ_TAB[4]=trailing_zeros(ncol)`) sind für das neue
Zielprofil falsch (§2/§3.1). Das ist eine Folgeaktion für den Lead
(Gate-Zeile `00-GATE.md` auf 🟡 zurücksetzen), die dieser Plan nicht selbst
vornimmt (außerhalb des Analyse-Auftrags).

### 4.2 Konkret erfragte Findings

**V3-010 (`vq_color` flag≠0 ohne DAC-Quantisierung) — wird durch Truecolor
NICHT gegenstandslos, sondern bekommt eine andere Quantisierungsformel.**
Das ursprüngliche Finding beschreibt den (Nicht-Truecolor-)DAC-Rundungsfehler
(z. B. Videl 500→502 über `vdi2videl`/`videl2vdi`, `vdi_col.c:456-467`).
Für Truecolor ist "actual" kein Hardware-Readback mehr, sondern die
**5-Bit-Rundung** der pro-Vwk-Pseudo-Palette: `vq_color16`
(`vdi_col.c:813-838`) liest `ext->palette[palnum]` und wendet
`fivebits2vdi(rgb>>11/6/0)` (`vdi_col.c:804-807`) an — das ist eine **andere,
eigenständige** Quantisierungsfunktion (Skalierung 0..31 ↔ 0..1000), die neu
zu implementieren ist, nicht nur der bestehende DAC-Pfad umbenannt. Der
NewTOS-Fix für V3-010 (`inquiry.rs:75-107`) muss also **zwei** Pfade kennen:
Nicht-Truecolor DAC-Quantisierung (bestehendes Finding) und
Truecolor-5-Bit-Quantisierung (neu, dieser Plan).

**V5-008 (`vrt_cpyfm` `MAP_COL[]` nicht angewandt — als "LOW/acceptable"
eingestuft) — Klassifikation ist NUR unter der heutigen Identitäts-`MAP_COL`
haltbar und muss nach diesem Umbau fallen.** Die Begründung im Audit
(`v5-raster-blit.md:65-67`: *"MAP_COL konsistent in Palette-Reihenfolge
eingefaltet … Ergebnisfarbe identisch, unbeobachtbar"*) gilt nur, solange
`MAP_COL` bei uns Identität ist. Sobald `MAP_COL`/`REV_MAP_COL` wie in §1.2b/
§3.2 real (nicht-identitäre, von `numcolors` abhängige) Permutation wird —
was für ein EmuTOS-treues Truecolor-Attributmodell nötig ist, weil
`vwk->line_color` etc. bereits `MAP_COL[...]` sind (`vdi_control.c:284` etc.)
— dann **muss** auch `vrt_cpyfm`s `fg`/`bg` (`INTIN[1]`/`[2]`, rohe VDI-Pen-
Nummern) durch `MAP_COL[]` laufen, exakt wie EmuTOS es tut
(`cpy_raster:1391,1395`, zitiert im Original-Finding). Sonst zeigt
`vrt_cpyfm` sichtbar eine andere Farbe als `vsl_color`/`vsf_color` für
denselben Pen — das ist dann ein **echter Bug**, keine akzeptable Abweichung
mehr. **Reklassifizierung: V5-008 wird Teil des Truecolor-Pakets, nicht mehr
"acceptable".**

**V6-001 (Cursor-Save-under, PEL-Roundtrip statt bit-exakt) — wird vom
"latenten, bei Truecolor-Content sichtbaren" Gap zum sofort relevanten
Muss-Fix.** Das Finding (`v6-mouse-input.md:37-43`) benennt bereits explizit:
*"sichtbar bei Truecolor-Bildbetrachter"*. Mit diesem Plan ist Truecolor
nicht mehr hypothetisch, sondern das aktuelle Zielprofil — jedes Bild/Icon
mit einer RGB565-Farbe außerhalb der (ohnehin nur 5-Bit-genauen, §1.2b)
Pen-Palette wird vom heutigen `read_pixel`/`fill_rect`-Rundtrip
(`mouse.rs:225-267`, rundet auf den nächsten Pen) sichtbar korrumpiert. Die
im Finding vorgeschlagene Lösung (`get_device_pixel`/`put_device_pixel`
statt `read_pixel`/`fill_rect`) ist unverändert richtig und unabhängig von
der Farbtiefe der zugrundeliegenden Werte — sie sollte **innerhalb** des
Truecolor-Pakets (nicht separat in VW6) mit erledigt werden, weil sie exakt
dieselbe 16↔32-Konvertierungsgrenze berührt wie `screen_to_mem`/
`mem_to_screen` (§3.4).

### 4.3 Weitere, in der Aufgabe nicht namentlich genannte, aber betroffene Findings

- **V2-009** (`vsf_udpat` Multi-Plane auf Plane 0 reduziert,
  `v2-line-marker-fill.md:90-94`) — die dort beschriebene
  Backend-Limitierung ("Pel-Backend ignoriert Multi-Plane") bleibt
  unverändert bestehen; **zusätzlich** braucht die Wortzahl-Validierung
  jetzt `planes=16` statt `log2(ncol)` (§3.3) — zwei unabhängige Teile
  desselben Findings.
- **V1-004** (DEV_TAB[3]/[4] Pixelgröße) — bleibt unverändert korrekt/gültig
  (278/278, planes-unabhängig).
- **SPEC-V1-A** (`01-v_opnwk.md:221`, `DEV_TAB[39]`) — Korrektur-Ziel ändert
  sich von 4096 (VIDEL-4bpp) auf 0 (Truecolor), siehe §2.

---

## 5. Bewertung V5-001/004/005 (32-bpp-Memory-MFDB-Modell)

- **V5-001** (`vrt_cpyfm` strippt `PAT_FLAG` nicht) — **bleibt vollständig
  valide, unverändert.** Der Fund betrifft nur das `INTIN[0]`-Mode-Byte-
  Parsing (`cpy_raster:1341,1397`), ist orthogonal zur Pixeltiefe. Kein
  Bezug zum Truecolor-Umbau.
- **V5-004** (`vro_cpyfm` Plane-Count-Gleichheits-No-Op fehlt) — **Prinzip
  bleibt valide, der konkrete Wert (`CHUNKY_PLANES=32`) wird durch diesen
  Plan obsolet.** Das Finding fordert, dass EmuTOS' `s_nxwd != d_nxwd`-Gate
  (`vdi_raster.c:1359-1360`) nachgebildet wird — richtig. Aber sobald das
  Memory-MFDB-Format auf 16-bit wechselt (§3.4), muss die "effektive Tiefe"
  des Bildschirms für dieses Gate von 32 auf **16** neu aufgelöst werden
  (Bildschirm-MFDB unter Truecolor: `fd_nplanes` effektiv `v_planes=16`,
  nicht mehr die interne 32-bpp-Rasterbreite). **Der Fix für V5-004 sollte
  also erst nach/mit dem Truecolor-Struktur-Umbau erfolgen**, sonst wird ein
  Wert committet, der sofort wieder falsch ist.
- **V5-005** (`vrt_cpyfm` nur Screen-Dest; Memory-Ziel No-Op) — **bleibt
  valide als Lücke, das Ziel-Format des Fixes ändert sich.** Der Fund selbst
  (`setup_info` erlaubt einen Memory-`d_form`, unser Code lehnt ihn ab,
  `raster.rs:223`/heute `:238`) bleibt bestehen; die Implementierung des
  Fixes muss aber direkt gegen das **16-bit**-Memory-Format arbeiten (§3.4),
  nicht gegen das heutige 32-bpp-`Mfdb`. Ein Fix, der V5-005 heute gegen das
  32-bpp-Modell schließt, würde beim Truecolor-Umbau doppelt gemacht werden
  müssen — **daher: V5-005 zurückstellen, bis der Memory-Format-Umbau
  (§3.4/§6 Schritt 3) steht, dann in einem Zug fixen.**

---

## 6. Sequenzierter Implementierungsplan

Reihenfolge nach Abhängigkeit; jeder Schritt einzeln testbar (Selbsttest +
gezielte Runtime-Verifikation, wie im bisherigen VDI-Audit-Prozess). Platz im
bottom-up VDI-Master: dieser gesamte Block ersetzt/erweitert **VW1 (bisher
"V5 Raster/Backend-Fundament")** und macht **VW2 (V1, bereits committed)**
für Schritt 6 erneut nötig.

1. **Backend-Pixel-Modell: `planes()` entkoppeln + `NUM_COLORS`→256.**
   `RasterBackend`-Trait um `planes()` erweitern (§3.6); `kernel/src/
   vdi_backend.rs` `NUM_COLORS`/`PALETTE`/`REQ_COL` auf 256 Einträge heben
   (Zwischenschritt, bevor die Tabelle in §4 pro-Vwk wandert — hält den
   Schritt klein und einzeln testbar). Test: `v_opnwk` liefert
   `DEV_TAB[13]=256` an ein selbstgebautes `INTOUT`-Dump.
   *Abhängigkeiten:* keine.

2. **Pro-Vwk-Farbtabelle (`VwkExt`-Äquivalent).** `tos-api/src/vdi/vwk.rs`
   `Vwk` um die 256er-Palette/`req_col`-Felder erweitern (§3.5); `init_wk`
   seedet sie aus dem geräteglobalen Default (analog `vdi_control.c:354-363`).
   Test: zwei `v_opnvwk`, unterschiedliches `vs_color(5,...)` auf jedem,
   `vq_color(5, flag=1)` liefert pro Handle unterschiedliche Werte.
   *Abhängigkeiten:* Schritt 1.

3. **DEV_TAB/INQ_TAB-Werte + Inquiry.** `workstation.rs` `PALETTE_PRECISION→0`,
   `dev[35]`/`inq[4]`/`inq[5]` auf `planes()`-basierte Formeln umstellen
   (§3.1); `01-v_opnwk.md` (Spec-Doc) auf die neuen Werte korrigieren
   (SPEC-V1-A final). Test: `vq_extnd` Mode 1 liefert `INQ_TAB[4]=16,
   [5]=0`.
   *Abhängigkeiten:* Schritt 1.

4. **Farb-Setter/-Query auf Truecolor-Zweig (`vs_color16`/`vq_color16`,
   `MAP_COL` real).** `attributes::vs_color` verzweigt (analog
   `vdi_col.c:656-667`) in einen Truecolor-Pfad: 5-Bit-Quantisierung
   (`vdi2fivebits`/`fivebits2vdi`) + Schreiben in die **aufrufende Vwk's**
   Tabelle (Schritt 2) statt ins Backend; `MAP_COL`/`REV_MAP_COL` real
   (nicht-identitäre) Permutation implementieren (§1.2b/§3.2); `vqm/vqf/vqt/
   vql_attributes` auf `REV_MAP_COL[...]` umstellen; `vq_color` (V3-010)
   für den Truecolor-Zweig fixen (§4.2). Test: `vs_color(1, 0,0,0)` (Pen 1
   auf Schwarz) und Rücklesen über `vq_color(1, flag=1)` rundet exakt wie
   EmuTOS' 5-Bit-Quantisierung.
   *Abhängigkeiten:* Schritt 2, 3 (numcolors=256 muss stehen, damit
   `validate_color_index` den vollen Bereich zulässt).

5. **Memory-MFDB-Wireformat auf 16-bit umstellen (`raster.rs`).** `Mfdb`
   von `u32`- auf `u16`-Pixel umbauen (`get_mem`/`put_mem`/
   `mem_stride_px`/`mem_capacity`/`mem_wdwidth`), `CHUNKY_PLANES: 32→16`;
   `screen_to_mem`/`mem_to_screen` an der Screen-Grenze 16↔32 konvertieren
   (§3.4); `pel_to_device` auf `u16` umstellen oder eine `_16`-Variante
   ergänzen; `bb_save_words`/`bb_save`/`bb_restore` auf das 16-bit-Format
   umstellen (einheitlich für App- und AES-Puffer, §3.4). **Dies löst
   V5-004/005 in einem Zug** (§5). Test: `bb_save`/`bb_restore`-Rundtrip
   (Dialog-Box-Hintergrund) bit-exakt bei 256 Farben; ein selbstgebautes
   16-bit-RGB565-Testbild via `vro_cpyfm` (Mode 3, replace) landet
   unverändert auf dem Schirm.
   *Abhängigkeiten:* Schritt 1 (planes-Wissen), unabhängig von 2-4.

6. **`vr_trnfm16` + `vro_cpyfm16`/`vrt_cpyfm16`-Truecolor-Kern.** Mit dem
   16-bit-Memory-Format aus Schritt 5 steht jetzt eine 1:1-Referenz für den
   in `raster.rs:523-543` dokumentierten STOP-Block: `vr_trnfm16`
   (`vdi_raster.c:190-256`) wörtlich portieren (Standard↔Device-Transpose
   auf `WORD*`, kein Formatsprung mehr). `vro_cpyfm16`/`vrt_cpyfm16`
   (bereits als "1:1 (16 Ops)"/"1:1 (4 MD)" im V5-Audit gegen
   `vdi_backend.rs:432/392` verzeichnet) auf das neue 16-bit-Memory-Format
   verdrahten, `MAP_COL`-Anwendung ergänzen (löst V5-008 auf, §4.2). Test:
   `vr_trnfm` Standard→Device→`vro_cpyfm`-Pipeline (ein Programm, das ein
   Standard-Truecolor-Bitmap lädt, transformiert und blittet) liefert
   pixel-identisches Ergebnis zum unkonvertierten direkten Schreiben.
   *Abhängigkeiten:* Schritt 5.

7. **Cursor-Save-under auf `get_device_pixel`/`put_device_pixel` (V6-001).**
   Aus VW6 vorgezogen (§4.2): `mouse.rs::draw`/`erase` auf die
   bit-exakten Device-Pixel-Zugriffe umstellen statt der PEL-Rundtrip.
   Test: Cursor über ein Truecolor-Testbild (Farbverlauf/Foto) bewegen,
   Pixel unter dem Cursor vor/nach Restore bit-identisch.
   *Abhängigkeiten:* Schritt 5 (dieselbe 16↔32-Konvertierungslogik).

8. **Gate-Folgeaktion (Lead, außerhalb dieses Plans):** `00-GATE.md`-Zeile
   "V1 control/workstation/inquiry" von 🟩 zurück auf 🟡 setzen (die in
   `727b33c` committeten 16-Farb-Werte sind für das neue Zielprofil falsch,
   §2/§4.1); V1 nach Schritt 3 dieses Plans erneut grün auditieren, bevor
   VW2-VW6 (V2/V3/V4/V6) angefasst werden.

---

## 7. Offene Sub-Entscheidungen für Lead/User

1. **Physischer Framebuffer bleibt 32-bpp XRGB8888** (Backend expandiert
   16→32 am Schreibpunkt) — empfohlen, frei/intern nach
   `docs/COMPATIBILITY.md`, keine EmuTOS-Gegenreferenz nötig. *Braucht
   vermutlich kein explizites Sign-off*, aber hier benannt, weil es die
   Kehrseite von Punkt 2 ist.
2. **App-sichtbares Memory-MFDB-Wireformat wird echtes 16-bit RGB565-chunky**
   (statt heutigem 32-bpp-chunky) — **erzwungen durch den Contract**
   (`vro_cpyfm16`/`vr_trnfm16` arbeiten roh auf `UWORD*`), aber die
   **Größe der Änderung** (§3.4, `raster.rs` fast komplett umzubauen,
   invalidiert die committeten V5-004/005-Constants) rechtfertigt
   ausdrückliches Lead-Go vor Schritt 5.
3. **Farbtabelle wandert von `kernel::vdi_backend` (Backend-global) nach
   `tos-api::vdi::vwk` (pro-`Vwk`)** — erzwungen durch `vwk->ext->palette`
   in EmuTOS (§3.5), aber eine Verantwortungs-Verschiebung zwischen den
   beiden Crates, die der Lead/Architektur-Eigentümer bestätigen sollte
   (welche Schicht künftig "die Farbtabelle" besitzt).
4. **V1 (bereits 🟩 committed) muss neu geöffnet werden** (§4.1/§6 Schritt 8)
   — Prozessentscheidung für den Gate-Workflow, nicht technisch strittig,
   aber explizit zu bestätigen, weil sie ein bereits abgeschlossenes Paket
   zurücksetzt.

---

## Kurz-Quellenindex (file:line, alle in dieser Session gelesen)

- `references/emutos/include/config.h:61-122,700-804,1078-1093,1580-1629,
  2151-2201` — Gating `CONF_WITH_VDI_16BIT`/`CONF_WITH_VIDEL`/
  `CONF_ATARI_HARDWARE`.
- `references/emutos/include/biosdefs.h:148,153-158` — `VIDEL_COMPAT`,
  `VIDEL_BPPMASK`, `VIDEL_4BPP`, `VIDEL_TRUECOLOR`.
- `references/emutos/vdi/vdi_defs.h:20,22,98,153-158,184,191,196,204,
  250-251` — `EXTENDED_PALETTE`, `TRUECOLOR_MODE`, `numcolors`, `VwkExt`,
  PEL-Kommentare, `REQ_COL`/`req_col2`.
- `references/emutos/vdi/vdi_control.c:48-52,58-174,178-230,269-403,
  407-460,495-568,573-599,619-654` — `SIZ_TAB_rom`/`DEV_TAB_rom`/
  `INQ_TAB_rom`, `update_rez_dependent`, `validate_color_index`, `init_wk`,
  `vdi_v_opnvwk`, `vdi_v_opnwk`, `vdi_vq_extnd`.
- `references/emutos/vdi/vdi_col.c:1-56,204-467,491-758,801-936` —
  `MAP_COL`/`MAP_COL_ROM`, `st_palette`/`videl_palette1/2`, `set_color`,
  `set_color16`, `vs_color16`, `vdi_vs_color`, `init_colors`, `vq_color16`,
  `vdi_vq_color`.
- `references/emutos/vdi/vdi_raster.c:1-330,950-1327` — MFDB/BLITVARS,
  `vr_trnfm16`, `vdi_vr_trnfm`-Gating, `vro_cpyfm16`, `vrt_cpyfm16`.
- `references/emutos/vdi/vdi_line.c:140-159,440-461` —
  `vdi_vsl_color`/`vdi_vql_attributes` (`MAP_COL`/`REV_MAP_COL`),
  `swblit_rect_common16` (Palette-Lookup).
- `references/emutos/bios/screen.c:790-913` — `get_palette`,
  `get_std_pixel_size`, `get_pixel_size`.
- `references/emutos/bios/videl.c:320-355,400-609,969-994` — `get_videl_bpp`,
  `set_videl_vga` (`VIDEL_TRUECOLOR`-Case), `videl_get_current_mode_info`.
- `references/emutos/aes/gemgsxif.c:320-340,520-546` — `bb_set`,
  `gl_nplanes`/`fix_target` (AES-eigener Truecolor-Puffer).

## Zitiert vs. abgeleitet

Alle Werte in §2 (`DEV_TAB`/`INQ_TAB`) sind **wörtlich zitiert** aus
`update_rez_dependent`/`get_palette`/`get_videl_bpp`. Die Aussage "AES'
eigener `bb_save`-Puffer ist ebenfalls 16-bit unter Truecolor" (§3.4) ist
eine **Ableitung** aus `gemgsxif.c:334,525` (`gl_nplanes` = `v_planes` = 16),
nicht ein wörtliches "16-bit"-Zitat in `gemgsxif.c` selbst — als solche
markiert. Die Empfehlung "einheitliches 16-bit-Memory-Format statt zweier
paralleler Formate" (§3.4/§6 Schritt 5) ist eine **Design-Empfehlung dieses
Plans**, durch die Ableitung gestützt, aber kein EmuTOS-Zitat für sich.

## Out-of-scope observations

- `docs/api/vdi/_state.md`, `_discrepancies.md`, `_working-list.md`,
  `attributes/_mode-clip-color.md`, `attributes/_line-marker.md`,
  `attributes/_fill.md`, `attributes/_text.md`, `inquiry/_color-pixel-input.md`,
  `input/_mouse.md`, `output/_fill.md`, `raster/_raster.md`,
  `workstation/100-v_opnvwk.md`, `workstation/102-vq_extnd.md` referenzieren
  alle das 16-Farben-Modell und brauchen nach der Umsetzung einen eigenen
  Korrektur-Durchgang — nicht in diesem Plan bearbeitet (reine
  Konversionsplanung, keine Spec-Neufassung).
- `workstation.rs:184`/`:234` (`dev[35] = if ncol <= 2 {0} else {1}`) ist
  eine konzeptionell falsche Formel (sollte `v_planes==1` sein), die im
  aktuellen wie im Truecolor-Zielprofil zufällig denselben Wert liefert —
  in §3.1/§3.6 benannt, aber kein eigenständiger Blocker.
