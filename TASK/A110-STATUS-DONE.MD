# A110 — STATUS: DONE (2026-09-23) — Pi-Sichtabnahme offen

**DONE bedeutet hier: beide Punkte gelandet, host-verifiziert (rot → gruen)
bzw. in QEMU raspi4b vorher/nachher belegt; die Sichtabnahme auf der echten
Pi 4 steht aus (Tester/Lead; nichts geflasht).** Bearbeitet 2026-09-23
zusammen mit A104 und A91 (ein Paket, ein Messverzeichnis), Branch
`desk-rebuild`, Basis HEAD `671b362`, nichts committet, kein
`git checkout/restore/stash/reset`.

---

## 1. Eigentuemer an den `wind_update`-Zaehlern

### Was EmuTOS tut (an der gepinnten Quelle geprueft)

* **Die Bildschirmsperre hat einen Eigentuemer.** `wind_spb` ist ein `SPB`;
  `tak_flag` schreibt den nehmenden Prozess hinein —
  `sy_tas++; if ((sy_owner != rlr) && (sy_tas != 1)) sy_tas--; else sy_owner =
  rlr;` (`references/emutos/aes/gemflag.c:68-86`) — und `unsync` loescht das
  Feld auf der 1→0-Flanke (`:125-129`). Das Feld benennt **genau einen**
  Prozess: ein zweiter wird abgewiesen und auf `sy_wait` geparkt
  (`wm_update`s `ev_block(MU_MUTEX, (LONG)&wind_spb)`, `gemwmlib.c:1785-1786`),
  der Zaehler ist die Schachtelungstiefe dieses einen.
* **Gelesen wird es an genau einer Stelle:** `sh_ldapp` nimmt die Sperre einem
  beendeten Programm nur ab, wenn es sie hielt — `if (wind_spb.sy_owner == rlr)
  unsync(&wind_spb);` (`aes/gemshlib.c:668-669`).
* **`ml_ocnt` hat keinen Eigentuemer** (`aes/gemfmlib.c:51`, ein blankes
  `WORD`). `ml_pmown` (`:58`) ist der *Maus*-Eigentuemer, den `fm_own` sich
  merkt, nicht der Halter des Zaehlers. EmuTOS kommt ohne aus: der einzige
  Zwangs-Drain ist `wm_new`s `while (ml_ocnt > 0) wm_update(END_MCTRL);`
  (`gemwmlib.c:1879-1881`), und der laeuft aus der Shell-Schleife, wenn das
  gesweepte Programm der einzige Klient war (`gemshlib.c:676-677`).

### Warum wir beide brauchen

A107s Sweep (`wm_new_for_slot`) laeuft, waehrend der **Elternprozess lebt**.
Ein blinder Drain wuerde dem Eltern die Maus zurueckgeben und seinen geparkten
Menuebaum mitten in seinem eigenen modalen Dialog entparken. Also: die Sperre
bekommt EmuTOS' eigenes Feld, der Mauszaehler eine **benannte
NewTOS-Ergaenzung** mit denselben Flanken wie `fm_own`s uebriger 0→1/1→0-Zustand.

### Was gebaut ist — `tos-api/src/aes/window.rs`

* `UPDATE_OWNER: AtomicUsize` (= `wind_spb.sy_owner`) und
  `MCTRL_OWNER: AtomicUsize`, Sentinel `NO_OWNER = usize::MAX` (= `sy_owner =
  NULL`, `gemflag.c:128`, `geminit.c:838`). Doc-Kommentare mit den obigen
  Zitaten, inklusive der Begruendung, warum der zweite eine Ergaenzung ist.
* `update_beg` ist jetzt `tak_flag`: inkrementieren, und `sy_owner = rlr`, wenn
  der Aufrufer die Sperre schon hat **oder** der Zaehler gerade 1 geworden ist.
  **Die Abweisung ist der eine Teil, der nicht nachgebaut ist** — es gibt
  keinen AES-Scheduler, auf dem man einen zweiten Nehmer parken koennte; der
  Zaehler steigt, der erste Eigentuemer behaelt die Sperre. Im Doc benannt.
* `update_end` ist `unsync`: klemmen bei 0, und auf der 1→0-Flanke den
  Eigentuemer loeschen.
* `mctrl_own`: `MCTRL_OWNER` auf der 0→1-Flanke gesetzt (neben `hold_menu` /
  `ML_PMOWN` / `ct_chgown`), auf der 1→0-Flanke geloescht.
* `release_locks_of(slot)`, neu, aufgerufen als letzte Anweisung von
  `wm_new_for_slot`: `wm_new`s beide Drain-Schleifen in ihrer Reihenfolge,
  jeweils gegattert auf `owner == slot`. **`ml_ocnt` geht durch
  `mctrl_own(false)`**, nicht durch einen Store — nur so laeuft `fm_own`s
  1→0-Rumpf (`ct_chgown(ml_pmown, &ml_ctrl); gl_mntree = ml_mnhold;`,
  `gemfmlib.c:99-104`) und sein `END_UPDATE` (`:105`), das die vom
  `BEG_MCTRL` des Kindes genommene Sperrstufe wieder abtraegt.
