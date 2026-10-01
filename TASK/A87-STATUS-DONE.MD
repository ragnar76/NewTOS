# A87 — STATUS: DONE (Hostbeleg rot/gruen + QEMU-Selbstbeleg vorher/nachher) / Pi-Sichtabnahme offen

Die Mausuebergabe an den Kontrollmanager (und die Inline-Interception, die
den Leisten-Track startet) nutzt jetzt `gl_ctwait` = das `THEACTIVE`-Rechteck
der registrierten Leiste (EmuTOS `mn_bar`, `aes/gemmnlib.c:717`), nicht mehr
die ganze Leistenzeile. Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis
HEAD `40be5fe`, **nicht committet**.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemmnlib.c:712-717` (`mn_bar(tree, TRUE)`: `gl_mntree = tree`, `ob_actxywh(tree, THEACTIVE, &gl_ctwait.m_gr)`), `:726-727` (`FALSE`: `gl_mntree = NULL`, `rc_copy(&gl_rmenu, &gl_ctwait.m_gr)`); `aes/gemctrl.c:437-438` (Initialwert `gl_rmenu`), `:451-452` (`if (gl_mntree) ev_which \|= MU_M1`), `:456-460` (`ev_multi(…, &gl_ctwait, …)`); `aes/geminput.c:76-79` (`in_mrect`), `:388-389` (`mchange`: Uebergabe nur `button == 0 && gl_mntree && in_mrect(&gl_ctwait)`); `aes/gemgraf.c:427` (`gl_rmenu = (0, 0, gl_width, gl_hbox)`). Druck in der Leistenzeile: `aes/geminput.c:96-97, 316-322, 334-335` (`chk_ctrl` `gl_rmenu` → `-1` → `ctl_pd`), `aes/gemctrl.c:467-470` (`wm_find` = 0 → nichts), `aes/geminput.c:225-231` (`set_mown` postet `button == 0`). `THEACTIVE`-Breite = Titel: `desk/deskmain.c:1528-1534` (`adjust_menu`: `mbar->ob_width = x`). |
| NewTOS (vorher) | `tos-api/src/aes/input.rs:178` (`mchange`: `ry < menu::desk_top()`), `tos-api/src/aes/event.rs:117` (`menu_intercept`: `my >= gl_hbox()`), `menu.rs menu_bar` speichert nur `MNTREE` |
| NewTOS (nachher) | `tos-api/src/aes/menu.rs` (`CTWAIT_RECT`/`CTWAIT_BAR`, `in_ctwait`, `install_bar`, `remove_bar`; `menu_bar` ruft sie), `input.rs mchange` und `event.rs menu_intercept` lesen `menu::in_ctwait(mx, my)` |

## Was gebaut ist

* `menu.rs`: `gl_ctwait.m_gr` als gepacktes `(x, y, w, h)` in einem
  `AtomicU64` + `CTWAIT_BAR` (Leiste registriert?); `in_ctwait` = EmuTOS
  `in_mrect(&gl_ctwait)` (`m_out` ist nie gesetzt); ohne Leiste `gl_rmenu`
  = `my < gl_hbox` (Zeiger ist auf den Schirm geklemmt, also ist die Breite
  des Geraets nicht zu speichern; beide Verbraucher gaten ohnehin zuerst auf
  `gl_mntree`). `install_bar(tree)` = `gl_mntree = tree; ob_actxywh(tree,
  THEACTIVE, …)` (`object::abs_rect`, `THEACTIVE` ist ein `G_IBOX` ohne
  `FL3DOBJ`); `remove_bar()` = `gl_mntree = NULL; gl_ctwait = gl_rmenu`.
* `input.rs mchange`: `in_ctwait(rx, ry)` statt `ry < desk_top()`.
* `event.rs menu_intercept`: `!in_ctwait(mx, my)` statt `my >= gl_hbox()`.
* `graf.rs`: nur der Kommentar zur `gl_hbox`-Verwendung berichtigt (die
  alte Aussage „`menu_intercept` testet gegen `gl_rmenu`“ waere nach dem Fix
  falsch gewesen).
* Spec: `docs/api/aes/menu.md` § `menu_bar` — neuer datierter Absatz
  „`gl_ctwait` with and without a bar“ (beide Werte, beide Verbraucher, die
  Folge fuer Bewegung vs. Druck); `docs/api/aes/_state.md` `gl_ctwait`-Zeile
  („full desktop“ war falsch — `gl_rmenu` ist die Leistenzeile).

## Ticketpraemisse, an der Quelle geprueft und korrigiert

Das Ticket erwartete „Klick in der Leistenzeile rechts von `THEACTIVE` → App
bekommt `MU_BUTTON`“. In EmuTOS geht ein *Druck* in der ganzen Leistenzeile
an `ctl_pd` (`chk_ctrl`: `inside(mx, my, &gl_rmenu)` → `-1`), der
Kontrollmanager findet dort kein Fenster und verwirft ihn; das Loslassen
gibt die Maus mit `button == 0` zurueck — kein Druck-Wait der App wird
erfuellt. Was die App tatsaechlich behaelt: die **Bewegung** (`MU_M1`/`MU_M2`
in der Zeile rechts der Titel), den Timer, die Tasten, und es startet kein
Track. Die Tests sind darauf ausgelegt; ein Test auf zugestelltes
`MU_BUTTON` waere auch gegen EmuTOS rot.

## Hosttests (`measurements/measurement-2026-09-21-a87-a88/host-tests/`)

`tos-api/tests/aes_a87_a88_menu_ctwait_popup.rs` (Harness der A73-Datei,
`THEACTIVE` 168 px breit in der 640-px-Leiste):

| Fall | HEAD `40be5fe` | nachher |
|---|---|---|
| `a87_the_app_keeps_running_while_the_pointer_rests_right_of_theactive` (Timer 100 ms, Zeiger ruht 300 Pakete) | rot: Timer bei 303 ms, Zeiger schon aus der Zeile | gruen: 101 ms, Zeiger in der Zeile, nichts gezeichnet |
| `a87_entering_the_bar_row_right_of_theactive_keeps_the_mouse_with_the_app` (`MU_M1` auf die Zeile rechts der Titel) | rot: nur Timer | gruen: `MU_M1` beim Eintritt |
| `a87_a_click_right_of_theactive_is_the_control_managers_and_blocks_nothing` | rot: 316 ms | gruen: kein `MU_BUTTON`, Timer bei ~100 ms |
| `a87_a_title_inside_theactive_still_starts_the_track` (Guard) | gruen | gruen: `MN_SELECTED (4,12)` |
| `a87_theactive_right_edge_is_the_boundary` (x = 168 App / x = 167 Track) | rot | gruen |
| `a87_with_no_bar_no_move_hands_the_mouse_over` (Guard) | gruen | gruen |

`cargo run -p xtask -- test`: 760 / 0 / 2 → **770 / 0 / 2** (6 A87 + 1 A88
Integrationsfall + 3 Modultests). Erster Lauf mit einem SIGSEGV im
tos-api-Lib-Testbinary (`xtask-test-AFTER-run1-SIGSEGV-pool-alias.log`):
Ursache der A88-Klammer-Modultest (siehe `A88-STATUS-DONE.MD`), behoben,
zweiter Lauf sauber; 32-Thread-Stress 0/300.

## Build / Clippy (`build/`)

`cargo xtask build --board rpi4`: tos-api 64 / kernel 94 (Baseline A99:
64/94, Delta 0), rc 0. `cargo clippy -p tos-api`: 80 (Baseline 80, Delta 0;
`menu.rs` 10 → 10 Treffer, alle die vorbestehenden `Aespb`-Sichtbarkeits-
Hinweise; `input.rs`/`event.rs`/`graf.rs` 0; die neue Testdatei 0).
Kein qemu-virt-Build (Lead).

## QEMU raspi4b (`screendumps/`, `serial/`, `method/`)

Injektor-Scratchkerne (A85/A99-Methode; vorher = `input.rs`+`event.rs`+
`menu.rs` von HEAD via `git show`, nachher = Baum; neuer `Step::Key(u8)`
feuert ein rohes IKBD-Tastenbyte). TeraDesk per `#Z` (`sd-tera.img`),
Skript `method/tera-a87.actions`: parkbr; A; Zeiger nach (400,8) — rechts von
„Options“, TeraDesks `THEACTIVE` = (16,0,280,19) (`desktop.rsc` Baum 0
Objekt 2, x 16..295); B; Klick; C; HELP (0x62/0xE2); D; parkbr; E; Cancel
(427,385); parkbr; F; File hovern (88,8); G; parkbr + Klick; H.

