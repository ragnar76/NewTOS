# SDK-Inventur: Werkzeugkette, App-Patches, Dokumentation (2026-10-20)

Auditor-Bericht. Stand: HEAD `dc11d42b`, Host macOS 27 / Apple Silicon,
Apple clang 21.0.0, rust-lld/LLVM 22.1.2 aus dem gepinnten Nightly.
Kennzeichnung: **[Lauf]** = selbst gebaut/gelinkt/ausgeführt und Ergebnis
gesehen; **[gelesen]** = aus Quelltext/Doku abgeleitet, nicht ausgeführt.
Kein Produktcode, keine Doku unter `docs/sdk/`, kein TASK/STATUS geändert.
Kratzdateien liegen im Session-Scratchpad (u. a. Link-Probe-Skript, Listen).

**Umfang (ein Satz):** Ist-Zustand der SDK-Werkzeugkette (mintlib, gemlib,
cflib, libm, LDG, libshare, WinDom, Codec-Bibliotheken) auf Bau- und
Link-Ebene, alle Overlay-Dateien gegen ihr Upstream-Original, und die 15
Kapitel `docs/sdk/` + `sdk/README.md` gegen diese Realität und gegen die Frage
aus Issue #4 („Dokumentation für Crosscompiler“).

---

## 0. Methode

1. Alle Bibliotheken so gebaut, wie die Skripte es tun, libc mit
   `MINTLIB_REFRESH=1` frisch aus dem Tarball: `build-libc.sh`,
   `build-gemlib.sh`, `build-cflib.sh`, `build-libm.sh`, `build-ldg.sh`,
   `build-libshare.sh`, `build-zlib/libpng/libjpeg/giflib/libexif.sh`,
   `build-windom.sh` — alle rc=0. [Lauf]
2. **Link-Probe pro öffentlichem Symbol:** für jedes global definierte Symbol
   jedes Archivs ein Mini-Programm, das genau dieses Symbol referenziert, mit
   `sdk/libc/crt0.S` + `newtos.ld` + allen Archiven in einer `--start-group`
   gelinkt; Fehlschlag = Funktion ist für einen Anwender **nicht nutzbar**
   (transitive Hülle, also realistischer als eine reine `nm -u`-Liste). [Lauf]
   libc: 913 Symbole, gemlib 677, cflib 188, WinDom 305, LDG 19, libshare 24,
   zlib 113, libpng 395, libjpeg 169, giflib 62, libexif 143, libm 212.
3. `nm`-Differenz „referenziert, nirgends definiert“ über `libc.a`. [Lauf]
4. Probe-Kompilat der mintlib-Verzeichnisse, die `DIRS` nicht enthält, mit
   den Flags von `build-libc.sh` (nur in den Scratchpad). [Lauf]
5. Stub-Grep (`ENOSYS`, `EINVFN`, konstante Rückgaben, „stub“) über
   `newtos-arch/` und alle Bibliotheks-Overlays, dann Lesen der Treffer. [gelesen]
6. `sdk/run-host-tests.sh`. [Lauf]
7. Apps gebaut: `build-qed.sh`, `build-teradesk.sh`, `build-coremark.sh`,
   `sdk/build.sh hello.c` — alle rc=0, 0 undefinierte Symbole. [Lauf]
8. Overlays gegen Upstream: Bibliotheks-Tarballs frisch in den Scratchpad
   entpackt, App-Overlays gegen die eingebetteten Git-Checkouts,
   `diff --strip-trailing-cr`. [Lauf]
9. Doku-Rezepte aus Kapitel 03 wörtlich nachgefahren, plus ein „typisches
   Anfängerprogramm“ (tmpfile + stat + powf). [Lauf]

---

## 1. Werkzeugketten-Inventar

### 1.1 Was gebaut wird (Ist) [Lauf]

