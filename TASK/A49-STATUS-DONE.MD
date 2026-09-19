# A49 — Quittung

**Status: DONE (Hostbeleg; Sichtpunkt fuer die echte Pi 4 offen)** ·
2026-09-20 · uncommitted im Baum (Basis `68c84ae`). `TASK/A49.md`s Kopfzeile
bleibt unveraendert.

## Was EmuTOS tut (gepinnte Quelle `1a7484ab`)

| Stelle | Befund |
|---|---|
| `include/aesdefs.h:531` | `#define HOTCLOSE 0x1000 /* PC-GEM feature */` |
| `include/config.h:1319-1321` | `CONF_WITH_PCGEM 1` (Default, also 512er-Profil); nur `TARGET_192` setzt es auf 0 (`:352-354`, Block `:210-394`) |
| `aes/gemctrl.c:208-214` | `hctl_window`, `case W_CLOSER:` → `#if CONF_WITH_PCGEM if (kind & HOTCLOSE) { message = WM_CLOSED; break; } FALLTHROUGH;` — **kein `gr_watchbox`**, der Druck ist der Klick |
| `aes/gemctrl.c:216-220` | ohne `HOTCLOSE`: `if (gr_watchbox(gl_awind, gadget, selected, normal)) message = WM_CLOSED/WM_FULLED` — Druck **und** Loslassen im Gadget |
| `aes/gemctrl.c:86-96` | `ct_msgup`: nach `ap_sendmsg` → `if ((message == WM_CLOSED) && (D.w_win[wh].w_kind & HOTCLOSE)) { button = 0; return; }` — „we fake button up, otherwise a slow click will cause us to stay in ctlmgr(), eating keys". Sonst `while (button & 0x0001) dsptch();` (`:100-101`) |
| `aes/gemevlib.c:185-200`, `aes/geminput.c:549` | `ev_multi`-Schnelltest und `abutton` lesen **`button`** (den vom AES verarbeiteten Stand), nicht das Rohbyte — die Faelschung wirkt auf beide; `set_mown` (`geminput.c:222`, `if (!button)`) gibt die Maus sofort zurueck |

Wirkung von `button = 0` (beobachtbar): (1) der Control Manager wird vom noch
gehaltenen Knopf nicht erneut ausgeloest; (2) die App bekommt den gehaltenen
Druck nicht als `MU_BUTTON`; (3) das physische Loslassen ist fuer `bchange`
kein Uebergang mehr; (4) der naechste Druck startet normal.

## Was NewTOS vorher tat

`w.kind` speicherte das volle Wort (`wind_create`, `window.rs:2366/2384`), das
Bit kam also an — aber `intercept()`s `Gadget::Closer`-Arm rief immer
`watchbox` (unser `gr_watchbox`): Druck und Loslassen im Closer noetig; ein
gehaltener Druck haengt im Wartelauf.

