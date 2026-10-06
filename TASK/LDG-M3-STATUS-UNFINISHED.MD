# LDG-M3 — Quittung (UNFERTIG): WinDom 1.21.3 als SDK-Bibliothek, LDG-Demo-Client gegen `EX_LIB.NLD`

Datum 2026-10-06 · Basis `desk-rebuild`, HEAD `d7fdec0`, Baum vorher sauber
· nicht committet · Belege [measurements/measurement-2026-10-06-ldg-m3/](../measurements/measurement-2026-10-06-ldg-m3/RUNINFO.TXT)
· Grundlage [docs/analysis/zview-ldg-2026-10-04.md](../docs/analysis/zview-ldg-2026-10-04.md) (P-WINDOM)
· Doku [docs/sdk/12-windom.md](../docs/sdk/12-windom.md)

**Warum UNFERTIG:** WinDom ist portiert, gebaut und läuft; der unveränderte
Demo-Client lädt `EX_LIB.NLD` und `POPUP.NLD` und zeigt das Ergebnis von
`information()` aus `EX_LIB.NLD` — unter `memprot=on` und `strict`. Der Weg zu
`sinus()`/`cosinus()` über die Oberfläche ist aber durch zwei Befunde
blockiert, die **nicht in WinDom** liegen und deren Behebung eine
Kernel-/Entwurfsentscheidung ist (Auftrag: vor einer Kernel-Änderung STOP
und berichten):

* **B1 (Kernel-AES):** `evnt_multi`/`evnt_button` melden in `EV_MB` den
  *lebenden* Tastenzustand nach dem Doppelklick-Fenster, EmuTOS den Zustand
  des Ereignisses (`bchange(gl_bdesired)`, `ev_rets`; `aes/geminput.c:155-180,
  328-345`, `aes/gemevlib.c:42-58`; Spec `docs/api/aes/event.md:259-266`).
  WinDom wertet einen Klick nur bei `evnt.mbut & 1` aus
  (`evnt_windom.c:334`) → **jeder normale Mausklick auf ein WinDom-Formular
  wird verworfen**; ein Druck länger als das Doppelklick-Fenster geht.
  Beleg: AES-Trace `checks/m3a-trace-excerpt.txt` (#104 `mb=0x00`, #107
  `mb=0x01`). Ort: `tos-api/src/aes/event.rs` `fill_rets`.
* **B2 (LDG-Modul-Heap):** das `exit()` eines Moduls schließt stdio
  (`fcloseall` → `fclose` → `__flshfp` → `init_stream`: `malloc` + `free`);
  der Block gehört dem Modulprozess und wird beim `Pexec(4)`-Ende frei, die
  statische Freiliste des Moduls zeigt weiter hinein. Das nächste `malloc`
  im Modul (vom Client gerufen) liest freien Speicher: `memprot=on` → Bus
  Error in `POPUP.NLD`s `malloc` (`screendumps/m3b-on-S03_POPUP.png`),
  `memprot=off` → Hänger. Trifft **jedes** Modul, das nach dem Laden
  `malloc`t — alle zView-Codecs (M4). Ort der Behebung (libc, libldg oder
  Kernel-Eigentum) offen.

## Umsetzung

* **Quelle gepinnt:** `sdk/windom/windom-1.21.3.zip` = SourceForge-Release,
  unverändert (SHA-1/MD5 = SourceForge; 397 Dateien bytegleich zu
  `Applications/windom-1.21.3`), zViews Patch unverändert unter
  `sdk/windom/patches/`, beides mit sha256/Herkunft/Lizenz (LGPL 2.1) in
  `sdk/windom/windom-sources.sha256`.
* **Bau:** `sdk/windom/build-windom.sh` — jedes Mal Zip + Patch + Overlay,
  WinDoms eigene `files.mak`-Liste (198 Dateien), `libwindom1.a`,
  `include/` mit `windom1.h` **und** `windom.h`; in `sdk/lib-stale.sh` als
  `windom` (Kette libc → gemlib → windom). Zwei Bauten bytegleich
  (`SOURCE_DATE_EPOCH` für `globals.c`s `__DATE__`), `lib_ensure`
  konvergiert.
