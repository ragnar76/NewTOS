# A56 — Quittung

**Zustand: DONE** · 2026-09-20 · kein Commit (Fix im Arbeitsbaum, Abnahme auf echter Pi 4 durch Tester/Lead offen)

**Diese Datei ersetzt `A56-STATUS-UNFINISHED.MD`** (Untersuchung vom
2026-09-20, Auditor; deren Inhalt ist in
[docs/analysis/aes-process-table-and-apid-2026-09-20.md](../docs/analysis/aes-process-table-and-apid-2026-09-20.md)
§ 2, 3, 5, 6 und in [A62-STATUS-UNFINISHED.MD](A62-STATUS-UNFINISHED.MD)
aufgehoben). Stufe 1 (Identität = Slot) ist `25ff70c`; dies ist Stufe 2.

Spec: [docs/api/aes/application.md](../docs/api/aes/application.md)
§ "AES process identity and pid", Vertrag **(b)** (`86b871d`).
Artefakte: [measurement-2026-09-20-a56/](../measurement-2026-09-20-a56/RUNINFO.TXT).
Basis `desk-rebuild` @ `25ff70c`, Baum vorher sauber.

## 1. Was gebaut wurde — Stelle → Wert vorher / nachher

| Stelle | vorher (Stufe 1) | nachher | Quelle |
|---|---|---|---|
| `appl.rs` `reported_pid(idx)` → `ap_id` (`appl_init`) | Slot + 1; Desktop 1 | **0** für jede Identität (16 Slots + `DESKTOP`) | `gemsuper.c:94`, `geminit.c:850-877` |
| `appl_find(Kettenname)` | Slot + 1 | **0**; nicht gefunden −1; **`"SCRENMGR"` → 1** (neu, `iprocess("SCRENMGR", ctlmgr)` → `pstart` → `p_nameit`) | `gempd.c:38-57,117`, `geminit.c:888` |
| `identity_of(pid)` (`appl_write`/`appl_read`-Ziel, Testhaken) | 1 → Desktop bzw. Slot 0 (vom Aufrufer aus), 2+ → Slot pid−1 | **0 → der aktuelle Klient** (`current_identity()`); alles andere `None` | `fpdnm(NULL, 0)` = `D.g_int[0]`, `gemsuper.c:102-105` |
| `appl_write(1, …)` | Ziel Slot 0 / Desktop | **1** (angenommen, verworfen — EmuTOS: Queue des Screen Managers, `ctlmgr` liest sie, keine Anwendung sieht sie) | `gemctrl.c:448-481` |
| `appl_write(2+, …)`, `appl_read(1/2+)` | 2..16 → Slots | **0** (kein Deskriptor innerhalb `totpds` = 2; EmuTOS: NULL-Deref in `doq`) | `geminit.c:819`, `gemqueue.c:28-74` |
| `event.rs` `menu_intercept` `MN_SELECTED msg[1]` | `current_pid()` (= Leser) | **1** (`SCRENMGR_PID`) | `gemctrl.c:353-367`, `gemwmlib.c:855` |
| `window.rs` `intercept` (`WM_TOPPED/CLOSED (beide)/FULLED/MOVED/SIZED/HSLID/VSLID/ARROWED`) `msg[1]` | `current_pid()` | **1** | `gemctrl.c:125,178,210-220,238,259,300-314` |
| `window.rs` `post_untopped` `WM_UNTOPPED msg[1]` | `current_pid()` | **1** | `gemctrl.c:146-155` |
| `window.rs` `send_redraw` `WM_REDRAW msg[1]` | `current_pid()` (= 1/2/…) | `current_pid()` = **0** (der Aufrufer — jeder Pfad ist ein `wind_*`/`form_dial`-Trap der Anwendung; `intercept` postet keine Redraws) | `gemwmlib.c:904,1426,1453,1612` |
| `wind_get(WF_OWNER)` `int_out[1]` | 1, 2, … | **0** für jedes Anwendungsfenster; Desktop-Fenster ohne Besitzer weiter 0 | `gemwmlib.c:1580,1377,212` |
| `mod.rs` `pid_of_slot(slot)` | Slot + 1 | `reported_pid(slot)` = 0 | — |
| `kernel/src/selftest_aes.rs:110` (unverdrahtet, baut) | `check(ap_id >= 1)` | `check(ap_id == 0)` | — |
| `kernel/src/shell_loop.rs` Doc (Lückenliste 10) | „NewTOS still slot + 1 … that is A56" | geschlossen | — |

