# A83 — STATUS: DONE (2026-09-20) — Pi-Sichtabnahme offen

**DONE bedeutet hier: Implementierung gelandet, host-verifiziert und in QEMU
raspi4b gegen die A79-Referenzdumps bytegleich; die Sichtabnahme auf der
echten Pi 4 steht aus (Tester/Lead; nichts geflasht).** Bearbeitet
2026-09-20, Branch `desk-rebuild`, Basis HEAD `9eece19`, nichts committet,
kein `git checkout/stash/reset`.

## Auftrag

`wind_set(WF_NAME/WF_INFO)` speichert den Zeiger der Anwendung (EmuTOS
`w_pname = *(char **)pinwds`, `aes/gemwmlib.c:1661`, `w_pinfo` `:1669`) und
liest ihn bei jedem Rahmenaufbau (`w_bldactive` `:501-502`); keine
Laengengrenze; NULL → `""` (Spec-Blockzitat `docs/api/aes/window-graphics.md`
„`WF_NAME`/`WF_INFO` store a pointer, not a copy“, Lead-Entscheid). Lead-
Entscheide zum Zugriffsmodell (Eigentuemer-Slot + `usermem::resolve_kva`),
zu den Desktop-Strings (im `WNode`, feste Puffer) und zur Abgrenzung (A96,
A97 nicht hier) — umgesetzt, nicht neu aufgemacht.

## Was getan wurde

### `tos-api/src/aes/window.rs`

* `LiveStr { ptr: u64, slot: Option<usize> }` ersetzt `name`/`info: [u8; 80]`
  im `Window`; `LiveStr::NONE` (= `w_setup`s `w_pname = ""`, `:215-216`) im
  `Window::EMPTY`.
* `LiveStr::of_caller(ptr)`: `slot = process::slot_for(process::current())`
  im `wind_set`-Trap — `Some` fuer einen EL0-Aufrufer, `None` fuer den
  Kernel-Desktop/Selftests (dieselbe Praemisse, unter der `current_abi()` den
  Parameterblock des Desktops als LP64 liest).
* `WF_NAME`/`WF_INFO`-Arme: `w.name = LiveStr::of_caller(wf_str_ptr(pb))`
  (kein Kopieren), dann `redraw_open` wie bisher.
* `LiveStr::read(&self, buf, limit) -> &[u8]`: liest bis NUL, `limit` oder
  Puffergroesse. `slot: None` → direkter Read am Kernel-VA. `slot: Some` →
  **jede Seite** des Strings einmal ueber `owner_kva(slot, va)` =
  `usermem::resolve_kva` (`tos-api/src/bdos/process/usermem.rs:583`) in den
  High-Half-Alias aufgeloest; Begruendung der Seitengranularitaet: alle
  Bloecke der Slot-Tabelle sind seitenausgerichtet und seitengerundet
  (`usermem::Block` Doku, `seed_tpa`, `map` (`page_round_up`, Cursor in
  Seiten), `expose` (`base_pa & !(PAGE-1)`, `span = page_round_up`)), also
  liegt eine Seite, die ein Byte des Strings enthaelt, ganz in demselben
  Block und ihr Alias ist innerhalb der Seite zusammenhaengend
  (`map_pa + offset`). Beim Seitenwechsel wird neu aufgeloest — ein String,
  der ueber sein Blockende laeuft, endet dort; unaufloesbar → Ende; NULL →
  leer. `read_volatile`, weil der Eigentuemer den Puffer jederzeit aendern
  darf.
* `draw_frame`: `name = w.name.read(&mut [0u8; LIVE_STR_MAX], label_walk(name_w))`,
  Zentrierung aus `name.len()`; Info analog mit `label_walk(b.w - 4)`.
  `label_walk(max_w) = max_w/8 + 1` (Glyphen, die passen, plus eine, damit
  ein laengerer String als laenger gemessen wird — die Zentrierung landet
  dann bei `max(2)`, identisch zum unbegrenzten Fall). `LIVE_STR_MAX = 512`
  ist als **Laufgrenze** dokumentiert (Gadget > 4096 px), nicht als
  Vertragslimit.
