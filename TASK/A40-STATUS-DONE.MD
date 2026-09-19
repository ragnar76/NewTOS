# A40 — STATUS: DONE (Hostbeleg + QEMU-Rauchtest) / Tester-Abnahme offen, Pi-1920-Abnahme offen

Die Zeichengitter→Pixel-Umrechnung der Resource-Koordinaten (`rs_obfix`)
läuft jetzt **zur Ladezeit im Kernel** mit den lebenden `gl_wchar`/`gl_hchar`/
`gl_width` — nicht mehr beim Konvertieren im Werkzeug mit fest verdrahteten
`8/16/640`. Das Werkzeug lässt die gepackten Wörter unangetastet (NRSC v9/v10),
der Lader rechnet, `rsrc_obfix` (114) ist dieselbe Funktion. Bearbeitet
2026-09-19, Branch `desk-rebuild`, Basis HEAD `57568c2`, Baum vorher sauber.
Nicht committet. Ticket-Kopfzeile unverändert.

## Soll (EmuTOS @ `1a7484ab`)

| | Datei:Zeile |
|---|---|
| Wortregel | `aes/gemrslib.c:53-83` `fix_chpos`: `coffset = HIBYTE`, `cpos = LOBYTE`; Fall 0/2 `× gl_wchar`, Fall 1/3 `× gl_hchar`, **Fall 2 mit `cpos == 80` → `gl_width`**; `+ (coffset > 128 ? coffset−256 : coffset)` |
| Ein Objekt | `:89-99` `rs_obfix(tree, curob)`: die vier Wörter ab `ob_x` |
| Alle Objekte beim Laden | `:606-637` `fix_objects`: `rs_obfix(obj, 0)` für jedes `rsh_nobs`-Objekt; `:816-820` `rs_fixit`; aufgerufen von `rs_load` `:846` **nach** `rs_readit` |
| Eingebaute Resource | `aes/gem_rsc.c:636-641` `gem_rsc_fixit` (dieselbe Schleife), `geminit.c:711` nach offener Workstation |
| App-Aufruf | `gemsuper.c:375-377` `RSRC_OBFIX → rs_obfix(RS_TREE, RS_OBJ)` |
| Werte | `gemgraf.c:393` `gl_width = ws_xres + 1`; `:405` `gl_wchar/gl_hchar` aus `vqt_attributes` |
| Spec | `docs/api/aes/resource-fileselector.md` § `rsrc_obfix` (114) — die Tabelle stimmte bereits; die NewTOS-Callouts („Identität", „bereits Pixel") sind nachgezogen |

**Die Bildschirmbreite ist keine Erfindung des Werkzeugs**: EmuTOS setzt für
`ob_width` mit Zeichenzahl 80 `gl_width` ein (`:71-74`). Falsch war nur, dass
das Werkzeug dafür `640` fest einsetzte statt der Kernel die lebende Breite.

## Bestand vorher

- `tools/mkrsc.py:85` `WCHAR, HCHAR, SCREEN_W = 8, 16, 640`, `obfix()` beim
  Konvertieren; NRSC v7/v8 trugen Pixel.
