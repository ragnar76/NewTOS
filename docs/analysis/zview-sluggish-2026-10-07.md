# zView „träge“ auf der Pi — wohin geht die Zeit? (2026-10-07)

Anlass: User auf der echten Pi 4, Kernel 8f2a3b3b (HEAD 0b2e05a: FS08, FS09, ZVIEW02):
„zView geht, aber insgesamt träge — man wartet ständig, der Browser ist langsam und
hängt hier und da.“ Bilder selbst laden inzwischen deutlich schneller.

Lauf, Methode und Rohdaten: `measurements/measurement-2026-10-07-zview-perf/RUNINFO.TXT`.
Messwerkzeug ZVPROF (Scratch-Patch, nicht im Produkt):
`measurements/measurement-2026-10-07-zview-perf/method/zvprof.rs`.

**Vorbehalt:** alle Zeiten unten stammen aus QEMU raspi4b (TCG). Sie sind nur
untereinander vergleichbar. Zählwerte (Aufrufe, Seiten, Pixel, Pexec) sind
deterministisch und übertragbar. Was das auf der Pi in Millisekunden heißt,
entscheidet der Pi-Lauf (Paket unten).

## Ergebnis in Kürze

| # | Ursache | Wann | QEMU-Messung | Status |
|---|---|---|---|---|
| 1 | Speicherschutz (memprot) schreibt bei jedem Pexec/Mshrink/Mxalloc die Seitentabellen der **ganzen** TPA neu (~916 MB = ~235 000 Seiten pro Durchgang) | zView-Start (10 Codecs) | 1 381 ms von 1 607 ms EL1-Arbeit; 17,4 Mio. Seiten; Mauszeiger 1,59 s eingefroren | gemessen; NewTOS-eigen, nicht EmuTOS |
| 2 | Jeder Klick in zView wird erst nach Ablauf des Doppelklick-Fensters gemeldet | jeder Klick | Doppelklick 260–275 ms, Einzelklick 200–214 ms, Rollpfeil/Schließfeld 200–215 ms | gemessen; **EmuTOS-treu** |
| 3 | Vorschaubilder: zView dekodiert bei **jedem** Ordnerbesuch jedes Bild neu, im Stück, ohne AES-Aufruf | Ordner öffnen | je Foto 2048×1536: EL0 ~63 ms + Fread ~100 ms (erster Besuch); 8 Lücken à 154–172 ms ohne Mausbewegung | gemessen; Pi-Wert offen |
| 4 | Software-Zeichnen bei 1920×1080×32 | jedes Neuzeichnen | je Ordneransicht ~46–48 Mio. Pixel `v_gdp`-Musterfüllung; in QEMU billig | Pi-Wert offen |
| 5 | Maus nur in `pump_input` (USB gepollt, nur im AES) | während 1 und 3 | = die Lücken oben | strukturell, gemessen |
| 6 | Einzelne `evnt_multi`-Rückkehr mit 34–45 ms „Arbeit“ ohne Pixel | bei Fensterelement-Klicks | 3× im hd-Lauf | **ungeklärt** (PLAUSIBEL: Warteschleife ohne sleep/pump) |

Widerlegt bzw. ausgeschlossen:

- **TPA-Nullen** (Verdacht `loader.rs:554-556`): Null-Phase 0 ms; gelöscht werden nur
  67–80 KB (Bild-Span), weil `PF_STANDARD` `PF_FASTLOAD` enthält
  (`tos-types/src/basepage.rs:51`). Die 41–46 ms je Pexec aus FS08 U4 sind memprot.
- **Pexec je Bild/Vorschau:** 0 Pexec während des Browsens. Die 30 Pexec
  (10 Codecs × Modus 5/3/4) laufen einmal beim Start.
- **memprot bei Malloc/Mfree im Browser:** 3 ms pro Ordnerbesuch (Blöcke klein).
- **AES-Polling-Overhead:** zView pollt mit 20-ms-Timer (`main.c:141` `TimerInit(20)`),
  ~0,1–0,25 ms EL1-Arbeit je Rückkehr — vernachlässigbar.
- **Bildschirm-zu-Bildschirm-Blit beim Rollen:** 4 Pfeilklicks = 18 ms EL1 gesamt.

## 1. memprot-Refresh über die ganze TPA (dominant beim Start)

Gemessen (hd seg 2, `checks/hd-PROFILE.TXT`), je Codec:

```
PEXEC mode 3 bmp.nld total 58 ms: alloc 25 zero 0 (69 KB of TPA 916860 KB) read 6 reloc 0
      basepage 0 icache 0 slot 12 memprot-sync 49 ms
PEXEC mode 4  total 51 ms: memprot-sync 50 ms
Mshrink 23 ms (memprot 23 ms)          PEXEC mode 5 6 ms (memprot 5 ms)
```

