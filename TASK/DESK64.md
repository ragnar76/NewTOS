# DESK64 — Hintergrund-Dialog: zeigt EmuTOS eine Vorschau von Farbe und Muster?

Status: **OFFEN (Frage)** · Prioritaet: **P3** · Typ: Konformitaetsfrage — Befund des Anwenders auf echter Pi 4 (2026-10-16)

## Beobachtung

Beim Setzen des Desktop-Hintergrunds erscheint ein Dialog, in dem man Farbe
und Muster waehlt. NewTOS zeigt dabei **keine Vorschau** des Ergebnisses.
Der Anwender erinnert sich, dass EmuTOS dort eine Vorschau hat.

Ob das eine Abweichung ist, ist **nicht** belegt — erst die Quelle lesen.

## Auftrag (Researcher, keine Aenderung am Produktcode)

1. EmuTOS: den Dialog finden (vermutlich `desk/deskins.c` /
   `desk/deskmain.c`, Ressource in `desk/desk_rsc.c` bzw. der `.rsc`-Quelle)
   und belegen, ob und wie er eine Vorschau zeigt (welches Objekt, welche
   Funktion aktualisiert es bei Auswahl, `CONF_WITH_*`-Schalter im
   Falcon/TOS4-Profil, `docs/api/_emutos-config.md`).
2. NewTOS: denselben Dialog in `programs/desktop/` und den Ressourcen
   gegenueberstellen (Datei:Zeile beider Seiten).
3. Wenn moeglich die Referenz sichtbar machen (Hatari Falcon truecolor,
   `rscview` fuer die Dialogressource) und mit einem QEMU-Screendump von
   NewTOS vergleichen.
4. Ergebnis: Abweichung ja/nein mit Beleg; falls ja, Ursache (fehlender
   Code, fehlendes Objekt, Renderfehler in AES/VDI) und Fix-Vorschlag.

Quellen: `docs/api/desktop/`, `docs/analysis/`, EmuTOS unter `references/`.

[Gemeinsame Regeln](README.md) gelten.

## Ergebnis (2026-10-16, Researcher)

**Abweichung: ja.** EmuTOS hat `BGSAMPLE` (`desk_rsc.c:2334`) und zeichnet es pro Auswahl per `draw_fld` neu (`deskinf.c:866-891`). NewTOS baut einen eigenen Stepper-Dialog ohne Vorschau, Swatches und Window-Umschalter (`programs/desktop/src/inf.rs:3699-3808`). Ursache im Desktop, nicht in AES/VDI. Kein Bildbeleg erzeugt (nur Quelle). Details und Fix-Vorschlag: `docs/analysis/desk64-background-preview-2026-10-16.md`.
