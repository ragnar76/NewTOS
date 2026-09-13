# Messrunde 2026-09-09 — Material zur Prüfung

Ein Satz Dumps vom aktuellen NewTOS-Kernel auf realer Raspberry Pi 4, gegen die
vorhandenen EmuTOS-1.4-Dumps eines echten Atari TT. Alles, was zum Nachprüfen
nötig ist, liegt in diesem Verzeichnis; nichts hier muss anderswo gesucht werden
außer den Binärdateien, die über ihre SHA-256 benannt sind.

**Anlass:** die bisherigen NewTOS-Dumps stammen vom 31.08. und 06.09. Ein
Vergleich gegen den *älteren* von zwei Läufen desselben Tages hat am 09.09. fünf
Abweichungsklassen erzeugt, die längst repariert waren. Diese Runde stellt einen
Stand her, der eindeutig datiert und identifiziert ist.

---

## Was hier liegt

```
RUNINFO.TXT              Lauf-Metadaten, beim Bestücken der Karte erzeugt:
                         Commit, SHA-256 aller Binärdateien, Tabellen-CRCs,
                         und welche Diagnose-Instrumente ABGESCHALTET sind
MESSRUN.TXT              die Anleitung, die auf der Karte lag (drei Durchgänge)
run1-pi4/                Durchgang 1, die drei Dumps + ihre Läufe-Logs
cases/                   die fünf Falltabellen, wie sie auf der Karte lagen
reference-tt/            die EmuTOS-1.4-Dumps vom Atari TT (Original: tosdump/reference/)
diff/                    Validierung und Vergleiche, mit dem jeweiligen Befehl im Kopf
```

Die Binärdateien (Kernel, drei Dumper) sind **nicht** kopiert, um keine zweite
Wahrheit zu schaffen. Sie liegen in `../tosdump/deploy-pi4/` und sind in
`RUNINFO.TXT` über SHA-256 eindeutig benannt.

---

## Der gemessene Stand

| | |
|---|---|
| Kernel | `2fbf84626cd0b6fa6b44cecf6e8f8fadfe7e9272a938e779de34cc7d4c6b098f`, 778960 Bytes |
| Dumper | `build 70ca` (steht in jedem `.LOG`) |
| Git HEAD | `70caee212078b11ffa0a70afa1279bdd82532959` |
| Arbeitsbaum | 50 offene Pfade gegenüber HEAD — der Commit beschreibt den Kernel **nicht** allein |
| Gerät | 1920×1080, 256 Farben, 16 Planes (`DEV_TAB[0],[1],[13]`, `ap_planes`) |
| Referenz | Atari TT, EmuTOS 1.4, 640×480, 16 Farben, 4 Planes |

Abgeschaltet im gemessenen Kernel, am Abbild geprüft und nicht bloß behauptet:
QED-Tracer (`QED_SVC_TRACE = false`, 0 Treffer auf `QEDTRACE` im Abbild),
`usb-diag` (0 Treffer auf `USBDIAG`/`Supported-Protocol`/`PORTSC`),
`gemdos-trace`, `aes-trace`, `initinfo-diag`. Enthalten und gewollt: der
USB-Fix im Normalbuild.

### Woher bekannt ist, dass dies der gemessene Kernel war

Die Dumps tragen keine Kernel-Identität — das ist eine bekannte Lücke, siehe
unten. Der Nachweis läuft deshalb über die Uhr: die Pi hat keine RTC und stellt
ihre Zeit aus der **Compile-Zeit des Kernels**. Der hier genannte Kernel wurde um
21:24 übersetzt, die drei Dumps tragen 21:30:46, :52 und :56 — 6,5 Minuten
Laufzeit nach dem Boot. Der Vorgänger (mit QED-Tracer) wurde 21:06 übersetzt, und
seine Sitzung ist durch `QEDTRACE.LOG` auf 21:09:30 datiert.

---

## Ergebnis

| Vergleich | Fälle | roh | ohne `global[]` bzw. Workstation-Handle |
|---|---:|---:|---:|
| AES r1 | 95 | 95 | **5** |
| VDI all | 182 | 182 | **14** |
| Raster | 59 | 59 | **1** |

Die abgezogenen Felder sind **nicht** für richtig erklärt, sondern getrennt
ausgewiesen: bei AES sechs `global[]`-Wörter je Fall aus `appl_init`
(`ap_id`, `ap_private`, `ap_planes`, `ap_3resv`), bei VDI/Raster `contrl[6]` und
`contrl_before[6]`, der Workstation-Handle, den der Dumper selbst hineinschreibt.
Die Rohwerte stehen vollständig in `diff/01-*-full.txt`.