Ursache im Code: die TPA ist (EmuTOS-treu, `bdos/proc.c` `alloc_tpa`) der größte freie
Block (`tos-api/src/bdos/process/arena.rs:129-149`). Jede Rechteänderung an einem Block
ruft sofort `touch(lo, hi)` + `sync()` über den **ganzen** Block
(`memprot.rs` `on_alloc`/`on_freed`/`set_mode`/`on_owner_change`, ab Zeile 226), und
`refresh` (`memprot.rs:587`) rechnet jede Seite jedes betroffenen 2-MiB-Bereichs für
alle lebenden Slots neu. Ein Codec von 69 KB bewegt so mehrmals ~916 MB. `Mshrink`
gibt danach den Rest zurück — noch ein Voll-Durchgang (`mem/traps.rs` `mshrink`,
`touch` + `sync` über `m_len`).

EmuTOS hat keinen Speicherschutz; das ist NewTOS-eigene Arbeit (ADR „Speicherschutz
nach FreeMiNT“). FreeMiNT kürzt genau diesen Fall ab: `mark_region` prüft zuerst, ob
die globale Modustabelle für alle betroffenen Seiten schon „kompatibel“ ist, und tut
dann für Nicht-Eigentümer nichts (`references/freemint/sys/arch/mprot040.c:1102-1130`).

Fix-Optionen (Developer, nach Pi-Messung priorisieren):

1. Refresh nur dort, wo sich Rechte tatsächlich ändern (FreeMiNT-Shortcut
   `mprot040.c:1102-1130`): ein freier und ein frisch allozierter Bereich desselben
   Eigentümers liefern meist identische Deskriptoren.
2. Gleichförmige 2-MiB-Bereiche als ein Block-Deskriptor statt 512 L3-Einträgen
   (`mmu.rs` kennt `allow_blocks` bereits für `map_shared`).
3. `touch`/`sync` sammeln: innerhalb eines Pexec (alloc → set_mode → set_owner →
   install → launch) nur einmal am Trap-Ende synchronisieren statt in jedem Schritt.

**Stand 2026-10-07 (MEMPROT-FAST):** Optionen 1 und 3 umgesetzt (Anwender-Auftrag
„1+2“), Option 2 nicht. QEMU, gleicher hd-Lauf: memprot beim zView-Start 1331 →
549 ms, Mausstillstand 1524 → 787 ms; Pexec 3/4 je 17–18 statt 47–52 ms, Pexec 5
0 statt 5 ms, Mshrink 160 statt 242 ms (11×). Der Rest sind echte
Deskriptoränderungen (die ~916-MB-TPA wechselt zweimal den Besitzer) — die nimmt
erst Option 2 weg. `measurements/measurement-2026-10-07-memprot-fast/RUNINFO.TXT`.

Pi entscheidet: Kosten pro Seite auf dem A72. Bei 10 ns/Seite wären es ~0,2 s, bei
50 ns ~0,9 s Mausstillstand beim zView-Start (plus erneut bei jedem anderen Programm-
oder LDG-Start).

## 2. Klick-Latenz = Doppelklick-Fenster (EmuTOS-treu)

zView wartet mit `evnt.bclick = 258` (0x102: bis zu 2 Klicks,
`sdk/zview/.../zview/main.c:143`). Damit ist `gl_bpend > 0`, und **jeder** Druck öffnet
das Doppelklick-Fenster; gemeldet wird erst, wenn es schließt:
`b_click` (`references/emutos/aes/geminput.c:112-146`): Fenster `gl_bdely = gl_dclick`,
+3 Ticks je weiterem Klick; Default `ev_dclick(3)` = `gl_dcrates[3]` = 220 ms
(`aes/gemevlib.c:33`, `aes/geminit.c:708`). Gemessen: Einzelklick 200–214 ms,
Doppelklick 260–275 ms (11 + 3 Ticks à 20 ms), Fensterelemente 200–215 ms bis zur
`WM_*`-Nachricht (der Control Manager handelt erst nach dem Fenster, ZVIEW02).

Das ist **kein Fehler**, sondern die EmuTOS-Semantik, 1:1 nachgebaut (ZVIEW02/A111).
Ein Atari mit EmuTOS und zView zeigt dieselbe Verzögerung. Für das Gefühl
„ständig warten“ ist es trotzdem ein Hauptbeitrag: jede Aktion in zView, auch jeder
Rollpfeil-Klick, kommt ~0,2 s verzögert.

