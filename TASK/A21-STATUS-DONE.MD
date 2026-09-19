# A21 — STATUS: DONE (Hostbeleg + QEMU-Stackmessung) / Tester-Abnahme offen

Die Stückliste eines Fensters fasst jetzt **160** Stücke — EmuTOS' gesamten
`ORECT`-Pool, `NUM_ORECT = NUM_WIN * 10` — statt 16, und ein Überlauf ist kein
stilles Verwerfen mehr, sondern ein benannter Abbruch an der Stelle, an der
EmuTOS durch einen NULL-Zeiger schreibt. Bearbeitet 2026-09-19, Branch
`desk-rebuild`, Basis HEAD `6ea4f9f`, Baum vorher sauber. Nicht committet.

## Abnahme 1 — EmuTOS bei erschöpftem Pool (Quelle gepinnt `1a7484ab`)

| | Datei:Zeile |
|---|---|
| Poolgröße | `aes/gemlib.h:88` `#define NUM_ORECT (NUM_WIN * 10)  /* is this enough??? */`; `NUM_WIN = 16` bei `AES_VERSION > 0x0320` (`include/sysconf.h:32-35`), also **160** im Falcon/TOS4-Profil (AES 0x0340, `include/config.h:1346-1347`); 80 bei AES ≤ 0x0320 |
| Pool anlegen | `aes/gemlib.h:127` `ORECT g_olist[NUM_ORECT]`; `or_start` fädelt alle auf die Freiliste `rul` (`aes/gemwrect.c:36-50`), einmalig aus `wm_init` (`aes/gemwmlib.c:1258`); `wm_new` ruft `or_start` **nicht** — es schließt/löscht Fenster, `wm_delete` gibt über `newrect` zurück (`gemwmlib.c:1476`) |
| Zellen zurückgeben | `newrect` hängt die alte Liste des Fensters an `rul` (`gemwrect.c:174-181`), `brkrct` das zerbrochene Rechteck (`:124-127`) → Belegung = Summe aller Listen (Desktop eingeschlossen) plus die Stücke, die während eines Bruchs in Arbeit sind |
| **Erschöpfung** | `get_orect` liefert den Kopf der Freiliste — und **`NULL`, wenn sie leer ist** (`gemwrect.c:53-61`: `if ((po = rul) != 0) rul = rul->o_link; return po;`). **Kein Aufrufer prüft:** `mkpiece` schreibt `rl->o_link = old` und die vier Rechteckwörter durch den Zeiger (`:68-75`) — Adresse 0..11, die ersten zwölf Bytes der m68k-Vektortabelle; `newrect` genauso für die eigene Zelle (`:201-203`); `wm_init` für die des Desktops (`gemwmlib.c:1287-1289`). Danach ist die Liste am Stück ohne Zelle abgeschnitten (`:121` hängt das `NULL` ein) und ihr Rest verwaist (`:125` verkettet durch `p == NULL`). Kein Fehlercode, keine Meldung, kein `panic`: **undefiniertes Verhalten.** |
| Zeitpunkt | nicht einmal eine Funktion der Fensteranordnung: `draw_change` baut alle Listen in einem `everyobj`-Durchlauf (`gemwmlib.c:1089`), jedes `newrect` gibt erst die **eigene** alte Liste frei — welche Anordnung den Pool sprengt, hängt an den noch gehaltenen alten Listen der Fenster, die der Durchlauf noch nicht erreicht hat |

Alle Zeilen per `sed -n` gegen `references/emutos` gegengelesen (`git rev-parse
HEAD` = `1a7484aba1bfba23e2c0899e243b6d8177fe41df`).

## Abnahme 2 — unser Verhalten

**Bestand vorher:** `tos-api/src/aes/window.rs` `const RCAP: usize = 16;`
(Zeile 1328 auf HEAD) als Größe jedes Scratch-Arrays (`border_pieces`,
`subtract`, `walk_rects`, `compute_rlist`, `w_union`, `paint_bg_clip`,
`draw_frame_pieces`, `redraw_vslider`) **und** des gespeicherten
`Window.rlist`; `put()` ließ das siebzehnte Stück wortlos fallen
(`if r.w > 0 && r.h > 0 && *m < RCAP`). Bestätigt im Test vor dem Fix:
30 Stücke gerechnet, 16 geliefert.

**Fix** (nur `window.rs`, zwei Stellen + ein Feldkommentar):

- `const RCAP: usize = NUM_WIN * 10;` — 160, EmuTOS' Poolgröße, **je Liste**.
  Wir halten eine Liste pro Fenster statt eines Pools; jede Anordnung, deren
  Listen in EmuTOS' Pool passen (Summe ≤ 160), passt hier Stück für Stück, und
  eine einzelne Liste über 160 ist mit Sicherheit jenseits von EmuTOS' Pool.
- `put()`: ein Stück ohne Fläche wird wie bisher nicht angelegt (`brkrct`
  erzeugt keines, strikte Ungleichungen `:113-116`); beim 161. Stück
  `assert!(*m < RCAP, "AES owner rectangle list: more than NUM_ORECT (160)
  … (aes/gemwrect.c:53-61, :68-69); NewTOS stops instead")`.

