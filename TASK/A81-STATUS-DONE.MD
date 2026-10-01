# A81 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen

`wind_set(DESKWH, WF_NEWDESK, tree, root)` liest den Root-Index jetzt an der
Stelle, die die Zeigerkonvention vorgibt — hinter dem gepackten Zeiger an
dessen Breite (`int_in[6]` fuer LP64, `int_in[4]` fuer ILP32) oder in
`int_in[2]` bei der nativen `addr_in[0]`-Form; gemlibs
`wind_set_str`/`wind_set_ptr`/`wind_set_ptr_int` senden die native Form.
Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis HEAD `26b0772`,
**nicht committet**. Spec (vom Lead, uncommitted, unveraendert gelassen):
`docs/api/aes/window-graphics.md` „Pointers in `addr_in[]`“ + Randfall 16,
`docs/COMPATIBILITY.md` A81, `docs/sdk/05-abi-pitfalls.md` B10.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemwmlib.c:1685-1689` `gl_newdesk = *(OBJECT **)pinwds; gl_newroot = pinwds[2];` mit `pinwds = &int_in[2]` (`aes/gemsuper.c:345-346`) — ein 2-Wort-Zeiger, Root im Wort danach |
| NewTOS (vorher) | `tos-api/src/aes/window.rs::set_newdesk`: Baum ueber `intin_ptr(2)` an Aufruferbreite (LP64: `int_in[2..5]`), Root **fest** aus `int_in[4]` = Bits 32..47 des Zeigers; gemlib pristine `a_wind_set_ptr_int.c:27-28` schreibt `W3` genau dorthin |
| NewTOS (nachher) | `tos-api/src/aes/mod.rs` (`Aespb::ptr_words`), `window.rs` (`set_newdesk`), `sdk/gemlib/overlay/a_wind_set_{ptr_int,ptr,str}.c` (neu) |

## Was gebaut ist

* **`tos-api/src/aes/mod.rs`** — `Aespb::ptr_words()` (4 bei `Lp64`, 2 bei
  `Ilp32`) als die eine Stelle, an der die Wortzahl eines gepackten Zeigers
  entschieden wird; `intin_ptr()` nutzt sie statt eines eigenen `match`.
* **`tos-api/src/aes/window.rs`** — `set_newdesk`: gepackter Arm liest
  `int_in[2 + ptr_words]` statt `int_in[4]`; Doc-Kommentar auf den
  Zweiformen-Vertrag mit EmuTOS-Zitat umgeschrieben (inkl. Hinweis, dass die
  gepackte LP64-Form `IN_LEN >= 7` deklarieren muss, sonst liest der
  dispatch-lokale Puffer 0). `wf_str_ptr` unveraendert. Neu
  `#[cfg(test)] mod newdesk_root_word_tests` (3 Faelle).
* **`sdk/gemlib/overlay/a_wind_set_ptr_int.c` / `a_wind_set_ptr.c` /
  `a_wind_set_str.c`** (neu, Kopfmuster wie `a_evnt_timer.c`):
  `AES_PARAMS(105,6,1,1,0)`, `aes_addrin[0] = (long)v`, `aes_intin[2] = W3`
  (ptr_int) bzw. `0`, `aes_intin[3..5] = 0`. Kein Store mehr ueber
  `int_in[4..7]`. Nachgeprueft im gelinkten `DESKTOP.NAP`:
  `mt_wind_set_ptr_int` speichert `w3` an `intin[2]` und `x2` an `addrin[0]`.
* **`sdk/gemlib/tests/wind_set_addrin.c`** (neu, Form von
  `evnt_timer_words.c`, Hostprogramm mit eigenem `aes()`): prueft
  `control[0..4] = 105,6,1,1,0`, `intin[0..1]`, `intin[2] = W3`,
  `intin[3..5] = 0`, `addrin[0] = ptr` fuer alle drei Bindungen.
* **`tos-api/tests/aes_newdesk_root_word.rs`** (neu): drei Faelle durch
  `tos_api::aes::dispatch` (LP64), beobachtet ueber die Pixel, die die AES
  nach `form_dial(FMD_FINISH)` als Hintergrund malt (genau die Ikone des
  registrierten Roots). Der Baum ist auf 32768 Objekte aufgefuellt, damit
  ein falsch gelesener Root auf dem alten Code ein leeres Objekt trifft
  statt neben das Array — der alte Code muss den Test **verlieren**, nicht
  abstuerzen. Der Root wird zur Laufzeit so gewaehlt, dass er von den Bits
  32..47 der Host-Adresse abweicht (mit Root 1 und einem Baum bei
  `0x1_…` waere der alte Code zufaellig gruen — die Zufallsgruen-Falle aus
  dem Ticket).