* `owner_kva(slot, va)`: der Seam; unter `#[cfg(test)]` fragt er den
  Resolver der In-Crate-Tests, sonst `usermem::resolve_kva`.
* Entfernt: `copy_cstr` (nur diese zwei Nutzer), `NAME_CAP`, `name_len`.
  `resolve_kva` selbst unveraendert (es meldet keine Blocklaenge; die
  Seitenregel ersetzt das).

### `kernel/src/desk/win.rs`, `kernel/src/desk/main.rs`

* `WNode.w_name: [u8; W_NAME_LEN = LEN_ZPATH + 2 = 116]`, `w_info: [u8;
  W_INFO_LEN = 72]` (EmuTOS `desk/deskwin.h:73,79`, `include/sysconf.h:51`),
  NUL-terminiert; `Default` von Hand (Arrays > 32); `set_name`, `name_ptr`,
  `info_ptr`; `fill_cstr`. Die ungenutzten `String`-Felder sind weg.
* `main.rs`: `set_window_strings(handle, slot, title, count, size)` — fuellt
  Titel und Infozeile im Node **unter `global::with`**, gibt die zwei
  Adressen heraus und ruft **ausserhalb des Locks** `wind_set(WF_NAME)` und
  `wind_set(WF_INFO)` (Reihenfolge wie bisher, wie `win_sname`/`win_sinfo`,
  `deskwin.c:847-858,905`). Ersetzt `set_window_title` und die sechs
  Stack-Puffer-Stellen (`set_window_mask` :1103/:1108, `open_dir_window`
  :1626/:1630, `navigate_window` :1701/:1705). `fmt_info(out: &mut [u8], …)`
  schreibt in `w_info` (gleiche Ausgabe, Puffer 72 statt 64).

**Adress-Stabilitaet der Desktop-Strings:** `DESK` ist ein `static
SpinMutex<Option<DeskState>>` (`desk/global.rs:519`); `DeskState` wird genau
einmal in `global::init` gebaut (`guard.is_none()`-Wache, `:527-532`) und nie
ersetzt. `windows.slots: Vec<WNode>` wird nur in `win_start` dimensioniert
(`clear` + `resize_with(NUM_WNODES)`, `win.rs:150-151`), das nur `init`
ruft; sonst kein `push`/`resize`/`insert`/`remove` auf `slots` (grep ueber
`kernel/src/desk/`), der Vec-Puffer wird also nie umgelegt. `win_alloc` und
`win_free` ueberschreiben das Element **in place** (`slots[slot] = node`
bzw. `= free_slot()`) — die Adresse des Elements und damit der inline
liegenden Arrays bleibt; `win_free` laeuft erst nach `wind_delete` (der
AES-Slot ist dann nicht mehr `in_use`, der naechste `wind_create` setzt
`EMPTY`), `win_alloc` nullt den Node, bevor `set_window_strings` den Zeiger
herausgibt. Feste Arrays statt `String`, weil ein `String`-Puffer bei
Wachstum umziehen kann.

## Pruefung