* **D (Diskriminator):** nachher `[aes] op 25 … out=(1,400,8,0,0,25088,0)`
  = `MU_KEYBD` HELP sofort, Hilfedialog sichtbar, Zeiger bei (400,8);
  vorher nichts — der Dialog erscheint erst bei E, nach dem Parken
  (`op 25 … out=(1,527,135,…)` beim ersten Park-Schritt): TeraDesk hing im
  Track. Pixeldiff D vorher/nachher 103 668 px (bbox (161,86)-(479,412) = der
  Dialog); A/B/C/E/F/G/H 0 px.
* Der Klick bei (400,8) liefert in **beiden** Kernen kein `MU_BUTTON`
  (kein `op 25`-Return), C == A bis auf den Zeiger (70 px) — das
  EmuTOS-Verschlucken.
* G: File-Titel oeffnet die Tafel (beide), H == A (0 px): sauber geschlossen.
* Kernel-Desktop (`sd-base.img`, `desk-a87.actions`, Marken A–F): 6/6
  bytegleich vorher/nachher — sein `THEACTIVE` ist schirmbreit, siehe unten.
* 0 `EXC`/`PANIC` in allen vier Serial-Logs.

## Nicht angefasst (Nebenbefunde)

* `kernel/src/desk/menu.rs build_menu` baut `THEACTIVE` mit `ob_width = sw`;
  EmuTOS `adjust_menu` setzt `mbar->ob_width = x` (Summe der Titel,
  `desk/deskmain.c:1534`). Deshalb gehoert beim Kernel-Desktop die Zeile
  rechts der Titel weiter dem Kontrollmanager. desk/, eigener Task.
* `menu::track_owned`s `in_bar = my < gl_hbox()` (`START_STATE`, `THEBAR`
  als `MU_M2`-Rechteck, `gemmnlib.c:286-292`) ist `THEBAR`, nicht
  `gl_ctwait` — korrekt so, unveraendert.
* `window::chk_ctrl`s `my < desk_top()` ist `inside(gl_rmenu)` — korrekt,
  unveraendert.

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht).