* **`docs/STATUS.md`** — Phase-13-Zeile (datierter Eintrag in der Form der
  juengsten Eintraege; die Zeile numeriert nur die zurueckgestellten Punkte
  #51/#59/#108, nicht die gelandeten) + Zeile in „Recent Decisions“.
* **`measurements/measurement-2026-09-20-a81/`** — RUNINFO, Screendumps, Serial,
  Hosttest-Quittungen, Build-Logs, Methode (Skripte, Overlays, Kernel-Diff),
  SHA-256-Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach

Gegenprobe in einer Scratch-Kopie der Crates mit `window.rs`/`mod.rs` = HEAD
`26b0772` und angehaengtem Testmodul (`measurements/measurement-2026-09-20-a81/host-tests/newdesk_root_word-RED-before.txt`,
`…-GREEN-after.txt`):

| Test | vor | nach |
|---|---|---|
| Modul `lp64_packed_root_is_the_word_after_the_four_pointer_words` (Baum `0x0000_0001_0177_E4C0` in `int_in[2..5]`, Root 2 in `int_in[6]`) | `left: (4319601856, 1) right: (4319601856, 2)` — Root aus Bits 32..47 | gruen |
| Modul `native_form_root_is_int_in_2` | gruen | gruen |
| Modul `ilp32_packed_root_is_int_in_4` (Regressionswaechter, `int_in[6]` vergiftet) | gruen | gruen |
| Integration `lp64_packed_form_takes_the_root_from_the_word_after_the_pointer` | „tree 0x000000786d52c000 … root 1 in int_in[6] (a fixed int_in[4] read yields 120) … 1024 of its 1024 pixels are untouched“ | gruen |
| Integration `lp64_packed_form_with_root_0_paints_the_whole_background` (TeraDesks Aufruf) | rot auf dem Host (Baum ueber 4 GiB → Root 120; auf dem Ziel war dieser Fall nie falsch) | gruen |
| Integration `native_form_takes_the_root_from_int_in_2` | gruen | gruen |
| gemlib `wind_set_addrin` gegen Overlay / gegen pristine Tarball-Bindungen | pristine: 17 Fehler, u. a. `control[3] got 0x0`, `intin[4] got 0x2` | Overlay: OK, exit 0 |

Der ILP32-Fall ist auf dem Host nur als Modultest fahrbar: ein 32-Bit-Baum
unter 4 GiB ist in einem macOS-64-Bit-Prozess nicht abbildbar (PAGEZERO);
der Modultest liest `NEWDESK_TREE`/`NEWDESK_ROOT` direkt, ohne Deref.

`cargo run -p xtask -- test` **674 → 680 / 0 / 1**. `cargo xtask build
--board rpi4` rustc tos-api 64 / kernel 94 — Delta 0. Clippy
(`-p tos-api --tests`): keine Treffer in `set_newdesk`, `ptr_words`, dem
Testmodul oder der neuen Testdatei. Bibliothekskette: gemlib 578/578,
cflib 129/129 (beide „failures match expected-failures.txt: 0/0/0“), QED
und TeraDesk LINK OK mit 0 undefinierten Symbolen.

## QEMU raspi4b (`measurements/measurement-2026-09-20-a81/`)

TeraDesk per `#Z`-Autorun (INF-Dateien aus A78), 640×480, headless, QMP.
Vorher = Scratch-Kern mit HEAD-`window.rs`/`mod.rs` + `DESKTOP.NAP` aus einer
Scratch-Kopie von `sdk/` ohne die drei Overlays (`GEMLIB_REFRESH=1`, pristine
Bindungen, 282112 Bytes); nachher = Baum-Kern + Baum-`DESKTOP.NAP` (282128
Bytes). Beide Images per Mount verifiziert. **Alle vier Screendumps
bytegleich** (sha256 `5148fde2…`; A/B je Lauf = gesettelt, vorher/nachher =
Hintergrund unveraendert), `grep -c "EXC\|PANIC\|panic"` = 0 in beiden
Serial-Logs. Erwartet, da TeraDesk Root 0 uebergibt (`icon.c:2511`).

## Nicht angefasst / offen

* **Pi-Sichtabnahme offen:** TeraDesk-Hintergrund auf der echten Pi 4 mit
  dem neuen `DESKTOP.NAP` unveraendert (Tester/Lead; nichts geflasht).
* `wf_str_ptr` und die Spec-Dateien des Leads unveraendert.
* Erster Gesamtlauf von `xtask test` brach an
  `tos-api/tests/aes_icon_unaligned_raster.rs:357` ab
  (`a_g_icon_with_odd_raster_pointers_draws_like_an_even_one`); isoliert
  3/3 gruen im Baum **und** 3/3 gruen in der HEAD-Scratch-Kopie, zweiter
  Gesamtlauf 680/0/1 — Flake unter dem parallelen Lauf, nicht A81
  (`host-tests/xtask-test-run1-icon-flake.log`). Nicht angefasst.
* Die gepackte LP64-Form verlangt `IN_LEN >= 7`; die Spec nennt nur
  `I_SIZE`. Im Doc-Kommentar von `set_newdesk` vermerkt, Spec nicht
  angefasst (Lead).
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
