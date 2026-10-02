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
