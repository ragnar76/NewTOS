# DESK37 — STATUS: DONE (2026-09-24) — implementiert, Pi-Abnahme offen

**Auftrag:** Hinter einer laufenden GEM-Anwendung steht Weiss; EmuTOS zeigt
dort den Desktop-Hintergrund. Vorher klaeren, woher unser Gegenstueck zu
`rs_trees[DESKTOP]` seinen `ob_spec` nimmt, dann `sh_show`/`sh_draw`s
`FALSE`-Zweig an EmuTOS' Stelle rufen. Ob die Menueleiste den Programmnamen
zeigt, gehoert mitgemessen.

Basis: Branch `desk-rebuild`, HEAD `fa1d403`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurements/measurement-2026-09-24-desk37-desk36-desk30/`](../measurements/measurement-2026-09-24-desk37-desk36-desk30/)
§2. Spezifikation: `docs/api/aes/shell-loop.md`, Abschnitt *`sh_show(fname)`,
the screen behind a launched GEM program (DESK37)*. Hardwarepunkt **61** in
`docs/api/desktop/_hw-test-2026-09-17.md`.

---

## 1. Die offene Frage des Tickets, am Quelltext beantwortet

Das Muster haelt die **AES**, nicht der Desktop.

`sh_draw` zeichnet `rs_trees[DESKTOP]` (`gemshlib.c:271`). Unser Gegenstueck
ist der Drei-Objekt-Baum `DESKTOP` in `tos-api/src/aes/init.rs:139`, aus
`DESKTOP_ROM` — der Ressourcenkonstanten `4419` = `0x1143` von
`gem_rsc.c:551-556` (Randfarbe 1, Textfarbe 1, Fuellmuster 4, Fuellfarbe 3 =
gruen). Das **untere Byte** ersetzt `set_aes_background` beim AES-Start aus
der `#Q`-Zeile der `.INF`-Datei (`geminit.c:635-641`, gerufen aus `:719-728`);
bei uns sind das `process_inf1_q` und `gem_rsc` in derselben Datei.

`G.g_patcol` des Desktops (DESK09) ist eine **zweite, unabhaengige** Lesung
derselben Zeile (`deskapp.c:953-960`) und spielt hier keine Rolle:
`desktop_relinquish` hat den Desktop-Hintergrundbaum mit
`wind_set(DESKWH, WF_NEWDESK, NULL)` bereits abgemeldet, bevor das Programm
laeuft. Deshalb ist dieser Schritt die Sache der AES — und deshalb zeigt der
Schirm hinter einer Anwendung die `#Q`-Farbe, obwohl der Desktop nicht lebt.

Mit `#Q 41 40 43 40 43 40` (unser `desk_inf_data1`) ist das >2-Plane-Byte
`0x43`, also genau das untere Byte der Ressourcenkonstante — Muster 4,
Farbe 3. Die Karte, die `make-sd-image.sh` schreibt, traegt eine
Platzhalter-`NEWDESK.INF` ohne `#Q`, dann bleibt es bei `0x1143`. Beide Wege
enden bei demselben gruenen Raster.

## 2. Was gebaut wurde

`crate::shell_loop::sh_gem_show` (`kernel/src/shell_loop.rs`) ruft
`tos_api::aes::shell::sh_show(sh_name(cmd))` — beides gab es schon, der
Bootpfad benutzt `sh_show("")` fuer den Desktop selbst (`gemshlib.c:603`).
Neu ist der Aufruf an der **zweiten** Stelle: `desk::main::run_gem_app`
(`kernel/src/desk/main.rs`) ruft ihn zwischen `desktop_relinquish()` und
`run_program_with_tail`, also genau dort, wo `sh_ldapp` ihn hat — zwischen
dem `sh_main`-Block (`gemshlib.c:715-720`) und `dos_exec` (`:645`).

Zwei Dateien, zwei Funktionen; kein Verhalten der AES geaendert.

## 3. Wie nah am Hatari-Vergleichsbild

Strukturell deckungsgleich. Verglichen wurden `d37g-after-C-app.png` (unseres)
und `measurements/measurement-2026-09-24-hatari-appbg/screendumps/emutos-bgwin-falcon-tc.png`
(EmuTOS 1.4.0+ auf Hatari 2.6.1, Falcon/VGA):

| | Hatari/EmuTOS | NewTOS |
|---|---|---|
| Zeilen 0..17 | weiss, Programmname schwarz zentriert | dasselbe |
| Zeile 18 | durchgehend schwarz | dasselbe |
| Zeilen 19..479 | 320 gruene + 320 weisse Pixel je Zeile | dasselbe |
| Rasterphase Zeile 19 | `WGWGWGWGWGWG` | `WGWGWGWGWGWG` |
| Rasterphase Zeile 20 | `GWGWGWGWGWGW` | `GWGWGWGWGWGW` |
| Programmname | `BGWIN.PRG` | `GEMWIN.NAP` |
| Farbworte | `(0,251,0)` / `(255,251,255)` | `(0,255,0)` / `(255,255,255)` |

Die letzte Zeile ist der einzige Unterschied, und es ist Hataris
Falcon-VGA-Rampe (5 Bit je Kanal), kein Unterschied im Zeichnen.

`d37q-after-D-qed.png` gegen `emutos-qed-falcon-tc.png`: QEDs eigene
Menueleiste oben, dasselbe gruene Raster darunter.

## 4. Pruefung

* `cargo xtask build --board rpi4` / `--board qemu-virt`, je nach
  `cargo clean -p kernel -p tos-api`: `tos-api` 64 / `kernel` 94 Warnungen —
  Basis 64/94, Delta 0.
* `cargo clippy --features board-rpi4` / `board-qemu-virt`: 80/125 — Basis
  80/125, Delta 0; null Treffer in `kernel/src/desk/`.
* `cargo run -p xtask -- test`: 939 bestanden / 0 gescheitert / 1 uebersprungen
  — Basis 939/0/1. Kein Hosttest dazugekommen: beide beruehrten Dateien
  liegen in der `no_std`-aarch64-Kiste `kernel`.
* Vier QEMU-Laeufe (`d37g`/`d37q`, je before und after), 0 `[EXC]`/`[PANIC]`,
  `fsck_msdos -n` sauber auf jedem geschriebenen Abbild.

## 5. Offen

Die echte Pi 4. Punkt 61 der Checkliste, insbesondere derselbe Hintergrund in
**1920x1080** (das Raster darf keine Naht zeigen).
