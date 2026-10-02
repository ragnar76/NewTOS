# PORT06-EDDI — Quittung

**Status: IMPLEMENTIERT · in QEMU raspi4b belegt · Abnahme durch den Tester
offen** · 2026-10-02 · Basis `desk-rebuild`, HEAD `44573a6`, Baum vorher
sauber · Belege [measurements/measurement-2026-10-02-eddi/](../measurements/measurement-2026-10-02-eddi/)

NewTOS legt den Cookie `EdDI` an und beantwortet `vq_scrninfo` (VDI 102,
`CONTRL[5] == 1` und `INTIN[0] == 2`) mit der Falcon-HiColor-Tabelle der
freigegebenen Spec [`docs/api/vdi/eddi-vq_scrninfo.md`](../docs/api/vdi/eddi-vq_scrninfo.md)
(E1–E9 wie §7 empfohlen, E3 = `0x02`). Gegen den echten EdDI (VDI-Enhancer
1.00 auf EmuTOS Falcon-Truecolor in Hatari) stimmen **272 von 273 Woertern**;
das eine abweichende ist `[14]`, genau das, das die Spec abweichen laesst.

## Was tatsaechlich getan wurde

| Datei | Stelle | was |
|---|---|---|
| `tos-api/src/vdi/eddi.rs` | neu | `is_scrninfo` (E7: `CONTRL[5] == 1` **und** `INTIN[0] == 2`, nur bei 16 Planes), `scrninfo_table()` (die 273 Woerter, aus Geraetewort-Konstanten abgeleitet; Doc-Tabelle Wort fuer Wort gegen den Enhancer), `vq_scrninfo` (`INTOUT[0..=272]`, `CONTRL[4] = 272`, `CONTRL[2] = 0`, PTSOUT unberuehrt) |
| `tos-api/src/vdi/mod.rs` | `dispatch` | `scrninfo` einmal vor den Handlern entschieden (falls ein Aufrufer INTOUT auf INTIN legt); `102 if scrninfo` vor `102 => vq_extnd`; `d0` fuer `vq_scrninfo` = 0 (kein `flip_y`-Handler lief) |
| `tos-api/src/cookie.rs` | `EDDI`, `EDDI_MARKER`, `seed_defaults` | Tag `'EdDI'` = `0x45644449`, Wert `0x00000001` (E2, s. u.), beim Boot gesetzt |
| `tos-api/src/bdos/process/loader.rs` | `COOKIE_JAR_SLOTS`-Assert, Doc `write_cookie_jar` | Bootsatz jetzt 5 Cookies: Assert `>= 6`, Text „5 cookies / 11 further" |
| `kernel/src/selftest_cookie.rs` | zwei Kommentare | „4 cookies" → 5 (Pruefungen unveraendert, keine haengt an der Anzahl) |
| `tos-api/tests/vdi_eddi_scrninfo.rs` | neu, 6 Tests | s. „Pruefung" |
| `tosdump/abi/newtos_scrninfo.c` | neu | AArch64-Backend der Sonde: Jar ueber `$5A0` (pro Prozess gespiegelt, user-lesbar, 32-Bit-Zellen), `td_eddi_call0` verweigert mit Meldung + `exit(2)` (wird bei ungeradem Wert nie erreicht), Uhr ueber osbind |
| `tosdump/build-scrninfo-newtos.sh`, `tosdump/Makefile` | neu / Ziel `scrninfo-newtos` | baut `SCRNINFO.NAP`; `core/scrninfo.c` unveraendert |
| `tosdump/core/scrninfo.h` | Kopfkommentar | nennt beide Backends |
| `docs/COMPATIBILITY.md` | „Sanctioned deviations" | neuer Eintrag „The VDI speaks EdDI" (E8), vier beobachtbare Unterschiede zu EmuTOS |
| `docs/api/vdi/workstation/102-vq_extnd.md` | Edge case 2, neuer Abschnitt | Verweis auf die EdDI-Spec, welcher 102-Aufruf was ist (Spec §8) |
| `docs/api/bios/_cookie-jar.md` | Cookie-Tabelle, Gaps | Zeile `EdDI`, „advertises 5 cookies" |
| `docs/api/vdi/eddi-vq_scrninfo.md` | Statuskopf | Vermerk „Umgesetzt 2026-10-02" mit Markerwert und Fundstellen (Inhalt der Spec unveraendert) |

## Entscheidung E2: der Cookie-Wert