| Beleg | Ergebnis |
|---|---|
| `tos-api/tests/aes_window_name_pointer.rs` (T10, 79-Zeichen, NULL) | `#[ignore]` entfernt; **3/3 gruen** (`measurements/measurement-2026-09-20-a83/host-tests/aes_window_name_pointer-GREEN.txt`); rot auf der Kopie: `measurements/measurement-2026-09-20-a83-a84/host-tests/aes_window_name_pointer-RED-tree-a8d3163-include-ignored.txt` |
| `window.rs::live_title_tests` (In-Crate, 5 Faelle) | **5/5 gruen** (`host-tests/live_title_tests-GREEN.txt`): Eigentuemer-Mapping statt aktuellem Slot (Slot 3 und 5 bilden dieselbe VA auf „OWNER“/„OTHER“ ab, Titel = „OWNER“, Resolver nur mit Slot 3 gefragt), haengende VA → leerer Titel ohne Zugriff, String am Blockende endet dort, String ueber zwei getrennte Host-Seiten wird zusammengesetzt, `label_walk`. **Modelliert am Seam**, weil der Host keine Prozesstabelle installieren kann (`process::current()` ist null) und `resolve_kva`s `map_pa + off + VA_OFFSET` auf dem Host keine Adresse ist (Overflow im Debug-Profil) |
| `cargo run -p xtask -- test` | **734 passed / 0 failed / 2 ignored** (Basis 727/0/4: +2 aktivierte, +5 neue) — `host-tests/xtask-test.log` |
| `cargo xtask build --board rpi4` frisch (`cargo clean -p kernel -p tos-api --target …`) | rc 0, **tos-api 64 / kernel 94** (Basis 64/94, Delta 0) — `build/xtask-build-rpi4-fresh.log`; `kernel8.img` sha256 `e24d72eb…` (`build/kernel8-tree-sha256.txt`) |
| Clippy rpi4 (`cargo clippy -p kernel --no-default-features --features board-rpi4`) | tos-api 81 / kernel 125 → **81 / 125** (Basis per Hand-Tausch der HEAD-Kopien der drei Dateien gemessen, `build/clippy-rpi4-baseline-HEAD-9eece19.txt`; danach zurueckgetauscht, `cmp` identisch) — `build/clippy-rpi4-tree.txt`; `cargo clippy -p tos-api --lib --tests`: lib-test 83 (81 Duplikate + 2 vorbestehende Test-only, `pd.rs` „items after a test module“ u. a.), **0 Treffer in `window.rs` ab Zeile 4839 (Testmodule) und 0 in `aes_window_name_pointer.rs`** — `build/clippy-tos-api-lib-tests.txt` |
| qemu-virt | nicht gebaut (Lead-Vorgabe) |

### QEMU raspi4b (A79-Methode, `measurements/measurement-2026-09-20-a83/`)

Scratch-Kernel aus einer rsync-Kopie der Crates mit dem H02/A46/A78-Injector
(`method/make_scratch.sh`, Scripts `method/final_scripts.rs` = A79 desk/tera
bytegleich, `method/nav_scripts.rs` = `desk-nav.actions`); SD-Images
`tools/make-sd-image.sh` 64 MiB (+ `newdesk-tera.inf`/`teradesk-window.inf`
per `sd-put.sh` fuer TeraDesk); jeder Lauf auf einer frischen Kopie; 640x480,
`-display none`, QMP-Screendump 0,3 s nach jeder Serial-Marke; keine `*.img`
abgelegt.

