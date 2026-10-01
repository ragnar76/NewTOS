# T12 — STATUS: DONE (2026-10-01) — Riegel gesetzt, unter Last belegt

**Auftrag:** die drei `sched_*`-Testdateien, die `PROCESS_TABLE` ohne Riegel
beschreiben, mit **demselben** Mechanismus wie die `aes_*`-Dateien
serialisieren; pruefen, ob weitere Testdateien globale Zustaende teilen
(`TASK/T12.md`).

## Was tatsaechlich wettlaeuft

Selbst nachgeprueft, nicht aus dem Ticket uebernommen:

- **Jede Datei unter `tos-api/tests/` ist ein eigenes Testbinary, also ein
  eigener Prozess.** Statics werden nur **innerhalb** einer Datei geteilt,
  nie zwischen zwei Dateien.
- `sched_pexec_parent_suspend.rs` hat **sechs** `#[test]` in einem Binary;
  jede beginnt mit `fixture()`, das **alle** Slots der crate-globalen
  `PROCESS_TABLE` zuruecksetzt. Laeuft ein zweiter Test dazwischen, setzt
  dessen `fixture()` z.B. `ACC.runnable` wieder auf `false`, und
  `on_yield(CHILD)` liefert `None` statt der Accessory. Die Fehlschlaege
  verteilen sich entsprechend ueber **vier der sechs** Tests (Zeilen 91, 109,
  146, 160 der alten Datei), nicht nur den aus REF04 bekannten.
- `sched_dsptch_admits_the_caller.rs` und `sched_dsptch_keeps_caller_ready.rs`
  haben je **einen** `#[test]` — bewusst so geschrieben („One `#[test]`
  function: the process table is global…“). Sie **konnten bisher nicht**
  wettlaufen; sie bekommen den Riegel, damit die naechste hinzugefuegte
  Testfunktion es auch nicht kann (Ticket Punkt 3).
- Der Produktcode (`tos-api/src/bdos/process/sched.rs`) ist nicht schuld:
  `PROCESS_TABLE`, `CURRENT`, `KERNEL_PARKED`, `RESUME_RESTARTABLE` sind
  absichtlich ungesperrte Statics (Einkern-Trap-Pfad). **`tos-api/src` ist
  unveraendert.**

## Der Riegel

Uebernommen aus `aes_rsrc_per_process.rs:69-75` (dieselbe Form tragen
`aes_pd_by_slot.rs:199-205`, `aes_a96_zombie_windows.rs:353-359`,
`aes_appl_read_blocks.rs:215-223`, dort zusaetzlich mit `install_host()`,
das die `sched_*`-Dateien nicht brauchen):

```rust
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
```

und `let _g = lock();` als erste Zeile jedes `#[test]`. Das
`unwrap_or_else(into_inner)` verhindert, dass ein echter Fehlschlag die
uebrigen Tests per vergiftetem Mutex mitreisst.

Es gibt im Baum auch den gemeinsamen `aes_reference::harness::AES_LOCK`;
er ist ebenfalls je Binary lokal und damit gleichwertig, braucht aber das
ganze Harness-Modul. Die vier im Ticket genannten `PROCESS_TABLE`-Dateien
nutzen den lokalen `LOCK` — dem folgt diese Aenderung.

Geaendert, nur Testdateien:

- `tos-api/tests/sched_pexec_parent_suspend.rs` — `LOCK`/`lock()`, 6× `let _g = lock();`
- `tos-api/tests/sched_dsptch_admits_the_caller.rs` — dito, 1×; Modulkommentar
  nennt jetzt den Riegel statt „deshalb nur ein Test“
- `tos-api/tests/sched_dsptch_keeps_caller_ready.rs` — dito

## Belege — Wiederholungslaeufe

Die Binaries **vor** und **nach** der Aenderung wurden als Kopie
festgehalten und direkt aufgerufen (Standardrunner = parallele Threads,
bzw. `--test-threads=1`). „Last“ = eine Dauerschleife
`rm -rf <tgt>; CARGO_TARGET_DIR=<tgt> cargo test -p tos-api -p tos-types --no-run`
in einem privaten Zielordner, parallel zu den Laeufen.

| Binary | Modus | vorher | nachher |
|---|---|---|---|
| `sched_pexec_parent_suspend` | parallel, ohne Last | 99/100 | **100/100** |
| `sched_pexec_parent_suspend` | `--test-threads=1`, ohne Last | 100/100 | 100/100 |
| `sched_pexec_parent_suspend` | parallel, unter Last | 99/100 | **100/100** |
| `sched_pexec_parent_suspend` | `--test-threads=1`, unter Last | 100/100 | 100/100 |
| `sched_pexec_parent_suspend` | parallel, unter Last, Langlauf | **491/500** | **500/500** |
| `sched_dsptch_admits_the_caller` | alle vier Modi | je 100/100 | je 100/100 |
| `sched_dsptch_keeps_caller_ready` | alle vier Modi | je 100/100 | je 100/100 |

