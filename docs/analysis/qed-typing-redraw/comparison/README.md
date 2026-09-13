# EmuTOS / NewTOS: direkter Messvergleich

Auswertung 2026-09-09 mit dem vorhandenen `tosdump/host/build/tosdiff`.
Keine neuen Hardwaremessungen, keine Änderungen an Dumps oder Falltabellen.
Die vollständigen Ausgaben enthalten jeweils den reproduzierbaren Befehl.
Alle sechs Vergleiche wurden angenommen (Exit 1 = Unterschiede; kein Exit 2 = verweigerter Vergleich).

| Vergleich | Fälle | Roh abweichend | Nach separat ausgewiesenen Globals/Handles |
|---|---:|---:|---:|
| AES r1, NEWTOS-AES.BIN | 95 | 95 | 74 |
| AES r1, NEWTOS-AES-r2.BIN | 95 | 95 | 5 |
| AES Fenster, NEWTOS-AES-r2wind.BIN | 78 | 78 | 5 |
| VDI all, NEWTOS-VDI-all-0906.BIN | 182 | 182 | 15 |
| Raster, NEWTOS-RASTER.BIN | 59 | 59 | 2 |
| Raster, NEWTOS-RASTER-fixed.BIN | 59 | 59 | 1 |

## Bedeutung der getrennten Felder

AES: sechs global-Wörter pro Fall (Indices 2,3,4,10,11,12), insgesamt 570 bzw. 468 Abweichungen. Diese separat auszuwerten bedeutet NICHT, sie als korrekt zu erklären. Darunter liegen Prozess-ID, Farbtiefe, reservierte/adressartige Werte und ap_private; die Rohwerte stehen in den vollständigen Logs. Insbesondere ap_private darf nicht pauschal als Maschinenunterschied abgetan werden.

VDI/Raster: ausschließlich contrl[6] und contrl_before[6] aus der kompakten Ansicht entfernt; dies sind die Workstation-Handles. Die vollständigen Dateien bewahren sie. Der Comparator lässt selbst bereits als nicht vergleichbar deklarierte MFDB-Zeigerfelder aus.

## AES: entscheidender Unterschied zur bisherigen Befunddatei

Mit NEWTOS-AES-r2.BIN stimmen die gemessenen wind_calc-Rahmenmetriken, Desktop-Slider-Abfragen, wind_find-Grenzfälle und OUT_LEN-Fälle überein. Die alte NEWTOS-AES.BIN zeigt diese Abweichungen noch. Die Verbesserung ist damit gemessen, nicht lediglich aus Reparaturmeldungen abgeleitet.

Die fünf verbleibenden r1-Fälle: 155–158 sind Bildschirmmaße (640×480 gegen 1920×1080, jeweils 19 Pixel Menüleiste); Fall 167 ist WF_OWNER (Rückgabe 0 gegen 1). Die Versionsfrage zu WF_OWNER muss getrennt von einem Implementierungsfehler beurteilt werden.

Fensterrunde: Fall 154 neuntes Fenster (9 gegen -1), 230 Rechteck vor Öffnen (0,0,0,0 gegen 10,20,300,200), 244/247 WF_OWNER, 253 vorheriges Rechteck nach Öffnen (30,40,320,240 gegen 10,20,300,200). Das beschreibt diese gespeicherten Dumps, nicht zwingend den heutigen Kernel.

## VDI

15 Fälle mit 31 weiteren unterschiedlichen Wörtern: 33,34,42,54,72,77,113,160,161,173,250,252,270,272,274. Exakte Werte: vdi-all-without-handles.txt.

Auflösung, Farbtiefe und verfügbare Farbindices erklären einen Teil der Unterschiede. Auffällig bleiben insbesondere vqin_mode (Fall 77: 1 gegen 0) und v_get_pixel (54/160/161: 0 gegen 0xFFDF; 270/272: Index 15 gegen 0 und Farbwert 1 gegen 0). Ein Dump allein belegt deren Ursache nicht. Spätere Reparaturen sind ohne erneuten Hardwarelauf nicht in diesen Dateien sichtbar.

## Raster

Der ursprüngliche Dump unterscheidet sich zusätzlich zu Handles bei Fall 144 (PAT_FLAG|3, 64 Zielwörter) und 157 (abweichende Zielausdehnung, 128 Zielwörter). Im fixed-Dump ist Fall 144 identisch; Fall 157 bleibt mit denselben 128 Unterschieden. Beispiel buf_b[0]: EmuTOS 0x774B, NewTOS 0x445B. Ob dies ein Fehler oder eine Referenz-/Versionsabweichung ist, ist durch den Vergleich allein nicht entschieden.

## Aussage zum QED-Symptom

Die Messungen belegen keine Ursache für den unsichtbaren getippten Buchstaben. Parameterblock-Gleichheit von Zeichenaufrufen beweist keine Pixelgleichheit; die Raster-Fälle messen dagegen tatsächlich Speicherergebnisse. Die Rechteckiteration WF_FIRSTXYWH/WF_NEXTXYWH und der konkrete interaktive QED-Ablauf sind durch diese Tabellen nicht abgedeckt. Die alte Rahmenmetrik-Differenz taugt angesichts NEWTOS-AES-r2.BIN nicht als aktueller Ursachenbeleg.
