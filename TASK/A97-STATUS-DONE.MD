# A97 — STATUS: DONE (2026-09-21) — Pi-Sichtabnahme offen

**DONE bedeutet hier: Implementierung gelandet, host-verifiziert am
A83-Seam (SIGSEGV → gruen) und in QEMU raspi4b pixelgleich zum bisherigen
Rendering; die Sichtabnahme auf der echten Pi 4 steht aus (Tester/Lead;
nichts geflasht).** Bearbeitet 2026-09-21 zusammen mit A96, Branch
`desk-rebuild`, Basis HEAD `1c4414a`, nichts committet, kein
`git checkout/stash/reset`.

## Auftrag

`NEWDESK_TREE` nach dem A83-Modell lesen: Eigentuemer-Slot mit dem Baum
merken, Zugriff zur Zeichenzeit ueber `usermem::resolve_kva(slot, va)`,
unaufloesbar → Baum gilt als nicht registriert (Fallback-Root, kein
Fault); Hosttest mit zwei Slots; mit A96 den Baum eines toten Slots beim
Sweep loeschen.

## Befund zum Seam (fuer den Lead)

Der Accessor `ObjTree` deckt nur die OBJECT-Records ab. Der Objektzeichner
dereferenziert daneben roh: die INDIRECT-Spec-Zelle, die Sub-Structs
TEDINFO/ICONBLK/CICONBLK/BITBLK/USERBLK (`read_abi_ptr`/`read_abi_word`),
Strings (`read_cstr`, `te_ptext`/`te_ptmplt`/`ib_ptext`, G_STRING/
G_BUTTON/G_TITLE), Raster (`blit_plane` fuer `ib_pmask`/`ib_pdata`/
`bi_pdata`, CICON-Bild ueber `blit_device_mem`, Dither-Maske) — 32
Aufrufstellen, alle in `object.rs`. TeraDesks Hintergrundbaum besteht aus
genau diesen Icons. Eine Aufloesung nur der Records waere ein halber Fix
gewesen; ein ambienter „aktueller Eigentuemer“-Zustand haette die parallel
laufenden Hosttests des Zeichners beeinflusst. Deshalb traegt `ObjTree`
den Eigentuemer, und jede dieser Stellen liest ueber ihn.

Zweiter Befund: `ob_user` machte fuer einen App-Baum die EL0-Callback im
**laufenden** Prozess (`slot_for(current())`), also mit TeraDesks
`ub_code`-VA im Adressraum eines `Pexec`-Kindes — dieselbe Klasse wie der
rohe Datenzugriff, fuer Code. Jetzt nur, wenn der Eigentuemer der laufende
Prozess ist.

## Was getan wurde

### `tos-api/src/aes/object.rs`

* `ObjTree { …, owner: Option<usize> }`; `new`/`lp64`/`app_at` → `None`,
  neu `app_owned(base, abi, owner)`.
* `ObjTree::ua(va)`: ohne Eigentuemer `Some(va)`, sonst
  `window::owner_kva(slot, va)` (= `usermem::resolve_kva`, der A83-Seam mit
  Test-Resolver). `ObjTree::ua_span(va, len)`: erste Seite plus jede
  Seitengrenze im Span aufgeloest; Alias muss `base + offset` fortsetzen
  (Blocks sind seitenausgerichtet, seitengerundet, physisch
  zusammenhaengend — `usermem::Block`), sonst `None`.
* Records: `field_va`, `rd_ptr` (nicht abgebildet → `NIL_RECORD`, ein
  Blatt mit `next/head/tail = -1`, Typ 0, Spec 0 — wird als Kettenende
  gelaufen und als nichts gezeichnet), `wr_ptr` (Schreiben verworfen);
  INDIRECT-Zelle → `spec_none()` wenn nicht abgebildet.
* `read_abi_ptr(t, base, off)` / `read_abi_word(t, base, off)` (0 wenn
  nicht abgebildet); `BitBlk/TedInfo/IconBlk/Cicon/UserBlk::read_abi(base,
  t)`, `ciconblk_mainlist(base, t)`.
* `read_cstr(t, spec)`: seitenweise wie `LiveStr::read`, Ende an der
  ersten Seite ohne fortsetzenden Alias; `blit_plane(t, …)` ueber
  `ua_span` (uebersprungen wenn nicht abgebildet), `blit_plane_kernel` fuer
  den Dither-Puffer (Kernel-Adresse); CICON `col_data`/`sel_data`-Span und
  Dither-Maske ueber `ua_span`; `blit_image(t, …)`, `draw_icon(t, …)`,
  `edit_char(t, …)`.
* `ob_user`: App-Callback nur wenn `t.owner` fehlt oder gleich
  `slot_for(current())`.
* `render_tree_app(…, abi, owner, backend)`; `record_stride(abi)`.

### `tos-api/src/aes/window.rs`