**AES r1, die fünf:** 155–158 sind Bildschirmmaße (beide Maschinen ziehen
dieselben 19 px Menüleiste ab: 461 = 480−19 gegen 1061 = 1080−19), 167 ist
`WF_OWNER` auf Handle 0 (EmuTOS 0, NewTOS 1) — laut Fallkommentar „AES 3.30 and
later only", also Versionsfrage oder Fehler, **hier nicht entschieden**.

**VDI, die vierzehn:** 33, 34, 42, 54, 72, 113, 160, 161, 173, 250, 252, 270,
272, 274. Gegenüber dem Lauf vom 06.09. ist **Fall 77 (`vqin_mode` string)
verschwunden** — die `chkkbd`/`vsin_mode(4,2)`-Reparatur vom 06.09., bisher nur
im Host-Harness belegt, jetzt auf echter Hardware.

**Raster, der eine:** Fall 157, `vro_cpyfm` mit zu großem `PTSIN[6..7]` —
D-VDI-25, am 06.09. dokumentiert und bewusst zurückgestellt.

---

## Was ein Auditor prüfen soll

1. **Bindung.** `diff/00-validate.txt` zeigt für alle sechs Dumps, dass sie ihre
   Falltabelle über die CRC binden. Der Vergleicher weist zwei Dumps mit
   verschiedener Tabellen-CRC ab; ohne diesen Nachweis ist jeder Vergleich wertlos.
2. **Reproduzierbarkeit.** Jede Datei in `diff/` trägt den Befehl im Kopf. Sie
   lassen sich mit `tosdump/host/build/tosdiff` wörtlich nachfahren.
3. **Die Abzugsregel.** Ob das Weglassen der `global[]`-Wörter und der
   Workstation-Handles in der kompakten Sicht (`diff/02-*`) gerechtfertigt ist,
   und ob die Rohdateien das tragen. Insbesondere `ap_private` darf nicht
   pauschal als Maschinenunterschied gelten.
4. **Die Einordnung der 20 Abweichungen.** Für jede: Geräteunterschied,
   Versionsunterschied, Fehler oder ungeklärt. Diese Runde nimmt sie **nicht**
   vor; sie liefert nur die Werte.
5. **Die Grenze der Methode.** Gleichheit von Parameterblöcken ist nicht
   Gleichheit von Pixeln. Nur die Raster-Gruppe misst tatsächlich Speicherinhalt.

## Was diese Runde ausdrücklich nicht abdeckt

- **`evnt_multi`, `graf_mouse`** und 71 weitere AES-Funktionen: in keiner
  Falltabelle. „Nicht geprüft" ist etwas anderes als „keine Abweichung gefunden".
- **`WF_FIRSTXYWH` / `WF_NEXTXYWH`**, die Rechteckliste: dafür ist
  `cases/aes-r2b-rlist.tsv` neu geschrieben (15 Fälle, 46 Preludes, CRC
  `B4CC4A5A`), aber **weder auf der Pi noch auf dem TT gelaufen**. Ohne den
  TT-Lauf gäbe es dazu auch keinen Vergleichswert.
- **Durchgang 2** (`aes-r2-wind.tsv`, 78 Fälle) ist ebenfalls noch offen; der
  TT-Dump dazu existiert (`reference-tt/` enthält ihn nicht, er liegt als
  `tosdump/reference/EMUTOS14-AES-r2wind.BIN`).

## Bekannte Lücke im Verfahren

Kein Dump trägt die Identität des Kernels, gegen den er gemessen wurde. Die
`BUILD_ID` in jedem `.LOG` (`70ca`) ist `git rev-parse HEAD | cut -c1-4` zum
Zeitpunkt des **Dumper**-Baus — sie benennt den Commit, nicht den Arbeitsbaum,
und bei 50 offenen Pfaden beschreibt sie den gemessenen Kernel nicht. Hier ist
das über die Compile-Zeit-Uhr aufgelöst (oben), aber das ist ein Indiz, kein
Feld im Datensatz. Der saubere Weg: der Kernel meldet seine Build-Identität
(Cookie-Jar über `Ssystem`/`S_GETCOOKIE`, ADR-012), der Dumper schreibt sie ins
`.LOG` — kein Formatbruch, weil das `.LOG` Freitext ist und die Bindung über die
CRC läuft.
