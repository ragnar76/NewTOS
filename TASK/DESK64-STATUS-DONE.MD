# DESK64 — STATUS: DONE (2026-10-17, QEMU) — Hintergrund-Dialog 1:1 nach EmuTOS

**Auftrag:** Der Dialog „Set background“ verhaelt sich wie EmuTOS `inf_backgrounds` (`deskinf.c:825-921`) mit dem Baum
`ADBKGND` (`desk_rsc.c:2230-2452`): Vorschau `BGSAMPLE`, 8 Muster- und 16 Farbfelder, Desktop/Fenster-Schalter,
Fensterhintergrund wirksam. Sofortiges Speichern bleibt (Anwender 2026-10-17, DESK65).

## Aenderung (`9172349c`)

| | EmuTOS | NewTOS |
|---|---|---|
| Baum | `ADBKGND` 37 Objekte | `inf::adbkgnd_tree`, Objekt fuer Objekt, Zellen bei 8x16 aufgeloest |
| Schleife | `draw_fld(BGSAMPLE)` + `form_do`, `:866-893` | `inf::inf_backgrounds` + `bg_turn` |
| Farben ausblenden | `gl_nplanes`, `:832-852` | `gsx::gsx_nplanes` (`vq_extnd(h,1)[4]`), `global::patcol_index` |
| Fensterboxen | `G_BOX`, `set_background` `deskapp.c:1306-1331` | `obj_init` `G_BOX`, `DeskState::set_background`; `win_bldview` erzwingt kein Weiss mehr |
| Rueckgabe | TRUE nur bei Fensteraenderung → `BACKGROUND_HAS_CHANGED` | `do_viewmenu` → `BACKGROUND_CHANGED` → `desk_all` |
| Speichern | erst bei *Save desktop* | sofort `cnx_put(); app_save(true)` — benannte Abweichung |
| `set_aes_background` | `:904` | fehlt (AES-intern) — offen |

## Belege (`measurements/measurement-2026-10-17-desk64/RUNINFO.TXT`)

* Referenz Hatari Falcon truecolor (`screendumps/ref-hatari-falcon-tc-1-adbkgnd.png`) gegen QEMU (`kb-1-dialog.png`): gleiche
  Box und Objektlagen; Unterschiede nur Titel (GUI05), Mauszeiger, Musterfeld 6 (AES-Mustertabelle, Nebenfund).
* Vorschau aendert sich nach Muster- und Farbklick (`kb-2`, `kb-3`), Fenster-Schalter (`kb-4`, `kb-5`), nach OK Desktop rot
  Muster 2, Fenster cyan (`kb-6`), neues Fenster cyan (`kb-7`); `#Q 41 40 43 40 22 45` sofort gespeichert; Neustart damit
  zeigt beides (`kr-0`, `kr-1`).
* Selbsttest `[desk-bkgnd]` 30/0; uebrige 64/1 wie HEAD auf derselben Karte (kartenabhaengiger FAIL).
* Basislinien: Hosttests 1374/0/1, SDK 20/0/0, Bau 64/94 beide Boards, Clippy 76/125, Programme-Clippy 0, Driftprobe 0.

## Offen

* Abnahme auf echter Pi 4.
* `set_aes_background(curdesk & 0xff)` — der Hintergrund, den die AES beim Programmstart malt, folgt erst nach Neustart
  (AES liest `#Q` beim Start). Braucht eine Entscheidung (Lead).
* Nebenfund: `tos-api/src/aes/object.rs` `IP_PATTERNS[6]` ≠ EmuTOS (Hatari zeigt Zeilen FFFF/AAAA).
