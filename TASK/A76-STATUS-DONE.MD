# A76 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Abnahme offen

`Mfree(ap_rscmem)` auf einem von der AES geladenen Ressourcenblock liefert
jetzt `E_OK` und gibt den Pool-Block frei wie bei jedem `Malloc`-Block; die
AES erfaehrt es, so dass ein spaeteres `rsrc_free` 0 antwortet, ohne den
freigegebenen Speicher anzufassen, und das Prozessende nichts doppelt
freigibt. Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis HEAD
`bd61b95`, **nicht committet**. Spec (vom Lead, uncommitted, unveraendert
gelassen): `docs/api/gemdos/memory/49-mfree.md` § „AES resource block (A76)“
+ ADR-021-Korrektur, `docs/api/aes/resource-fileselector.md` A39-Callout
Punkt 8 + § `rsrc_free`, `docs/COMPATIBILITY.md` „Resource-block ownership
(A76)“.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemrslib.c:775` `rs_hdr = dos_alloc_anyram(rslsize)`, `:787` `ap_rscmem = rs_hdr`; `util/gemdos.c:117-120` `dos_alloc_anyram` = `Mxalloc(nbytes, MX_PREFTTRAM)`, `:130-133` `dos_free` = `Mfree`; `bdos/umem.c:135-159` `xmfree`: `m_start`-Suche auf `mp_mal`, keine Eigentuemerpruefung → `E_OK`; `aes/gemrslib.c:685-708` `rs_free`: `dos_free(ap_rscmem)` schlaegt auf dem ausgehaengten Block fehl → `rc = 0` |
| NewTOS (vorher) | `tos-api/src/bdos/process/usermem.rs::free` verweigerte `Kind::Rsc` → `EIMBA`; gemessen A41 inj5 `Mfree(0x427000) -> -40` (TeraDesk `free_icons` → `xrsrc_free` = `Mfree(_AESrscmem)`, `sdk/apps/teradesk/rsc_load.c:480-492`); der Blob blieb bis zum Prozessende belegt |
| NewTOS (nachher) | `usermem.rs` (`free`), `tos-api/src/gemdos/memory.rs` (`el0_free`), `tos-api/src/aes/resource.rs` (`RscEntry::{blob_released,cicon_released}`, `on_mfree`, `rsrc_free`) |

## Was gebaut ist

* **`tos-api/src/bdos/process/usermem.rs`** — `free`: `Kind::Rsc` wird wie
  `Kind::Heap` freigegeben (Eintrag aus der Fenstertabelle, Pool-Schluessel
  zurueck); nur `Kind::Shared` bleibt `EIMBA`. Doc-Kommentar mit
  EmuTOS-Zitat; `Kind::Rsc`-Doc ergaenzt.
* **`tos-api/src/gemdos/memory.rs`** — `el0_free(slot, user_va)` (neu,
  privat): der gemeinsame Schwanz von `Mfree` und `Mshrink(len=0)`:
  `usermem::free` → `traps::mfree` → bei `E_OK`
  `aes::resource::on_mfree(slot, m_start)`. Der Hook sitzt an der Stelle, die
  die Fenster-VA in den Pool-Schluessel aufloest.
* **`tos-api/src/aes/resource.rs`** — `RscEntry` bekommt
  `blob_released`/`cicon_released`; `on_mfree(slot, m_start)` (neu, pub)
  setzt sie fuer den Eintrag des Slots, dessen `raw` bzw. `cicon_raw` der
  Schluessel ist; `rsrc_free` antwortet fuer einen released-Schluessel 0 ohne
  `mfree`/`free_by_mstart` (der Schluessel kann inzwischen ein fremder
  Block sein) und gibt den jeweils anderen Block wie bisher frei;
  `on_process_exit` bleibt im Code unveraendert (Doc-Hinweis) — es gibt
  nichts per Schluessel frei, der Pool-Sweep `free_all_owned` sieht den
  ausgehaengten Block nicht. EmuTOS' Header-Lauf ueber den freigegebenen
  Block (`free_cicon_buffers`) wird bewusst nicht nachgebaut, nur das
  beobachtbare `rsrc_free → 0`.
  Neu `#[cfg(test)] mod mfree_tests` (9 Faelle); Helfer von
  `selector_tests` und `cicon_tests::blob` auf `pub(super)` gesetzt.
* **`docs/STATUS.md`** — Phase-13-Eintrag (datiert, Form der A75-Zeile) +
  Zeile in „Recent Decisions“.
* **`measurements/measurement-2026-09-20-a76/`** — RUNINFO, Serial, Screendumps,
  Hosttest-Quittungen, Build-/Clippy-Logs, Methode (Scratch-Skripte,
  Injektor-Skript, INF-Dateien, Produkt-Diff, Scaffolding-Diff), SHA-256-
  Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach

Modul `resource.rs` `mfree_tests`, in-crate, weil ein Eintrag mit echtem
Pool-Schluessel `register`/`relocate_and_register_core`/`report_load`
braucht und `load_into_user`s Kernel-Alias (`raw + 0xFFFF_0000_0000_0000`)
auf dem Host nicht abbildbar ist. Echt sind: der BDOS-Pool (reine
Buchhaltung, einmal je Testbinary auf `Long`-Adressen installiert, die
niemand dereferenziert), `usermem::map` als `Kind::Rsc`, der GEMDOS-
Trap-Eintrag `gemdos::memory::mfree` mit dem Testprozess als `RUN`,
`rsrc_free` ueber den Dispatcher. Stand-in ist nur der Zugriffs-Alias des
Blobs (Testpuffer, wie in `selector_tests`).