| Bibliothek | Version/Pin | Kompiliert | Bemerkung |
|---|---|---|---|
| libc = mintlib | 0.60.1 | 616 von 625 Dateien in `DIRS`, 9 erwartete Fehlschläge, 5 bewusst ausgeschlossen | 14 eigene Dateien in `newtos-arch/`, 23 Overlay-Dateien |
| gemlib | 0.44.0 | 578/578 | 20 Overlay-Dateien (svc #2, LP64-Accessoren) |
| cflib | Commit ce57817 | 129/129 | 2 Overlays |
| libm = OpenLibm | 0.8.8 | 175/175 | ohne `long double` |
| LDG (devel) | svn r133 | 4/4 | 3 Overlays (davon 1 Demo-Client) |
| libshare | 0.1.1 | 7/7 (+ mintlib `posix_memalign.c`) | 3 Overlays |
| zlib / libpng / libjpeg / giflib / libexif | 1.3.2 / 1.6.59 / 9f / 6.1.3 / 0.6.26 | 15/18/46/8/25 | je 0–1 Konfig-Overlay |
| WinDom | 1.21.3 + zView-Patch | 198/198 | 10 Overlays, 3 Fehler-Klassen auf Warnung herabgestuft (`-Wno-error=int-conversion` u. a.) |

Link-Probe: **gemlib, cflib, WinDom, LDG, libshare und alle fünf Codecs
linken zu 100 %** (0 Fehlschläge). Die Lücken sitzen ausschließlich in libc
und libm.

### 1.2 Befunde, nach Schwere

**W1 — HOCH — `tmpfile`, `mkstemp`, `mktemp`, `mkdtemp`, `tmpnam(_r)`,
`tempnam`, `glob`/`globfree`, `wordexp`/`wordfree`, `getlogin`, `cuserid`,
`sem_open`/`sem_unlink`, `__get_tmpdir` linken nicht. [Lauf]**
Ursache: Das Verzeichnis `pwdgrp/` (getpwuid, getgrent, setgrent, endgrent,
getpwnam_r, getpwuid_r) steht nicht in `DIRS` (`sdk/libc/build-libc.sh`, Zeile
mit `DIRS="stdio stdlib …"`); `unix/getgroups.c` und `posix/glob.c`/
`wordexp.c` referenzieren es. Die gesamte Temp-Datei-Familie hängt über
`__exists` → `__euidaccess` → `__group_member` → `getgroups` daran.
Probe: Alle 16 `pwdgrp/*.c` kompilieren mit den libc-Flags fehlerfrei; mit
diesen Objekten (plus `socket/`) zusätzlich im Archiv linken *alle* oben
genannten Funktionen. Das ist also **kein** Portierungsproblem, sondern ein
fehlender Eintrag in `DIRS`. Der Fall aus
`measurements/measurement-2026-10-18-proc04-fdelete/checks/tmpfile-link.txt`
ist damit erklärt und reproduziert. Laufzeitverhalten nach dem Hinzufügen
(mintlib liest `/etc/passwd`, das es nicht gibt → `getpwuid` liefert NULL)
ist **nicht** gefahren.
Drive: Ein dreizeiliges Programm `tmpfile(); stat(); powf();` endet mit
`undefined symbol: getpwuid, getgrent, setgrent, endgrent, __addtf3,
__extendsftf2, __trunctfsf2`. [Lauf]

**W2 — MITTEL — libm: `powf`, `remainder`, `remainderf`, `fdim`/`fdimf`/
`fdiml`, `nexttowardf`, `scalbln`/`scalblnf`/`scalblnl`, `cpowf`, `casin(h)`,
`cacos(h)`, `catanh` (+f) linken nicht. [Lauf]** 21 von 212 Symbolen.
Ursache: keine compiler-rt-Bausteine für binary128 (`__addtf3`, `__multf3`,
`__divtf3`, `__gttf2`, `__netf2`, `__extendsftf2`, `__trunctfsf2`,
`__trunctfdf2`, `__subtf3`) und komplexe Multiplikation (`__muldc3`,
`__mulsc3`). Dokumentiert in `docs/sdk/07-mintlib-limits.md` §4/§6 — aber
`powf` ist eine Alltagsfunktion; das ist eine echte Lücke der Werkzeugkette,
kein Grenzfall. Dieselbe Ursache macht in libc `qecvt`/`qfcvt`/`qgcvt`(_r)
unlinkbar. `newtos-arch/softfloat.c` liefert bisher genau einen dieser
Bausteine (`__extenddftf2`) von Hand.

**W3 — MITTEL — `ttyname`, `ttyname_r`, `ctermid` linken nicht. [Lauf]**
Ursache: `mintlib/do_fstat.c` ist durch `newtos-arch/do_fstat.c` ersetzt; das
Original definierte auch `__sys_fstat` (`mintlib/do_fstat.c:17`), das
`mintlib/ttyname.c:81` und `ttyname_r.c:80` rufen. Der Ersatz liefert nur
`__do_fstat`. In `07-mintlib-limits.md` §6 steht `__sys_fstat` in der
Rohliste, die Auswirkung auf diese drei Funktionen ist nirgends genannt.

**W4 — MITTEL — `stat()`/`fstat()` liefern `st_mtime/st_atime/st_ctime = 0`
für jede Datei. [gelesen]**
`newtos-arch/do_stat.c` macht `__do_stat = __quickstat`; `__quickstat`
„gets everything but the timestamps“ (`mintlib/quickstat.c:26`) und fällt
auf NewTOS immer in den Fsfirst-Zweig, weil der Kernel kein `Fstat64`/`Fxattr`
hat (`/usr/bin/grep -rn -i "fstat64|fxattr" tos-api/src` → kein Treffer).
Upstream-mintlib auf TOS füllt die Zeiten aus der DTA
(`mintlib/do_stat.c:281-282`, `__unixtime(d.dta_time, d.dta_date)`). Damit
weicht NewTOS-libc von mintlib-auf-TOS beobachtbar ab (make-artige Werkzeuge,
Dateilisten, `ls -l`-Ports). `fstat` setzt Zeiten ebenfalls auf 0
(`newtos-arch/do_fstat.c`, Kommentar „Times … left zeroed“). In 07 §5 als
„zero“ vermerkt — dokumentiert, aber eine Funktionslücke, keine
HW-Abwesenheit. Nicht auf der Maschine gefahren.

**W5 — NIEDRIG/MITTEL — `SIZE_MAX` und `PTRDIFF_MIN/MAX` sind auf LP64 die
32-Bit-Werte. [Lauf]** `_Static_assert(SIZE_MAX == (size_t)-1)` schlägt fehl
(`sdk/libc/mintlib/include/stdint.h:334-336` → `4294967295U`;
`:316-320` → `2147483647`). Ebenso `SSIZE_MAX == INT_MAX` bei 64-Bit-`ssize_t`
(`bits/posix1_lim.h:129`). Wirkung heute: Überlaufprüfungen (giflib
`dgif_lib.c:1217`) werden strenger, nicht falsch; Code, der `SIZE_MAX` als
Sentinel gegen `(size_t)-1` vergleicht, wird falsch. In
`02-prerequisites.md` als „still the 32-bit values“ erwähnt — ein bekannter,
aber nicht behobener Header-Fehler (Overlay `include/stdint.h` hat nur
`intptr_t` korrigiert).

**W6 — NIEDRIG — libc.a enthält Testprogramme und Host-Generatoren. [Lauf]**
`unix/ucontext.c` ist ein mintlib-*Testprogramm* und landet als
`unix__ucontext.o` im Archiv: es definiert `main`, `func1_stack`,
`func2_stack` (Link-Probe: „duplicate symbol: main … ucontext.c“). Ein
Programm ohne eigenes `main` linkt still gegen dieses `main` (das unaufgelöste
`getcontext` macht es dann zum Linkfehler mit irreführender Meldung).
Genau diese Falle hat `build-libc.sh` für `syscall/main.c` schon beseitigt,
für `ucontext.c` nicht. Außerdem liegen `syscall/{check,generate,
syscallparser,traps}.o` (Host-Generator des Trap-Bindungsbaus, mit `yyparse`/
`yyin`-Referenzen) im Archiv.

**W7 — NIEDRIG — fehlende/unlinkbare Einzelfunktionen. [Lauf]**
- `getcontext`/`makecontext`/`swapcontext`: nirgends definiert (m68k `.S`).
- `gets`, `getwd`, `siggetmask`: fehlen wegen einer `.stabs`-Direktive
  (`expected-failures.txt`); drei echte Funktionen fehlen wegen einer
  kosmetischen Debug-Zeile — mit einem Overlay ohne `.stabs` trivial behebbar.
- `nf_debug*`, `nf_exec*`, `nf_get_name`, `nf_get_fullname`, `nf_shutdown`,
  `nf_version`: unlinkbar, weil `mintlib/nf_ops.c` (m68k-asm) nicht baut und
  `nf_init` fehlt. TeraDesk bringt dafür seinen eigenen Ersatz als App-Overlay
  mit (siehe 2.1) — die Lösung gehört in `newtos-arch/`.
- `if_link`, `if_getifflags` u. a. (`socket`), `__gethostid`
  (`gethostbyname`): `socket/` nicht gebaut. Ohne Netzwerk im Kern ist das
  ehrliches „nicht vorhanden“; `socket/` kompiliert 67/69.
- `crypt/`, `login/`, `shadow/`, `sysvipc/`, `argp/`, `stdiio/`: nicht in
  `DIRS`; kompilieren in der Probe zu 5/8, 14/16, 10/10, 12/12, 9/9, 2/2.

**Stub-Prüfung (Regel 2). [gelesen]** Kein Treffer, der eine Funktion mit
festem Wert ohne Grund zurückgibt: `ioctl` → echter `Fcntl`-Trap,
`__sys_stat` → echter `Fstat64`-Trap (EINVFN → mintlibs Fsfirst-Pfad),
`Getcookie` → `Ssystem`, `get_sysvar` → echter Lesezugriff, `Super`/
`Supexec` → begründete Reimplementierung ohne Privileg, `Flopfmt` →
`EUNDEV` (HW-Abwesenheit, Spec `docs/api/xbios/_floppy-stubs.md`), `access`
→ echte Rechteprüfung ohne Gruppen-DB. Die Lean-Shims sind **keine** Stubs,
aber W3/W4 zeigen, dass sie Funktionsumfang abschneiden, ohne dass die Doku
die Folgen für den Anwender ausweist.

**Host-Tests:** `sdk/run-host-tests.sh` → 20 PASS, 0 FAIL, 0 SKIP. [Lauf]
Hinweis: `setjmp-host` verlangt einen AArch64-Host — auf Linux/x86-64 wird er
übersprungen (nicht gefahren).

**Alias-Prüfung. [Lauf]** Alle 307 `strong_alias`/`weak_alias` der
kompilierten mintlib-Dateien wurden gegen die definierten Symbole geprüft;
die 7 „fehlenden“ stehen in nicht aktiven `#ifdef`-Zweigen
(`USE_IN_LIBIO`, `OLD_CLOSE`). Die clang-Alias-Falle (05 B13) betrifft heute
nur noch das bereits überlagerte `labs`.

---

## 2. Patch-Inventar

Grundsatz der Bewertung: Eine Datei unter `sdk/overlays/<app>/` (bzw.
`sdk/zview/overlay/`) **ist ein Patch der Anwendung**, gleichgültig, ob er per
Ersatzdatei oder per `diff` angewandt wird. Die Frage ist nur, ob seine
Ursache in der Werkzeugkette einmal behoben werden könnte, sodass der Patch
verschwindet.

In-place-Änderungen an App-Quellen: **keine gefunden.** `git status` in
`sdk/apps/{qed,teradesk,coremark,highwire}` und `Applications/doomgeneric`
zeigt nur `build/`; die entpackten Bäume `sdk/zview/z-tools-04805b7` und
`sdk/ldg/ldg-r133` unterscheiden sich vom Tarball nur durch die eingespielten
Overlays und `.DS_Store`. [Lauf]

### 2.1 TeraDesk (7 Dateien, Basis `freemint/teradesk` @ `b5532b8`)

| Datei | Geänderte Zeilen¹ | Was | Warum | Werkzeugkette kann es einmal lösen? |
|---|---|---|---|---|
| `library/utility/nf_ops.c` | ganze Datei | NatFeats-Erkennung (m68k-Opcodes 0x7300/0x7301) durch „nicht vorhanden“ ersetzt | m68k-asm | **Ja.** NewTOS-`nf_ops.c` („NatFeats absent“) nach `newtos-arch/`, damit libc's `nf_*` linkt (W7); TeraDesks eigene Datei dann per Build-Ausschluss wie `library/utility/malloc.c` weglassen. Patch → Build-Regel |
| `environm.c` | 2 Zeilen + Kopf | `_BasPag->p_env` über `(const char *)(uintptr_t)` gecastet | LP64: Basepage-Overlay macht `p_env` zu `uint32`, clang wertet int→Zeiger als Fehler | **Ja, mit Abwägung.** Mit `-Wno-error=int-conversion` (wie bei WinDom schon gesetzt) wird die implizite Konvertierung null-erweitert — korrekt, solange EL0-VAs < 2^32 sind. Gleiches Muster in `sdk/ldg/overlay/src/devel/ldgload.c`. Preis: echte int→Zeiger-Fehler werden nur noch Warnung |
| `window.h` | 5 Zeilen | `WDFLAGS`-Bitfelder `unsigned int` → `unsigned short` | LP64/AArch64-Alignment: Struktur 4 statt 2 Byte, `VERIFY_TYPE` | Nein (kein clang-Schalter für m68k-Alignment ohne ABI-Bruch zu den Bibliotheken). Bleibt App-Patch |
| `library/xdialog/library.h` | ~30 Zeilen gelöscht | eigenes `AES_GLOBAL`-Struct + `_AESrsc*`-Umdefinition entfernt | LP64: Struct 56 Byte, Felder hinter `aes_global[16]` | Nein (TOS-Layout hat 4-Byte-Zeigerfelder). Bleibt App-Patch |
| `icon.c` | 8 Zuweisungen | `_AESrscfile = p` → `_AESrscfile_set(p)` | Folge von library.h: 32-Bit-Zeigerfeld ist in C kein lvalue | Nein. Bleibt App-Patch |
| `rsc_load.c` | 3 `#ifdef __GNUC__`-Blöcke | 8-Byte-Zeigerzugriffe auf `aes_global[5]/[7]` → gemlib-Accessoren | LP64 | Nein. Bleibt App-Patch |
| `main.c` | 6 Makros + 1 Zeile | Sysvar-Zugriffe `$4F2/$420/$43A/$426/$42A` als `uint32_t`; Reset-Vektor bei OSHEADER+0x10 | LP64 `long` = 8 Byte | Nein (Rohzugriffe auf Low-Memory). Bleibt App-Patch |

¹ ohne CR-Unterschiede (`--strip-trailing-cr`). Zusätzlich auf Build-Ebene:
`-D_TOS_ONLY` (Variantenwahl, kein Patch) und Ausschluss von
`library/utility/malloc.c` (laut Skript wie im Upstream-`SRCS`; nicht
gegengeprüft).

Bilanz TeraDesk: 1 Datei ist m68k-asm, 6 sind LP64-Anpassungen.
Werkzeugkettenseitig entfernbar: `nf_ops.c` (sicher), `environm.c` (per Flag,
mit Abwägung). Die übrigen 5 bleiben App-Patches, solange die App LP64 ist.

### 2.2 QED (2 Dateien, Basis `freemint/qed` @ `f12055f`)

| Datei | Umfang | Was | Warum | Werkzeugkette? |
|---|---|---|---|---|
| `memory.c` | Allokator neu geschrieben (Upstream-Suballokator → libc `malloc`/`free` mit eigener Alloc-Liste; `BLOCK_SIZE`-Grenze entfernt) | Kein „Fix“, eine **Neuimplementierung** des Speicherverwalters mit Erhalt der Schnittstelle | Upstream setzt `sizeof(pointer)==4` an drei Stellen voraus | Nein (nur ein ILP32-Build würde es lösen — verworfen). Bleibt App-Patch, und zwar der tiefste im ganzen Bestand |
| `set.c` | Alle Mengenoperationen byteweise neu | `SET` = `unsigned long[32]`, byte- und big-endian-wortweise adressiert | LP64 + Little-Endian | Nein. Bleibt App-Patch |

### 2.3 CoreMark (2 Dateien)

`overlays/coremark/newtos/core_portme.{c,h}` ist die **vorgesehene
Portierungsschicht** von CoreMark (Run-Rules erlauben genau diese Dateien;
die sechs Benchmark-Dateien werden per MD5 geprüft). **Kein App-Patch.**
(`.DS_Store`-Dateien unter `sdk/overlays/` sind per `sdk/.gitignore:126` ignoriert, nicht getrackt.)

### 2.4 zView (13 Dateien unter `sdk/zview/overlay/`, Basis z-tools @ 04805b7)

Steht **nicht** unter `sdk/overlays/`, obwohl es App-Patches sind
(`sdk/README.md` behauptet, `overlays/{qed,teradesk,coremark}` sei „the only
place application-level changes live“ — falsch).

| Klasse | Dateien | Werkzeugkette? |
|---|---|---|
| m68k-asm | `plugins/img/xp32.c` (neu, ersetzt `xp32.S`), `plugins/jpg/jpgdsp.c` | legitime Ausnahme |
| ADR-018-Suffix `ldg`→`nld` | `plugins.c` | Nur durch Projektentscheidung (Codecs als `.LDG` benennen) vermeidbar |
| PDF weggelassen | `main.c`, `winimg.c` | **Ja, groß:** Ursache ist fehlende C++-Laufzeit im SDK (xpdf/`pdflib.cc`) |
| Endianness (Big-Endian-Dateiformate, Wortbitmaps) | `degasldg.c`, `neoldg.c`, `godldg.c`, `rawldg.c`, `imgldg.c`, `custom_font.c`, `av_prot.c` | Nein (Little-Endian-Userland). Bleibt App-Patch |
| LP64 | `jpgldg.c` | Nein |

### 2.5 Bibliotheks-Overlays (gehören zur Werkzeugkette, gelistet der Vollständigkeit halber)

- **libc (23):** Trap-Retarget (`osbind.h` 1188 geänderte Zeilen, `mintbind.h`,
  `falcon.h`, `metados.h`), feste 32-Bit-TOS-Strukturen (`basepage.h`,
  `ostruct.h`), LP64 (`stdint.h`, `inttypes.h`, `vfprintf.c`,
  `gmp-mparam.h`, `thread.c`, `spawnve.c`, `spawnvp.c`), Endian
  (`byteswap.h`, `sys/wait.h`), clang-Alias (`labs.c` neu), NewTOS-Endungen
  `.ntp/.nap/.ncp` (`spawn.c`, `spawnvp.c`, `execvp.c`, `quickstat.c`),
  `malloc.c` (+ `__newtos_malloc_maxhunk`), `setjmp.h`, generierte
  `features.h`/`linker.h`.
- **gemlib (20):** `svc #2`-Primitive, LP64-Accessoren für `AESGLOBAL`,
  zeigerspaltende Bindungen (`a_wind_set_*`, `a_evnt_timer`, `vq*`).
- **cflib (2):** `fontsel.c` (m68k-asm-Marshalling unter `__NEWTOS__` aus),
  `inline.rsh` (Arch-Erkennung).
- **LDG (3):** `ldgload.c` (`p_env`-Cast, s. o.), `ldgutil.c` (Cookie-Jar über
  `Getcookie`), Demo-`client.c`.
- **libshare (3):** `sbrk(intptr_t)`, `malloc.c` (Alignment eines statischen
  Blocks), `calloc.c`.
- **WinDom (10):** Zeiger-in-Nachricht hi/lo (4), Cookie/Sysvar (4),
  `cicon.c` (32-Bit-Pixel), `objc_str3d.c` (Bitfeld-Reihenfolge).
- **libjpeg/libexif:** je eine Konfig-Header-Datei.

**Wiederkehrende Muster** (Kandidaten für *einen* SDK-Baustein statt vieler
Patches): Rohzugriff auf den Cookie-Jar `*(long*)0x5a0` (WinDom ×3, LDG,
historisch TeraDesk); `p_env`-int→Zeiger (TeraDesk, LDG); Sysvar-`long`-
Zugriffe (TeraDesk `main.c`, WinDom `vq_tos.c`/`cookieptr.c`); Zeiger in
AES-Nachrichten (WinDom ×4, zView `av_prot.c`). Ein dokumentierter
SDK-Header (`<newtos/lp64.h>` o. ä.: Sysvar-Leser, Message-Zeiger hi/lo,
`TOS_PTR(x)`) würde die Patches nicht beseitigen, aber vereinheitlichen und
die nächste Portierung verkürzen.

---

## 3. Doku-Audit `docs/sdk/` + `sdk/README.md`

### 3.1 Falsche Grundsatzaussage: „App-Quellen bleiben unberührt“ [gelesen + Lauf]

Die Doku erklärt durchgehend, Korrekturen gingen in die Werkzeugkette und die
App bleibe unverändert, einzige Ausnahme m68k-asm:

- `docs/sdk/README.md:61-65` („the application source is not edited … The
  only exception is genuine m68k assembly“)
- `docs/sdk/04-porting-anatomy.md:3-31` (Abschnittstitel „the application
  source stays pristine“, Zitat ADR-019 „sacrosanct … never in the app“, „An
  overlay is therefore not ‚a patch to the app‘“), `:247-251` (Skelett:
  „anything else → find the toolchain gap“)
- `docs/sdk/08-porting-checklist.md:35-37` („never in the app“)
- `docs/sdk/09-port-material.md:116-123` (§5 „Die portierte Anwendung bleibt
  unveraendert“)
- `sdk/README.md:12` („the overlay mechanism that keeps ported application
  sources pristine“), `:31` („the only place application-level changes live“)
- Skriptköpfe: `sdk/apps/build-qed.sh` („src/ untouched“, „STOP: fix the
  toolchain“), `build-teradesk.sh:15-30` („If a NON-asm .c fails … it is a
  TOOLCHAIN gap … NOT a TeraDesk edit“).

Realität (2.1–2.4): Von 9 App-Overlay-Dateien für QED/TeraDesk ist **1**
m68k-asm; **8** sind LP64-Patches, darunter eine vollständige Neufassung des
QED-Speicherverwalters. Bei zView sind 11 von 13 keine asm-Retargets. Das
Root-`README.md` sagt seit `dc11d42b` bereits korrekt „per-program patches“ —
`docs/sdk/` widerspricht ihm jetzt. Die Regel in 04/08 („jeder Nicht-asm-
Fehler ist eine Werkzeugkettenlücke“) ist für LP64-Annahmen in App-Code
**nicht erfüllbar** und führt einen Portierer in die Irre.

### 3.2 Veraltete oder falsche Einzelaussagen

| Stelle | Aussage | Ist | Quelle |
|---|---|---|---|
| `docs/sdk/README.md:10-11` | Stand Rev. `49b2c2c`, Branch `desk-rebuild`, 2026-09-17 | heute `main`, `dc11d42b`; viele Kapitel seither ergänzt | [Lauf] |
| `01-overview.md` Tabelle | „three ported Atari C libraries“, „eleven AArch64 shims“ | 11 Bibliotheken (+libm, LDG, libshare, WinDom, 5 Codecs); 14 Dateien in `newtos-arch/` | [Lauf] |
| `01` „What is NOT possible“ | „613 of 624 core files … the eleven that did not“ | 616 von 625, 9 Fehlschläge | [Lauf] |
| `03-first-programs.md` | readobj-Ausgabe `Type: Executable (0x2)`, Größen 84376 / 14224 Bytes | `Type: SharedObject (0x3)` (ET_DYN seit ARCH03 0d — der Text sagt es selbst), 101216 / 56232 Bytes | [Lauf] — Rezepte selbst funktionieren |
| `04` libc-Fehlerliste | „libc records eleven failures“ | 9 | [Lauf] |
| `04` TeraDesk Schritt 3 | Overlays: environm.c, nf_ops.c, window.h, icon.c, rsc_load.c, library.h | zusätzlich `main.c` (seit 2026-10-17, RST01/X04) — 7 | [Lauf] |
| `07-mintlib-limits.md` §6 | „31 symbols … still 33, the same list“, inkl. `bios_table`/`gemdos_table`/`xbios_table` | heute 22 (inkl. `_end`); die `*_table` fehlen nicht mehr | [Lauf] |
| `07` §4/§6 | listet unaufgelöste *Symbole* | nennt nicht die *Anwenderfunktionen*, die daran scheitern: tmpfile, mkstemp, mktemp, tmpnam, tempnam, glob, wordexp, getlogin, ttyname, ctermid, sem_open (W1, W3) | [Lauf] |
| `08-porting-checklist.md:66-69` | „`environ`/`getenv` and the `ARGV=` extension do not [work]“ | funktionieren seit PORT09 (so auch 01, 05 B6, 07 §5) — **Widerspruch innerhalb der Doku** | [gelesen] |
| `08:84` | „No `long double` arithmetic; **no libm calls**“ | libm existiert seit T07; Zeile 9 derselben Datei sagt das auch | [gelesen] |
| `09-port-material.md` | ganze Seite deutsch (Rest englisch); §1 QED unter `Applications/qed`; §2 zView, WinDom, LDG als „noch nicht portiert“ | QED unter `sdk/apps/qed`; zView/WinDom/LDG portiert (Kap. 10, 12, 13). Root-`README.md:174-175` verweist für Versionen/Lizenzen auf genau diese veraltete Seite | [gelesen] |
| `02-prerequisites.md` | Herkunft von QED/TeraDesk „unverified — no provenance file“ | `git remote`: `github.com/freemint/qed` @ `f12055f`, `github.com/freemint/teradesk` @ `b5532b8` — feststellbar, aber nirgends als Bezugsanweisung festgehalten | [Lauf] |
| `15-host-checklist.md` C | `sdk/run-host-tests.sh # 19 / 0 / 0` | 20 / 0 / 0 | [Lauf] |
| `15` A | App-Bäume „must be copied from the Mac mini“ | für einen Außenstehenden unmöglich; ein Klon-Rezept fehlt | [gelesen] |

Nicht im Einzelnen nachgeprüft: Zeilennummern-Zitate (`:NNN`) in 04/05/06/07
— bei dieser Änderungsrate sind sie mit hoher Wahrscheinlichkeit verschoben;
Stichprobe in 01 (`build-libc.sh:3-43` „a handful of overlays“) passt nicht
mehr zur heutigen Datei.

### 3.3 Issue #4: Kann ein Außenstehender mit Mac oder Linux damit einen Crosscompiler einrichten und ein einfaches GEM-Programm portieren?

**Mac: teilweise. Linux: nein (nicht beschrieben, nicht erprobt).** Konkret
fehlt:

1. **Ein Einstieg „von null“.** Es gibt keine Schrittfolge `git clone` →
   Werkzeuge installieren → `sdk/libc/build-libc.sh` … → erstes Programm.
   Kapitel 02 beschreibt, *was die Skripte aufrufen*, nicht *wie man es
   installiert* (kein `xcode-select --install`, `brew install …`,
   `rustup` im Repo-Verzeichnis; nur 15-D nennt Befehle beiläufig).
2. **Linux.** Kein Wort zu Linux. `tools/make-sd-image.sh` ist
   „macOS only“ (hdiutil/diskutil/newfs_msdos), Kapitel 06 kennt keinen
   Ersatz (mtools/mkfs.fat/losetup). Die Bibliotheksskripte selbst sind
   vermutlich Linux-tauglich (rustc-Sysroot, `md5`/`md5sum`-Weiche, `shasum`
   nötig) — **nicht gefahren**.
3. **Kein Compiler-Treiber, kein Makefile-Vorbild.** Ein eigenes Programm
   baut man nur durch Abschreiben der drei Befehle aus 03 (oder Kopieren von
   `build-qed.sh` mit fest verdrahteten QED-Pfaden). Es fehlt ein
   `newtos-cc`/`newtos-gcc`-Wrapper oder ein Makefile-Fragment, das
   Zielflags, Include-Reihenfolge, crt0, Linkerskript und Archivgruppe
   kapselt — das, was ein Atari-Entwickler als „m68k-atari-mint-gcc“ kennt.
4. **Keine Installation außerhalb des Repos.** Header liegen nur in
   entpackten Build-Bäumen (`sdk/libc/mintlib/include`,
   `sdk/gemlib/gemlib-0.44.0`), die erst ein Bibliotheksbau erzeugt; kein
   `prefix`/Sysroot zum Mitnehmen.
5. **Ausführen ohne eigenen Kernelbau.** 06 setzt `cargo xtask run` voraus;
   dass ein Release-ZIP (Root-README) einen fertigen Kern/ein Kartenabbild
   enthält, auf das man nur sein Programm kopiert, steht nicht in `docs/sdk/`.
   Für eine echte Pi-Karte gibt es keine Kopieranleitung für Linux/Windows.
6. **Ein Portierungsbeispiel eines *fremden* einfachen GEM-Programms.** Die
   Beispiele sind QED/TeraDesk (groß, gepatcht) oder selbstgeschriebenes
   MINIGEM. Ein Durchgang „kleines Atari-GEM-Programm mit `.RSC` nehmen,
   Flags, `mkrsc.py convert`, typische LP64-Fehler, fertig“ fehlt.
7. **Ehrliche Liste „was nicht linkt“ als Funktionsnamen** (W1–W3) statt
   Symbol-Interna; ein Anfänger mit `tmpfile()` oder `powf()` landet heute
   bei einer `getpwuid`-/`__addtf3`-Fehlermeldung ohne Hinweis.
8. **Sprache/Zielgruppe:** Kapitel 09 deutsch, Rest englisch; die Doku ist
   in weiten Teilen Projektprotokoll (TASK-IDs, Messläufe, Zeilennummern)
   statt Anleitung — für Außenstehende schwer zu filtern.

---

## 4. Was gefahren wurde, was offen bleibt

Gefahren [Lauf]: alle 12 Bibliotheksbauten (libc frisch aus Tarball); 3218
Einzel-Link-Proben über alle Archive; Probe-Kompilat von 9 nicht gebauten
mintlib-Verzeichnissen; Gegenprobe „pwdgrp+socket ergänzt → W1 verschwindet“;
`_Static_assert`-Prüfung von 17 LP64-Grenzwerten; Alias-Vollprüfung; Host-
Tests; App-Bauten QED/TeraDesk/CoreMark/hello; Rezepte aus 03 wörtlich;
Anfängerprogramm tmpfile/stat/powf; `diff` aller 85 Overlay-Dateien gegen
Upstream (Bibliotheken gegen frisch entpackte Tarballs, Apps gegen
Git-Checkouts); `git status` aller vendorierten App-Bäume.

Nicht gefahren / offen:
- Laufzeit auf NewTOS (QEMU/Pi) für irgendeinen Befund — W4 (`st_mtime=0`)
  ist nur aus dem Code abgeleitet.
- Verhalten von `tmpfile()` & Co. *nach* Aufnahme von `pwdgrp/` (linkt —
  ob es auf NewTOS eine Datei anlegt, ist offen).
- Linux-Host (kein Linux verfügbar).
- `build-zview.sh`, `build-doom.sh`, `build-present.sh`, LDG-Demo/Client nicht
  neu gebaut (nur ihre Bibliotheken).
- Ob `library/utility/malloc.c` auch upstream von TeraDesks `SRCS`
  ausgeschlossen ist (Skriptbehauptung, nicht geprüft).
- gemlib-Overlays inhaltlich nicht gegen die AES/VDI-Spec geprüft (nur
  Umfang/Zweck) — außerhalb dieses Auftrags.

## 5. Out-of-scope-Beobachtungen

- `CLAUDE.md` nennt `kernel/src/desk/` als EmuDESK-Ort; existiert nicht, der
  Desktop liegt unter `programs/desktop/` (Root-README stimmt).

---

## 6. Vorgeschlagene Arbeitspakete (priorisiert, ohne Umsetzung)

| # | Paket | Größe | Wirkung |
|---|---|---|---|
| 1 | `pwdgrp` in `DIRS` aufnehmen (16/16 kompilieren); Laufzeitprobe tmpfile/mkstemp/getpwuid auf NewTOS (QEMU) | S | W1: Temp-Dateien, glob, wordexp, getlogin linken |
| 2 | Doku-Grundsatz korrigieren: „App-Patches“ statt „App bleibt unberührt“ in `docs/sdk/README.md`, 04, 08, 09 §5, `sdk/README.md`, Skriptköpfen `build-qed.sh`/`build-teradesk.sh`; Regel neu: „erst Werkzeugkette prüfen, sonst App-Patch mit Kopfkommentar“; zView-Overlay-Ort erwähnen | S | Konsistenz mit Root-README, keine Irreführung |
| 3 | Veraltete Fakten bereinigen (Tabelle 3.2), 08 environ/libm-Widerspruch, 09 aktualisieren oder ins Englische + Bestand neu, Revisionsangabe im README | S–M | Doku stimmt wieder |
| 4 | compiler-rt-Bausteine für binary128 + `__muldc3`/`__mulsc3` (LLVM-Builtins als Quell-Tarball pinnen und mitbauen, statt Einzelfunktionen in `softfloat.c`) | M | W2: powf, remainder, fdim, qecvt … linken |
| 5 | Kapitel „Quick start für Außenstehende“ (Mac **und** Linux): Installation, Klonen, Bibliotheken bauen, erstes Programm, auf Release-Karte/-Abbild kopieren; App-Bezugsquellen (freemint/qed @ f12055f, freemint/teradesk @ b5532b8, doomgeneric @ dcb7a8d) als Klon-Rezept | M | Issue #4 |
| 6 | Compiler-Treiber `sdk/bin/newtos-cc` (+ `newtos-ar`) bzw. `sdk/newtos.mk` mit Flags, Includes, crt0, Linkerskript, Archivgruppe; Beispielprojekt `sdk/examples/template/` | M | Issue #4: eigenes Programm ohne Abschreiben von Skripten |
| 7 | `newtos-arch/nf_ops.c` („NatFeats absent“), dann TeraDesk-Overlay `nf_ops.c` durch Build-Ausschluss ersetzen | S | W7 + ein App-Patch weniger |
| 8 | `__sys_fstat` in `newtos-arch/do_fstat.c` bereitstellen | S | W3: ttyname/ctermid |
| 9 | Zeitstempel in `stat`/`fstat`: entweder `Fxattr`/`Fstat64` im Kern oder mintlibs Fsfirst-Zeitstempelpfad wieder zulassen (`__unixtime(dta_time, dta_date)`) | M | W4 |
| 10 | `stdint.h`-Overlay: `SIZE_MAX`, `PTRDIFF_MIN/MAX` für LP64; `SSIZE_MAX` = `LONG_MAX`; `_Static_assert`-Host-Test | S | W5 |
| 11 | `unix/ucontext.c` und `syscall/*` (Generator) aus `libc.a` ausschließen; `gets.c`/`getwd.c`/`siggetmask.c` als Overlay ohne `.stabs` | S | W6/W7 |
| 12 | Linux-Pfad für das Abbild (mtools/mkfs.fat oder `cargo xtask`-Rust-Implementierung) | M | Issue #4, Linux |
| 13 | Entscheidung `-Wno-error=int-conversion` für App-Bauten (macht `environm.c`-/`ldgload.c`-Patches überflüssig, kostet Fehlerschärfe); dokumentieren, was gewählt wurde | S | 1–2 Patches weniger |
| 14 | Gemeinsamer SDK-Header für wiederkehrende LP64-Muster (Sysvar-Leser, Cookie, Zeiger-in-Message hi/lo) + Kapitel „Muster für App-Patches“ | M | kürzere, einheitliche Patches künftiger Ports |
| 15 | Prüfung, ob `socket/`, `crypt/`, `shadow/`, `argp/` aufgenommen werden sollen (kompilieren fast vollständig) — erst nach Bedarf einer echten App | S | Funktionsumfang |
| 16 | C++-Laufzeit im SDK (Voraussetzung für zView-PDF und weitere Ports) | L | 2 zView-Patches entfallen, neue Ports möglich |