* Die drei pauschalen Ruecksetzer (`wm_new`, `wm_new_keep_desktop`, `reinit`)
  loeschen die Eigentuemerfelder mit.
* Testhaken `test_update_lock()` / `test_mctrl_count()` → `(count, owner)`.

### Tests: rot vor, gruen nach

`tos-api/tests/aes_a96_zombie_windows.rs`, drei neue `a110_*`-Faelle auf der
A107-Prozessfixture (zwei echte GEMDOS-Slots, `pterm_child_of`):

| Fall | rot (ohne `release_locks_of`) | gruen |
|---|---|---|
| `a110_a_living_clients_screen_lock_survives_a_childs_death` | gruen (Wache) | gruen |
| `a110_a_dead_clients_screen_lock_is_released` | `(2, Some(1))` statt `(0, None)` | gruen |
| `a110_a_dead_clients_mouse_control_hold_is_released_and_unparks_the_menu` | `(1, Some(1))` statt `(0, None)` | gruen |

Rote Zeile mit der Mutation „`release_locks_of(slot)` auskommentiert", danach
aus der Sicherungskopie zurueckgeholt (kein `git`-Eingriff):
`measurements/measurement-2026-09-23-a104-a110/host-tests/a110-RED-no-lock-release.txt` /
`a110-GREEN-after.txt` (8/8).

---

## 2. Infozeile des Kernel-Desktops: `STINFST2` und seine Aufrufer

### Quelle

`win_sinfo(pwin, check_selected)` (`desk/deskwin.c:867-906`) zaehlt optional
die selektierten `FNODE`s und summiert `f_size`, waehlt dann zwischen
`STINFST2` (`" %ld bytes used in %d selected items."`, `desk/desk_rsc.c:2469`)
und `STINFOST` (`" %ld bytes used in %d items."`, `:2468`, mit `p_count`/
`p_size` des ganzen Verzeichnisses) und schreibt mit `sprintf(pwin->w_info,
alert, select_size, select_count)` (`:905`), gefolgt von `wind_set(pwin->w_id,
WF_INFO, pwin->w_info, 0, 0)`. Fuenf Aufrufstellen: `FALSE` in
`rebuild_window` (`deskfun.c:141`) und `do_diropen` (`desksupp.c:450`), `TRUE`
in `hndl_button` (`deskmain.c:982-983`), `desk_clear` (`desksupp.c:150-153`),
`mark_matching_fnodes` (`deskfun.c:380-382`) und `fun_selall`
(`deskfun.c:627-631`).

### Was gebaut ist

* **`kernel/src/desk/main.rs`**: `fmt_info(out, count, size, selected)` —
  `selected` waehlt `STINFST2`. Neu `win_sinfo(handle, check_selected)`, die
  Funktion oben 1:1; das Desktop-Handle geht durch wie in EmuTOS
  (`win_find(DESKWH)` liefert `&G.g_wdesktop`, `deskwin.c:202-203`, dessen
  Dateiliste leer ist, und das `wind_set` landet auf einem Fenster, das
  `VF_INUSE`, aber nie `VF_ISOPEN` ist — es speichert einen Zeiger und
  zeichnet nichts). Neu `desk_clear(wh)` als Huelle um die Zustandshaelfte
  (`DeskState::desk_clear`) plus `win_sinfo(wh, true)` fuer alles ausser dem
  Desktop — genau `desksupp.c:121-153`.
* Aufrufstellen verdrahtet: `hndl_button` (Einzelklick-Zweig, nach
  `act_bsclick`/`act_bdown`), der neue `desk_clear` (und ueber ihn
  `desk_clear_all`, `kbd_arrow`, `top_msg`, der Drag-Abschluss in `act_bdown`),
  `search_current_window`, und **`kernel/src/desk/dir.rs`** `fun_selectall` +
  `fun_win2win`.
