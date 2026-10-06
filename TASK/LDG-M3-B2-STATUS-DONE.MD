# LDG-M3-B2 — Quittung: libshare 0.1.1 als Allokator jedes LDG-Moduls (Befund B2 geschlossen)

Datum 2026-10-06 · Basis `desk-rebuild`, HEAD `e429e29` (letzter Lauf auf `1f894cf`, nur TASK-Doku dazwischen) · nicht committet
· Belege [measurements/measurement-2026-10-06-libshare/](../measurements/measurement-2026-10-06-libshare/RUNINFO.TXT)
· Ursache [docs/analysis/ldg-module-malloc-2026-10-06.md](../docs/analysis/ldg-module-malloc-2026-10-06.md) (Option A)
· Herkunft des Befunds [TASK/LDG-M3-STATUS-UNFINISHED.MD](LDG-M3-STATUS-UNFINISHED.MD)
· Doku [docs/sdk/10-ldg-modules.md](../docs/sdk/10-ldg-modules.md) §6

## Lizenz: unklar

libshare 0.1.1 hat keine Lizenzdatei und keinen Lizenzkopf; einzige Aussage
ist die Webseiten-Fußzeile „Copyright (c) 2004-2005 Arnaud BERCEGEAY“.
`chdir.c`: „By ERS: it's in the public domain“; die Allokator-Dateien stammen
„from the TOS GCC library“. Portiert nach Entscheidung des Users vom
2026-10-06 (Original, LP64-/Ziel-Änderungen als Overlays); **der User fragt
beim Autor nach.** Vermerkt in `sdk/libshare/libshare-sources.sha256` und
`docs/sdk/10-ldg-modules.md` §6.

## Umsetzung

* **Pin:** `sdk/libshare/libshare-0.1.1.tgz` = Original-Release (sha256
  `2a6c58aa…bdecb`, gleich `references/libshare/`), Herkunfts-URL + Lizenzvermerk
  in `libshare-sources.sha256`. Das mitgelieferte m68k-`libshare.a` und die
  m68k-Testprogramme werden nach dem Entpacken gelöscht, nie benutzt.
* **Bau:** `sdk/libshare/build-libshare.sh` — libshares eigene Objektliste
  (`makefile:28`) und Flags (`makefile:17-26`) über `vendor-lib.sh`; in
  `sdk/lib-stale.sh` als `libshare`, Kette libc → ldg → libshare.
* **Overlays (3, je eine Zeile, Kopf mit Original-Datei:Zeile und Grund):**
  `sbrk.c:61` `size_t` → `intptr_t` (mintlib 0.60.1 `<unistd.h>:934`, sonst
  Compilerfehler); `calloc.c:25` `__bzero` → `bzero` (SDK-libc hat kein
  `__bzero`); `malloc.c:51` `static_mblock` wie `struct mem_chunk` ausgerichtet
  (LP64: unverändert lag es bei `.bss+0x11`).
* **Ergänzung (nicht aus libshare):** mintlibs eigenes
  `mintlib/posix_memalign.c` unverändert mit `-D__malloc=malloc` im Archiv —
  `memalign/posix_memalign/aligned_alloc` rufen in mintlib 0.60.1 `__malloc`
  direkt und zögen sonst libcs Allokator neben libshares.
* **Linken:** Module linken `libshare.a` **zuerst** in der Gruppe (wie zViews
  `-lshare -lldg`), mit `-Map`; `ls_check_module_map`
  (`sdk/libshare/libshare-link.sh`) bricht den Bau ab, wenn ein
  libc-Allokator-Mitglied gezogen wurde. `EX_LIB.NLD` und `POPUP.NLD`
  (Werkzeugentscheidung: `popup.mint:10` linkt libshare nicht, `popup.c:74`
  verletzt LDGs Regel `guides/src/ldg.u:1047-1051`). Clients
  (`LDGTEST.NCP`, `LDGDEMO.NAP`) unverändert und bytegleich.
* **LP64-/Interna-Durchsicht:** `_heapbase/_split_mem/_stksize/
  __libc_unix_names/_enoent/_unx2dos` in der SDK-libc vorhanden, Typen
  passend; `struct mem_chunk` 24 Byte = libcs `malloc_int.h`; `(long)`-Casts
  unter LP64 korrekt. Ergebnis im RUNINFO.
