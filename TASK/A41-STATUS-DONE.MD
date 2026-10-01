# A41 — Quittung

**Status: DONE (Hostbeleg + QEMU-Abnahme; echte Pi 4 offen)** · 2026-09-19 ·
Teil 1 (Konverter, Lader, Zeichner; `f4e6435`) + Teil 2 (nativer `RSHDR` am
Kopf des Blobs, `cicons.rsc` ausgeliefert, Abnahme; uncommitted im Baum).
`TASK/A41.md`s Kopfzeile bleibt unveraendert. Diese Datei **ersetzt**
`A41-STATUS-UNFINISHED.MD` (Text erhalten in `f4e6435`; die erste
Researcher-Fassung in `c26734a`).

## Teil 1 — was gebaut wurde (`f4e6435`)

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Konverter | `tools/mkrsc.py` `read_cicon_table`, `convert` (NRSC v11/v12, 84-Byte-Header mit `off_cib/n_cib`, `off_cic/n_cic`), `RSC_CAP`-Abbruch; `--rshdr` unveraendert | fertig |
| Lader | `tos-api/src/aes/resource.rs`: `NRSC_VER_CICON`, `NrscHeader` +4 Felder, `header_is_sound` + `cicon_tables_are_sound`, Relokation beider ABIs, `fix_cicons` (= `best_match` + `transform_cicon`/`cicon_pixel`, `CiconDevice`, ein Geraeteform-Puffer je Ressource), Freigabe in `rsrc_free`/`on_process_exit`; `vdi/vwk.rs` `pack_color16` | fertig |
| Zeichner | `tos-api/src/aes/object.rs`: `Cicon`, `ciconblk_mainlist`, `draw_icon(state, cicon)` = `gr_gicon`-Farbzweig; `vdi/raster.rs` `blit_device_mem` (= `gr_colourblit`) | fertig |
| Tests | `tools/xtask/src/rsc_cicon_tests.rs` (4), `resource.rs` `cicon_tests` (8), `tos-api/tests/aes_cicon_draw.rs` (5) | 627 / 0 / 1 |
| Spec | `docs/api/aes/resource-fileselector.md` §5.9 Status-Callout, §5.11; `docs/api/aes/object-form.md` G_CICON | nachgezogen |

Formatentscheidung (belegt, weicht von §5.9 ab): die nativen CICONBLK-/
CICON-Records sind die **C-Structs der Anwendung** (`obdefs.h:203-217`; LP64
56 + 48 B, ILP32 40 + 24 B) — TeraDesk kopiert einen CICONBLK by value
(`icon.c:1640,1675,2231`) und liest `monoblk` ueber `ob_spec.ciconblk`.
Zeigerfelder tragen Blob-Offsets, `mainlist`/`next_res` verketten die
Varianten, das `G_CICON`-`ob_spec` ist der Byte-Offset des CICONBLK. Das
Backend meldet 16 Planes; `best_match` nimmt die 4-Plane-Variante, Expansion +
Truecolor-Palettenpfad (`REV_MAP_COL`-Faltung) sind der Live-Pfad.

Blocker am Ende von Teil 1: TeraDesks eigener `xrsrc_gaddr`
(`rsc_load.c:496-541`, `icon.c:2750`) parst den Speicher hinter `ap_rscmem`
als klassischen `RSHDR` (`rsh_trindex` u16 @18); bei EmuTOS steht dort das
Dateiabbild mit `RSHDR` am Kopf (`gemrslib.c:787`), bei uns stand der
NRSC-Header → Data Abort (FAR = `"NRSC"`+Version als Zeiger). Anwenderentscheid
2026-09-19: **Option (a)**, der Konverter setzt den nativen `RSHDR` vor den
Blob.