* `NEWDESK_OWNER: AtomicI32` (−1 = Kernel), `newdesk_owner()`,
  `clear_newdesk()`; `set_newdesk` speichert `process::slot_for(current())`;
  ein NULL-Baum loescht auch den Owner.
* `draw_desk_tree`: App-Baum → `root_va = tree + root * record_stride(abi)`
  ueber `owner_kva` aufloesen; `None` → `draw_fallback_root` (der
  `gl_newdesk == NULL`-Arm von `w_drawdesk`); sonst `render_tree_app` mit
  dem Eigentuemer.
* `owner_kva` `pub(crate)` (ein Seam fuer Rahmen und Baum).
* `wm_new_keep_desktop` loescht einen App-Baum (A96).

### Spec

`docs/api/aes/window-graphics.md`: Block „`WF_NEWDESK` stores a pointer
read through the registrant's mapping (NewTOS rule, A97, 2026-09-21)“ nach
dem A83-Block — EmuTOS-Zitate (`wm_set` `gemwmlib.c:1685-1689`,
`w_drawdesk` `:340-359`), das Adressraum-Argument, der Vertrag (Baum,
Sub-Structs, Strings, Raster durch die Abbildung des Registranten; Wurzel
nicht aufloesbar = nicht registriert; Callback nur im Eigentuemerprozess).

## Pruefung

| Beleg | Ergebnis |
|---|---|
| `window.rs::newdesk_owner_tests` (In-Crate, 5 Faelle; `NEWDESK_*` direkt gesetzt, `draw_desk_tree` auf einem Raster, das Fills/Blits/Glyphen protokolliert; Resolver ueber Host-Seiten am `owner_kva`-Seam; `VA = 0x0040_0000` ist im Hostprozess nicht abgebildet) | **rot**: auf einer Scratch-Kopie mit `ua`/`ua_span` = Identitaet und ohne Wurzelpruefung (= HEADs rohes Lesen) stirbt das Lib-Testbinary im ersten Fall mit **SIGSEGV** (`host-tests/newdesk_owner_tests-RED-raw-reads-SIGSEGV.txt`) — der Fault des Tickets; **gruen** 5/5 (`…-GREEN.txt`): Baum des Eigentuemers (Slot 3) statt Slot 5 bei gleicher VA, nur Slot 3 gefragt; haengend → genau ein Lookup, Fallback-Root (`Pattern(0,0,640,480, desktop_root_obspec)`); Icon: Maske+Daten-Blits (100,100,32,32) und 4 Label-Glyphen durch die Abbildung; nicht abgebildetes Datenraster uebersprungen, Maske+Label gezeichnet; Zwei-Seiten-Raster: Alias mit Luecke uebersprungen, fortlaufender Alias gelesen |
| `tests/aes_a96_zombie_windows.rs` Fall 2 | Baum des toten Programms nach dem Sweep 0 (rot ohne Sweep) |
| `cargo run -p xtask -- test` | **804 / 0 / 1** (Basis 797/0/1) |
| `cargo xtask build --board rpi4` frisch | rc 0, tos-api 64 / kernel 94 (Basis 64/94) |
| Clippy rpi4 | 80 / 125 → 80 / 125; `object.rs` 16 → 16, `window.rs` 11 → 11; Testmodul 0 Treffer |
| QEMU raspi4b (`measurements/measurement-2026-09-21-a96-a97/`) | TeraDesk per Desktop-Doppelklick: sein Hintergrundbaum (Laufwerks-/Papierkorb-Icons = ICONBLK/CICON-Raster + Labels, `G_USERDEF`-Wurzel) rendert waehrend seiner eigenen Traps durch `resolve_kva` **pixelgleich** (`tera-before-E` vs `tera-after-E` 0 px; D/G/H/I ebenfalls 0 px); 0 Exceptions |

## Abgrenzung / Offen

* Pi-Sichtabnahme (Tester/Lead; nichts geflasht). Kein Commit.
* Der Fall „EL0-Kind komposiert den Baum des EL0-Eltern“ (TeraDesk startet
  QED, QEDs `wind_open` malt TeraDesks Hintergrund unter QEDs `TTBR0`) ist
  hostseitig modelliert (Slot 3 gegen Slot 5, dieselbe VA), in QEMU nicht
  gefahren.
* `usermem::resolve_kva` unveraendert (meldet keine Blocklaenge; die
  Seitenregel aus A83 ersetzt das).

## Artefakte

Siehe `TASK/A96-STATUS-DONE.MD` (gemeinsames Messverzeichnis
`measurements/measurement-2026-09-21-a96-a97/`). Geaenderte Dateien:
`tos-api/src/aes/object.rs`, `tos-api/src/aes/window.rs`,
`docs/api/aes/window-graphics.md`, `docs/STATUS.md`, `TASK/A97.md`, dieses
Ticket.