`0x00000001` — **ungleich 0** (wer nur `value != 0` prueft, sieht den Cookie)
und **ungerade** (nie eine m68k-Codeadresse; `jsr` darauf ist ein
Adressfehler). Geprueft, wer im Baum den Cookie liest: zView
(`zvdi/vdi.c:28`) und HighWire (`romvdi.c:70`) pruefen nur die Anwesenheit und
rufen dann `vq_scrninfo`; Doom liest ihn noch nicht; die Sonde
`tosdump/core/scrninfo.c` ruft einen Wert, der 0 oder ungerade ist, nicht auf
— im Lauf belegt („skipped: … not a callable address"). Der Wert traegt
**keine** Versionsnummer: die gibt es bei EdDI nur ueber Dispatcher-Opcode 0,
und den gibt es hier nicht. Wer den Dispatcher trotzdem ruft, stuerzt ab —
dokumentiert in COMPATIBILITY.md und im Doc-Kommentar von `EDDI_MARKER`.

## Wort fuer Wort gegen den Enhancer

`COMPARE.TXT` im Belegordner: 273 Woerter verglichen, **1 verschieden**:

| | Enhancer 1.00 | NewTOS | Begruendung |
|---|---|---|---|
| `work_out[14]` | `0000` | `0002` | E3; der Enhancer ist EdDI 1.00, `[14]` gibt es erst ab 1.10 |
| Cookie-Wert | `00023EC0` | `00000001` | E2 (b) |
| Dispatcher(0) | `d0 = 0100` | nicht gerufen | E2 (b) |
| `CONTRL[2]/[4]` nachher | unberuehrt (`5A5A`) | `0` / `272` | der NewTOS-Dispatcher schreibt die Zaehler bei jedem Aufruf neu (wie EmuTOS' `screen()`); 272 ist der dokumentierte Wert (Spec §3.1) |

Gleich: alle uebrigen 272 Woerter, `INTOUT[273..]` unberuehrt, PTSOUT
unberuehrt. `vq_extnd(0)`, `vq_extnd(1)` und `v_opnvwk` gibt NewTOS
wortgleich zu beiden EmuTOS-Falcon-TC-Laeufen (mit/ohne Enhancer) zurueck.

## Pruefung, die gelaufen ist

* **Hosttests** `tos-api/tests/vdi_eddi_scrninfo.rs`, 6/6:
  volle 273-Wort-Antwort = Enhancer-Tabelle mit `[14] = 2` (virtuelles **und**
  physisches Handle, E6), Zaehler, nichts hinter `[272]`, PTSOUT unberuehrt,
  `d0 = 0`; Bitnummern bauen `pack_color16`-Woerter nach; `CONTRL[5] == 1`
  mit `INTIN[0]` ∈ {0, 1, 3, −1, 0x102} = `vq_extnd` wortgleich zu
  `CONTRL[5] == 0`; `vq_extnd(0/1)` unveraendert, `INTIN[0] == 2` ohne
  Sub-Opcode bleibt Modus 1; ungueltiges Handle schreibt nichts; Cookie
  `EdDI` = `0x00000001`.
* **QEMU raspi4b**: `SCRNINFO.NAP` per `#Z`-Autorun, Bericht
  `newtos-qemu-raspi4b/SCRNINFO.TXT`, seriell 0 `[EXC]`/PANIC, Programm endet
  mit rc 0, Desktop kommt danach hoch.
* **Basislinien** (nach der Aenderung gemessen):
  * Hosttests `cargo test -p tos-api -p tos-types -p xtask --no-fail-fast`:
    **970 / 0 / 1** (964 + 6 neue).
  * Frische Release-Bauten nach `cargo clean -p kernel -p tos-api --target
    aarch64-unknown-none-softfloat --release`, je Board einzeln (je
    „Compiling tos-api" + „Compiling kernel"): rpi4 **64 / 94**, qemu-virt
    **64 / 94** — unveraendert.
  * clippy `--features board-qemu-virt` und `board-rpi4`: je **80 / 125** —
    unveraendert; keine Meldung in `eddi.rs`/`cookie.rs`; die Testdatei ist
    clippy-sauber.
  * `sh tools/desk-drift-probe.sh`: **31** — unveraendert.
  * VDI-Referenztests (`tos-api/tests/vdi_*`) gruen (Teil der 970).
  * `make -C tosdump scrninfo-m68k` baut weiter (text 111376 wie vorher),
    `make host-test` ok.

## Was offen bleibt

1. **Abnahme durch den Tester.** Handoff: Hosttest-Binary
   `vdi_eddi_scrninfo`; Selbsttest `selftest_cookie` (Getcookie-Pruefungen
   unveraendert, Anzahl jetzt 5); QEMU-Lauf nach `method/` des Belegordners
   (`seed-scrninfo.sh`, `run-scrninfo.sh`, `fetch-report.sh`, `compare.py`).
   Erwartet: `COMPARE.TXT` „differing: 1" (`[14]`).
2. **Echte Pi 4** nicht gefahren; die Antwort haengt nur an `PLANES = 16`, das
   auf beiden Boards gleich ist.
3. **Doom** ist in diesem Schritt nicht angefasst (Auftrag); der EdDI-Weg in
   Doom ist der naechste PORT06-Schritt.
4. **Offene Fragen an den Lead**:
   * Markerwert `1` ist meine Wahl innerhalb von E2 (b) (ungleich 0, ungerade).
     Ein anderer ungerader Wert ginge genauso; bitte bestaetigen.
   * `CONTRL[4] = 272` (dokumentiert) statt 273 (geschriebene Woerter) und
     `d0 = 0`: beides von der Spec nicht ausdruecklich festgelegt, so gewaehlt
     wie oben begruendet.
   * Andere Planezahl als 16: die Spec leitet nur die 16-Plane-Antwort ab.
     NewTOS hat kein anderes Geraet (ADR-024); falls doch, bleibt 102/1/2 dort
     `vq_extnd` (alte Antwort) statt einer nicht abgeleiteten Tabelle.

## Nebenbefunde, nicht angefasst

* Mit dem Enhancer liefert `v_get_pixel` auf EmuTOS Falcon-TC `INTOUT[0] =
  Wort, [1] = FFFF`, ohne ihn `0000 / Wort` — NewTOS liefert das zweite (wie
  EmuTOS ohne Enhancer). Nur notiert.
* Spec §0 sagt noch „NewTOS tut dasselbe" (`vq_extnd` fuer 102/1/2) — seit
  dieser Umsetzung ueberholt; der Statuskopf vermerkt die Umsetzung, §0
  selbst habe ich als Researcher-Text nicht umgeschrieben.
* `docs/api/vdi/_dispatcher.md` nennt `CONTRL[5]` nur fuer Opcode 5/11; der
  EdDI-Fall 102 steht jetzt in `102-vq_extnd.md`, nicht dort.