- `resource.rs::rsrc_obfix` rechnete mit `vdi::font::CELL_W/CELL_H` (fest
  8/16) und der Rasterbreite; der Lader rechnete nichts (Modulkommentar
  „already in pixels").
- `init.rs::init` (Desktop-Baum, `gem_rsc_fixit`-Gegenstück) mit `CELL_W/CELL_H`.
- Betroffene Objekte mit Breite 80 Zeichen in den ausgelieferten Quellen:
  `desktop.rsc` (TeraDesk) Objekte 0, 1, 8 (Menübaum: Wurzel, Leiste,
  Dropdown-Fläche), `qed.rsc` Objekt 5 (Menüleiste), `gem.rsc` 53, 63, 64, 65.
  Auf der Pi (1920) blieben die bei 640 px.

## Fix

- `tos-api/src/aes/graf.rs`: `pub(super) fn gl_width()` (`gemgraf.c:37/:393`,
  Rasterbreite; ohne Raster `80 × gl_wchar()`), neben `gl_wchar()/gl_hchar()`.
- `tos-api/src/aes/resource.rs`:
  - `NRSC_VER_CHARGRID = 9` (v9 LP64 / v10 ILP32, 68-Byte-Header unverändert,
    Koordinatenfelder = gepackte Wörter).
  - `pub(super) fn rs_obfix(t: ObjTree, obj)` — die vier Wörter mit
    `graf::gl_wchar()/gl_hchar()/gl_width()`; `fix_chpos` bleibt die reine Regel.
  - `unsafe fn fix_objects_coords(access_base, abi)` — `fix_objects`-Lauf über
    `n_objects` bei `version >= 9`, ABI-breit über `ObjTree::app_at`; v1–v8
    unangetastet (keine Doppelumrechnung).
  - Aufgerufen aus `relocate_and_register_core` (nach `fix_tedinfo_std`, =
    `rs_fixit` nach `rs_readit`, EL0 **und** EL1-Pfad) und aus `relocate_blob`
    (= `gem_rsc_fixit`, der eingebettete Dateiwähler).
  - `rsrc_obfix` (114) → `rs_obfix(ObjTree::new(tree), obj)`.
  - `use crate::vdi::font::{CELL_H, CELL_W}` entfernt.
- `tos-api/src/aes/init.rs`: Desktop-Baum über `rs_obfix(ObjTree::lp64(tree), i)`;
  `CELL_W/CELL_H`-Import entfernt (der A20-Nebenbefund „verbliebene Leser").
- `tools/mkrsc.py`: `convert` schreibt `x, y, w, h` roh, Version 9/10;
  `obfix()` und `WCHAR/HCHAR/SCREEN_W` gelöscht; Kommentare. `--rshdr`
  unverändert (TeraDesks eigener Lader rechnet selbst).
- Neu konvertiert (Werkzeug unverändert aufgerufen wie in `build-teradesk.sh`
  bzw. `docs/sdk/04-porting-anatomy.md`), Skripte **nicht** geändert:

| Blob | vorher (v7) | nachher (v9) | Pixel bei 8/16/640 |
|---|---|---|---|
| `sdk/apps/teradesk/build/dist/desktop.rsc` | `0bc54458…` | `f03744cc…` | alt == neu+`fix_chpos(8,16,640)`: **0 abweichende Bytes** (601 Objekte) |
| `sdk/apps/qed/build/qed.rsc` | `5a2cefa2…` | `3383dd39…` | **0 abweichende Bytes** (682 Objekte) |
| `tos-api/src/aes/gem_fsel.nrsc` (committet, `xtask gen-fsel-rsc`) | `eb366350…` | `d520757b…` | **0 abweichende Bytes** (66 Objekte) |
| `sdk/apps/teradesk/build/dist/icons.rsc` (`--rshdr`) | `8d4bc8a2…` | `8d4bc8a2…` | bytegleich |

Format-Entscheidung: neue Versionspaar v9/v10 nach dem v7/v8-Muster (der
Lader gattet überall nach `version`, die xtask-Leser nach Parität); kein
Flag. Kein STOP nötig — die Spec beschreibt das Versionsmuster, die
Alternative (Flagbit im Header) hätte einen neuen Header-Leser gebraucht.

## Tests

| Datei | Fälle | rot vor / grün nach |
|---|---|---|
| `tos-api/tests/aes_rsrc_obfix_at_load.rs` (neu) | `a_char_grid_resource_is_fixed_with_the_screen_it_is_loaded_on` (1920: Wurzel 1920 statt 640, alle Wörter mit `graf_handle`-Zelle), `at_640_the_load_time_fix_equals_the_old_pre_converted_pixels`, `rsrc_obfix_computes_what_the_loader_computes`, `a_pixel_coordinate_blob_is_left_alone` (v7) | **3 rot vor** (`[0,0,80,25]` statt `[0,0,1920,400]` / `[0,0,640,400]`; obfix ≠ Lader), 1 grün (v7) → **4/4 grün** |
| `resource.rs` `mod tests` (neu) | `a_char_grid_ilp32_blob_is_fixed_at_load` (v10 über `relocate_and_register_core(…, Ilp32)`), `a_pixel_ilp32_blob_is_left_alone` (v8) | Mutations-Gegenprobe (ILP32 im Lader übersprungen, danach aus Kopie zurückgesetzt, MD5 gleich): `[1029,-254,80,258]` statt `[44,31,640,33]` → rot; **2/2 grün** |
| `tools/xtask/src/rsc_obfix_tests.rs` (neu) | `mkrsc_convert_lp64/ilp32_keeps_the_char_grid_coordinates` (echte `gem.rsc`, alle 66 Objekte wortgleich, DESKTOP-Wurzel `0x0050`), `the_committed_gem_fsel_blob_carries_char_grid_coordinates` | **3 rot vor** („got 640 = 0x0280", left 640 / right 80) → **3/3 grün** |

Rig: Metadaten-Backend mit schaltbarer Breite (640/1920), Zelle aus
`graf_handle` `int_out[1..2]`; handgebauter v9-Blob mit `0x0050`, `0x0405`,
`0xFF02`, `0x8001` (Offset 128 bleibt +128), `0x8101` (129 → −127).

## Beleglage

| | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **602 grün / 0 rot / 1 ignoriert** (Baseline 593 + 9 neue) |
| Bau rpi4 / qemu-virt (je `cargo clean -p kernel -p tos-api --target …` vorher, „Compiling kernel/tos-api" bestätigt) | beide `rc=0`, tos-api **64**, kernel **94** rustc-Warnungen — Delta 0 |
| Clippy kernel `board-rpi4` / `board-qemu-virt` (`--lib --bins`), tos-api | **125 / 125 / 81** — Delta 0; neue Dateien 0 Clippy-Warnungen (xtask-Test-Binary: 1 Warnung, `main.rs:228`, vorbestehend) |
| QEMU raspi4b, release `--board rpi4` (kernel8 SHA-256 `d06155e9…`), Basisimage `tools/make-sd-image.sh` mit den neu konvertierten Blobs | TeraDesk `#Z` (NEWDESK.INF/TERADESK.INF wie A39, SHA `457a70e7…`/`5be1b5c7…`): `smoke-tera-up.png` **0 von 307200 Pixeln verschieden**, PNG byte-identisch (`fb1e8b90…`) zu `measurement-2026-09-19-a39/screendumps/smoke-tera-up.png`; QED `#Z`: `smoke-qed-up.png` byte-identisch (`ec79c00a…`); eigener Desktop: `smoke-desk-up.png` byte-identisch (`dee57cca…`). Kein panic/EXCEPTION in allen drei Serial-Logs, kein QEMU-Prozess übrig, keine Images hängen |
| `git status --short` | `M docs/api/aes/resource-fileselector.md`, `M docs/sdk/04-porting-anatomy.md`, `M docs/sdk/05-abi-pitfalls.md`, `M tools/mkrsc.py`, `M tools/xtask/src/main.rs`, `M tos-api/src/aes/gem_fsel.nrsc`, `M tos-api/src/aes/{graf,init,resource}.rs`, `?? tools/xtask/src/rsc_obfix_tests.rs`, `?? tos-api/tests/aes_rsrc_obfix_at_load.rs`, `?? TASK/A40-STATUS-DONE.MD` |

Scratch (Baulogs, rote Ausgaben, alte Blobs, Screendumps, Serial):
`…/scratchpad/a40/`.

## Spec / Doku

- `docs/api/aes/resource-fileselector.md`: Kopf-Callout, `rsrc_load`-Callout
  (neuer Absatz „coordinate fix runs at load"), `rsrc_obfix`-NewTOS-Status,
  Applicability note, Implementation notes — „Identität"/„bereits Pixel" raus.
- `docs/sdk/05-abi-pitfalls.md` B8: Symptom 4 (Neukonvertieren).
- `docs/sdk/04-porting-anatomy.md`: Hash-Notiz zu `qed.rsc`.

## Nebenbefunde (unverändert, je eigener Task)

- `kernel/src/dbg_qedtrace.rs:515,545-553`: Kommentare „our rsrc_obfix is a
  no-op identity" — veraltet (waren es seit D-101).
- `tools/make-sd-image.sh:210`: „qed.rsc its native NRSC-v3" — veraltet (v9).
- `sdk/apps/build-qed.sh` erzeugt `qed.rsc` nicht (bekannt, `04-porting-anatomy.md`).
- `gl_wchar/gl_hchar` sind zur Laufzeit fest 8/16 (VDI `font::DEFAULT`,
  A20-Befund „Was fehlt") — der Test variiert deshalb nur `gl_width`.
- `docs/STATUS.md` nicht angefasst (nicht im Auftrag; A20-Präzedenz).

## Offen

- Tester-Abnahme (Boot + `svc`-Selftests + Screendumps + `fsck_msdos`, beide
  Boards).
- **Echte Pi 4 (1920×1080): sichtbare Änderung erwartet und gewollt** —
  TeraDesks Menüwurzel/-leiste (`desktop.rsc` Objekte 0/1/8) und QEDs Leiste
  (Objekt 5) werden 1920 statt 640 px breit (EmuTOS `gl_width`). Vorher/
  Nachher-Screendump auf der Pi ist die Hardwareabnahme; QEMU 640×480 kann
  das nicht zeigen. Karten des Users müssen die neu konvertierten
  `desktop.rsc`/`qed.rsc` bekommen (alte v7-Dateien laden weiter, bleiben
  aber bei 640).