Optionen ohne Abweichung von EmuTOS: Doppelklick-Geschwindigkeit im Desktop höher
stellen (`#E`-Zeile, niedrige Bits → `ev_dclick`, `aes/geminit.c:422-425`; Stufe 4 =
165 ms). Eine Verkürzung im Kernel wäre eine Abweichung und bräuchte eine
User-Entscheidung.

## 3. Vorschaubilder

Erster Besuch von SAMPLES mit 8 Fotos (hd seg 5): 8 Lücken ohne Input-Pumpe à
154–172 ms, je EL0 ~63 ms (Dekodieren/Skalieren in zView/Codec) + Fread ~100 ms
(QEMU-SD). Zweiter Besuch (seg 13): EL0 513 ms (zView dekodiert erneut, kein
Vorschau-Cache in zView), Fread 4 ms (FS09-Cache greift). Auf der Pi zählt: Dekodierzeit
eines echten Fotos auf dem A72 (EL0) plus Lesezeit beim ersten Besuch. zView ruft
zwischen zwei Bildern das AES auf, also friert die Maus pro Bild, nicht pro Ordner.

Fix-Optionen: keine im Kernel für EL0-Rechenzeit (die App ist unverändert zu lassen,
ADR „Ported app stays unmodified“). Die Mausstarre selbst gehört zu Punkt 5.

## 4. Zeichnen bei 1920×1080×32

In QEMU billig (gesamter hd-Lauf: `fill_pattern` 127 Mio. Pixel in 189 ms, Cache-Clean
148 MB in 43 ms; Bildfenster x01 `vro_cpyfm` 1,86 Mio. Pixel in 11,6 ms). QEMU hat
keine Caches, `dc cvac` kostet dort anders (GUI06). Pro Ordneransicht ~46–48 Mio.
Pixel Musterfüllung über `v_gdp` (WinDom-Fensterhintergrund). Auf der Pi offen; der
Pi-Lauf liefert `v_gdp`/`fill_pattern`/`dcache clean` je Schritt.

## 5. Maus nur im AES

`usb::poll` läuft nur aus `pump_input` (`kernel/src/host_env.rs:87-121`,
`kernel/src/drivers/usb/mod.rs:9-11`); während EL0 rechnet oder ein langer Trap läuft
(IRQs maskiert), bewegt sich der Zeiger nicht. Sichtbar genau in 1 (1,59 s) und 3
(~160 ms je Bild). Auf dem ST bewegt der IKBD-Interrupt den Zeiger immer — das ist
Hardware-Verhalten, kein API-Vertrag (Altitude-ADR), aber spürbar. Fix-Option:
USB-HID per Timer-IRQ (200 Hz) pollen und den Cursor dort bewegen; größere Arbeit.

## 6. Ungeklärt: `evnt_multi` mit 34–45 ms „Arbeit“

3 Rückgaben im hd-Lauf (seg 4, 5, 14), jeweils ohne gezeichnete Pixel, um Klicks auf
Fensterelemente. Wahrscheinlich eine Warteschleife im Control Manager/Doppelklick-Pfad,
die weder `sched::sleep` noch `pump_input` im 2-ms-Takt durchläuft und deshalb vom
Profiler als Arbeit gezählt wird. Nicht weiter verfolgt (Scope); der Pi-Lauf zeigt, ob
es dort relevant ist (Ereignis `LONG aes 0x19`).

## Was der Pi-Lauf entscheidet

Paket: `measurements/measurement-2026-10-07-zview-perf/bin/kernel8.img` +
`bin/LIESMICH.TXT` (Szenario S0–S11 mit Stoppuhr und 5-s-Pausen; jede Pause schließt
einen Abschnitt in `C:\PROFILE.TXT`). Auswertung mit `method/zvprof-sum.py`.

| Frage | Abschnitt / Zahl in PROFILE.TXT |
|---|---|
| Wie lange steht die Maus beim zView-Start, wie viel davon memprot? | S1: `GAP`, `memprot sync … refresh … pages`, `PEXEC … memprot-sync` |
| Wie teuer ist Zeichnen bei 1920×1080 auf dem A72? | S2/S3/S9: `v_gdp`, `fill_pattern`, `dcache clean`, `vro_cpyfm` |
| Wie lange dekodiert zView ein echtes Foto? | S3/S9: `EL0 run`, `GAP … EL0 … ms` |
| Wie viel ist Kartenlesen beim ersten Besuch? | S3 vs S9: `Fread` work |
| Klick-Latenz auf echter Hardware | jede Stufe: `clicks … avg/max`, `CLICK` |
| Gibt es lange Traps, die QEMU nicht zeigt? | `LONG …`-Ereignisse |

Rangfolge nach QEMU: 1 (Start, nur einmal) und 2 (jeder Klick, EmuTOS-treu) sind
belegt; 3 und 4 hängen an Pi-Zahlen.
