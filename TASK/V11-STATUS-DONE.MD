# V11 — STATUS: DONE (Diagnose bestätigt, Test rot→grün) / Pi-Abnahme offen

Weiß-Rohwort `0xFFDF` und Bit 5 (`OVERLAY_BIT`) sind isoliert: gespeichertes
Pixelwort, Schreibkonvertierung und Readback sind je einzeln belegt und durch
einen reproduzierbaren Hosttest getrennt. Bearbeitet 2026-09-21, Branch
`desk-rebuild`, Basis HEAD `194b057`, **nicht committet**, nichts geflasht.
Gemeinsames Paket mit V12 und V03 (`measurement-2026-09-21-v11-v12/`).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `vdi/vdi_control.c:610-618` (`v_clrwk`: `fill = 0xff` unter `TRUECOLOR_MODE`, `memset(v_bas_ad, fill, size)`); `vdi/vdi_fill.c:671-681` (`pixelread`: `return *addr`, ungemaskt); `:1095-1110` (`vdi_v_get_pixel`: `INTOUT[0] = 0; INTOUT[1] = pel`); `vdi/vdi_col.c:601-613` (`set_color16`: `(r<<11)\|(g<<6)\|b`, Bit 5 nie gesetzt → Stift-0-Weiß `0xFFDF`); `vdi/vdi_fill.c:26` (`OVERLAY_BIT 0x0020`), `:702-709,726-733,755` (Seed-Fill maskiert das Bit auf der Schirmseite) |
| Gemessen (Soll) | Hatari Falcon030, VIDEL-Truecolor (`$FF8266 = 0100`), EmuTOS 1.4 Release-ROM **und** gepinnter Quellstand: Fälle 54/160/161 `INTOUT[1] = 0xFFFF` — `tosdump/reference/EMUTOS14-VDI-all-falcontc.BIN` (Klassifikation `docs/analysis/vdi-v-series-classification-2026-09-21.md` § 2) |
| NewTOS `tos-api` (richtig) | `tos-api/src/vdi/workstation.rs` `v_clrwk` → `clear_screen_word(0xFFFF)`; `inquiry.rs` `v_get_pixel` → `device_to_rgb565(get_device_pixel())` |
| NewTOS Kernel (der Produzent, vorher) | `kernel/src/vdi_backend.rs` `rgb565_to_device` = `pal555_to_xrgb` (drei 5→8-Bit-Kanonen, X-Byte fest `0xFF` aus `hal::framebuffer::rgb()`), `device_to_rgb565` = obere 5 Bit je Kanone; Bit 5 hat keinen Platz, `0xFFFF` und `0xFFDF` fallen auf `0xFFFFFFFF` zusammen, Readback `0xFFDF` |

Der TT-Palettenwert 0 (`REV_MAP_COL`) ist keine Truecolor-Sollfarbe — der
Vergleich lief ausschließlich gegen die Falcon-Truecolor-Referenz.

## Reproduzierbarer Test (die drei Schritte getrennt)

`tos-api/tests/vdi_reference_overlay_bit.rs` → `vdi_reference/overlay_bit.rs`
(Auditor 2026-09-21; eigenes Binär, weil `install_host` einen Host je Prozess
erlaubt; das Backend-Double ist das Kernel-Pixelmodell Ausdruck für Ausdruck
transkribiert, mit `kernel/src/vdi_backend.rs`-Zeilenangaben):

| Test | prüft | vorher | nachher |
|---|---|---|---|
| `the_device_pixel_round_trips_the_cleared_word_0xffff` | Schreibkonvertierung + Readback des Worts `0xFFFF` am Backend-Saum | rot (`0xFFDF`) | grün |
| `v_clrwk_then_v_get_pixel_reports_the_falcon_word_0xffff` | Fälle 54/160/161 über den echten `tos_api::vdi::dispatch` | rot | grün |
| `a_cleared_pixel_and_a_pen_0_pixel_differ_by_overlay_bit_alone` | gelöschtes Wort ⊕ Stift-0-Wort = `0x0020` | rot (`0`) | grün |
| `pen_0_fill_then_v_get_pixel_reports_the_pseudo_palette_white_0xffdf` | Kontrolle: Stift 0 bleibt `0xFFDF` (5 Grünbits) | grün | grün |

Belege: `measurement-2026-09-21-v11-v12/host-tests/overlay-bit-RED-before.txt`
(verbatim), `overlay-bit-GREEN-after.txt`.

## Betroffene Funktionen, minimaler Eingriff (→ V12)

`kernel/src/vdi_backend.rs` Konvertierungspaar (`rgb565_to_device` /
`device_to_rgb565`) und alles, was darüber liest/schreibt: `v_get_pixel`,
`vro_cpyfm` Screen↔Memory (`bb_save`/`bb_restore`), `v_contourfill`-Sonde,
`clear_screen_word`; dazu `xor_pixel` (EmuTOS komplementiert das ganze Wort).
Der Eingriff ist in V12 umgesetzt: Bit 5 lebt im X-Byte des 32-bpp-Worts.

## Offen

Pi-Abnahme (nicht geflasht): Fälle 54/160/161 auf der Pi = `0xFFFF` und
unverändertes Bild. Nach `TASK/V12.md`.