Die neun Fehlschlaege des Langlaufs vorher: 5×
`an_awake_accessory_still_gets_the_cpu_while_the_child_blocks` (`:109`,
`expect("the accessory is runnable")`), 2× `blocking_child_never_yields_to_its_own_parent`
(`:91`, `left: 1 right: 2`), je 1× `the_childs_pterm_puts_the_parent_back_exactly_as_it_was`
(`:146`) und `a_parent_that_was_never_a_peer_does_not_become_one` (`:160`).

**Abnahme des Tickets** („`cargo test` waehrend eines parallelen Builds
zwanzigmal gruen“): `cargo test -p tos-api --test sched_pexec_parent_suspend
--test sched_dsptch_admits_the_caller --test sched_dsptch_keeps_caller_ready`
**30/30** gruen unter Last.

## Ticket Punkt 3 — weitere Testdateien mit geteiltem Zustand

Alle Dateien unter `tos-api/tests/` mit mehr als einem `#[test]` und ohne
eigenen `LOCK` durchgesehen:

- **Serialisiert ueber einen anderen, bestehenden Riegel:** 19 `aes_*`-Dateien
  ueber `harness::AES_LOCK`, `vdi_mouse_park`/`vdi_vex_wheelv` ueber
  `harness::VDI_LOCK`, die `gemdos_*`-Dateien ueber `fatvol::fs()`
  (`FS_LOCK`).
- **Bewusst ein einziger Test:** `aes_launch_boundary_reset.rs` (Modulkommentar
  `:45-47`).
- **Ohne globalen Zustand:** `loader_ilp32_rejected.rs` (reines `elf::parse`).
- **Ohne Riegel, mit eigenem Backend je Test** (`render_tree_lp64` auf
  lokalem `PenBackend`): `aes_cicon_draw`, `aes_icon_unaligned_raster`,
  `aes_object_font`, `aes_object_nicelines`; dazu `aes_global_layout`
  (2 Tests, beide `appl_init`, Host per `Once`). Je 300× parallel unter Last:

  | Binary | parallel, unter Last |
  |---|---|
  | `aes_cicon_draw` | 300/300 |
  | `aes_global_layout` | 300/300 |
  | **`aes_icon_unaligned_raster`** | **296/300**, im zweiten Lauf **398/400** |
  | `aes_object_font` | 300/300 |
  | `aes_object_nicelines` | 300/300 |

  **`aes_icon_unaligned_raster` ist ein zweiter, bisher unbemerkter
  Wettlauf.** Es scheitert immer
  `a_g_icon_with_odd_raster_pointers_draws_like_an_even_one` (`:357`, ein
  Bild komplett leer); `--test-threads=1` 200/200. Die Tests teilen kein
  Backend, der geteilte Zustand muss also im Renderpfad von `tos-api/src`
  liegen — **nicht ermittelt, nicht angefasst** (ausserhalb dieses Tickets;
  Kandidaten waeren Statics in `aes/object.rs`/`aes/gsxif.rs`, das ist eine
  Vermutung, kein Befund). Eigenes Ticket noetig, und dort zuerst die Frage,
  ob es ein Test- oder ein Produktfehler ist.

## Basislinien

| | gemessen | Basis |
|---|---|---|
| `cargo test -p tos-api -p tos-types -p xtask --no-fail-fast` | **964 / 0 / 1** | 964/0/1 |
| `cargo xtask build --board qemu-virt --release` (nach `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat --release`) | `tos-api` **64**, `kernel` **94** | 64/94 |
| `cargo xtask build --board rpi4 --release` (ebenso frisch) | `tos-api` **64**, `kernel` **94** | 64/94 |
| `cargo clippy --features board-qemu-virt` (in `kernel/`) | **80 / 125** | 80/125 |
| `cargo clippy --features board-rpi4` (in `kernel/`) | **80 / 125** | 80/125 |
| `cargo clippy -p tos-api --test sched_*` (die drei) | 0 Treffer in den Testdateien | — |
| `sh tools/desk-drift-probe.sh -q` | **32** | 32 |

Testanzahl unveraendert (964): es wurde kein Test hinzugefuegt, entfernt
oder geteilt.

## Offen

- **`aes_icon_unaligned_raster`** — Wettlauf unter Last (6/700), Ursache im
  Renderpfad nicht ermittelt; Vorschlag fuer ein Folgeticket.
- Nebenbei: vier `rustc-ice-*.txt` landeten im Repo-Wurzelverzeichnis, weil
  die eigene Lastschleife ihren Zielordner waehrend eines Baus geloescht
  hatte; wieder entfernt, keine Wirkung auf die Messungen (die Laeufe
  verwenden kopierte Binaries, der Hostlauf kam danach).