| Lauf | Dump | Vergleich |
|---|---|---|
| desk (sd-base, Kernel-Desktop; Doppelklick „C Hard disk“ (35,38)) | `desk-A-idle.png` | sha256 `9cd7377f…` = `a79/final-desk-A-idle.png` |
| | `desk-B-two.png`, `desk-C-two-2s.png` | sha256 `9638ccf0…` = `a79/final-desk-B-two.png` (**bytegleich**), B = C |
| tera (sd-tera, TeraDesk `#Z`; Doppelklick HARD DISC C (195,35)) | `tera-A-idle.png` | `7696570…` = `a79/final-tera-A-idle.png` |
| | `tera-B-window.png`, `tera-C-window-2s.png` | `2280f56d…` = `a79/final-tera-B-window.png` (**bytegleich** — Titel `C:\*.*` und Infozeile eines **EL0-Eigentuemers**, gelesen ueber `resolve_kva`) |
| nav (sd-base; Doppelklick Ordner `ACC` (94,104), dann Closer des neuen Fensters (83,78)) | `nav-A-idle.png` | = `a79/final-desk-A-idle.png` (0 px) |
| | `nav-B-folder.png` | Fenster `C:\ACC\`, Info ` 0 items   0 bytes used` (Titel/Info aus dem `WNode`, `open_dir_window`) |
| | `nav-C-navigated.png`, `nav-D-navigated-2s.png` | `navigate_window`: **derselbe** Node, Titel in place → `C:\`, Info ` 24 items   373645 bytes used`; C = D (0 px); gegen `a79/final-desk-B-two.png` 1081 px in (2,30)-(72,127) = nur der Icon-Auswahlstreifen (dort das gewaehlte Laufwerks-Icon, hier das gewaehlte `ACC`-Icon im Hinterfenster), Rahmen und Titelbaender gleich |

`grep -c "EXC\|PANIC\|panic"` = 0 in `serial/desk-serial.log`,
`serial/tera-serial.log`, `serial/nav-serial.log`.

## Abgrenzung (Lead-Vorgabe, im Quittungsticket festzuhalten)

* **A96 — Fenster eines beendeten Prozesses.** Der `LiveStr` traegt den
  Eigentuemer-Slot; ob eine haengende VA als `""` erscheint, haengt vom
  Zustand der Slot-Tabelle ab: `pterm::release_slot` raeumt die
  `usermem`-Blocktabelle **nicht** (sie wird erst beim naechsten Laden per
  `seed_tpa` neu gesetzt, `pterm.rs:170-178`) — bis dahin loest
  `resolve_kva` weiter in den freigegebenen Poolblock auf (lesbar ueber den
  High-Half-Alias: **kein Fault**, alter Text); nach einer Neubesiedlung des
  Slots loest eine TPA-VA in die TPA des **neuen** Prozesses auf (fremder
  Text), eine alte Heap-Fenster-VA ist weg (`""`). Ein Fault ist in keinem
  Fall moeglich (nur Alias-Reads auf Speicher, den die Tabelle nennt). Das
  Wegraeumen der Fenster bleibt A96.
* **A97 — `NEWDESK_TREE`** wird weiterhin roh gelesen; nicht angefasst.

## Artefakte

`measurements/measurement-2026-09-20-a83/`: `method/` (`product-a83.diff`,
`make_scratch.sh`, `desk-nav.actions`, `nav_scripts.rs`, A79-Skripte),
`build/`, `host-tests/`, `screendumps/`, `serial/`, `manifest.sha256`,
`RUNINFO.TXT`. Geaenderte Dateien im Baum: `tos-api/src/aes/window.rs`,
`tos-api/tests/aes_window_name_pointer.rs`, `kernel/src/desk/win.rs`,
`kernel/src/desk/main.rs`, `docs/STATUS.md`, `TASK/A83.md`, dieses Ticket
(ersetzt `A83-STATUS-UNFINISHED.MD`).

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht). Kein Commit.

---


*(Stand des Zwischenberichts vom Nachmittag, Basis HEAD `a8d3163`; die
Vorpruefung wurde zur Grundlage des Lead-Entscheids und ist hier unveraendert
nachgefuehrt.)* Spec-Vertrag (Researcher, uncommitted, unveraendert gelassen):
`docs/api/aes/window-graphics.md`, Blockzitat „`WF_NAME`/`WF_INFO` store a
pointer, not a copy; no length limit (contract, A83, 2026-09-20)“ mit dem
Lead-Entscheid „NewTOS matches this contract“.

## Vorpruefung (Zwischenbericht, unveraendert)

### Auftrag und Stoppregel

Zeiger speichern (`w_pname`/`w_pinfo`, EmuTOS `aes/gemwmlib.c:1661,1669`),
bei jedem Rahmenaufbau lesen (`w_bldactive` `:501-502`), NULL → `""`.
Vorgabe des Leads: vor der Implementierung (a) das Zugriffsmodell fuer
Anwendungszeiger zur Zeichenzeit pruefen — „if the raw read is only valid
while the owner is current, say so and STOP rather than improvising a fix“
— und (b) pruefen, was mit den Fenstern eines beendeten Eigentuemers
geschieht. Beides ist geprueft; (a) trifft den Stoppfall.

### (a) Zugriffsmodell: kein flacher Adressraum

**Heutiges Vorbild.** `NEWDESK_TREE` (`tos-api/src/aes/window.rs:1070`)
haelt den EL0-Zeiger der Anwendung (TeraDesks Hintergrundbaum) und
`draw_desk_tree` (`:1281`) dereferenziert ihn **roh** zur Composite-Zeit
(`render_tree_app(tree as *const Object, …)`), unabhaengig davon, welcher
Prozess gerade laeuft; nur die ABI-Breite wird aus der Registrierungszeit
nachgespielt (`NEWDESK_ABI`, Kommentar `:1085-1091`: „possibly when a
different (or no) process is current“). `copy_cstr` (`:2935`) liest den
`WF_NAME`-Puffer ebenfalls roh — aber **innerhalb des Traps des
Eigentuemers**, also immer unter dessen Adressraum.

**Adressraum.** NewTOS ist nicht flach:

* `kernel/src/arch/aarch64/el0.rs:45-47`: „subsequent processes share the
  same VA layout via per-process TTBR0 swap“.
* `tos-api/src/bdos/process/mmu.rs`: je Prozess-Slot eigene L0..L3-Tabellen,
  `install_ttbr0` (`:381`) schreibt `TTBR0_EL1`.
* `kernel/src/arch/aarch64/exceptions.rs:1486-1502`: ein verschachteltes
  EL0-`Pexec` installiert das `TTBR0` des Kindes; `:973`: „Parent and child
  share identical EL0 VAs (both TPAs map at VA 0x100)“; `:988-990`: beim
  `Pterm` des Kindes zurueck auf die Boot-Tabellen (`restore_boot_ttbr0`,
  Identitaetsabbildung 0..4 GiB) und dann auf das Eltern-`TTBR0`
  (`parent_l0_pa`), bei einem vom Kernel gestarteten Kind nur auf die
  Boot-Tabellen.
* `tos-api/src/bdos/process/usermem.rs`: der EL0-Heap ist ein
  **versetztes** Fenster (user VA ≠ PA), je Slot eine Blocktabelle
  (`seed_tpa` `:266`, `map`), `resolve_kva(slot, user_va)` (`:583`) liefert
  die Kernel-High-Half-Adresse eines Prozess-VA per Containment — genau das
  Muster, das `xbios/screen.rs::caller_base_to_kva` (`:181`) fuer `Setscreen`
  schon nutzt.

**Folge.** Ein roher Lesezugriff auf eine niedrige VA von EL1 aus laeuft
ueber das *gerade installierte* `TTBR0`. Der gespeicherte Titelzeiger ist
also nur gueltig, solange sein Eigentuemer der laufende Prozess ist (oder
der Eigentuemer der Kernel-Desktop ist, dessen Zeiger in der High Half
liegen). Ein Rahmen wird aber auch aus dem AES-Aufruf eines **anderen**
Prozesses neu gebaut:

1. **Verschachteltes Kind ueber Eltern-Fenster.** Ein EL0-Programm (A,
   z. B. TeraDesk) hat Fenster offen und startet per `Pexec` ein Kind (B).
   Bs `wind_open`/`wind_close`/Verschieben komposiert die Rahmen von As
   Fenstern (Rechteckliste, `draw_change`, `WM_REDRAW`-Fanout) — unter Bs
   `TTBR0`. As Titel-VA liest dann Bs Speicher an derselben VA (Garbage-Titel)
   oder, falls die VA in B nicht abgebildet ist (Heap-Fenster, Bump-Cursor),
   loest sie einen EL1-Data-Abort aus. Die AES raeumt As Fenster beim
   Kindstart nicht weg (kein `wm_new`; EmuTOS kennt diesen Weg nicht: dort
   beendet `shel_write` das aufrufende Programm, `sh_ldapp` sweept mit
   `wm_new`, `gemshlib.c:676-677`). Dieselbe Luecke besteht heute schon fuer
   `NEWDESK_TREE` (TeraDesks Baum unter QEDs `TTBR0`), ist aber nicht
   Gegenstand dieses Tickets.
2. **Desktop ueber Fenster eines beendeten Kindes** (siehe (b)): nach dem
   `Pterm` laeuft `desktop_resume` → `redraw_desktop`
   (`kernel/src/desk/main.rs:571`, `desk/redraw.rs:214`) →
   `form_dial(FMD_FINISH)` ueber alle noch offenen AES-Fenster — unter den
   Boot-Tabellen. Ein stehengebliebenes Kind-Fenster wuerde seinen Titel
   von PA = VA lesen (Garbage, bei VA jenseits des RAMs ein Bus-Fehler).

Heute ist beides unschaedlich, weil `copy_cstr` den Titel im Trap des
Eigentuemers kopiert. Unter dem Zeigervertrag ist das rohe Lesen zur
Zeichenzeit deshalb **nicht** das Modell von `NEWDESK_TREE` „einfach
uebernehmen“, sondern eine Uebersetzung noetig: STOP nach Vorgabe.

### (b) Fenster ueberleben ihren Eigentuemer (Befund fuer den Lead)

* `tos-api/src/bdos/process/pterm.rs:142-239` (`terminate_inner`,
  `release_slot`): `resource::on_process_exit(slot)` (RSC-Listen),
  `free_all_owned`/`reserve_all_owned` (Pool), `usermem::clear_uncached`,
  `aes::release_client(slot)` → `pd::release` (Deskriptor-Flags/Queue). Die
  **AES-Fenstertabelle wird nicht angefasst.**
* Shell-Loop-Pfad (`kernel/src/shell_loop.rs:327-331`): nach einem
  Grafikprogramm `wm_new()` — EmuTOS-getreu (`gemshlib.c:676-677`). TeraDesk
  per `#Z` nimmt diesen Weg: sauber.