* **Overlays: 10 in WinDom** (`sdk/windom/overlay/src/`, je Kopfkommentar mit
  Zeile und Grund) — 4× Zeiger in GEM-Nachrichten als 32-Bit-hi/lo-Paar
  (Endianness + Breite: `evnt_windom.c`, `av_init.c`, `shel_write.c`,
  `debug_windom.c`), 2× Systemvariablenbreite (`cookieptr.c` `$5A0`,
  `vq_tos.c` `$4F2`), 2× Cookie-Jar über `Getcookie`/`Ssystem(S_SETCOOKIE)`
  statt Spiegel-Walk (`set_cookie.c`, `new_cookie.c`), 1× LP64-Pixelbreite
  (`cicon.c`), 1× Bitfeld-Reihenfolge (`objc_str3d.c`); **+1 im Client**
  (`sdk/ldg/overlay/demo/client/client.c`: nur die Modulendung `.ldg` →
  `.nld`, ADR-018-Nachtrag Punkt 4). Kein m68k-Assembler in WinDom. Rest über
  Flags: `-fsigned-char`, `-D__RSXHDR`, drei `-Wno-error=…`,
  `-Wno-header-guard`.
* **LP64-Durchsicht** aller 76 `(long)`-Cast-Zeilen und der übrigen Klassen
  (Wortfeld-Punning, Low-Memory, Bitfelder, Fremd-Cookie-Strukturen,
  Varargs): Ergebnis und bewusst Belassenes in `docs/sdk/12-windom.md` §4.
* **Client:** `sdk/ldg/build-ldg-client.sh` — `LDGDEMO.NAP` (client.c +
  `exlibbnd.c`/`popupbnd.c` unverändert), `POPUP.NLD` (`popup.c`
  unverändert), `CLIENT.RSC` (mkrsc), im Verzeichnisbaum der Demo
  (`CLIENT\`, `LIBS\`).
* **Doku:** `docs/sdk/12-windom.md` (neu), `docs/sdk/10-ldg-modules.md`
  (Client-Zeile, B2 als offene Grenze), `docs/sdk/README.md`.
* **Kernel, tos-api, libc, gemlib: nicht berührt.**

## Abnahme (was belegt ist)

QEMU raspi4b, `#Z C:\LDGDEMO\CLIENT\LDGDEMO.NAP`, Eingaben per
Scratch-IKBD-Injektor (nur Eingabequelle, Kernelcode unverändert):

* Fenster „LDG Client“ mit WinDom-Formular gezeichnet, beide Module geladen
  (`probe-on-t35.png`); ohne Referenzvergleich — WinDom auf Hatari wäre das
  Orakel.
* Alt+C → `FormAlert(mathinfo())` → „Example of math library!!!“ =
  `information()` aus `EX_LIB.NLD`; dasselbe per langem Mausdruck; Esc 1 2 3
  ins Wertfeld; Return → Close → `AP_TERM` → `ldg_close`×2 → `exit(0)`,
  zurück auf dem Desktop. `memprot=on` und `strict`: alle 9 Screendumps
  pixelgleich.
* Regression (`memprot=on`, HEAD-Kernel): APITEST 106/0/5 (= M1), QED und
  TeraDesk starten; 4 Screendumps bytegleich zu M1. SDK-Hosttests 15/0/0
  (= Baseline). `libldg.a`, `EX_LIB.NLD` bytegleich zu M1.

## Offen

* **B1** und **B2** (oben) — Entscheidung/Auftrag nötig; erst danach
  `sinus()`/`cosinus()` über die Oberfläche.
* **O1 (kosmetisch, Kernel-AES):** die Schreibmarke von `objc_edit`
  ignoriert `te_just` (EmuTOS `pxl_rect` → `gr_just`,
  `aes/gemobed.c:180-182`); im zentrierten Feld der Demo 18 px zu weit links.
* Konvention für Zeiger in Nachrichten zwischen Programmen (AV, VA_START,
  BubbleGEM) gibt es auf NewTOS noch nicht; die Overlays nehmen das
  Protokoll-Layout (hi/lo, 32 Bit, gilt weil alle Nutzeradressen < 2 GiB).
* `RsrcXload` ungetestet (kein M3/M4-Nutzer); `appl_tplay`-Records
  (`MouseObjc`, `snd_mu_button`) unbeobachtbar, solange `appl_tplay` nichts
  abspielt (A45).
* **Für M4 (zView):** B1 trifft jede Mausbedienung von zView, B2 jeden Codec;
  `plugins.c`-Overlay für `.NLD`, libshare; Client-Code mit
  `*(LDG **)&evnt.buff[3]` (`LdgQuit`) ist LP64-falsch, aber ohne TSR
  unerreichbar.
* `docs/STATUS.md`-Eintrag steht aus (Lead, wie bei M1/M2).
