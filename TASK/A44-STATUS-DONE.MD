# A44 — Quittung

**Status: ERLEDIGT** · 2026-09-19 · Verhaltensfix `graf_mouse` (78), hostverifiziert

## Was getan wurde

1. `tos-api/src/aes/graf.rs` `graf_mouse`: der Arm `0..=7 => set_preset` und
   `_ => {}` sind zu **einem** `default:`-Arm geworden, wie `gr_mouse`
   (`references/emutos/aes/gemgrlib.c:419-455`, gepinnter Commit `1a7484ab`):
   Nummer ausserhalb `ARROW..=OUTLN_CROSS` → `ARROW` (`:426-429`), dann
   `set_preset` (= das `gsx_mfset(maddr)` `:454`, in das der Arm faellt).
   `M_OFF`/`M_ON`/`USER_DEF`/`M_SAVE`/`M_RESTORE`/`M_PREVIOUS` unveraendert.
   Neue Konstanten `ARROW = 0`, `OUTLN_CROSS = 7` (`aesdefs.h:424-431`).
2. Spec `docs/api/aes/window-graphics.md` § `graf_mouse`: der Satz
   „out-of-range preset falls back to ARROW" ist praezisiert — jede nicht
   benannte Nummer (8…254, ≥ 261, negativ) ist der `default:`-Arm, wird auf
   `ARROW` geklemmt **und installiert**; es gibt keinen „Cursor bleibt"-Pfad.
3. Neuer Hosttest `tos-api/tests/aes_graf_mouse_invalid.rs`
   (Dispatcher-Harness `aes_reference/harness.rs`, LP64). Beobachtungskanal,
   weil weder AES noch VDI die Form zuruecklesen: `v_hide_c` (VDI 123),
   Quadrat um die Zeigerposition (`graf_mkstate`) in eine Farbe legen, die
   kein Sprite malt, `v_show_c(0)` (VDI 122), dann das 48×48-Bild gegen das
   Bild vergleichen, das `PRESET_FORMS[n]` (Hotspot, Maske → Stift 0, Daten →
   Stift 1) erzeugen muss. Vorab: die acht Soll-Bilder sind paarweise
   verschieden und jede gueltige Nummer 0..7 zeigt genau ihre Form. Befund:
   `BUSY_BEE` (2) setzen, dann `graf_mouse(n)` fuer
   `n ∈ {99, -1, 8, 254, 261, MAX, MIN}` → aktive Form `ARROW` (0);
   `M_PREVIOUS` danach bringt die Sanduhr zurueck (der Install hat
   `gl_prevmouse` fortgeschrieben). Rueckgabe stets 1.

## Pruefung

* Test vor Fix rot am Befund A-3: nach `graf_mouse(99)` war weiter Form 2
  aktiv (`left: Some(2)`, `right: Some(0)`); die Kanalpruefung (acht Presets
  unterscheidbar, 0..7 → eigene Form) ging vorher durch. Nach dem Fix gruen.
* `cargo run -p xtask -- test`: **633** gruen / 0 rot / 1 ignoriert
  (Grundlinie 631 + A43 + A44).
* `cargo xtask build --board rpi4 --release` und `--board qemu-virt --release`
  (je frisch): Exit 0, `tos-api` 64 / `kernel` 94 Warnungen, Delta 0.
  Clippy `-p kernel --lib` je Board: `tos-api` 81 / `kernel` 125, Delta 0.
* Kein QEMU-Lauf: kein Zeichenpfad ausser dem Cursor-Sprite selbst betroffen,
  gueltige Nummern unveraendert (im Test belegt). Auf echter Pi 4 nur
  sichtbar, wenn ein Programm eine unbekannte Nummer schickt — keine offene
  Hardwareabnahme.

## Nebenbefunde (unberuehrt)

* `M_SAVE`/`M_RESTORE`/`M_PREVIOUS` (`CONF_WITH_GRAF_MOUSE_EXTENSION`) sind
  in NewTOS vorhanden (`vdi::mouse::save_form/restore_form/previous_form`);
  `M_SAVE` ist ein globaler Slot statt `rlr->p_mouse` je Prozess
  (Single-Tasking, im Modul dokumentiert).
* `graf_mouse` tut ohne Raster-Backend (`host().raster() == None`) nichts
  — im Kernel immer vorhanden, hier nur der Vollstaendigkeit halber.
* Im 4-Planes-Referenzprofil des Harness bleibt die Pseudo-Palette
  ungeseedet (`init_wk` seedet nur bei `planes > 8`), sodass der
  `MemBackend` Stift 0 und 1 auf dasselbe Geraetewort abbildet; der Test
  identifiziert die Form ueber Umriss und Hotspot und prueft die
  Unterscheidbarkeit vorab.
