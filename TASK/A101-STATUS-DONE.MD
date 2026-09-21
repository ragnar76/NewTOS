# A101 — STATUS: DONE

Bearbeitet 2026-09-21, Branch `desk-rebuild`, **nicht committet**. Reine
Dokumentation, keine Codeänderung.

## 1. `docs/HARDWARE-PROFILE.md` § 3.1 (+ zwei weitere Kopien im selben File)

§3.1 komplett gegen `kernel/src/drivers/mod.rs:31-33` (`BOOT_WIDTH`/
`BOOT_HEIGHT`, negoziiertes 1920×1080-Default auf beiden Boards) und
`kernel/src/vdi_backend.rs:33,40` (`NUM_COLORS=256`, `PLANES=16`)
gegengelesen und korrigiert, mit datierten Korrekturblöcken (Originaltext
sichtbar belassen, A72/A86-Muster):

- Geometrie: war „640×480, to be changed (ADR-017 pending)" → jetzt „live
  seit 2026-07-16" (`c95ad37`/`5ffaab0`, ADR-017 rev 2).
- Farbmodell: war „16-colour, 4-plane (unchanged by ADR-017)" → jetzt
  Falcon-Truecolor, 256 Stifte / 16 Planes (gelandet 2026-07-22/23,
  `09c9ddc`..`11ca57e`, jetzt ADR-024).
- Palette-Bullet: „16 CLUT entries" → 256 Stifte.
- `DEV_TAB`/`INQ_TAB`-Bullet: alte Werte (`[13]=16`, `[39]=16`, `INQ[4]=4`,
  `INQ[5]=1`) → `[13]=256`, `[35]=1`, `[39]=0`, `INQ[4]=16`, `INQ[5]=0`;
  dieselbe Pixelgrößen-Falschaussage wie Mismatch #2 (unten) mitkorrigiert:
  `DEV_TAB[3]/[4]` sind gepatcht (278/278 bei 1920×1080), nicht ROM-372.

Zwei weitere Kopien derselben Aussage **im selben File** gefunden (nicht im
engen Ticket-Wortlaut „§3.1" genannt, aber direkter Widerspruch geblieben
wäre) und mitkorrigiert, mit eigenem Korrekturvermerk:

- §3.9-Tabelle: „replaced by the single 640×480×16 framebuffer mode".
- §4 CONF-Matrix: `CONF_WITH_VDI_16BIT` / Videl = **off** (jetzt: **on**,
  belegt gegen `docs/analysis/aes-audit/emutos-config-profile.md:150,314`).

**Bewusst unverändert gelassen:** Zeile ~136 „1280×800 (16 colours)" im
Getrez/Setscreen-Absatz — zitiert dort die ADR-017-Formulierung zur offenen
Rez-Mapping-Frage, kein Fakt über das aktuelle Farbmodell, außerhalb des
Belegtrios dieses Tickets.

## 2. `100-v_opnvwk.md` Mismatch #2 vs. D-VDI-7

Mismatch #2 als **aufgelöst** markiert (gleiches Muster wie Mismatch #4,
A86): der Code patcht `dev[3]`/`dev[4]` tatsächlich
(`tos-api/src/vdi/workstation.rs:193,197-198`, `get_std_pixel_size` → 278/278
bei 1920×1080), die alte „never patched, ROM 372"-Behauptung war die
veraltete. D-VDI-7 (`docs/api/vdi/_discrepancies.md`) hatte recht. Zwei
vorgelagerte Stellen im selben Dokument, die dies noch als „nicht
re-verifiziert"/„unresolved" auswiesen (die Wertetabelle und der
Absatz direkt darunter), mitkorrigiert.

## 3. ADR-024 (neu, `docs/MEMORY.md`)

Nächste freie Nummer nach ADR-023. Datum der Entscheidung 2026-07-22
(`09c9ddc`, TC1) / 2026-07-23 (`11ca57e`, TC1-TC5 komplett), rückwirkend
dokumentiert 2026-09-21. Entscheidung: Falcon-Truecolor-Workstation
(16-Bit-RGB565-Gerätepixel über 256 Stiften/16 Planes, physischer
Pi-Framebuffer bleibt 32-bpp XRGB8888 als Interna). Begründung:
Referenzprofil Falcon030/TOS4 fordert `CONF_WITH_VIDEL` +
`CONF_WITH_VDI_16BIT` = 1/1 (`docs/analysis/aes-audit/emutos-config-profile.md:
80,150,314`); EmuTOS kennt kein 24-/32-Bit-VDI (`vdi_defs.h:22`,
`bios/videl.c:320-341`). Konsequenzen: 256 Stifte, `fd_nplanes=16`, D-VDI-7
erfüllt; Memory-MFDB-Wireformat wechselt contract-seitig auf 16 Bit/Pixel.
Verworfene Alternative: 16-Farben-CLUT-Modell — Kostenargument ist der
fehlende Freiraum oberhalb der festen GEM-16 (Colour-Icons,
CONF_WITH_COLOUR_ICONS), **nicht** ein defekter `init_3d`-Zweig (bei 16
Stiften war `ws_ncolors>LWHITE(8)` bereits erfüllt — als Ableitung markiert,
nicht als Beleg behauptet); TeraDesk-Colour-Icon-Kollision unter dem alten
Modell ist ebenfalls als unverifizierte Annahme gekennzeichnet, nicht als
Messung. Ein 24-/32-Bit-VDI-Gerätepixel wurde als „erfundenes" EmuTOS-Verhalten
verworfen (CLAUDE.md Regel 1).

Platzhalter `[ADR-0xx truecolor VDI]` in `102-vq_extnd.md:198` durch
`ADR-024` ersetzt; dieselbe Platzhalter-Erwähnung in `_raster.md:530-535`
(A86s eigener Korrekturblock, der das Fehlen des ADR vermerkt hatte) mit
einem „Resolved (A101)"-Zusatz versehen.

## Geprüft, nicht verändert

- `docs/api/_discrepancies.md` D-VDI-7: bereits korrekt, unverändert.
- `docs/STATUS.md`: laut Auftrag nicht angefasst.
- Kein `git checkout/stash/reset`, nichts committet, nichts geflasht.

## Offen / nicht Teil dieses Tickets

- ADR-017 Punkt 5/6 (Switch-UI, `Getrez`/`Setscreen` Klassik-Rez-Mapping)
  bleiben offene Lead-Entscheidungen, unverändert durch diese Korrektur.
- Die TeraDesk-Colour-Icon-Kollisionsannahme in ADR-024 ist nicht
  nachgemessen (siehe ADR-024 „Alternatives considered").