## Was gebaut wurde

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Kind-Bit | `tos-api/src/aes/window.rs`: `const HOTCLOSE: u16 = 0x1000;` (dokumentiert: zeichnet nichts, braucht `CLOSER`) | fertig |
| Closer-Arm | `window.rs` `intercept()`, `Gadget::Closer`: bei `w.kind & HOTCLOSE` → `msg[0] = WM_CLOSED; post_to_owner(...); input::ct_msgup_hotclose(); return;` — sonst unveraendert `watchbox` | fertig |
| `button = 0` | `tos-api/src/aes/input.rs`: `pub(super) fn ct_msgup_hotclose()` (`BUTTON.store(0)`, `gemctrl.c:92`) und `pub(super) fn button() -> u8` (EmuTOS `button`). **Ausserhalb der genannten Dateien `window.rs`/`event.rs`** — `input.rs` ist der Ort des `button`-Modells (`BUTTON`, `b_click`, `w_setactive`); ein HOTCLOSE-eigenes Flag in `window.rs` haette das Modell verdoppelt | fertig |
| Control-Manager-Eintritt | `window.rs` `intercept()`: Eingangstest `input::button() & 1` statt `vdi::mouse_buttons()` — das ist der Stand, den `ctlmgr`s `ev_multi(MU_BUTTON, 0x0001ff01)` prueft; identisch zum Rohbyte ausser nach der Faelschung | fertig |
| App-Schnelltest | `tos-api/src/aes/event.rs` `evnt_multi`: `arm_mb` (Pass 0) und `mb` (Pass > 0) lesen `input::button()` statt `vdi::mouse_buttons()` — `downorup(button, buparm)`, `gemevlib.c:196`. Ohne das haette die „dokumentierte Ausnahme" (App armt neu, waehrend der Druck noch gehalten wird → `MU_BUTTON`) den konsumierten HOTCLOSE-Druck als Klick an TeraDesk-artige Hauptschleifen durchgereicht; mit EmuTOS' `button = 0` gibt es diesen Klick nicht | fertig |
| Test | `tos-api/tests/aes_window_hotclose.rs` (neu; Harness wie `aes_menu_click_ownership.rs` mit `IDLE_CAP`, `open_window` aus `aes_window_drag.rs`; Zeiger nur ueber `pump_input`, `feed_relative`): (1) `NAME\|CLOSER\|HOTCLOSE`, Zeiger auf dem Closer (`gadget_name_at` = „closer"), Druck, dann 3× gehalten + 1× Loslassen **in der Warteschlange**: `evnt_multi(MU_MESAG\|MU_TIMER 0)` liefert `WM_CLOSED` fuer das Handle und pumpt genau **einmal** (das abschliessende `dsptch()` jedes AES-Aufrufs, `mod.rs:429-434`) — 3 von 4 Paketen bleiben liegen; (2) Druck gehalten: zwei TeraDesk-Polls (`MU_MESAG\|MU_BUTTON 0x102/3/0\|MU_KEYBD\|MU_TIMER 0`) liefern **nur** `MU_TIMER` (kein zweites `WM_CLOSED`, kein `MU_BUTTON`); Loslassen → nur `MU_TIMER`; neuer Druck → wieder `WM_CLOSED`; (3) Regression `NAME\|CLOSER`: dieselben 4 Pakete werden alle verbraucht (≥ 4 Pumps), `WM_CLOSED` kommt erst nach dem Loslassen, danach kein `MU_BUTTON` | 3/3 |
| Spec | `docs/api/aes/window-graphics.md`: Kind-Bit-Tabelle `HOTCLOSE` mit Verhaltensabsatz (`gemctrl.c:208-214`, `:86-96`, `CONF_WITH_PCGEM`, NewTOS-Umsetzung, Hosttest, HW nur Sicht); `WM_CLOSED`-Zeile der Nachrichtentabelle; Testerwartung 8 (`graf_watchbox`) um die HOTCLOSE-Ausnahme ergaenzt | fertig |

## Geprueft (Belege in `measurement-2026-09-20-a49-a61/`)

* **Rot vor / gruen nach** (`host-tests/a49-red-before.txt` mit dem ersten
  Testtext, `a49-red-before-final-text.txt` mit dem finalen Text bei per Hand
  abgeschaltetem HOTCLOSE-Arm, Rueckbau aus der Kopie, `cmp` identisch;
  `a49-green-after.txt`): vorher Probe 1 `(pumps, left) = (5, 0)` statt
  `(1, 3)` (Wartelauf durch alle 4 Pakete), Probe 2 haengt bis `IDLE_CAP`
  („still waiting after 20000 pumps … buttons 0x01"), Probe 3 gruen; nachher
  3/3.
* `cargo run -p xtask -- test`: **653 / 0 / 1** (Baseline 648 + 3 A49 + 2 A61)
  — darunter `aes_menu_click_ownership` (3/3) und `aes_window_drag` (4/4), die
  `intercept`/`arm_mb` direkt treiben.
* Boards rpi4 / qemu-virt (je nach `cargo clean -p kernel -p tos-api`):
  rustc 64/94, Clippy 81/125 — Delta 0; Warnungsarten in
  `window.rs`/`input.rs`/`event.rs` identisch zur Vorquittung; Testdatei
  clippy-sauber (`build/clippy-tests.txt`).
* QEMU nicht messbar (kein Eingabeweg, `TASK/A49.md`); nicht gefahren.

## Bewusst nicht angeglichen (Randfaelle der Faelschung, notiert)

- `menu_intercept`s Eingangstest und `evnt_button`/`graf_mkstate` lesen
  weiter das Rohbyte. EmuTOS' `button = 0` wuerde dort waehrend des noch
  gehaltenen HOTCLOSE-Drucks „oben" melden (Menue-Hover trotz gehaltenem
  Knopf, `evnt_button(Druck)` wartet, `graf_mkstate` = 0). Keine App trifft
  das; nicht in A49s Umfang.

## Offen

- **Sichtpunkt fuer die HW-Liste (echte Pi 4):** ein Fenster mit
  `CLOSER|HOTCLOSE` schliesst beim **Druck** auf den Closer, ein gehaltener
  Knopf loest weder ein zweites Schliessen noch einen Klick unter dem Zeiger
  aus; ein Fenster mit nur `CLOSER` schliesst erst beim Loslassen im Closer.
  Kein Selftest zu verdrahten.
- Nicht committet; `docs/STATUS.md` nicht angefasst.
