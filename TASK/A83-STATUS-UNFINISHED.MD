# A83 — STATUS: UNFINISHED (blockiert nach Vorpruefung; Rueckfrage Lead)

**UNFINISHED bedeutet hier: die Implementierung ist nach der vom Lead
verlangten Vorpruefung gestoppt, nicht: sie ist versucht und gescheitert.**
Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis HEAD `a8d3163`, nichts
committet. Spec-Vertrag (Researcher, uncommitted, unveraendert gelassen):
`docs/api/aes/window-graphics.md`, Blockzitat „`WF_NAME`/`WF_INFO` store a
pointer, not a copy; no length limit (contract, A83, 2026-09-20)“ mit dem
Lead-Entscheid „NewTOS matches this contract“.

## Auftrag und Stoppregel

Zeiger speichern (`w_pname`/`w_pinfo`, EmuTOS `aes/gemwmlib.c:1661,1669`),
bei jedem Rahmenaufbau lesen (`w_bldactive` `:501-502`), NULL → `""`.
Vorgabe des Leads: vor der Implementierung (a) das Zugriffsmodell fuer
Anwendungszeiger zur Zeichenzeit pruefen — „if the raw read is only valid
while the owner is current, say so and STOP rather than improvising a fix“
— und (b) pruefen, was mit den Fenstern eines beendeten Eigentuemers
geschieht. Beides ist geprueft; (a) trifft den Stoppfall.

## (a) Zugriffsmodell: kein flacher Adressraum

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

## (b) Fenster ueberleben ihren Eigentuemer (Befund fuer den Lead)

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

## Kernel-Desktop: Stack-Puffer als `WF_NAME`/`WF_INFO`

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

## Vorschlag (nicht umgesetzt)

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
`measurement-2026-09-20-a83-a84/method/scratch-a83-pointer-model-host-only.diff`
(ohne Slot/`resolve_kva`, nur Host) hat 83 Diff-Zeilen und macht T10 gruen.

## Tests (bereitgestellt)

`tos-api/tests/aes_window_name_pointer.rs` (neu; eigener 1920×1080-Backend,
weil ein Titel ueber 79 Glyphen bei 640 px nie in das Gadget passt):

| Test | heute (Baum) | Scratch-Zeigerfassung |
|---|---|---|
| `t10_title_rewritten_in_place_shows_on_the_next_frame_repaint` — `#[ignore]` | rot: „after the in-place rewrite and a re-top the title band still shows AAAAAAAA“ (Hash gleich) | gruen |
| `a_title_longer_than_79_characters_is_not_truncated` — `#[ignore]` | rot: 100-Zeichen-Titel rendert wie die ersten 79 (Hash gleich) | gruen (86. Glyphe gemalt) |
| `a_null_pointer_is_an_empty_title` — aktiv | gruen | gruen |

Belege: `measurement-2026-09-20-a83-a84/host-tests/aes_window_name_pointer-RED-tree-a8d3163-include-ignored.txt`,
`…-GREEN-under-scratch-pointer-model.txt`, `…-TREE-ignored.txt`. Die
`#[ignore]`-Begruendung im Test verweist hierher; beim Umsetzen das Attribut
entfernen.

## Nicht angefasst

* `window.rs` `name`/`info`/`copy_cstr`/`WF_NAME`/`WF_INFO`-Arme unveraendert;
  `kernel/src/desk/` unveraendert; Spec unveraendert.
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
