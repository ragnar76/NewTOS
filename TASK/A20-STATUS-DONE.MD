# A20 — STATUS: DONE (Hostbeleg + QEMU-Rauchtest) / Tester-Abnahme offen

Die Menüleistenhöhe kommt jetzt aus **einer** Ableitung, `gl_hbox = gl_hchar + 3`,
mit `gl_hchar` aus der Zellenhöhe des VDI-Systemfonts — nicht mehr aus der
Konstante 19, die **viermal** im Baum lag (das Ticket nannte eine, A01–A04
drei; die vierte liegt im Kernel-Desktop und ist außerhalb des Auftrags, s.u.).
Bearbeitet 2026-09-19, Branch `desk-rebuild`, Basis HEAD `ed576f0`, Baum vorher
sauber. Nicht committet.

## Soll

| | Datei:Zeile |
|---|---|
| EmuTOS, Fontmetrik lesen | `references/emutos/aes/gemgraf.c:405` (`gsx_textsize(&gl_wptschar, &gl_hptschar, &gl_wchar, &gl_hchar)`) → `gemgsxif.c:505-512` (`vqt_attributes`, `gl_hchar = ptsout[3]`) → `vdi/vdi_text.c:1018` (`top + bottom + 1`) |
| EmuTOS, Ableitung | `gemgraf.c:412` (`gl_hbox = gl_hchar + 3`) — das Ticket nannte die Zeile richtig |
| EmuTOS, Verbraucher | `gemgraf.c:424` (`gl_rfull`), `:427` (`gl_rmenu`); `gemsuper.c:288-291` (`graf_handle`, `int_out[1..4]`); `gemctrl.c:438` (`gl_ctwait.m_gr = gl_rmenu`, die Hitbox der Leiste) |
| EmuTOS, welcher Font | `vdi/vdi_text.c:612` (`def_font`: 16 Zeilen ab 400 Bildzeilen, sonst 8) |
| Spec | `docs/api/aes/window-graphics.md` § `graf_handle` (77) nennt die Ableitung `gl_hbox = gl_hchar+3` bereits; `docs/api/aes/menu.md` § Notes („Leiste = `gl_hchar`/`gl_hbox` aus `graf_handle`"). **Keine Spec behauptet die Konstante — Spec unverändert.** |

Gepinnte Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.

## Bestand vorher

| Stelle | Wert | Rolle |
|---|---|---|
| `tos-api/src/aes/window.rs:153` `GL_HBOX` | `const 19` | Gadget-Zellenhöhe, `adj_hbox`, Drag-Grenzen, `gl_rfull` (über `menu::desk_top`) |
| `tos-api/src/aes/window.rs:141` `GL_HCHAR` / `:136` `GL_WCHAR` | `const 16` / `8` | Sizer-Minimum |
| `tos-api/src/aes/menu.rs:81` `BAR_H` | `const 19` | Leiste zeichnen/löschen, Trennlinie, `track` (`in_bar`), Submenü-Clamp, `desk_top()`, Selftest-Baum, **und** `SETTINGS.height` (Fehlgebrauch, s. Nebenbefunde) |
| `tos-api/src/aes/event.rs:64` `MENU_BAR_H` | `const 19` | `menu_intercept`-Grenze |
| `tos-api/src/aes/graf.rs:506-507` | Literale `8`, `16` | `graf_handle` `int_out[1..2]` |
| `tos-api/src/aes/graf.rs:509-513` | Kommentar „only one 19 in the tree" | **falsch** |
| `tos-api/src/aes/form.rs:224,935,1016`, `fsel.rs:36,790` | Leser von `window::GL_HBOX` | Dialogzentrierung, `gl_rcenter`, Elevator-Minimum |
| `kernel/src/desk/global.rs:36` `MENU_H` | `const 19` | Kernel-Desktop (EmuDesk-Äquivalent) — **außerhalb des Auftrags, unverändert** |

Woher die AES ihre Fontmetrik heute bezieht: `vdi::font::CELL_W/CELL_H`
(`const 8/16`, `object.rs`, `resource.rs`, `init.rs`, `menu.rs::clamp_height`);
das VDI öffnet jede Workstation auf `vdi::font::DEFAULT = F8x16` (Konstante,
`vwk.rs:150`). Das Backend (`RasterBackend`) kennt keinen Font.

## Fix

Eine Quelle in `tos-api/src/aes/graf.rs` (das `gemgraf.c`-Äquivalent, wo
`graf_handle` wohnt):

- `pub(super) fn gl_wchar()` / `gl_hchar()` — die `vqt_attributes`-Antwort des
  VDI-Standardfonts (`max_cell_width`; `top + bottom + 1`), gelesen aus dem
  Fontheader `vdi::font::font(vdi::font::DEFAULT)`.
- `pub const fn hbox_of(hchar) = hchar + 3` — die Regel allein (re-exportiert
  als `tos_api::aes::hbox_of`, damit der Test sie mit einer zweiten Zellenhöhe
  füttern kann).
- `pub(super) fn gl_hbox() = hbox_of(gl_hchar())`.

Zurückgeführt: `window.rs` (`GL_HBOX`/`GL_HCHAR`/`GL_WCHAR` entfernt, 22+1+1 Leser
→ `gl_hbox()`/`gl_hchar()`/`gl_wchar()`, `adj_hbox` von `const fn` zu `fn`),
`menu.rs` (`BAR_H` entfernt, `desk_top()` → `gl_hbox()`, 9 Leser), `event.rs`
(`MENU_BAR_H` entfernt, `menu_intercept` → `graf::gl_hbox()`), `graf.rs`
(`graf_handle` meldet `gl_wchar()/gl_hchar()/gl_hbox()`; falscher Kommentar
ersetzt), `form.rs`/`fsel.rs` (mechanisch: `window::GL_HBOX` →
`graf::gl_hbox()`), `mod.rs` (`pub use graf::hbox_of`). `GL_WBOX` bleibt
Konstante (Ticket: „Nur `BAR_H`"; s. Nebenbefunde).

Kein Umbau der Menüzeichnung; jede Zeichen-/Hitbox-Stelle liest nur den Wert
aus der neuen Quelle.

## Test: `tos-api/tests/aes_menu_bar_height.rs` (4 Fälle)

| Fall | Was |
|---|---|
| `gl_hbox_is_gl_hchar_plus_3_for_both_ring_fonts` | Regel mit den zwei echten Ringfonts: `FON_8X16` (Zelle 16) → **19**, `FON_8X8` (Zelle 8) → **11**; ungleich |
| `graf_handle_reports_gl_hbox_derived_from_its_own_gl_hchar` | `int_out[4] == hbox_of(int_out[2])`, `int_out[2] == 16`, `int_out[4] == 19` (Abnahme 3) |
| `the_desktop_work_area_starts_at_graf_handles_gl_hbox` | `wind_get(0, WF_WXYWH) == (0, hbox, W, H−hbox)` mit `hbox` aus `graf_handle`, bei 640×480 und 1920×1080 |
| `the_menu_bar_hitbox_ends_at_graf_handles_gl_hbox` | Menü installiert (Baum aus `graf_handle`s `hbox` gebaut); Zeiger auf Zeile `hbox` + Druck → `MU_BUTTON`, kein Menü; Zeiger auf Zeile `hbox−1` + Geste auf *Open* → `MN_SELECTED (File, Open)`, kein `MU_BUTTON` |

Rig: metadaten-Backend mit schaltbarer Breite/Höhe, das Zeichnen ignoriert,
plus Skript-Host (Zeiger bewegt sich nur in `pump_input`) — die Harness aus
`aes_menu_click_ownership.rs`, reduziert.

**Rot vor dem Fix:** gegen den unveränderten Baum (MD5 `window.rs`
`14782d38…`, `menu.rs` `db16d5f9…`, `event.rs` `3edb7aa9…`, `graf.rs`
`eb76cee4…`) kompiliert die Datei nicht — `error[E0425]: cannot find function
hbox_of in module tos_api::aes` (5×): die Regel ist gegen die Konstante nicht
ausdrückbar. Ein Laufzeit-Rot ist mit dem festen Font nicht erzeugbar (s.
„Was fehlt"). **Grün nach dem Fix:** 4/4.

**Mutations-Gegenprobe der Hitbox** (Scratch, beide Tore eine Zeile zu hoch —
`event.rs` `my > gl_hbox()`, `menu.rs` `in_bar = my < gl_hbox() + 1` —, danach
von Hand aus Kopien zurückgesetzt, MD5 vorher = nachher `2ce652b0…` /
`44c01e05…`): `the_menu_bar_hitbox_ends_at_graf_handles_gl_hbox` fällt
(„still waiting after 20000 pumps … pointer (84, 19)") — eine veraltete
Leistenhöhe würde erkannt. Nur ein Tor zu mutieren reicht nicht (das andere
schließt das Menü sofort wieder): die Hitbox ist das Und beider Leser.

## Was fehlt (aus dem Auftrag: „im Bericht sagen, was fehlt")

Der Font ist fest im VDI: `vdi::font::DEFAULT` ist eine Konstante (`F8x16`),
`Vwk::EMPTY` (const) setzt `cur_font` daraus; EmuTOS wählt `def_font` nach
`V_REZ_VT` (`vdi_text.c:612`, 8×8 unter 400 Zeilen). Das Backend meldet keine
Zellenhöhe. Damit `gl_hchar` zur Laufzeit mitgeht, bräuchte es (1) die
`def_font`-Wahl nach Bildhöhe im VDI (`font::DEFAULT` → Funktion, `init_wk`
statt `Vwk::EMPTY`) und (2) die übrigen AES-Leser von `CELL_H` (`object.rs`
`AesFont::cell_h`, `resource.rs` `fix_chpos`, `init.rs`, `menu.rs::clamp_height`)
auf `graf::gl_hchar()`. Beides VDI-/Fontarbeit außerhalb A20 — nicht
angefasst. Der Test prüft deshalb die Regel mit zwei echten Fontheadern und
pinnt die drei Leser bei der einen lebenden Größe aneinander.

## Beleglage

| | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **587 grün / 0 rot / 1 ignoriert** (Baseline 583 + 4 neue) |
| Nachbarsuiten | `aes_wind_get_desktop_geometry` 8/8, `aes_reference` 204/204 (Fälle 155–158 dabei), `aes_menu_click_ownership` 3/3, `aes_dialog_undraw` 2/2, `aes_desk_background` 12/12, `aes_window_drag` 4/4 |
| Bau rpi4 / qemu-virt (frisch, `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` vor jedem Board; „Compiling kernel"/„Compiling tos-api" je Board bestätigt) | beide `rc=0`, tos-api **64**, kernel **94** rustc-Warnungen — Baseline, Delta 0 |
| Clippy kernel (`board-rpi4` und `board-qemu-virt`, `--lib --bins`) / tos-api | kernel **125**, tos-api **81** — Baseline, Delta 0; neue Testdatei **0** Clippy-Warnungen |
| QEMU raspi4b, release `--board rpi4`, TeraDesk per `#Z` (NEWDESK.INF/TERADESK.INF aus `measurement-2026-09-18-h05/method/`, SHA `457a70e7…`/`5be1b5c7…`; Basisimage `tools/make-sd-image.sh`) | Serial: `[shell] sh_ldapp: starting C:\APPS\TERADESK\DESKTOP.NAP, sh_nextapp=2, sh_isgem=1`, kein panic/EXCEPTION; Screendumps bei +6 s und +12 s: **0 von 307200** Pixeln verschieden gegen `measurement-2026-09-19-a39/screendumps/smoke-tera-up.png`, PNG sogar byte-identisch (SHA-256 `fb1e8b90…`); Zeilen 0–17 weiß (Leiste), Zeile 18 durchgehend schwarz (Trennlinie = `gl_hbox−1`) → Leiste weiterhin 19 hoch |
| `git status --short` | `M tos-api/src/aes/{event,form,fsel,graf,menu,mod,window}.rs`, `?? tos-api/tests/aes_menu_bar_height.rs`, `?? TASK/A20-STATUS-DONE.MD` |

Kernel für den Rauchtest: SHA-256 `b4bbff8fac8bd6394393ee425acb393e5d10d4e3a0cec2c8999e779e2dfde369`
(Scratch `…/scratchpad/kernel8-a20-smoke.img`; Screendumps/Serial unter
`…/scratchpad/a20smoke/out/`, Baulogs unter `…/scratchpad/build-*.log`).

## Nebenbefunde (unverändert, je ein eigener Task)

- `kernel/src/desk/global.rs:36` `MENU_H = 19` — vierte Kopie; EmuDesk holt
  `gl_hbox` per `graf_handle` (`desk/deskmain.c:1956`).
- `tos-api/src/aes/window.rs` `GL_WBOX = 19` — `gl_wbox = max(gl_wchar+4,
  gl_hbox·ws_hpixel/ws_wpixel)` (`gemgraf.c:413-415`) ist selbst aus `gl_hbox`
  abgeleitet; bei einem 8×8-Font wäre er 12. Ticket sagte „Nur `BAR_H`".
- `tos-api/src/aes/menu.rs` `SETTINGS.height` war `BAR_H` (Pixel) für einen
  Item-Zähler; Wert 19 unverändert als `SETTINGS_HEIGHT_DEFAULT` übernommen.
  EmuTOS: `INIT_HEIGHT_VALUE 16` (`gemmnext.c:74`), geklemmt in `mnext_init`
  (`:109`); auch `display 0/drag 0/speed 10` weichen von `200/10000/0` ab.
- AES-Leser von `vdi::font::CELL_H` direkt (`object.rs`, `resource.rs`,
  `init.rs`, `menu.rs::clamp_height`) statt über `graf::gl_hchar()` — s. „Was
  fehlt".
- Test-Orakel mit eigener 19 (`tests/aes_dialog_undraw.rs:677`,
  `tests/aes_desk_background.rs:467`, `tests/aes_menu_click_ownership.rs:444`)
  — Erwartungswerte, bewusst nicht angefasst.

## Offen

- Tester-Abnahme (Boot + `svc`-Selftests + Screendumps + `fsck_msdos`, beide
  Boards) — der Rauchtest hier ist ein Entwickler-Smoke, kein Regressionslauf.
- Keine Hardwareabnahme nötig: für 8×16 ändert sich kein Pixel (Abnahme 3);
  die Messung vom 2026-09-09 (19 px auf TT und Pi) bleibt erklärt.
- Nicht committet; `docs/STATUS.md` nicht angefasst; Ticket-Kopfzeile
  unverändert.