* **Doku:** `docs/sdk/10-ldg-modules.md` (Teile, Linkzeile, Grenzen-Zeilen
  korrigiert, neuer §6 mit Lizenz), `12-windom.md` (B2-Zeile: geschlossen),
  `docs/sdk/README.md`, `docs/analysis/libshare-2026-10-06.md` §4
  (Korrekturvermerk, Text bleibt), `docs/analysis/ldg-after-arch03-2026-10-06.md`
  (Nachtrag zu Zeile 11), `references/README.md`.
* **Kein Rust, keine libc-/libldg-Quelle geändert.**

## Prüfungen

| Prüfung | Ergebnis |
|---|---|
| Link-Map der Module | nur `libshare.a(malloc.o, sbrk.o)`, kein libc-Allokator-Mitglied (`checks/module-link-maps.txt`) |
| Linkprobe | alle Einstiegspunkte inkl. `memalign/posix_memalign/aligned_alloc/valloc` → libshare; `__malloc`-Bezug → Map-Prüfung schlägt an (`checks/link-probes.txt`) |
| Hosttest `libshare-host` | 15/15; Empfindlichkeit: ohne statischen Block 6/15 rot |
| SDK-Hosttests | 16/0/0 (Baseline 15/0/0 + neuer Test) |
| Reproduzierbarkeit | `libshare.a`, Module bytegleich bei Neubau; `lib_ensure` konvergiert |
| Rust | nicht berührt; Scratch-rpi4-Release 64/94 Warnungen (= Baseline) |
| LDGTEST (69) | off / on / strict: ALL PASS |
| POPUP-Pfad (`m3b`) | Pop-up erscheint, Sinus → 0.479426, Cosinus → 0.877583, Client endet rc=0; **on = strict = off**, 15/15 Bilder pixelgleich, off hängt nicht mehr |
| M3-Abnahme (`evb-acc`) | on = strict, 10/10 pixelgleich zur B1-Abnahme |
| Instrumentiert | Modul-`Pterm`: `libshare_mchunk_free_list.next = 0` (beide Module) |
| Regression on | APITEST 106/0/5, QED und TeraDesk starten, Bilder bytegleich zu M3 |

## Ehrliche Abweichung von der Erwartung

Erwartet war „das Modul ruft während seines `Pexec(4)`-Laufs kein `Malloc`
mehr“. **Gemessen ruft es einmal `Mxalloc(0x4000)`** — `exit()` schließt
stderr und stdout; stderrs Puffer nimmt den statischen 16-KiB-Block, den
libshare nach dem `free` endgültig aus der Liste nimmt
(`malloc.c:271-289`), stdouts Puffer bekommt daher einen Block per
`ldg_Malloc` mit dem Modul als Besitzer. libshares `free` gibt ihn aber
**vor** dem `Pterm` selbst zurück (`Mfree`), weil es — anders als libcs
mintlib-0.60.1-`free` mit `if (q && …)` (`sdk/libc/mintlib/mintlib/malloc.c:190`)
— den letzten Block zusammenführt. Was B2 verlangt (kein Freilisteneintrag
in Speicher, den der `Pterm` freigibt), gilt; belegt im Lauf und im Hosttest.

## Beobachtungen (nicht behoben)

* Im **zweiten** Pop-up fehlt die Zeile „Informations“: `client.c:75/78/81`
  (upstream) kopiert „Sinus“/„Cosinus“ per `strcpy` in den 5-Byte-String
  „Info“, dem in `client.rsc` direkt „  Informations“ folgt — dessen erstes
  Byte wird NUL. Gleich auf dem Original (gleiches .rsc-Layout); kein
  NewTOS-Defekt.
* `sdk/libc/newtos-arch/memops.c` verspricht im Kopf `__bzero`/`_bzero`/
  `__bcopy`/… , definiert aber nur `bzero`/`bcopy` (Grund für das
  `calloc.c`-Overlay).
* `docs/sdk/12-windom.md` §6 führt B1 noch als offen (ist seit LDG-M3-B1 behoben).
* `ldgutil.c` `ldg_Malloc`: `Sversion`-Bitfelder sind endianabhängig; auf
  beiden Wegen landet es bei `Mxalloc` — wirkungslos.
* Restfall (wie auf dem Original): > 16 KiB, die ein Modul im eigenen
  `main()`/`exit()` holt und **behält**, hängen nach dem `Pterm` weiter —
  dokumentiert in `10-ldg-modules.md` §4.

## Offen

* Lizenzantwort des Autors (User).
* Echte Pi nicht gefahren (keine HW-Aktionen durch Worker).
* `docs/STATUS.md`-Eintrag steht aus (Lead).
