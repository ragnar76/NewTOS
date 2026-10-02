# PORT06 — Quittung, Schritt 1: DOOM laeuft auf NewTOS ueber `vr_trnfm`

Datum 2026-10-02 · Basis `desk-rebuild`, HEAD `9d131c7` · Belege
[measurements/measurement-2026-10-02-port06/](../measurements/measurement-2026-10-02-port06/)

**UNFINISHED heisst hier:** Schritt 1 der Reihenfolge aus „Entscheidungen
2026-10-02" (NewTOS, `vr_trnfm`-Weg) ist umgesetzt und in QEMU `raspi4b`
gefahren. Offen bleiben der EdDI-Weg, der m68k-Bau (TT + Nova), die
Abnahme „spielbar mit Tastatur" — und ein libc-Defekt, an dem der
ausgelieferte Bau **heute nicht linkt** (unten, Punkt 1).

## Was laeuft

| Punkt | Ergebnis |
|---|---|
| Titelbild, Wischblende, Demo-Wiedergabe | ✅ `screendumps/02..07`, 320x200 mittig auf 640x480, schwarzer Rand |
| Bildweg ohne Bildschirmbasis-Aufruf | ✅ Index → Standardform (N Planes aus `vq_extnd`) → `vr_trnfm` → `vro_cpyfm`; Rueckleseabgleich aller 200 Zeilen beim ersten Bild: `VERIFIED` |
| `/usr/bin/grep -i -E "setscreen\|physbase\|logbase"` ueber `sdk/apps/doom/` und `sdk/apps/build-doom.sh` | ✅ 0 Treffer |
| doomgeneric unveraendert | ✅ `build-doom.sh` prueft Commit `dcb7a8d…` und `git status` des Baums vor jedem Bau |
| Bildrate | timedemo demo1 **76,5–82,2 fps** (QEMU); je Bild c2p 0,48 ms, `vr_trnfm` 2,98 ms, `vro_cpyfm` 8,0 ms |
| Kein Kerneleingriff | ✅ nichts unter `kernel/`, `tos-api/` geaendert |

## Dateien

* `sdk/apps/doom/doomgeneric_gem.c` (neu) — die Plattformschicht:
  `DG_Init`, `DG_DrawFrame`, `DG_SleepMs`, `DG_GetTicksMs`, `DG_GetKey`,
  `DG_SetWindowTitle`, `main`. AES/VDI/BIOS/XBIOS/GEMDOS. Kopfkommentar
  begruendet Bildweg, Farbmessung, Bildschirmuebernahme, Eingabe, Zeit,
  Speicher.
* `sdk/apps/build-doom.sh` (neu) — Manifest = upstreams `SRC_DOOM` mit
  unserer Datei statt `doomgeneric_xlib.c`; `-DCMAP256
  -DDOOMGENERIC_RESX=320 -DDOOMGENERIC_RESY=200`; Knoepfe `DOOM_ARGS`,
  `DOOM_STATS`.
* `sdk/.gitignore` — Ausnahme fuer `sdk/apps/doom/*.c|*.h` (wie
  apitest/chaintest), sonst waere die Portierung unversioniert.

## Entscheidungen in diesem Schritt (mit Grund)

* **Farbe gemessen, nicht gerechnet:** `vs_color` je Stift, ein Pixel je
  Stift per `vr_recfl`, Rueckleser per `vro_cpyfm` + `vr_trnfm` in die
  Standardform. Mit Farbtabelle (`INQ_TAB[5]=1`, TT/Nova 8 Bit) einmal
  messen, danach ist ein Palettenwechsel nur `vs_color`; ohne (NewTOS,
  `INQ_TAB[5]=0`) bei jedem Wechsel neu messen.
* **Zeit:** `get_sysvar(_hz_200)` (mintlib; auf TOS `Super`, unter MiNT
  `Ssystem`, auf NewTOS Lesen der Sysvar-Seite). Schlafen: `evnt_timer`.
* **Eingabe:** `evnt_multi(MU_KEYBD|MU_TIMER, 0 ms)`; Shift/Control/
  Alternate aus `kstate` als echter Haltezustand; uebrige Tasten gelten als
  gehalten, solange Wiederholungen eintreffen (Zeitgrenzen aus
  `Kbrate(-1,-1)`), weil TOS ohne IKBD-Vektor kein Loslassen meldet.