* Desktop-Doppelklick-Pfad (`kernel/src/desk/main.rs:524` `run_gem_app` →
  `kernel/src/launch.rs:66` `run_program_with_tail_rc` → `desktop_resume`):
  **kein `wm_new`**; nach `Pterm` nur `close_client_if_open`. Die Luecke ist
  als G1/G2 im Kommentar von `desktop_relinquish` (`main.rs:546`) benannt.
  Ein Programm, das seine Fenster nicht selbst mit `wind_close`/`wind_delete`
  schliesst (Absturz, schlampiges Ende), hinterlaesst sie in der Tabelle;
  `redraw_desktop` komposiert sie weiter (heute mit kopiertem Titel).
* Verschachteltes EL0-`Pexec`-Kind (`exceptions.rs`): ebenfalls kein Sweep.

EmuTOS haelt `w_pname` beim Anlegen auf `""` (`w_setup`, `gemwmlib.c:215-216`)
und verlaesst sich sonst auf `wm_new` nach jedem Shell-Programm.
→ Eigener Task fuer den Lead: Fenster eines beendeten Prozesses
`wm_new`-artig schliessen/loeschen (Desktop-Startpfad und `Pexec`-Kind),
oder `run_gem_app` durch den Shell-Loop fuehren (G1/G2).

