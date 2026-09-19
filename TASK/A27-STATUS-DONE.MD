# A27 — STATUS: DONE (Beleg + Festlegung + Spec + Hosttest) / Tester-Abnahme offen

Das Nullen des dispatch-lokalen `int_out` bleibt; es ist jetzt **belegt**
(Inventar je Opcode gegen EmuTOS-Quelle, gemlib-Bindung und Messung),
**festgelegt** (benannte Abweichung in `docs/COMPATIBILITY.md`), die
Spec-Aussage D-095 („unchanged") ist **korrigiert**, und ein Hosttest hält
das Verhalten. Bearbeitet 2026-09-19, Branch `desk-rebuild`, Basis HEAD
`dee18d1`, Baum vorher sauber. Kein Produktcode außer dem Kommentar in
`mod.rs`. Nicht committet.

## Abnahme 1 — Inventar: `docs/analysis/aes-underwriting-handlers-2026-09-19.md`

Alle 79 Opcodes des Dispatchers, Tabelle Opcode · Name · `OUT_LEN` (gemlib
`AES_PARAMS`, per `/usr/bin/grep` über `sdk/gemlib/gemlib-0.44.0/a_*.c`) ·
geschriebene Wörter EmuTOS (`crysbind` + Handler, `aesdefs.h`-Makros) ·
geschriebene Wörter NewTOS (`set_intout`/`set_ret` je Pfad) · unterschreibend.
`wind_get` in elf Feldklassen, `objc_sysvar` je Modus, `evnt_multi`, `fsel_*`
und der `default`-Arm einzeln.

| Klasse | Opcodes |
|---|---|
| beide Seiten unterschreibend, gemlib-Standardbindung | **104** `wind_get` (`WF_HSLIDE/VSLIDE/HSLSIZ/VSLSIZ`, `WF_TOP`: `[2..4]`; `WF_COLOR`/`WF_DCOLOR`: `[1]`, `[4]`; Listenende P1: `[1..2]`; Absage/ungültiges Handle: `[1..4]`), **48** `objc_sysvar` SET-Modus (`[1..2]`), und der **`default`-Arm** bei jeder Bindung mit `OUT_LEN > 1` |
| beide Seiten, nur unter Erweiterungsbindungen | **77** `graf_handle` (`graf_xhandle`, `OUT_LEN 6`), **100** `wind_create` (`wind_xcreate`, 5), **105** `wind_set` (`wind_xset_grect`, 5), **50** `form_do` (`form_xdo`, 2) |
| nur EmuTOS unterschreibend, NewTOS schreibt das Wort explizit 0 | **25** `evnt_multi` `[5]`/`[6]` ohne gefeuertes `MU_KEYBD`/`MU_BUTTON` (`gemevlib.c:179,193,200,275,277`), **90/91** `fsel_*` `[1]` auf den frühen `return FALSE` (`gemfslib.c:630-631,658-661`), 48 GET Einwort-/unbekannter Selektor |
| voll schreibend, kein Befund | die übrigen **70** |
| nicht Gegenstand | `WF_SCREEN` (A54), NewTOS-Schutzpfade `objc_offset`/`form_center` |

Messbelege aus den BIN selbst gelesen (eigener Recordleser, Offset 76,
big-endian): r4 Fall 283 `graf_handle OUT_LEN 7` TT `…, 0, 1` / NewTOS `…, 0, 0`;
415/418/421 TT `[2..4] = 20, 300, 200` (das `WF_FXYWH`-Rechteck davor);
433/436 `63, 296, 194`; 452 (P1) `31, 63`; 336/397/400/445/448 die
`WF_SCREEN`-Antwort `0x0218/0xA000`; 568–574 `objc_edit` `[2] = 63`; r2b
127/133/204 `31, 63` / `0, 19` (A15). Runden 1, 2, m05–m07 haben `OUT_LEN` je
Fall auf die geschriebenen Wörter zugeschnitten und sind wortgleich.

Dazu der Befund, dass gemlib selbst 0 in solchen Wörtern **vorlegt**
(`a_wind_get.c:355-358`, `a_graf_xhandle.c:48-49`) und EmuTOS' unbedingte
Rückkopie das überschreibt — NewTOS liefert, was die Bindung vorlegen wollte.

## Abnahme 2 — Festlegung: `docs/COMPATIBILITY.md`, „Sanctioned deviations"

Neuer Eintrag nach dem `WF_SCREEN`-Eintrag (A54), Muster wie V16/A54:
*„An AES handler that writes fewer than `OUT_LEN` words delivers 0 in the
unwritten words; EmuTOS delivers the uninitialised dispatcher stack there"* —
mit `gemsuper.c:432,444-445`, den Handlerzeilen, den Messwerten, und den drei
Gründen aus dem Ticket: (1) Anwendung — unbestimmt ist dort ohnehin kaputt,
konforme Programme lesen die Wörter nicht (QED `aktion.c:148`), und gemlib
legt 0 vor; (2) Messung — Diff-Rauschen, Verweis auf ERRATA/A15 und die
r4-Schublade S3-a, der Harness muss die Wörter selbst benennen;
(3) Schutzgrenze — kein Kernelstapel-Leck nach EL0, dieselbe Regel wie für
`int_in`/`addr_in` (A19). Ausdrücklich unverändert: die Kopiergrenze selbst
(genau `OUT_LEN`, nichts darüber, nichts bei 0). Der Kommentar an
`tos-api/src/aes/mod.rs` (`dispatch`, vor `let mut local_intin`) verweist auf
den Eintrag, das Inventar und den Test.

## Abnahme 3 — Spec

* `docs/api/aes/window-graphics.md`, **D-095-Block** (`wind_get (104)`): der
  Satz „`int_out[1]` … reads back unchanged / the input gadget number" ist
  ersetzt: `[1]` und `[4]` schreibt niemand, der Aufrufer behält innerhalb
  `OUT_LEN` nie seinen Wert (`gemsuper.c:432,444-445`), EmuTOS liefert Stapel,
  NewTOS 0 (Verweis COMPATIBILITY); gemlib kopiert `int_out[1]` sogar über
  die Gadget-Variable der Anwendung zurück (`a_wind_get.c:366,372`).
* Ebenda, **„The rectangle-list iteration"**, `w_owns`-Absatz: die
  A18-Formulierung „the caller's buffer content unchanged if the owner list
  was already empty" — durch die A15-Poison-Messung widerlegt — ersetzt durch
  den dispatch-lokalen Puffer (`gemsuper.c:343,432,444-445`), Stapel/0.
* Ebenda, **„Edge cases discovered in source"** Punkte 2 und 15: „left
  unchanged" / „left untouched" präzisiert auf „unwritten → dispatcher's
  local word, stack on EmuTOS, 0 on NewTOS".
* `docs/api/aes/_dispatcher.md`: § 2 Schritt 5 (Rückkopie aus dem
  uninitialisierten lokalen Feld, Verweis Inventar) und § 4 ein Punkt zur
  NewTOS-Regel.

## Abnahme 4 — Test: `tos-api/tests/aes_intout_underwrite.rs` (4 Fälle)

Harness `aes_reference/harness.rs` (`Pb`, Poison `0xDEAD`, `AES_LOCK`,
`context()`), LP64, 640×480/4 Planes.

| Fall | Aufruf | geprüft |
|---|---|---|
| `wind_get_wf_color_…` | `wind_get(0, WF_COLOR, W_NAME)`, `OUT_LEN 6`; danach `WF_DCOLOR` | `[0] = 1`; `[2..3] = 0x11a1, 0x1101` (`wm_start`-Defaults `gemwmlib.c:1347-1352`, D-095); `[1]`, `[4]`, `[5]` ≠ Poison **und** = 0; `[6]` = Poison (außerhalb `OUT_LEN`) |
| `graf_handle_at_out_len_7_…` | Fall 283 nachgestellt, `OUT_LEN 7`, dann 5 | `[0]` = Handle, `[1..4]` > 0, `[5..6] = 0, 0`; bei `OUT_LEN 5` bleiben `[5..6]` Poison (Fall 282) |
| `wind_get_wf_hslide_…` | erst `WF_FXYWH` (Rechteck), dann `WF_HSLIDE`, je `OUT_LEN 5` — die Folge der Fälle 412→415 | `[1] = 0`; `[2..4]` ≠ Poison, ≠ Rechteckwort des vorigen Aufrufs, = 0; `[5..6]` Poison |
| `objc_sysvar_set_mode_…` | SET `LK3DIND` mit `-1, -1` (zustandsneutral), `OUT_LEN 3` | `[0] = 1`, `[1..2] = 0, 0`, `[3]` Poison |

**Grün:** 4/4.

**Rot-Gegenprobe** (Scratch, von Hand): `mod.rs` kopiert (MD5
`64f23d0a…`), `let mut local_intout = [0i16; O_SIZE]` durch
`[0x7777i16; O_SIZE]` (Stellvertreter für Stapelinhalt) ersetzt →
**0 passed; 4 failed**, jeder Fall genau am benannten Wort:

    graf_handle_…:  int_out[5..6]: left: (30583, 30583)  right: (0, 0)
    objc_sysvar_…:  int_out[1..2]: left: (30583, 30583)  right: (0, 0)
    wind_get_wf_color_…:  int_out[1] … left: 30583  right: 0
    wind_get_wf_hslide_…: int_out[2] … left: 30583  right: 0

Danach aus der Kopie zurückgespielt, MD5 wieder `64f23d0a…`, Test 4/4 grün.
Kein `git checkout`/`restore`.

## Beleglage

| | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **593 grün / 0 rot / 1 ignoriert** (Baseline 589 + 4 neue), `rc=0` |
| Bau rpi4 / qemu-virt (frisch, `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` vor jedem Board; „Compiling kernel/tos-api" je Board bestätigt) | beide `rc=0`, tos-api **64**, kernel **94** rustc-Warnungen — Baseline, Delta 0 |
| Clippy kernel+tos-api (`board-rpi4`, `board-qemu-virt`, `--lib --bins`) | tos-api **81**, kernel **125** — Baseline, Delta 0; neue Testdatei **0** Clippy-Meldungen |
| `git status --short` | `M docs/COMPATIBILITY.md`, `M docs/api/aes/_dispatcher.md`, `M docs/api/aes/window-graphics.md`, `M tos-api/src/aes/mod.rs` (nur Kommentar), `?? docs/analysis/aes-underwriting-handlers-2026-09-19.md`, `?? tos-api/tests/aes_intout_underwrite.rs`, `?? TASK/A27-STATUS-DONE.MD` |

## Nebenbefunde (unverändert, je ein eigener Task)

- `tos-api/src/aes/window.rs` `init_desk_window` (≈ `:497-507`) setzt das
  Desktop-Fenster aus `Window::EMPTY` mit den **Compile-Zeit-Defaults**
  `GL_WTCOLOR_DEFAULT`/`GL_WBCOLOR_DEFAULT` auf; EmuTOS' `w_setup(ppd, DESKWH,
  NONE)` kopiert die **aktuellen** `gl_wtcolor`/`gl_wbcolor` (`gemwmlib.c:222-225`),
  die `wind_set(WF_DCOLOR)` geändert haben kann. `wind_create` (`:2370`) macht
  es richtig. Sichtbar nur nach `WF_DCOLOR`-Set + `wm_init`.
- `objc_sysvar` GET mit Einwort-Selektor schreibt `[2]` **explizit** 0
  (`object.rs:1372-1380`), EmuTOS lässt es stehen — beim Aufrufer identisch,
  aber ein zweiter Ort, an dem die Regel „0" entsteht (vgl.
  A15-Kompensationsprüfung). Gleiche Klasse: `evnt_multi` `[5]`/`[6]`, `fsel_*` `[1]`.
- Fälle 568–574 (`objc_edit`) und 283 (`graf_handle`) der eingefrorenen
  r4-Tabelle deklarieren mehr `OUT_LEN` als gemlib (3 statt 2, 7 statt 5);
  wer r4 einmal als Referenz nachspielt, muss diese Wörter benennen wie der
  Harness es für `WF_OWNER` tut. Kein Erratum nötig — der Tabellenkopf sagt es
  selbst („from xif's own stack").

## Offen

- Tester-Abnahme (Boot + `svc`-Selftests + Screendumps + `fsck_msdos`, beide
  Boards). Für diese Änderung ist der Hosttest der Beleg; Produktverhalten ist
  **unverändert** (nur ein Kommentar in `mod.rs`), auf der Hardware ist kein
  Byte anders. Kein neuer Selftest zu verdrahten.
- Keine Hardwaremessung nötig; keine Tabelle geändert.
- Nicht committet; `docs/STATUS.md` nicht angefasst; `TASK/A27.md`-Kopfzeile
  unverändert.