**Abweichung, mit Grund:** EmuTOS' Pfad ist die Dereferenzierung des `NULL`
aus `get_orect` — stilles Überschreiben der Vektortabelle. Das ist nicht
reimplementierbar. Der Abbruch mit benannter Ursache ist die ehrliche
Übersetzung („`.expect()` statt NULL-Schreibzugriff"); ein sauberes
Abschneiden der Liste wäre genau das stille Verwerfen, das dieses Ticket
abstellt, nur mit höherer Schwelle. Der Punkt ist unverändert derselbe wie
bei EmuTOS: das `wind_open`/`draw_change`, dessen Listenneubau den Pool
sprengt. Falls der Lead lieber Abschneiden + Markierung will: das ist ein
Zweizeiler in `put()`, die Doku an `RCAP` benennt beide Wege.

## Abnahme 3 — Test: `tos-api/tests/aes_window_rectlist_capacity.rs` (2 Fälle)

Harness wie `aes_window_rectlist_zorder.rs` (640×480/4 Planes, `Pb`,
`AES_LOCK`). Das Soll rechnet eine Referenz-Zerlegung (`brkrct`/`mkpiece`
TOP/LEFT/RIGHT/BOTTOM auf `WS_TRUE`, in z-Reihenfolge, dann `w_owns`-Clip auf
`WS_WORK`) — der Algorithmus, den `aes_window_rectlist_zorder.rs` Wort für
Wort an die TT-Messung bindet — **und** unabhängig davon eine Pixelmaske:
Vereinigung der gelieferten Stücke = Arbeitsfläche minus `WS_TRUE` aller
verdeckenden Fenster, pixelgenau; dazu Disjunktheit und Lage in der
Arbeitsfläche.

| Fall | Was |
|---|---|
| `thirty_pieces_all_reach_the_caller` | Fenster 1 `(30,40,500,400)` kind `0x0FEF`, darüber 7 Streifen (3 senkrecht, 4 waagerecht, kind 0) → **30** Stücke (Referenz), Walk FIRST/NEXT bis `w=h=0`; Maske, Disjunktheit, Sequenz = Referenz |
| `the_pool_limit_stops_the_open_that_crosses_it` | Desktop (Handle 0) unter 15 Haarlinien-Fenstern (9 senkrecht, 6 waagerecht, abwechselnd, jedes weiter außen und länger als das vorige): Referenz-Stückzahlen `4,6,12,17,26,32,44,51,67,75,96,105,130,` **`159`**`,` **`190`**. Nach jedem der ersten 14 `wind_open` der volle Desktop-Walk gegen Maske + Referenz (159 Stücke kommen komplett an); das 15. `wind_open` muss mit der `NUM_ORECT`-Meldung abbrechen (`catch_unwind`; Ok = rot), danach Abbau von oben nach unten (jeder Zustand ist ein schon geliefertes Präfix) + `wind_new` |

**Rot vor dem Fix** (Baum HEAD, `window.rs` MD5 `4be26b6c…`):

    test the_pool_limit_stops_the_open_that_crosses_it ... FAILED
      desktop under 4 strips: the delivered pieces do not cover the visible work area
      exactly — first difference at (0, 307); visible 293699 pixels, delivered 182979
      pixels, 16 pieces
    test thirty_pieces_all_reach_the_caller ... FAILED
      window 1 under seven strips owns 30 pieces but only 16 were delivered — the list
      was cut at the old RCAP = 16 and the rest dropped silently
    test result: FAILED. 0 passed; 2 failed

**Grün nach dem Fix:** 2/2; der gefangene Abbruch lautet
`panicked at tos-api/src/aes/window.rs:1373:5: AES owner rectangle list: more
than NUM_ORECT (160) pieces in one window's list — EmuTOS's ORECT pool is
exhausted here and it writes through the NULL get_orect() returns
(aes/gemwrect.c:53-61, :68-69); NewTOS stops instead`.

Die Anordnung für Fall 2 stammt aus einer Parametersuche über die
Referenz-Zerlegung (Scratch `…/scratchpad/a21/design3.py`): mit 15
verdeckenden Fenstern sind bis 221 Stücke in einer Liste erreichbar, die
Grenze 160 ist also mit `NUM_WIN = 16` real überschreitbar.

## Spec

`docs/api/aes/window-graphics.md`, Abschnitt „The rectangle-list iteration",
ein neuer Aufzählungspunkt **„Pool size and exhaustion (A21, 2026-09-19)"**
vor dem Correction-Block vom 2026-09-12: Poolgröße mit Zeilen, der fehlende
Erschöpfungspfad mit Zeilen, die Historienabhängigkeit, unser Stand. Der
bisherige Text nannte `NUM_ORECT = NUM_WIN·10` nur als Referenz (Zeile ~1191)
und schwieg zur Erschöpfung.

## Stack — gemessen, nicht geschätzt

Die AES läuft auf dem 16-KiB-Fiber-Stack (`tos-api/src/aes/thread.rs:100`,
**ohne Guard-Page**, Größe dort als „OFFEN, nicht gemessen" markiert); die
Scratch-Arrays sind von 128 B auf 1280 B gewachsen. Frames im Debug-Kernel
`board-rpi4` (llvm-objdump, Prolog `sub sp`): `walk_rects` 0x510+0x60,
`border_pieces` 0x5e0+0x60 (`subtract` inline), `draw_frame_pieces` 0x5a0+0x60,
`compose` 0x560+0x60 (`paint_bg_clip` inline), `wind_get` 0x500,
`send_redraw_in` 0x510, `redraw_v/hslider` 0x510.

Hochwassermarke am laufenden Debug-Kernel (QEMU raspi4b, TeraDesk per `#Z`
aus dem A20-Image, 20 s nach `[launch] EL0: slot 0 entry`, QMP `pmemsave`
von PA `0x2e5330`/`0x4000` = `AES_THREAD_STACK`, erster Nicht-Null-Byte):
**4464 von 16384 B belegt (27 %), 11 920 B frei** — untere Schranke, weil die
tiefsten Frames genullte Arrays halten; die Zahl ist exakt
`compose`+`walk_rects`+`border_pieces` (1472+1392+1600). Der Fensterpfad
(zusätzlich `draw_frame_pieces` 1536) liegt damit rechnerisch bei ≈ 6,5 KiB
für den Listenbau und ≈ 8 KiB für den Zeichenzweig. Screendump zum
Messzeitpunkt zeigt TeraDesk mit Menü und Icons, keine Fenster (Serial: kein
panic/EXCEPTION). Artefakte: `…/scratchpad/a21/out/debug2-fiber.bin{,.png}`,
`debug2-serial.log`; Kernel `kernel8-a21-debug.img` SHA-256 `3fa415ff…`.

Speicher: `Window` wächst um 1152 B; `TABLE` und `SAVE` je 0x6202 = 25 090 B
(`.data`, vorher ≈ 6,7 KiB) → Kernelabbild +≈ 37 KiB. Linker-Assert
`STRAM_OS_RESERVE` hält.

## Beleglage

| | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **589 grün / 0 rot / 1 ignoriert** (Baseline 587 + 2 neue) |
| Nachbarsuiten | `aes_reference` 204/204 (darin `cases_r2b_rlist`), `aes_window_rectlist_zorder` 5/5, `aes_wind_get_desktop_geometry` 8/8, `aes_window_drag` 4/4 |
| Bau rpi4 / qemu-virt (frisch, `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` vor jedem Board; „Compiling kernel/tos-api" je Board bestätigt) | beide `rc=0`, tos-api **64**, kernel **94** rustc-Warnungen — Baseline, Delta 0 |
| Clippy tos-api / kernel (`board-rpi4`, `board-qemu-virt`, `--lib --bins`) | tos-api **81**, kernel **125** — Baseline, Delta 0; die 11 Clippy-Meldungen in `window.rs` sind alle vorbestehend (`top` never read :1964, `Aespb`-Sichtbarkeit ×10); neue Testdatei **0** |
| `git status --short` | `M docs/api/aes/window-graphics.md`, `M tos-api/src/aes/window.rs`, `?? tos-api/tests/aes_window_rectlist_capacity.rs`, `?? TASK/A21-STATUS-DONE.MD` |

## Nebenbefunde (unverändert, je ein eigener Task)

- `tos-api/src/aes/thread.rs:90-113`: der AES-Fiber-Stack hat keine
  Guard-Page; ein Überlauf liefe still in `.bss`. Die Messung oben ist der
  erste Zahlenwert zu der dort als OFFEN markierten Größe.
- `tos-api/src/aes/window.rs:1964`: Clippy `value assigned to top is never
  read` — vorbestehend, nicht angefasst.
- Die Scratch-Arrays werden je Aufruf auf dem Stack angelegt, während EmuTOS'
  Pool statisch ist (`D.g_olist`); ein statischer Pool mit Index-Verkettung
  wäre die 1:1-Struktur, ist aber ein Umbau der ganzen Listenmechanik, nicht
  „nur die Kapazitätsgrenze".

## Offen

- Tester-Abnahme (Boot + `svc`-Selftests + Screendumps + `fsck_msdos`, beide
  Boards). Für diese Änderung sind die Hosttests der Beleg; auf der Hardware
  ist bei ≤ 16 Stücken pro Fenster kein Pixel anders. Sichtbar wird der Fix
  erst bei einem Fenster unter ≥ 5 anderen mit Überkreuzung — kein Selftest
  dafür verdrahtet.
- Keine neue Hardwaremessung nötig; A32 (Rechtecklisten-Abnahme auf echter
  Hardware) bleibt wie bisher offen.
- Nicht committet; `docs/STATUS.md` nicht angefasst; `TASK/A21.md`-Kopfzeile
  unverändert.