| Test | vor | nach |
|---|---|---|
| `mfree_of_ap_rscmem_releases_the_blob_like_any_malloc_block` (a) | `left: -40 right: 0` | gruen (Block zurueck im Pool, zweites `Mfree` → -40) |
| `rsrc_free_after_the_apps_mfree_reports_zero_and_frees_no_pool_block` (b) | `-40` | gruen (Fremdblock auf demselben Schluessel bleibt unangetastet) |
| `process_exit_after_the_apps_mfree_frees_nothing_twice` (c) | `-40` | gruen (`free_all_owned` = 0, Freiliste koalesziert) |
| `rsrc_free_alone_still_frees_the_blob_and_a_later_mfree_is_eimba` (d, Waechter) | gruen | gruen |
| `mfree_of_one_blob_leaves_the_processs_other_load_intact` | `-40` | gruen |
| `mfree_of_ap_rscmem_leaves_the_colour_icon_buffer_until_rsrc_free` (e) | `-40` | gruen |
| `process_exit_after_the_apps_mfree_reclaims_the_icon_buffer_once` | `-40` | gruen (`free_all_owned` = 1) |
| `mfree_of_the_colour_icon_buffer_is_not_freed_again_by_rsrc_free` | `-40` | gruen |
| `the_freed_blobs_window_entry_is_gone` | `-40` | gruen |

Gegenprobe (Hand-Edit, `on_mfree`-Aufruf uebersprungen, danach
zurueckgesetzt): (b) und der Puffer-Fall rot mit `left: 1 right: 0` —
`rsrc_free` haette den fremden Block freigegeben. Die Tests sichern also
auch die AES-Haelfte.

`cargo run -p xtask -- test` **696 → 705 / 0 / 2**. `cargo xtask build
--board rpi4` (frisch) rustc tos-api 64 / kernel 94 — Delta 0. Clippy
tos-api 81 / kernel 125 (Target) — Delta 0; `--tests`: kein Treffer in den
geaenderten Regionen. Kein qemu-virt-Build (Vorgabe des Leads).

## QEMU raspi4b (`measurements/measurement-2026-09-20-a76/`)

A41-Teil-2-Sequenz (Injektor `quit2`: Doppelklick HARD DISC C → File →
Quit/Shutdown → Dialog Quit → TeraDesk beendet sich → `[desk] desktop up`)
auf Scratch-Kopien mit Injektor + Serial-Trace (kein Produkt-Logging);
vorher = HEAD-`usermem.rs`/`resource.rs`/`memory.rs`, nachher = Baum. TeraDesk
per `#Z` mit `cicons.rsc` (`tools/make-sd-image.sh`).

* vorher: `[a76] Mfree(0x4bc000) -> -40` (der `cicons.rsc`-Blob,
  `ap_rscmem=0x4bc000`; A41 mass ihn bei 0x427000, das Fensterlayout ist
  seither gewandert) — **nachher: `[a76] Mfree(0x4bc000) -> 0`**. Sonst
  identische Serial-Logs: `rsrc_free(selector 0x400000) -> 1`,
  `exited with rc=0`, `[desk] desktop up`, 0 `EXC`/`PANIC`.
* Screendumps A/B/D/G: Lauf 2 **alle vier Paare byteidentisch**; in Lauf 1
  fiel das 1-s-Settle von B exakt auf den Menue-Schritt des Injektors
  (Harness-Rennen, kein Produktunterschied), A/D/G auch dort byteidentisch.

## Nicht angefasst / offen

* **Pi-Abnahme offen:** TeraDesk mit `cicons.rsc` auf der echten Pi 4
  starten und beenden (Lead/Tester; nichts geflasht).
* Spec-Dateien des Leads unveraendert; `pterm.rs`/`on_process_exit` im Code
  unveraendert.
* Nebenbefund (nicht A76, vorher = nachher): an MARK B oeffnet der
  Doppelklick auf HARD DISC C in beiden Laeufen kein Fenster mehr (A41 inj4
  vom 2026-09-19 zeigte „374035 bytes used in 24 items.“); das Icon ist
  markiert. Vermutlich Doppelklick-Timing nach A73/A75 oder der 8-Tick-
  Abstand des Skripts — dem Lead gemeldet, nicht angefasst.
* `rsrc_gaddr` nach dem App-eigenen `Mfree` liest wie EmuTOS aus dem
  freigegebenen Blob (Deskriptorwerte bzw. Kernel-Alias, kein Fault) —
  nicht Teil des Vertrags, unveraendert.
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.

## Nachtrag (2026-09-20, Lead)

Die Nebenbeobachtung „Doppelklick oeffnet an MARK B kein Fenster" ist
aufgeklaert: kein Produktbefund, sondern der Park-Sweep des Injektors durch
den Menuetitel *File* — seit A73 bleibt der Kasten im OUTSIDE_STATE offen,
der erste Druck faellt auf ein DISABLED-Item und wird verbraucht (EmuTOS
identisch). Details in `measurements/measurement-2026-09-20-a76/RUNINFO.TXT`, Nachtrag.
