# DESK50 — STATUS: DONE (2026-10-01) — implementiert, in QEMU belegt; Pi-Abnahme offen

**Auftrag:** `do_wopen` (`references/emutos/desk/desksupp.c:294-322`) und
`do_diropen` (`:422-471`) bauen, die EmuTOS-Aufrufer auf unsere Wege abbilden,
`pro_run`s `(wh, curr)`-Zweig anschliessen (`TASK/DESK50.md`).

## Spezifikation zuerst

Es gab keinen Spec-Eintrag fuer die beiden Funktionen, nur Zeilenverweise in
`deskpro.md`, `deskmain.md` §DESK34 und `deskinf.md` §`opn_appl`. Neu:
**`docs/api/desktop/desksupp-wopen.md`** — beide Funktionen aus der Quelle,
die vollstaendige Aufrufertabelle, die Haelften der Aufrufer (`do_dopen`,
`do_fopen`, `cnx_get`, `search_display`, `pro_run`) und die NewTOS-Abbildung
mit vier benannten Unterschieden. Die ueberholten Gegenbehauptungen sind an
allen Kopien nachgezogen: `deskmain.md` (DESK34-Tabelle, Abweichung 1 und 3,
`cnx_get`-Abweichung 3, `process_funkey`-Absatz), `deskinf.md`
(„Still missing“), `_overview.md`, `pro.rs`-Doc, `do_fopen`-Doc,
`inf.rs:948`.

## Was gebaut ist

`kernel/src/desk/`:

| EmuTOS | NewTOS |
|---|---|
| `do_wopen` `desksupp.c:294-322` | `main::do_wopen(new_win, wh, curr, pt)` |
| `do_diropen` `desksupp.c:422-471` | `main::do_diropen(handle, new_win, curr_icon, pathname, title, pt, redraw)` |
| `do_dopen` `desksupp.c:1041-1079` | `main::do_dopen(curr)` — ersetzt `open_root_window` |
| `do_fopen` `desksupp.c:1089-1160` | `main::do_fopen` — beide Arme enden in `do_diropen` |
| `cnx_get` `deskmain.c:1489-1499` | `main::cnx_get` |
| `w_setpath` `deskfun.c:809-825` | `main::do_close_folder` → `do_fopen(handle, 0, …, false)` |
| `win_alloc`/`win_free` `deskwin.c:141-190` | `main::win_alloc_window`/`win_free_window` (AES- + Listenhaelfte) |
| `obj_get_obid` `deskobj.c:249-261` | `main::obj_get_obid` |
| `graf_growbox_grect` `gembind.c:589-600` | `desk::graf_growbox` (`mod.rs`, Opcode 73, 8/1/0) |
| `pro_run(…, wh, curr)` `deskpro.c:52-64` | `pro::pro_run(isgraf, cmd, tail, wh, curr)` |
| `pn_active`s `WORD` `deskfpd.c:297` | `PNode::pn_active` liefert den gefalteten GEMDOS-Code |
| `pn_open`s `NULL` `deskfpd.c:73-74` | `PNode::pn_open` → `false` ab `LEN_ZPATH` |
| `pn_close` `deskfpd.c:59-63` | `PNode::pn_close` |
| `act_chg(…, dodraw)` `deskact.c:419-466` | `main::act_chg_draw` hat jetzt `dodraw` |

Entfernt: `open_root_window`, `open_dir_window`, `open_dir_window_at`,
`navigate_window` — ihr Rumpf ist `do_diropen`.

**Die Aufrufwege:**

| EmuTOS-Aufrufer von `do_diropen` | NewTOS |
|---|---|
| `do_dopen` `desksupp.c:1067` | `do_dopen` — aus `open_item` (Laufwerkssymbol) und `check_alt_letter_key` (`-letter`) |
| `do_fopen` `desksupp.c:1156` | `do_fopen` — Ordner im Fenster, Ordner-Shortcut, Close-folder |
| `cnx_get` `deskmain.c:1496` | `cnx_get` |
| `search_display` `deskfun.c:413` | **kein Gegenstueck** — die rekursive Suche ist nicht gebaut (`main::do_search` markiert nur im aktuellen Fenster). Wenn sie kommt, oeffnet sie ueber `do_diropen(…, redraw = false)` |

`pro_run`s `do_wopen` erreichen alle drei `do_aopen`-Auspraegungen mit
`G.g_cwin` und dem Symbol (`open_item`s App-Arm, `open_document`,
`do_aopen_drop` mit `dst_obj`/`dobj` aus `dir.rs`), die Funktionstaste mit
`curr = -1` (`deskmain.c:1102`), die Konsole mit `wh = -1`
(`deskmain.c:753`).

**Was sich dadurch beobachtbar aendert** (alles EmuTOS-Verhalten):

1. Zoom-Umriss vom Quellsymbol zum Fenster bzw. bei `pro_run` zum ganzen
   Desktop.
2. Laufwerkssymbol nach dem Oeffnen **abgewaehlt** (vorher blieb es
   ausgewaehlt — Vorher/Nachher-Screendump, 880 px genau im Symbol).
3. Neue Fenster werden per `do_xyfix` (`desksupp.c:300`) ausgerichtet — das
   zweite Fenster steht jetzt bei x=80 statt x=72.