`WM_ARROWED`: NewTOS hat keinen nicht-gequeuten Pfad (`p_msg`-Fastpath,
`gemctrl.c:129-133`/`gemevlib.c:100-117`); jeder Pfeil ist gequeut und trägt
1 — der Fastpath-Fall (0) existiert hier nicht (im Kommentar an `intercept`
benannt).

Neue Konstanten `appl::AP_CHAIN_PID = 0`, `appl::SCRENMGR_PID = 1` (Stempel,
kein Prozess — H02 bleibt). `reported_pid` bleibt die eine
Übersetzungsstelle (`debug_assert` auf die Identitätsdomäne; Accessories
2+ sind die ersten Identitäten, die sie nicht auf 0 schickt, sobald es sie
gibt).

## 2. Tests — rot vor, grün nach

`tos-api/tests/aes_pd_by_slot.rs`: (a)–(c) pinnen jetzt 0 statt Slot + 1;
**(d) neu** `reported_values_follow_emutos`: `ap_id` 0 für Desktop, Slot 0,
Slot 1; `appl_find` 0 / −1 / `"SCRENMGR"` → 1; `WF_OWNER` 0 (auch nach
`WF_NEWDESK`); `WM_REDRAW` aus `wind_open` `msg[1]` = 0; `appl_write(0)` →
der Aufrufer liest (Eltern und Kind je für sich); `appl_write(1)` → 1 und
niemand liest; `appl_write(2)`/`(17)` → 0; **`WM_UNTOPPED`/`WM_TOPPED`
(Frame-Klick auf ein nicht-oberstes Fenster) und `HOTCLOSE`-`WM_CLOSED`
`msg[1]` = 1** — dafür hat der Testhost einen skriptbaren Zeiger
(`feed_relative` + Release-Pakete für `wait_release`). Gegenprobe: mit
`intercept` auf `current_pid()` zurückgesetzt schlägt genau die
`WM_TOPPED`-Zeile fehl (`left: (1, 21, 0, 2)`, `right: (1, 21, 1, 2)`).

Rot vor dem Fix (`measurement-2026-09-20-a56/host-tests/red-before-aes_pd_by_slot.txt`):

    every_nested_process_gets_a_descriptor_and_appl_init_never_returns_minus_one ... FAILED  (left: 1, right: 0)
    reported_values_follow_emutos ... FAILED                                                   (desktop ap_id: left 1, right 0)
    releasing_a_slot_resets_its_descriptor_and_queue ... FAILED
    nine_programs_in_one_slot_rename_one_descriptor ... FAILED

Grün danach: 4/4. `aes_window_drag.rs` (`WM_MOVED msg[1] == 1`) und
`aes_menu_click_ownership.rs` (`MN_SELECTED msg[1] == 1`) je eine neue
Assertion mit Quelle. Tests, die alte Werte pinnten, nachgezogen:
`aes_app_return_descriptor.rs`/`aes_launch_boundary_reset.rs` (`desk_pid > 0`
→ `== 0`, Kopfkommentar), `aes_rsrc_per_process.rs` + `resource.rs`-Unittest
(`assert_ne!(pid, pid)` → `assert_ne!(slot, slot)` + beide 0).

    cargo run -p xtask -- test   670 / 0 / 1   (Baseline 670 / 0 / 1; keine Datei hinzu)

## 3. Referenzsuite (`tos-api/tests/aes_reference/harness.rs`)