* **WAD:** nur auf das Scratch-Abbild gelegt
  (`method/seed-doom.sh`); `tools/make-sd-image.sh` ist **nicht** erweitert.
* **Zone `-mb 2`:** siehe Punkt 2.

## Offen

1. **BLOCKER fuer den ausgelieferten Bau — libc `system()`.**
   `build-doom.sh` uebersetzt 81/81 Dateien und bricht beim Linken ab:
   `spawnvp`, `vfork`, `__waittime`, `__waitval` fehlen (aus `libc.a`
   `posix__system.o`/`unix__wait4.o`). doomgeneric ruft `system()` nur in
   `ZenityErrorBox`, die `I_Error` auf NewTOS nie erreicht (stdout ist ein
   Terminal). Die Luecke ist bekannt (`sdk/libc/expected-failures.txt`
   spawnve.c/spawnvp.c, `docs/sdk/07-mintlib-limits.md`). Gemessen wurde mit
   einem Diagnose-Link (`method/build-variants.sh`, `diag_system.c` lehnt laut
   ab), **nicht** Teil der Portierung. Entscheidung noetig: libc-Ticket
   (spawnve/spawnvp LP64 + vfork/wait4) oder ein anderer Weg — nicht in
   doomgeneric, nicht in der Plattformdatei.

   > **Korrektur 2026-10-02 (PORT10), der Text oben bleibt als Stand von
   > damals stehen:** „`I_Error` erreicht `ZenityErrorBox` auf NewTOS nie
   > (stdout ist ein Terminal)" ist **widerlegt**. `I_Error` ruft sie, wenn
   > `exit_gui_popup && !I_ConsoleStdout()` (doomgeneric `i_system.c:410`),
   > und doomgenerics `I_ConsoleStdout` liefert ausserhalb von `ORIGCODE`
   > immer 0 (`i_system.c:210-221`; das `isatty` steht im `#if
   > ORIGCODE`-Zweig). `system()` wird also bei **jedem** `I_Error` gerufen —
   > auch nach der timedemo-Schlusszeile, die DOOM ueber `I_Error` ausgibt.
   > PORT06 sah das nicht, weil der `%f`-Absturz davor lag. Gemessen:
   > `measurements/measurement-2026-10-02-port10/serial/doom-tdemo-serial.log`
   > (Diagnose-Ersatz meldet sich nach der timedemo-Zeile) und, mit dem
   > echten `system()`, `measurements/measurement-2026-10-02-port10b/`
   > (`/dev/null: …`-Zeile von mintlibs `system_hack` nach der timedemo-Zeile
   > und nach „No IWAD file was found"). Der Blocker selbst ist mit PORT10
   > behoben: `build-doom.sh` linkt ohne Ersatz (`TASK/PORT10-STATUS-DONE.MD`).
   > Die Begruendung im Kopf von `measurement-2026-10-02-port06/method/
   > build-variants.sh` traegt denselben Fehler; sie bleibt als Beleg
   > unveraendert.
2. **Speicher:** malloc laeuft in der 4-MiB-TPA; nach Bild/BSS (~820 KiB),
   Stapel (64 KiB) und `DG_ScreenBuffer` (250 KiB) bleiben ~2961 KiB, also
   `-mb 2` (`-mb` nimmt ganze MiB; 3 passt nicht). Titel, Demos und
   timedemo demo1 laufen damit; alle neun Karten nicht geprueft. Groessere
   Zone braucht libc-malloc ueber GEMDOS oder eine groessere TPA (MEM02-Umfeld).
3. **libc `printf("%f")` faellt** in `__printf_fp` (Data Abort EL0, FAR
   wechselnd) — trifft DOOMs Schlusszeile von timedemo. Eigenes Ticket.
4. **libc `intptr_t`/`uintptr_t` sind auf LP64 32 Bit**
   (`mintlib/include/stdint.h:131-144`, `__MSHORT__`-Weiche) — clang warnt
   in `p_saveg.c:187/192`. Eigenes Ticket; die Plattformdatei benutzt sie
   nicht.
5. **Konsole ohne VT52-Escape:** `tos-api/src/vdi/escape.rs::conout`
   ignoriert ESC; Ausgaben der Plattform enden deshalb mit `\r` bzw. loesen
   ein Neuzeichnen des Randes aus.
6. **Tastatur/Beenden/echte Pi** ungeprueft (raspi4b ohne Tastatur);
   Tastenwiederholung auf NewTOS offen (`tick.rs`: 50-Hz-Teil fehlt).
7. **Dateinamen:** doomgeneric legt `.default.cfg` und `./.savegame/` an —
   fuehrender Punkt in 8.3-FAT; Speichern/Konfiguration ungeprueft.
8. **Weiter laut Reihenfolge:** EdDI-Spec → NewTOS `vq_scrninfo` → EdDI-Weg
   in Doom → m68k-Bau (`SYS_BIG_ENDIAN` kommt ueber `i_swap.h`
   `__BYTE_ORDER__`, die Plattformdatei hat keine CPU-/Endian-Annahme).

## Grundwerte

Kein Rust-Quelltext angefasst. Host-Tests `cargo run -p xtask -- test`:
**964 passed / 0 failed / 1 ignored** (unveraendert). Builds/Clippy/Drift
nicht neu gefahren (kernel/tos-api unberuehrt); der Release-Bau rpi4 fuer
die Laeufe meldete 64/94 Warnungen. Nebenwirkung: `lib_ensure` hat
`sdk/libm/libm.a` (unversioniertes Bauprodukt) als veraltet neu archiviert.

## Schritt „Vergroesserung" 2026-10-02 — Stufen x1..x4 mit 4:3-Korrektur

Basis `desk-rebuild`, HEAD `14d5a92` · Belege
[measurements/measurement-2026-10-02-port06-scale/](../measurements/measurement-2026-10-02-port06-scale/RUNINFO.TXT)

Umgesetzt nach „Praezisiert 2026-10-02". Kein Kerneingriff, nichts unter
`kernel/`/`tos-api/`, doomgeneric unveraendert.

| Punkt | Ergebnis |
|---|---|
| Stufen | `-scale 1` 320x200 (wie bisher), `-scale 2` 640x480, `-scale 3` 960x720, `-scale 4` 1280x960; waagerecht ganzzahlig, senkrecht Zeile y zeigt Quellzeile y*200/out_h (x1,2 je Stufe). Mittig, schwarzer Rand. |
| Ohne Parameter | `DOOM_GEM_SCALE` (eine Konstante in `doomgeneric_gem.c`), heute `0` = groesste passende Stufe: 1920x1080 → x4, 640x480 → x2. Nach dem Pi-Lauf festlegen. Zu grosse Stufe → naechstkleinere, mit Meldung. |
| Weg | c2p und `vr_trnfm` auf 320x200, dann Pixel-/Zeilenwiederholung im **Geraeteformat**, dann `vro_cpyfm`. Nur bei > 8 Planes (gepackt, EmuTOS `vdi_raster.c:264-267`/`vr_trnfm16`); bei ≤ 8 Planes Stufe 1. Bytes werden kopiert, nicht gedeutet — keine Byte-Order-Annahme. |
| Pruefung | Rueckleseabgleich beim ersten Bild jetzt ueber das vergroesserte Bild, jedes Pixel: `VERIFIED via vr_trnfm+scale+vro_cpyfm` fuer x2, x3, x4 (QEMU raspi4b mit `-global bcm2835-fb.xres=1920/yres=1080`), x1 wie bisher. |
| Kommandozeile | `main()` haengt `DOOM_GEM_ARGS` (`-mb 2`) jetzt an **jeden** Start an, hinter die gegebenen Argumente — sonst haette `-scale N` allein die 6-MiB-Zone verlangt. Erstes Vorkommen gewinnt (`M_CheckParmWithArgs`). Gefahren ueber `system()` → `Pexec(0)` mit Kommandozeile (Messhilfe `method/launch.c`). |
| Kosten je Bild (QEMU TCG, us) | x1: c2p 635 / trnfm 3095 / scale 0 / cpyfm 7913 · x2: 634 / 3076 / 1165 / 38148 · x3: 611 / 2949 / 1392 / 84166 · x4: 634 / 3076 / 1779 / 152857. `vro_cpyfm` 122–124 ns/px, linear in der Pixelzahl. |
| timedemo demo1 (QEMU) | x1 76,45 · x2 22,64 · x3 10,92 · x4 6,38 fps |
| Bildratenzeile | `build-doom.sh` baut jetzt mit `DOOM_STATS=5` (vorher 0): Kostenprobe beim ersten Bild (~4–5 s Standbild) und alle 5 s `DOOM xN: … fps, … ms/frame, … pal` mit `\r` (PORT06s Weg, < 80 Spalten), seriell gespiegelt. |
| grep Bildschirmbasis | 0 Treffer |
| Hosttests | `sh sdk/run-host-tests.sh` 11 / 0 / 0 |
| m68k | `doomgeneric_gem.c` uebersetzt mit `m68k-atari-mint-gcc` 7.5.0 `-Wall -Wextra` ohne Meldung (nur `-c`, nicht gelinkt) |

### Offen nach diesem Schritt

1. **Standardstufe** nach dem ersten Pi-Lauf festlegen (`DOOM_GEM_SCALE`).
   Die QEMU-Zahlen sind keine Pi-Zahlen. **Korrektur Lead 2026-10-02:** auch
   die 80,49 ns/px aus PORT07 sind eine **QEMU**-Zahl
   (`measurements/measurement-2026-09-28-port07/RUNINFO.TXT` Z. 21-23, `GUI07.md:59`),
   nicht auf der Pi gemessen; dieselbe Fehlzuschreibung steht im Beleg
   `measurement-2026-10-02-port06-scale/RUNINFO.TXT:69` (Beleg bleibt unveraendert).
   Die Pi-Zahl fuer `vro_cpyfm` gibt es noch nicht — der erste Pi-Lauf liefert sie.
2. **Parameterbox ungeprueft:** ein Start mit Stufe geht auf NewTOS heute
   nur ueber `.NTP` (einzige Endung mit `AF_ISPARM`, `desk/main.rs isparm`),
   und `.NTP` startet im Zeichenmodus. Gezeigt: DOOM laeuft im
   Zeichenmodus-Start (`#Z 00`, Lauf i) und die Kommandozeile erreicht die
   Stufe (Laeufe b..g). Die Box selbst braucht Tastatur → echte Pi.
3. ≤ 8 Planes und die 24/32-Planes-Kopierzweige sind auf NewTOS nicht
   erreichbar und ungefahren (TT-/Hatari-Lauf).
4. `DOOM_STATS=5` als Vorgabe ist ein Messbau; fuer die Vorfuehrung ggf.
   wieder 0 (Kostenprobe = Standbild am Anfang).

## Schritt „EdDI-Weg in Doom" 2026-10-02

Basis `desk-rebuild`, HEAD `cb45aae` · Belege
[measurements/measurement-2026-10-02-port06-eddi/](../measurements/measurement-2026-10-02-port06-eddi/RUNINFO.TXT)

Nur `sdk/apps/doom/doomgeneric_gem.c` geaendert. Kein Kerneingriff, nichts
unter `kernel/`/`tos-api/`, doomgeneric unveraendert.

| Punkt | Ergebnis |
|---|---|
| Weg | Cookie `EdDI` vorhanden (`Getcookie`, Wert nur auf Anwesenheit geprueft, **nie aufgerufen**) → `vq_scrninfo` (gemlib, `CONTRL[5]=1`, `INTIN[0]=2`). Gebaut wird direkt im Geraeteformat: format 2 + clut 2 aus den Bitnummern-Tabellen `[16..63]` (Bytes je Pixel = (planes+7)/8, Kanalbreiten `[8..10]`, 8 Bit auf n Bit gerundet, Genlock/Alpha/ungenutzt = 0); format 2 bzw. 0 + clut 1 bei 8 Planes: Stifte per `vs_color`, Pixelwert `[16+i]` (gepackt bzw. verschraenkte Planes). Danach Vergroessern wie bisher, `vro_cpyfm`. Sonst (kein Cookie, `-noeddi`, format −1/1, clut 0, Planes ≠ `vq_extnd`, unstimmige Tabelle) der `vr_trnfm`-Weg unveraendert, Grund auf der Konsole. |
| `[14]` | gedruckt, nicht angewandt: die Bitnummern zaehlen im Pixelwert, wie die CPU ihn liest (Spec §3.2 Crazy-Dots-Beispiel: Vertauschung steckt schon in den Nummern; E3 fuer NewTOS); den Flag zusaetzlich anzuwenden hiesse doppelt tauschen. Ohne Dispatcher gibt es keine Version, und `[14]` existiert erst ab 1.10 (Enhancer 1.00 meldet 0). |
| Eigenpruefung vor Benutzung | 256 Testfarben gemalt, per `vro_cpyfm` (ohne `vr_trnfm`) gelesen, mit den Tabellen dekodiert; Abweichung > 1 Stufe → `vr_trnfm`-Weg. Erstes Bild: jede Ausgabezeile gegen das Gebaute. |
| **E3-Abnahme (NewTOS, QEMU raspi4b)** | „256 of 256 test colours exact, 0/0/0" in jedem Lauf; `VERIFIED via EdDI[+scale]+vro_cpyfm` fuer x1/x2/x4 auf 1920x1080 und x2 auf 640x480; Screendumps farbrichtig. **E3 = 0x02 traegt.** |
| Rueckfall | `-noeddi` → „vr_trnfm route", `VERIFIED via vr_trnfm…` x1/x2/x4 |
| Kosten je Bild (QEMU, 1920x1080, us) | x1: vr_trnfm-Weg c2p 672 + trnfm 1615 + cpyfm 4830 = 7117 → EdDI build 244 + cpyfm 4784 = 5028. x2: 25199 → 24892. x4: 98162 → 96569. Der `vr_trnfm`-Anteil ist weg; ab x2 ist `vro_cpyfm` der ganze Preis. |
| timedemo demo1 (QEMU, 1920x1080) | x1 109,53 → **145,86 fps**; x2 36,58 → 38,18; x4 EdDI 10,19. (Der Kern ist seit `14d5a92` auf beiden Wegen schneller; Vergleich nur innerhalb dieser Messung.) |
| Zweite Implementierung | m68k-DOOM (alles gelinkt, Messhilfe `method/build-doom-m68k.sh`) in Hatari: Falcon-TC + VDI-Enhancer 1.00 → EdDI-Weg, 256/256 exakt, VERIFIED, farbrichtig, ~4x schneller als der `vr_trnfm`-Weg dort (351 gegen 2543 ms je Bild). TT-low 8 Planes + Enhancer (format 0, clut 1) → EdDI-Weg verschraenkt, VERIFIED; Gewinn dort nur die 9 ms `vr_trnfm`. |
| Fehler unterwegs | Erster Durchgang: x1 auf dem EdDI-Weg kopierte ein genulltes MFDB (`dev_mfdb` nur in `stage_trnfm` gesetzt) → Rueckleseabgleich schlug an. In `stage_cpyfm` behoben, alles neu gefahren. |
| Grundwerte | `build-doom.sh` 81/81 LINK OK, Plattformdatei ohne Warnung; m68k `-Wall -Wextra` sauber; grep Bildschirmbasis 0; `sh sdk/run-host-tests.sh` 12/0/0. |

### Offen nach diesem Schritt

1. Echte Pi und echter TT/Nova: nicht gefahren. Nova-Antwort (15/16/24/32
   Bit, Byte-Reihenfolge) weiter unbekannt; die 3-/4-Byte-Pfade und
   packed-8 (Nova 256) sind nur im Quelltext.
2. Der m68k-Bau ist hier eine Messhilfe im Belegordner, nicht der
   ausgelieferte Bau (naechster PORT06-Schritt). Er braucht eine FPU
   (`-m68020-60`) und `<math.h>` aus `sdk/libm/include`.
3. Auf Palettengeraeten (TT) faerbt sich der Rand mit Dooms Paletteneffekten
   mit (Stift 0 = Dooms Farbe 0) — auf beiden Wegen, schon vorher so.