4. Ein `do_diropen`, dessen Verzeichnis nicht lesbar ist (`pn_active < 0`,
   z.B. `EPTHNF`), oeffnet **kein** Fenster mehr (vorher: leeres Fenster);
   `do_dopen` waehlt dann das Symbol mit Neuzeichnen ab.
5. Laufwerkssymbol ohne Laufwerk: `valid_drive`s Alarm (`desksupp.c:1059`),
   vorher stumm.
6. Kein Fensterknoten mehr frei: `STNOWIND`-Alarm statt stillem Abbruch.
7. Busy-Zeiger um jedes Verzeichnisoeffnen (`desksupp.c:428,468`).

## Abnahme (Ticket) — Stand

| Punkt | Beleg |
|---|---|
| Fenster oeffnet mit `graf_growbox` aus dem Symbol, sichtbar im Screendump | `measurements/measurement-2026-10-01-desk50/screendumps/a02-a04` (Laufwerk), `b01-b03` (`cnx_get`), `b07-b09` (Ordner im selben Fenster), `a06-a08` (Alt+C) |
| Nach erfolgreichem Start abgewaehlt | `serial/d50b-serial.log`: `act_chg curr=12 ob_state 0x0001 -> 0x0000 dodraw=false` vor `sh_ldapp: starting C:\D50\HELLO.NCP`; Zoom `b12-b14`. **Nicht neu gezeichnet** — `dodraw = new_win = FALSE` (`desksupp.c:316`), in EmuTOS ebenso; ein Screendump kann es dort wie hier nicht zeigen |
| Nach *Cancel* bleibt es ausgewaehlt | `b06-after-cancel.png`, keine `do_wopen`-Zeile zwischen `B_DIALOG` und `B_CANCELLED` |
| Die `do_diropen`-Aufrufwege gehen durch **eine** Funktion | drei von vier: der vierte (`search_display`) hat bei uns keinen Aufrufer, s.o. |

Methode, Laeufe und alle Serial-Zeilen: `RUNINFO.TXT` im Messordner.
0 `[EXC]`/`[PANIC]` in allen vier Logs, `fsck_msdos -n` sauber.

## Tests und Basislinien

Neu: `tos-api/tests/aes_graf_growbox_desk.rs` — Opcode 73 genau in der
Bindungsform des Desktops (8/1/0) durch den Dispatcher, mit aufzeichnendem
Raster-Backend: positiv „Zoom beginnt am Symbol, endet am Fenster, waechst“,
Rand/Fehlerfall „leeres Quellobjekt + ganzer Desktop bis an den
Bildschirmrand: keine Null-Umrisse, Ende am Desktop, jeder XOR-Umriss wieder
genommen“. Die Desktop-Haelfte selbst ist nicht hosttestbar (`kernel` ist auf
`aarch64-unknown-none-softfloat` gezwungen) — dafuer der QEMU-Lauf.

| | gemessen | Basis |
|---|---|---|
| `cargo test -p tos-api -p tos-types -p xtask --no-fail-fast` | **964 / 0 / 1** | 962/0/1 + 2 neu |
| `cargo xtask build --board qemu-virt --release` (nach `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat --release`) | `tos-api` **64**, `kernel` **94** | 64/94 |
| `cargo xtask build --board rpi4 --release` (ebenso frisch) | `tos-api` **64**, `kernel` **94** | 64/94 |
| `cargo clippy --features board-qemu-virt` | **80 / 125** | 80/125 |
| `cargo clippy --features board-rpi4` | **80 / 125** | 80/125 |
| `cargo clippy -p tos-api --test aes_graf_growbox_desk` | 0 Treffer in der Testdatei | — |
| `sh tools/desk-drift-probe.sh` | **32** | 32 |

## Offen

- **Echte Pi 4:** Doppelklick auf ein Laufwerk (Zoom sichtbar, Symbol danach
  abgewaehlt), Ordner im Fenster (Zoom vom Ordner), Programmstart (Zoom auf
  den ganzen Schirm), *Cancel* im `.NTP`-Kasten (bleibt ausgewaehlt).
- **Nicht gefahren:** Alt gehalten beim Ordneroeffnen (neues Fenster aus
  `do_fopen`), ein scheiterndes `do_diropen` (z.B. veralteter `#W`-Pfad),
  `STNOWIND`.
- **`search_display`** (`deskfun.c:391-433`) — eigenes Paket mit der
  rekursiven Suche.
- Ausserhalb dieses Tickets gesehen, unangetastet: `fun_full_close`s und
  `w_setpath`s `graf_shrinkbox` (`deskfun.c:809-856`, nutzt das jetzt
  gesetzte `w_obid`); `do_fopen`s Laufwerks-Probe `set_default_path` →
  `fun_close` und `STDEEPPA` (`desksupp.c:1097-1108`); `refresh_window`
  (ESC, `desksupp.c:1692`) liest bei uns nicht neu ein und geht nicht ueber
  `do_fopen`; `do_xyfix`s 3D-Rechtsrand-Korrektur (`desksupp.c:265-269`);
  `win_sname` zeigt in EmuTOS die ganze `p_spec`; zwei Wurf-Pfade in
  `dir.rs` verwerfen `do_aopen_drop`s Rueckgabe statt `exit_desktop` zu
  setzen (`deskfun.c:1034,1249`).