- `GLOBAL_EXCLUDED` **[2, 11, 12] → [11, 12]**: `global[2]` (`ap_id`) wird
  jetzt gegen den Record verglichen (TT: 0). Die alte Begründung („Eigenschaft
  des Prozessmodells, NewTOS 1") lizenzierte die Abweichung; der Wert ist
  Vertrag (b). Der Eigenschafts-Check `ap_id == appl_init ret` bleibt.
- `Expected::ApId` **entfernt**; r2 Fälle **244/247 `int_out[1]`** →
  `Expected::Value(0)` (`w_owner->p_pid`, `gemwmlib.c:1580`; `w_setup(rlr, …)`
  `:1377` → `w_owner` `:212`; Kette = pid 0). Kein Fall wird weggefiltert;
  Fall 167 (r1) unverändert. 204/204 grün.
- r3/r4 werden nicht auf dem Host gefahren; **r4-Fall 644** (`ap_id`, TT 0,
  NewTOS 1) kippt auf der Hardware — Tester.

## 4. Bauten, Warnungen, Clippy

    xtask build --board qemu-virt   exit 0, rustc tos-api 64 / kernel 94
    xtask build --board rpi4        exit 0, rustc tos-api 64 / kernel 94
    clippy --features board-qemu-virt   tos-api 81 / kernel 125
    clippy --features board-rpi4        tos-api 81 / kernel 125
    Baseline 64/94, 81/125 — Delta 0.

## 5. QEMU raspi4b (Produktkernel `--release --board rpi4`, `8eee1eb3…`, keine Instrumentierung; Injektor-Kernel nur aus einer Scratch-Kopie)

Alle Läufe `grep -c "EXC\|panic"` = 0. Details `measurement-2026-09-20-a56/RUNINFO.TXT`.

- **NESTTST**: Tiefe 1–16 `appl_init` = **0**, `WF_OWNER` = **0**,
  `appl_write(self)` = 1, `appl_find(NESTTST)` = **0**; 17. `Pexec` → −39; alle
  kehren zurück; `desktop up`. (Stufe 1: 1…16 / 1…15 / 1 / 1.)
- **Smoke** desk/tera/qed: je **0 Pixel** zu Stufe 1 (= A41).
- **H05 (full1)**: sechs Dumps **0 Pixel** zu Stufe 1; D vs K 134 Pixel = der
  Mauszeiger; Serial: QED (idx 1) `appl_init` → 0; `WM_REDRAW msg[1]=0`,
  `WM_MOVED msg[1]=1`, `MN_SELECTED msg[1]=1`. TeraDesk vergleicht `msg[1]`
  nicht — läuft unverändert.
- **quit2** (TeraDesk beenden → Desktop): A/B/D **0 Pixel**; G nur die
  Uhrzeitspalte (Karte heute erzeugt); `exited rc=0` → `desktop up`.
- **H02** (File → Info): `post_msg idx=0 [10, 1, 0, 4, 20, …]` = `MN_SELECTED`
  mit `msg[1]` = **1**, Dialog erscheint.
- **A78** tera-move / desk-move: alle Dumps **0 Pixel** zu A78; `WM_MOVED`
  `msg[1]` = 1 (auch an den Kernel-Desktop, idx 16), der verweigerte Blit
  bringt `WM_REDRAW msg[1]` = 0.

## 6. STOP / offen

- **`appl_write(1, WM_REDRAW, …)`**: EmuTOS (512-Profil, `CONF_WITH_3D_OBJECTS`)
  lässt `ctlmgr` die Nachricht lesen und `w_redraw_desktop(rect)` ausführen
  (`gemctrl.c:380-384,448-481`, `gemwmlib.c:1902-1941`). Kein Spec-Eintrag;
  nicht gebaut (Nachricht wird angenommen und verworfen, Rückgabe 1 wie
  EmuTOS). Keine unserer Apps sendet an 1 (Sweep `sdk/`). → Researcher.
- Abnahme auf echter Pi 4 — Tester/Lead.
- `docs/STATUS.md` nicht angefasst (Lead-Scope; beim Commit nachziehen, wie Stufe 1).

## 7. Nebenbeobachtungen (nicht angefasst)

- `kernel/src/selftest_sleepwake_switch.rs` (unverdrahtet): B schreibt per
  `appl_write(A_PID = 1)` in As Schlange — unter dem EmuTOS-Modell
  strukturell unmöglich (alle Kettenmitglieder 0, Ziel 0 = der Aufrufer,
  1 = Screen Manager). Der Test braucht einen Slot-Testhaken statt einer
  pid; Kommentar `:71` veraltet. Baut weiter.
- `tos-api/src/aes/trace.rs`/`pd.rs`: der Trace-Datensatz `msg_post(pid)`
  trägt jetzt für jedes Ziel 0 (der berichtete Wert); die Identität steht nur
  in den CALL-Records (`slot`).
- `kernel/src/desk/mod.rs:180-186,646`: Doc-Kommentare nennen noch „pid 1"
  und `pd::by_pid(1)` (schon in Stufe 1 gemeldet).
- `measurement-2026-09-17-h02/method/run2.actions` parkt den Zeiger auf
  (0,0) = Menüleiste; mit dem heutigen Hover-Menü frisst das den Doppelklick.
  Für diese Messung mit `1206 move 300 300` ergänzt (`method/h02-info.actions`).

## 8. Aufräumen

Kein `git checkout/restore/clean/stash`, kein Commit, keine Hardware-/Outward-Aktion.
QEMU-Prozesse beendet, QMP-Sockets entfernt, hdiutil-Images ausgehängt.
Scratch nur unter dem Session-Scratchpad; Kopien der Methoden und Logs unter
`measurement-2026-09-20-a56/`.
