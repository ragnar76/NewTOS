# EmuTOS / aktuelles NewTOS — Gegenüberstellung

Am 2026-09-09 unabhängig neu mit dem vorhandenen `tosdump/host/build/tosdiff` verglichen, nicht aus der mitgelieferten Ergebniszusammenfassung abgeschrieben. `make -C tosdump/host build/tosdiff` meldete den Vergleicher aktuell. Alle sechs Dumps wurden gegen ihre Tabellen validiert (Exit 0); alle drei Vergleiche wurden angenommen (Exit 1 = Unterschiede, kein Exit 2 = Verweigerung). Befehle stehen in den Rohdateien.

Die drei mitgelieferten `reference-tt`-BINs sind bytegleich mit den Originalen unter `tosdump/reference/`. SHA-256 und Größen der verwendeten Originale, aktuellen Dumps und Tabellen stehen in [manifest.json](evidence/manifest.json).

## Identität und Grenzen

| Eigenschaft | Atari TT | Raspberry Pi 4 |
|---|---|---|
| Referenz / Lauf | EmuTOS 1.4, gespeicherte Originale | aktuelle run1-pi4-Dumps |
| Bildschirm | 640×480 | 1920×1080 |
| Farbstifte / Planes | 16 / 4 | 256 / 16 |
| AES appl_init-ID | 0 | 2 |
| Kernel-SHA laut RUNINFO | nicht enthalten | 2fbf84626cd0b6fa6b44cecf6e8f8fadfe7e9272a938e779de34cc7d4c6b098f |
| Dumper laut aktuellem LOG | siehe Referenz-LOG | build 70ca, Format 2 |

Git HEAD laut Laufmanifest: `70caee212078b11ffa0a70afa1279bdd82532959`, aber 50 offene Pfade. Der Commit identifiziert den gemessenen Quellstand daher nicht vollständig. Der Kernelhash ist eine Manifestangabe; der Dump selbst enthält keine Kernelidentität. Zeitstempel stützen die Zuordnung nur indirekt. M04 erfasst diese Lücke. Die ältere Anleitung MESSRUN nennt 21:06, RUNINFO/README einen späteren Build; nicht zu einem vermeintlich widerspruchsfreien Nachweis zusammenziehen.

## Unabhängig ausgezähltes Ergebnis

| Gruppe / Tabelle | CRC | Fälle | Roh abweichend | Zusätzlich zu Globals/Handles abweichend |
|---|---|---:|---:|---:|
| AES r1 | 33C13C8F | 95 | 95 | 5 |
| VDI all | D135ADAF | 182 | 182 | 14 |
| Raster | 702EEA7D | 59 | 59 | 1 |

Der Rohvergleich enthält 570 AES-Global-Wortunterschiede (sechs pro Record). Nach separater Betrachtung dieser Globals bleiben 9 andere Wörter in 5 Fällen. VDI enthält zwei Handle-Wörter pro Record und 30 andere Wörter in 14 Fällen. Raster enthält zwei Handle-Wörter pro Record und 128 andere Zielwörter in einem Fall. Diese Statistik ist keine Fehleranzahl.

## AES: alle fünf Fälle

| Fall | Funktion | TT → Pi | Einordnung | Task |
|---|---|---|---|---|
| 155 | wind_get(0, WF_WXYWH) | w/h 640/461 → 1920/1061 | Gerätegeometrie, H minus 19 | [A01](A01.md) |
| 156 | wind_get(0, WF_CXYWH) | 640/480 → 1920/1080 | Gerätegeometrie | [A02](A02.md) |
| 157 | wind_get(0, WF_PXYWH) | 640/480 → 1920/1080 | Gerätegeometrie im Anfangszustand | [A03](A03.md) |
| 158 | wind_get(0, WF_FXYWH) | 640/461 → 1920/1061 | Gerätegeometrie, H minus 19 | [A04](A04.md) |
| 167 | wind_get(0, WF_OWNER) | ret 0 → 1 | Vorhandener Harness belegt Einführung nach EmuTOS 1.4; nicht allein aus AES-Version ableiten | [A05](A05.md) |

`wind_calc`, die alten Desktop-Slider-/OUT_LEN- und wind_find-Abweichungen sind in dieser Runde nicht erneut aufgetreten. Sie bekommen keine erfundenen Reparaturtasks.

### Globals nicht verschweigen

| Feld | TT | Pi | Prüfauftrag |
|---|---|---|---|
| global[2], ap_id | 0 | 2 | Identität zur appl_init-Rückgabe: [A06](A06.md) |
| global[3..4], ap_private | Wörter 0, 0x1143 | 0x117B, 0 | Endian-korrekt Werte 0x1143/0x117B; Konfigurationsbeleg erforderlich: [A07](A07.md) |
| global[10], ap_planes | 4 | 16 | Backendkonsistenz: [A08](A08.md) |
| global[11..12], ap_3resv | 0, 0xBE04 | 0x3340, 0x12 | Adressartiges Feld/ABI: [A09](A09.md) |