### Kernel-Desktop: Stack-Puffer als `WF_NAME`/`WF_INFO`

`kernel/src/desk/main.rs:1108` (`set_window_mask`, Info-Zeile) und `:1134`
(`set_window_title`, von dort gerufen), `:1626,1630` (`open_dir_window`),
`:1701,1705` (`navigate_window`): `let mut tbuf = [0u8; 96]; … wind_set_ptr(handle, WF_NAME,
tbuf.as_ptr())` und `let mut info = [0u8; 64]` — Stack-Puffer, deren
Kommentar ausdruecklich auf die Kopie im Trap baut. Unter dem Zeigervertrag
haengende Zeiger. EmuTOS' Desktop haelt beide Strings im `WNODE`
(`desk/deskwin.h:73` `w_name[LEN_ZPATH+2]`, `:79` `w_info[72]`;
`win_sname` `deskwin.c:847-858`, `win_sinfo` `:905` uebergibt
`pwin->w_info`). Unser `WNode` (`kernel/src/desk/win.rs:101-103`) hat
`w_name: String`/`w_info: String` **ungenutzt**; `slots: Vec<WNode>` wird
einmal in `win_start` angelegt (`:150-151`) und nie mehr umgelegt, die
`DeskState` liegt in einem `static SpinMutex` (`desk/global.rs:519`) — ein
NUL-terminierter Puffer im `WNode` haette eine stabile High-Half-Adresse
fuer die Lebensdauer des Desktops.