## Teil 2 — was gebaut wurde (uncommitted im Baum)

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Konverter | `tools/mkrsc.py` `convert`: **NRSC v13 (LP64) / v14 (ILP32)** — 36-Byte nativer `RSHDR` (18 LE u16, klassische Reihenfolge `rsdefs.h:16-36`, Offsets **ab Dateianfang**, `rsh_vrsn` = Quelldatei (0, bzw. 4 = `NEW_FORMAT_RSC` bei CICON — nie `& 3 == 3`), `rsh_rssize` = Abbildgroesse, `rsh_string`/`rsh_imdata` = Datenabschnitt), auf 40 gepolstert, **vor** dem NRSC-Header (jetzt bei 40; alle `off_*` ab Byte 0 — ein Bezugspunkt); `pack_rshdr` (u16-Grenze, `IS_XRSC_HEADER`-Schutz, auch von `--rshdr` genutzt — `icons.rsc` bytegleich); Grenze `RSC_CAP − 40`, laut; RSXHDR-Quelle abgewiesen | fertig |
| Lader | `resource.rs`: `NRSC_VER_RSHDR` = 13, `RSHDR_PAD` = 40, `RSHDR_SIZE`; `nrsc_header_offset` (Magic bei 0 = v1–v12, bei 40 = v13+), `hdr()` (alle Leser eines geprueften Blobs), `rshdr_is_consistent` (7 Offsets, 7 Zaehler, `rsh_rssize` = `total_size`, `rsh_vrsn & 3 != 3`, `rsh_string`/`rsh_imdata` im Abbild); `header_is_sound`: Generation muss zum Fundort passen (v13 bei 0 / v11 bei 40 → abgewiesen). `report_load` unveraendert: `ap_rscmem` = Blob-Basis = `RSHDR`, `ap_rsclen` = `rsh_rssize`, `ap_ptree` = `ap_rscmem + rsh_trindex`. `rsrc_gaddr`, `fix_objects_coords`, `fix_cicons` in der Sache unveraendert | fertig |
| Blobs | `tos-api/src/aes/gem_fsel.nrsc` (xtask `gen-fsel-rsc`, 5 047 B), `sdk/apps/teradesk/build/dist/{desktop.rsc 39 278, cicons.rsc 63 650}`, `sdk/apps/qed/build/qed.rsc` 41 378 — alle v13; Hashes in `measurements/measurement-2026-09-19-a41/part2/manifest.sha256` | neu konvertiert |
| Auslieferung | `sdk/apps/build-teradesk.sh` staged `cicons.rsc`; `tools/make-sd-image.sh` kopiert es (Kommentar; Logik „nur wenn in `dist/`" bleibt) | fertig |
| Tests | `resource.rs` `rshdr_tests` (4: TeraDesks `xrsrc_gaddr`-Lesart auf dem geladenen Abbild, jeder Zweig gegen `rsrc_gaddr`; inkonsistenter Vorspann abgewiesen; Generation ↔ Fundort, v11 laedt weiter; ILP32-v14-Abbild mit 4-Byte-Slots); `tests::nrsc_field`; `tools/xtask/src/rsc_cicon_tests.rs` `check_rshdr` (v13/v14, 63 650 B); `rsc_obfix_tests.rs`/`rsc_align_tests.rs` und fuenf `tos-api/tests/aes_rsrc_*.rs` lesen den Header bei 0 oder 40 | 631 / 0 / 1 |
| Spec/Doku | `resource-fileselector.md` (`rsrc_load`-Callout: der Vorspann-Vertrag; §5.9/§5.10/§5.11.5 Status), `docs/sdk/04-porting-anatomy.md`, `05-abi-pitfalls.md` B8 (Symptom 3 + neues Symptom 5) | nachgezogen |

## Geprueft (Teil 2; Belege in `measurements/measurement-2026-09-19-a41/part2/`)

* **Rot vor / gruen nach** (`host-tests/`): `rshdr_tests` 3 FAILED auf dem
  Stand vor dem Umbau — `_rs_trindex[0]` bei `rsh_trindex` 0 = 39784239694
  (= `"NRSC"`+Version als Zeiger, die tera1-Signatur) — dann 4/4 (der vierte, ILP32-v14, kam danach hinzu);
  `rsc_cicon_tests` 4 FAILED gegen HEADs `mkrsc.py` (Scratch-Worktree,
  entfernt), dann 4/4. `cargo run -p xtask -- test`: **631 / 0 / 1**
  (Baseline 627 + 4).
* **Boards** (`build/`): `cargo xtask build --board rpi4` / `--board
  qemu-virt` (je nach `cargo clean`): rustc tos-api 64 / kernel 94, exit 0;
  Clippy tos-api 81 / kernel 125 (beide Boards). Delta 0.
* **QEMU raspi4b, Produktkernel (keine Instrumentierung), `cicons.rsc` auf
  dem Image, TeraDesk per `#Z`:** kein EXC/panic;
  `screendumps/product1-tera-colour-icons-idle.png` — vier HARD-DISC-Farbicons
  und farbiger TRASH, **0 Pixel** Differenz zur Scratch-Shim-Referenz
  `measurements/measurement-2026-09-19-a41/screendumps/tera2-scratch-shim-colour-icons-idle.png`,
  3205 Pixel (die Icons) zur Mono-Referenz.
* **A39-Sequenz per Injektor** (Scratch-Kernel, `method/`): HARD DISC C
  doppelklicken → Fenster `C:\*.*`, Infozeile „374035 bytes used in 24
  items.", **gelbe Ordner-** und Dokument-Farbicons
  (`inj4-B-window-open-colour-icons.png`); File → Quit/Shutdown → Dialog
  „Quit / Shutdown Options" → Quit → TeraDesk `exited with rc=0` → `[desk]
  desktop up (1 drives)` (`inj4-G-desktop-up-after-quit.png`). Serial-Trace
  (inj5, Scratch): `rsrc_load(desktop.rsc) = 1` (`ap_rscmem` 0x400000,
  `ap_rsclen` 39278), `rsrc_load(cicons.rsc) = 1` (0x427000, 63650,
  Geraeteform 153 600 B), beim Beenden `rsrc_free(0x400000) = 1`.
* **QED per `#Z` (qed.rsc v13) und unser Desktop:** `smoke-qed-up.png`,
  `smoke-desk-up.png` **bytegleich** (0 Pixel) zu
  `measurements/measurement-2026-09-19-a39/screendumps/` und Teil 1.

## Beobachtet, nicht Teil des Auftrags

* TeraDesks `free_icons` → `xrsrc_free` = `Mfree(ap_rscmem)` auf dem
  AES-geladenen `cicons.rsc` liefert bei uns **EIMBA (−40)** (Serial inj5:
  `Mfree(0x427000) -> -40`); bei EmuTOS ist der Blob ein GEMDOS-Block der
  Anwendung und `Mfree` gelingt. TeraDesk ignoriert die Rueckgabe, der Block
  geht mit dem Prozessende zurueck (`on_process_exit`). Eigener Task:
  `Kind::Rsc`-Bloecke per `Mfree` freigebbar machen, ohne dass `rsrc_free`/
  Prozessende doppelt freigeben.

## Nicht getan

* Echte Pi 4 (Tester/Lead). Kein Commit. `docs/STATUS.md` nicht
  nachgezogen (nicht im Scope genannt; beim Commit nachtragen).