* **`docs/api/desktop/deskwin.md`** § `win_sinfo`: der A109-Absatz („kein
  Aufrufer") ist durch den A110-Stand ersetzt, mit einer Tabelle
  EmuTOS-Stelle → NewTOS-Stelle. **`docs/api/desktop/deskmain.md`** Schritt 5:
  ein Satz, dass der `:982-983`-Aufruf jetzt steht.

---

## Pruefung

| Beleg | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **926 passed / 0 failed / 1 ignored** (Basis 919/0/1; +3 hier, +4 A104) |
| `cargo xtask build --board rpi4` **und** `--board qemu-virt`, je frisch (`cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat`) | tos-api **64** / kernel **94** auf beiden, rc 0 — Delta 0 |
| `cargo clippy -p tos-api` / `-p kernel --features board-rpi4` (beide `--target aarch64-unknown-none-softfloat`) | **80** / **125** — unveraendert; `desk/main.rs` 0 → 0, `desk/dir.rs` 0 → 0, `window.rs` 11 → 11 |

### QEMU raspi4b (`measurements/measurement-2026-09-23-a104-a110/`)

A107-Methode (`method/`, Scratch-Kopie der Crates mit dem H02-Injektor,
640x480, `-display none`, QMP-Screendump 0,3 s nach jeder Serial-Marke,
frische Imagekopie je Lauf, `parkbr` vor jedem Dump). `BEFORE` = dieselbe
Scratch-Kopie mit den **vier** neuen `win_sinfo(…, true)`-Aufrufen entfernt
(nur im Scratch, der Baum wurde nicht angefasst).

**Kernel-Desktop** (`sd-base`, ohne `#Z`, `method/desk-a110.actions`), vier
Marken; 0 `EXC`/`PANIC` in beiden Serial-Logs:

| Marke | Aktion | Infozeile (nachher) | Diff vorher↔nachher |
|---|---|---|---|
| A | Ruhe | ` 418693 bytes used in 26 items.` | **0 px** |
| B | ein Symbol (`ACC`, ein Ordner) angeklickt | ` 0 bytes used in 1 selected items.` | **1011 px**, Bbox (58,70)-(317,83) |
| C | Gummiband ueber `SETJMPT.NCP` + `VDI.TSV` | ` 18140 bytes used in 2 selected items.` | **1189 px**, Bbox (58,70)-(349,83) |
| D | Klick ins Leere (deselect) | ` 418693 bytes used in 26 items.` | **0 px** |

Der Ganzbild-Diff ist auf allen vier Marken **gleich** dem Diff im
Infozeilen-Rechteck (58,66,500,20): ausserhalb der Zeile aendert sich kein
Pixel — die Auswahl-Invertierung der Symbole war schon vorher richtig, es
fehlte nur die Zeile. Die Groesse 0 beim Ordner ist EmuTOS-treu (`f_size`
eines Verzeichniseintrags ist 0, und `win_sinfo` summiert es ungeprueft). Die
A109-Geometrie ist unveraendert: `[a108] BOX1#11=(64,19,160,240)
ENTRY0=(0,0,160,16)` in beiden Logs, identisch zu
`measurements/measurement-2026-09-22-a107-a109/serial/desk-after-serial.log`.

**A107-Sonde** (`a107-sd-probe.img`, `method/probe-a107.actions`), fuenf
Marken gegen `measurements/measurement-2026-09-22-a107-a109/screendumps/probe-after-*.png`:
B, C, D, E **0 px**; 0 `EXC`/`PANIC`. Marke A zeigt **6779 px**, Bbox
(120,80)-(617,319) — das ist kein Verhaltensunterschied, sondern ein
Startrennen der Messung: bei +150 Ticks hatte der Elternprozess sein Kind hier
noch **nicht** gestartet, im Referenzlauf schon (dort waren A und B bitgleich,
`ref A vs ref B = 0 px`). Mein A zeigt genau das, was die Aktionsdatei fuer A
beschreibt („das Fenster des Eltern allein"), und mein A ist bildgleich mit
dem Zustand nach dem Sweep (C/D).

**TeraDesk-Kontrolle** (`a109-sd-tera.img`, `method/tera-a109.actions`), drei
Marken gegen `.../tera-after-*.png`: **0 px** auf allen dreien, 0
`EXC`/`PANIC`.

## Offen / Rest

* **Pi-Sichtabnahme** (Tester/Lead; nichts geflasht), HW-Checkliste **Punkt
  34** (`docs/api/desktop/_hw-test-2026-09-17.md`). Kein Commit.
* Die **Abweisung** in `tak_flag` ist weiterhin nicht nachgebaut (kein
  Scheduler zum Parken); im Doc von `update_beg` und in
  `docs/api/aes/window-graphics.md` § `wind_update` benannt.
* `wind_set(WF_INFO)` zeichnet bei uns den **ganzen** Fensterrahmen neu
  (`redraw_open`), EmuTOS nur die `W_INFO`-Zelle (`w_cpwalk(w_handle, W_INFO,
  …)`, `gemwmlib.c:1666-1672,1747`). Vorbestehend, durch A110.2 nur oefter
  ausgeloest; nicht angefasst.

## Artefakte

`measurements/measurement-2026-09-23-a104-a110/` (`RUNINFO.TXT`, `manifest.sha256`,
`method/`, `build/`, `host-tests/`, `screendumps/`, `serial/`; keine `*.img`).
Geaenderte Dateien im Baum: `tos-api/src/aes/window.rs`,
`kernel/src/desk/{main,dir}.rs`, `tos-api/tests/aes_a96_zombie_windows.rs`,
`docs/api/aes/window-graphics.md`, `docs/api/desktop/{deskwin,deskmain}.md`,
`docs/api/desktop/_hw-test-2026-09-17.md`, `docs/STATUS.md`, `TASK/A110.md`,
dieses Ticket.
