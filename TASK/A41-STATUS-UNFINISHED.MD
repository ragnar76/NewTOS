# A41 — Quittung

**Status: UNFINISHED** · 2026-09-19 · **Implementierung gelandet (Konverter,
Lader, Zeichner, Hosttests, beide Boards), Abnahme in QEMU durch einen neu
gefundenen Vertragsbruch blockiert.** Kein Commit. `TASK/A41.md`s Kopfzeile
(Status **OFFEN**) bleibt unveraendert.

Die erste Fassung dieser Quittung (Researcher, gleicher Tag: Spec §5 +
`object-form.md`-Callout, kein Code) ist in `c26734a` erhalten; dieser Text
ersetzt sie um den Implementierungsstand.

## Was gebaut wurde (uncommitted im Baum)

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Konverter | `tools/mkrsc.py` `read_cicon_table`, `convert` (NRSC **v11** LP64 / **v12** ILP32, 84-Byte-Header mit `off_cib/n_cib`, `off_cic/n_cic`), `RSC_CAP`-Abbruch; `--rshdr` unveraendert (`icons.rsc` bytegleich) | fertig |
| Lader | `tos-api/src/aes/resource.rs`: `NRSC_VER_CICON`, `NrscHeader` +4 Felder, `header_is_sound` + `cicon_tables_are_sound`, Relokation beider ABIs, `fix_cicons` (= `best_match` + `transform_cicon`/`cicon_pixel`, `CiconDevice`, ein Geraeteform-Puffer je Ressource), Freigabe in `rsrc_free`/`on_process_exit`; `vdi/vwk.rs` `pack_color16` | fertig |
| Zeichner | `tos-api/src/aes/object.rs`: `Cicon`, `ciconblk_mainlist`, `draw_icon(state, cicon)` = `gr_gicon`-Farbzweig; `vdi/raster.rs` `blit_device_mem` (= `gr_colourblit`) | fertig |
| Tests | `tools/xtask/src/rsc_cicon_tests.rs` (4), `resource.rs` `cicon_tests` (8), `tos-api/tests/aes_cicon_draw.rs` (5); rot-vor / gruen-nach in `measurement-2026-09-19-a41/host-tests/` | 627 / 0 / 1 (Baseline 610 + 17) |
| Auslieferung | `sdk/apps/build-teradesk.sh`, `tools/make-sd-image.sh` | **zurueckgehalten** (s. Blocker); `make-sd-image` kopiert `cicons.rsc` nur, wenn es in `dist/` liegt |
| Spec | `docs/api/aes/resource-fileselector.md` §5.9 Status-Callout, §5.11 Punkt 5; `docs/api/aes/object-form.md` G_CICON-Status | nachgezogen |

## Formatentscheidung (belegt, weicht von §5.9 ab)

Die nativen CICONBLK-/CICON-Records sind die **C-Structs der Anwendung**
(`obdefs.h:203-217` = gemlib `mt_gem.h`; LP64 56 + 48 B, ILP32 40 + 24 B),
nicht die vorgeschlagenen `NrscCiconBlk`/`NrscCiconVariant`-Tabellen:
TeraDesk kopiert einen CICONBLK **by value** (`icon.c:1640,1675,2231`) und
liest `monoblk` ueber `ob_spec.ciconblk` (`dir.c:2090-2102`). Zeigerfelder
tragen Blob-Offsets, `mainlist`/`next_res` verketten die Varianten, das
`G_CICON`-`ob_spec` ist der Byte-Offset des CICONBLK (generische Relokation).

Gemessen: `cicons.rsc` nativ **63 610 B** (LP64) / 61 770 B (ILP32) gegen
`RSC_CAP` 65 536 — `RSC_CAP` bleibt. Korrekturen zur Spec-Messung: Block 3 ist
48×32, Bloecke 20 und 31 haben kein `sel_data`. Das Backend meldet **16
Planes** (`vdi_backend.rs:41`), nicht 4: `best_match` nimmt die 4-Plane-Variante
als beste niedrigere, Expansion + Truecolor-Palettenpfad sind der Live-Pfad
(`REV_MAP_COL`-Faltung, `CiconDevice::rev_map_col`).

## Blocker (Abnahme Punkt 5)

QEMU raspi4b, TeraDesk per `#Z` mit `cicons.rsc`: `rsrc_load("cicons.rsc")` = 1
(Geraeteform-Puffer 153 600 B), `rsrc_gaddr(R_TREE, 0)` = 1 — danach **Data
Abort** in TeraDesk (`FAR = "vRSC"+11`). Ursache: TeraDesks eigener
`xrsrc_gaddr` (`rsc_load.c:496-541`, aufgerufen in `icon.c:2750`) liest
`aes_global[7]` = `ap_rscmem` und parst den Speicher dort **als klassischen
`RSHDR`** (`rsh_trindex` u16 @18). Bei EmuTOS steht dort das Dateiabbild mit
`RSHDR` am Kopf (`rs_readit`, `gemrslib.c:787`); bei uns der NRSC-Header. Das
ist ein Vertrag ueber das **Speicherabbild hinter `ap_rscmem`**, den §5 nicht
kannte und der ausserhalb von ADR-013s Freiheit ueber das Dateiformat liegt.

Scratch-Experiment (nur Scratch-Kopie, nicht im Baum, `method/scratch-*.diff`):
40-Byte nativer RSHDR vor dem Blob, `ap_rscmem` = Shim → TeraDesk zeigt die
**Farbicons** (`measurement-2026-09-19-a41/screendumps/tera2-scratch-shim-colour-icons-idle.png`).
Damit ist die Kette Konverter → Lader → Zeichner auf der echten Datei belegt.

**Entscheidung fuer den Anwender** (Spec §5.11 Punkt 5): (a) Konverter emittiert
den nativen RSHDR (36 B, LE u16, Offsets ab Dateianfang, auf 40 B gepolstert)
vor dem NRSC-Header (v13/v14; der Lader sucht das Magic bei 40) — das
geladene Abbild *ist* dann die Datei wie bei EmuTOS; oder (b) `load_into_user`
synthetisiert den Shim beim Laden (nur EL0). Beide brauchen `rssize`/Offsets
< 65 536. Bis dahin wird `cicons.rsc` nicht in `dist/` gestaged (Mono-Rueckfall
statt Absturz).

## Geprueft

* Hosttests: siehe Tabelle; `cargo run -p xtask -- test` 627/0/1.
* Boards: `cargo xtask build --board rpi4` / `--board qemu-virt` (je nach
  `cargo clean -p kernel -p tos-api --target …`): rustc 64/94, Clippy 81/125,
  Delta 0 (`measurement-2026-09-19-a41/build/`).
* QEMU, Produktkernel ohne Instrumentierung, Images ohne `cicons.rsc`:
  `smoke-tera/-qed/-desk` **bytegleich** (0 Pixel) zu
  `measurement-2026-09-19-a39/screendumps/smoke-*.png`; kein EXC/panic.
  `desktop.rsc` dabei bereits als v11 (84-Byte-Header) neu konvertiert.

## Nicht getan

* Auslieferung von `cicons.rsc` (Blocker), A39-Sequenz mit Farbicons, echte Pi 4.
* `gem_fsel.nrsc` (embedded, v9) nicht neu generiert — der Lader bleibt
  abwaertskompatibel, der xtask-Test verlangt nur ≥ v9.