Die unterschiedliche Wortreihenfolge von ap_private erklärt nicht die Wertdifferenz 0x38. Der vorhandene Harness benennt unterschiedliche #Q-Hintergrundwerte; die konkret gemessene Konfiguration muss dies bestätigen. Nicht pauschal „maschinenabhängig“ markieren.

## VDI: alle vierzehn Fälle

| Fälle | Funktion / Wert | TT → Pi | Einordnung und Tasks |
|---|---|---|---|
| 33 | vq_extnd(0): max x/y, Farben, Palette | 639/479/16/4096 → 1919/1079/256/0 | Geräteprofil; [V01](V01.md), [V02](V02.md), [V03](V03.md) |
| 34, 250, 252 | vq_extnd(1): Planes, LUT, ggf. Clipgrenzen | 4/1 → 16/0; Vollbild 639/479 → 1919/1079 | Geräte-/Clipregeln; [V04](V04.md), [V05](V05.md), [V06](V06.md) |
| 42 | vsl_color(99) | 1 → 99 | Stift 99 nur auf TT außerhalb des Bereichs; [V07](V07.md) |
| 113 | vsm_color(99) | 1 → 99 | dieselbe Bereichsfrage, andere Funktion; [V08](V08.md) |
| 72 | vq_color(255) | -1 → 255 | unterschiedliche gültige Stiftbereiche; [V09](V09.md) |
| 54, 160, 161 | v_get_pixel Weiß | INTOUT[1] 0 → 0xFFDF | Unterschiedliche Rückgabeformate; zusätzlich bekannter Weiß-/Overlaybefund. [V11](V11.md) → [V12](V12.md) |
| 270, 272 | v_get_pixel gezeichnetes Schwarz | (15,1) → (0,0) | Palette versus Truecolor, nicht automatisch verlorene Zeichnung; [V10](V10.md) |
| 173 | vq_chcells | (30,80) → (67,240) | Zellen aus Bildschirm-/Fontmaßen; [V13](V13.md) |
| 274 | v_opnvwk Ausgabe | wie Fall 33 | dieselben Gerätemetriken über eigenen Einstieg; [V14](V14.md) |

Workstation-Handles stehen zusätzlich in allen Records und bleiben im Rohdiff; [V15](V15.md) prüft ihre Beziehungen statt dieselbe absolute Zahl auf beiden Maschinen zu fordern. Die sekundäre Sicht blendet exakt contrl[6] und contrl_before[6] aus. Der Comparator selbst vergleicht bereits keine als nicht vergleichbar deklarierten MFDB-Pointer-Slots.

**Fall 77 vqin_mode ist aktuell gleich.** Die zuvor gemessene 1/0-Differenz besteht in diesem Lauf nicht mehr. Kein neuer Reparaturauftrag.

## Raster: Fall 157

`vro_cpyfm`, Quelle (0,0)–(7,7), Ziel (0,0)–(31,15), 16-plane Speicherformen: 128 Zielwörter unterscheiden sich. Beispiel buf_b[0]: TT `0x774B`, Pi `0x445B`.

D-VDI-25 beschreibt unterschiedliche Zielanker/Richtungswahl bei übergroßem Zielrechteck. Die bestehende Entscheidung lautet dokumentiert und zurückgestellt; [R01](R01.md) bewahrt dies. Es gibt keinen gemessenen neuen PAT_FLAG-Unterschied (Fall 144 ist gleich). Screen-MFDB-Pfade sind in dieser Tabelle nicht gemessen: [R02](R02.md)–[R05](R05.md).

## Fehlende Messungen und QED

- Aktueller Pi-Fensterlauf: [M01](M01.md); vorhandener TT-Dump bleibt Vergleichspartner, alte NewTOS-Fensterdumps nicht als aktuell behandeln.
- Rechteckliste: [M02](M02.md) und [M03](M03.md). Tabelle vorhanden, beide aktuellen Messpartner fehlen. Danach [A13](A13.md)–[A15](A15.md).
- Sichtbare Text-/Clip-/XOR-Wirkung und Eingabe: [Q01](Q01.md)–[Q06](Q06.md). Der aktuelle Dump beweist keine Ursache für QEDs unsichtbares a.
- Alle weiteren ungemessenen Hauptfunktionen stehen einzeln im [Backlog](README.md#späterer-backlog-genau-eine-funktion-pro-messdesign-task). Das bedeutet nicht, dass ältere Audits oder Tests keine Aussage hätten; sie sind nur kein Record dieses Messpakets.

Gleiche Parameterblöcke sind kein Pixelvergleich. Nur die Rasterrecords enthalten hier tatsächliche Speicherergebnisse. Ein gefundener Unterschied öffnet erst nach Vertragseinordnung einen Reparaturauftrag.
