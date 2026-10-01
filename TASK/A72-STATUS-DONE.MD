# A72 — STATUS: DONE

Drei veraltete Aussagen korrigiert, gegen den heutigen Quelltext
gegengelesen. Bearbeitet 2026-09-20, Branch `desk-rebuild`, **nicht
committet**. Reine Dokumentation, keine Codeaenderung, wie vom Ticket
verlangt.

## 1. `docs/api/vdi/raster/_raster.md` — Speicher-MFDB-Format

| | |
|---|---|
| Vorher (falsch) | „32-bpp chunky, ein `u32`/Pixel, `fd_nplanes = 32`" |
| Jetzt (Beleg) | 16-bit RGB565-chunky, ein `UWORD`/Pixel, `fd_nplanes = 16` — `tos-api/src/vdi/raster.rs:21-35` (Moduldoc), `:61-67` (`CHUNKY_PLANES = 16`), `:193-198` (`mem_wdwidth`), `:546-548` (`bb_save_words`), `:550-603` (`bb_save`/`bb_restore`, RGB565) |
| Wirkung | war schon vor A57 falsch — der Code kannte nie eine 32-bpp-Speicher-MFDB. Absatz neu geschrieben, mit datiertem `**Correction (2026-09-20, A72)**`-Absatz. Stimmt jetzt mit § „AES consumer" (unveraendert, war schon korrekt) ueberein. |

## 2. `docs/api/aes/object-form.md` + `docs/api/_discrepancies.md` D-092 — „Screen↔Memory `vro_cpyfm` ist No-op"

Die Ticket-Zeilenangaben (`:556-571`, `:856-859`) trafen nach Verschiebung
nicht mehr; per Inhalt gesucht (Memory: „Eine Begruendung steht selten nur
einmal") — **drei** Kopien derselben ueberholten Behauptung gefunden, alle
drei korrigiert:

| Stelle | Was |
|---|---|
| `object-form.md`, D-092-„blocking note" (Blockquote nach dem DIALERT-Objektbaum, vor `form_error (53)`) | neuer `**Correction (D-092, superseded 2026-09-20, A72)**`-Blockquote direkt danach, Original stehengelassen |
| `object-form.md`, Randfall **20** in der Edge-case-Liste | Satz mit `docs/STATUS.md item F3` durch datierten Korrektursatz ergaenzt (gleiches Muster wie Randfall 17 „An earlier version…") |
| `object-form.md`, „Dependencies → Requires" | Satz „currently a no-op gap" durch „implementiert seit Task #51 ... correction 2026-09-20, A72" ersetzt |
| `_discrepancies.md` D-092 | neuer „**Status (2026-09-20, A72)**"-Absatz nach „Decision:" — die EmuTOS-Quellenanalyse (fm_dial vs. fm_alert) bleibt unveraendert richtig, nur die NewTOS-Folgerung „no-op" ist ueberholt |

Beleg fuer „implementiert": `docs/STATUS.md` Phase-13-Zeile
(`~~#51 bb_save~~ erledigt 2026-09-20, A57/A71`) und Recent-Decisions-Eintrag
2026-09-20 (A57+A71); `tos-api/src/vdi/raster.rs` fuehrt alle vier
`vro_cpyfm`-Kombinationen (Screen↔Screen/Screen→Memory/Memory→Screen/
Memory↔Memory); `docs/api/vdi/raster/_raster.md` § „`vro_cpyfm` off-screen
coverage (implemented, task #51)".

## 3. `TASK/A64-STATUS-DONE.MD` — Randmasse Laufwerkskaesten

Zeile 142 nannte „Referenz Rand 217/248, innen 218..247". Gemessen (A64s
eigene [`compare-geometry.txt`](../measurements/measurement-2026-09-17-fsel/screendumps/compare-geometry.txt),
`ref =[(217, 246), …]` in 10 Zeilen; unabhaengig bestaetigt durch
[A67-STATUS-DONE.MD](A67-STATUS-DONE.MD): „Rand 217/246, Fase 1 px auf
218/245") ist es **217/246, innen 218..245**. Quittung **nicht**
umgeschrieben — Erratum-Block ans Ende angehaengt, mit Beleg und der
Einschraenkung, dass die urspruengliche Match-Aussage (unsere Fase =
Referenz) mit diesem Auftrag nicht neu geprueft wurde, nur die genannten
Zahlen.

## Nicht angefasst / gefunden, aber ausserhalb des Auftrags

* `docs/api/vdi/raster/_raster.md` § „Pi/BCM2711 binding" (Screen-MFDB:
  `fd_nplanes = 4`, 16-Farben-CLUT, `NUM_COLORS = 16`) widerspricht dem
  heutigen Backend (`kernel/src/vdi_backend.rs:33,40`: `NUM_COLORS = 256`,
  `PLANES = 16`, Truecolor) — ein groesserer, eigener Befund (Screen-MFDB,
  nicht die vom Ticket benannte Speicher-MFDB), nicht Teil dieses Auftrags,
  nicht geaendert.
* `sdk/cflib/cflib-21/purec.c:165-172` liest `WF_SCREEN` als 8-Byte-Zeiger
  unter LP64 — bereits als offener Punkt in
  `docs/analysis/a57-aes-screen-buffer-design-2026-09-20.md` §
  „Nebenbefunde" und `TASK/T04.md` gefuehrt, hier nicht angefasst.
* Kein `git checkout/stash/reset`, nichts committet, nichts geflasht.