### Vorschlag (Nachmittag; so umgesetzt)

1. `Window` bekommt `name: u64`, `info: u64` (die rohen Aufruferzeiger,
   Init 0 → `""`) **und** den Eigentuemer-Slot (`Option<usize>` aus
   `process::slot_for(process::current())` zur `wind_set`-Zeit; `None` =
   Kernel/Desktop). `draw_frame` liest den String bei jedem Aufbau ueber
   einen Helfer `live_cstr(owner, ptr, &mut buf)`: `None` → direkter
   High-Half-Read; `Some(slot)` → `usermem::resolve_kva(slot, ptr)` und Read
   an der Kernel-Adresse, begrenzt auf den Rest des Blocks; nicht
   aufloesbar oder NULL → `""`. Kein Zugriff ueber `TTBR0`, keine Abhaengigkeit
   vom laufenden Prozess. Laengenschranke nur durch die Gadget-Breite
   (`draw_label` bricht bei `max_w`).
2. `kernel/src/desk/`: `w_name`/`w_info` im `WNode` als NUL-terminierte
   Puffer fuehren (EmuTOS-Groessen) und deren Adresse an `wind_set` geben;
   die drei Aufrufstellen umstellen.
3. Der Befund (b) bleibt ein eigener Task (Fenster-Sweep nach Prozessende).

Aufwand fuer 1 auf dem Host: die Scratch-Fassung
`measurements/measurement-2026-09-20-a83-a84/method/scratch-a83-pointer-model-host-only.diff`
(ohne Slot/`resolve_kva`, nur Host) hat 83 Diff-Zeilen und macht T10 gruen.

### Tests (Nachmittag; jetzt aktiv)

`tos-api/tests/aes_window_name_pointer.rs` (neu; eigener 1920×1080-Backend,
weil ein Titel ueber 79 Glyphen bei 640 px nie in das Gadget passt):

| Test | heute (Baum) | Scratch-Zeigerfassung |
|---|---|---|
| `t10_title_rewritten_in_place_shows_on_the_next_frame_repaint` — `#[ignore]` | rot: „after the in-place rewrite and a re-top the title band still shows AAAAAAAA“ (Hash gleich) | gruen |
| `a_title_longer_than_79_characters_is_not_truncated` — `#[ignore]` | rot: 100-Zeichen-Titel rendert wie die ersten 79 (Hash gleich) | gruen (86. Glyphe gemalt) |
| `a_null_pointer_is_an_empty_title` — aktiv | gruen | gruen |

Belege: `measurements/measurement-2026-09-20-a83-a84/host-tests/aes_window_name_pointer-RED-tree-a8d3163-include-ignored.txt`,
`…-GREEN-under-scratch-pointer-model.txt`, `…-TREE-ignored.txt`. Die
`#[ignore]`-Begruendung im Test verweist hierher; beim Umsetzen das Attribut
entfernen.

### Nicht angefasst (Stand Nachmittag)

* `window.rs` `name`/`info`/`copy_cstr`/`WF_NAME`/`WF_INFO`-Arme unveraendert;
  `kernel/src/desk/` unveraendert; Spec unveraendert.
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
