# LIBC-FSTAT: fstat / __do_fstat fuer umgelenkte Standard-Handles (2026-10-17)

Scope: TASK/LIBC-FSTAT.md, Researcher-Schritt. Nur Doku/Analyse, kein Code, kein
Lauf. Basis: main bei 7ec72f4c. Pfade repo-relativ; `mintlib/` =
`sdk/libc/mintlib/` (aus `sdk/libc/mintlib-0.60.1.tar.xz`, gitignored).
Label: **[R]** = in dieser Sitzung im Code gelesen, **[I]** = Schlussfolgerung.

## 0. Zusammenfassung

1. Upstream `__do_fstat` (`mintlib/mintlib/do_fstat.c:80-159`) erkennt auf TOS
   (`__mint == 0`) Geraet vs. Datei so: `isatty(fd)` wahr -> sofort "TTY"
   (`:102-103`, `:107-114`), sonst `Fdatime(fd, get)`; scheitert das, ebenfalls
   "TTY". Eine auf eine Datei umgelenkte Standard-Handle ist damit `S_IFREG`,
   weil `isatty` (Fseek-Probe, `mintlib/isatty.c:28-48`) sie als Datei erkennt. [R]
2. NewTOS liefert seit FS10/FS11/LIBC-TTY jede dazu noetige Antwort korrekt
   (Quellspur 2.1). Upstream `__do_fstat` ist trotzdem **nicht** zu uebernehmen:
   es meldet `st_blksize = 1024` (`:150`, bricht die FS08-32-KiB-Regel), liest bei
   *jedem* Aufruf 2 Bytes ab Dateianfang (`:136`, SD-Zugriff) und wird
   von `__stdio_text_write` bei **jedem Flush** gerufen (`stdio/sysd-stdio.c:210`); es
   zieht Zeit-/uid-/inode-Abhaengigkeiten, die die lean-Variante bewusst meidet.
3. Richtige minimale Aenderung: im Overlay die Konsolen-Entscheidung nicht mehr
   per Handle-Nummer, sondern per `__isatty(fd)` fuer die Standard-Handles
   0..5 treffen (derselbe Cache, dieselbe Probe wie `isatty()`); 32-KiB-Regel von
   "fd >= 6" auf "kein Geraet, Fseek(END) lesbar, fd >= 0" erweitern. Details 4.
4. Folge fuer umgelenkte Standard-Streams: `S_ISREG` -> `main.c:68-71` setzt den
   Modus aus `__default_mode__` (Default **Text**, ohne `UNIXMODE=b`): stdout/stderr
   in eine Datei bekommen LF -> CRLF, stdin aus einer Datei CRLF -> LF; Puffer 32 KiB
   statt 1024. Heute (CHR gemeldet) bleiben sie **binaer**, 1024. Das ist die
   Upstream-TOS-Semantik und zugleich eine sichtbare Verhaltensaenderung.
5. Keine unserer Apps (TeraDesk, QED, Doom, APITEST) liest `fstat`/`isatty`/
   `fileno` im eigenen Code; ihr Konsolenstart bleibt bitgleich (CHR, 1024,
   Zeilenpuffer). EmuCON ist Kernel-Rust ohne Umlenkung. Betroffen sind nur
   Programme, deren Standard-Handles tatsaechlich auf eine Datei zeigen.

## 1. Was upstream auf TOS tut

### 1.1 Aufrufkette

`fstat()` = `__fstat` -> `__do_fstat(fd, st, 1)` (`mintlib/unix/fstat.c:5-9`).
`stdio_init.c` ruft `__do_fstat(fd, &statb, 0)` (`:25-28`, `:49`). `main.c` ruft
`fstat(fd, &st)` (`mintlib/main.c:63`, also exact=1). `__stdio_text_write`
ruft `fstat(fd, &statbuf)` (`stdio/sysd-stdio.c:210`) und nimmt `st_blksize`
(gedeckelt auf 8192) als Chunkgroesse (`:203-208`). [R]

### 1.2 `__sys_fstat` (native Wege), `do_fstat.c:16-65`

Reihenfolge: `Ffstat64(fd, st)` (Trap 0x15d, `mint/mintbind.h:267`); bei
`-ENOSYS` `Fcntl(fd, st, FSTAT64)`; bei `-ENOSYS`/`-EINVAL` `Fcntl(fd,&xattr,FSTAT)`.
Auf TOS ohne MiNT liefern alle `-32` (EINVFN = `-ENOSYS`); `__do_fstat` faellt bei
`r == -ENOSYS` in den TOS-Zweig (`:85-86`). [R]

### 1.3 TOS-Zweig, `do_fstat.c:94-156`

| Zeile | Schritt |
|---|---|
| `:95` | `memset(st, 0)` |
| `:102-105` | `if (!__mint && isatty(fd)) r = -EBADF; else r = Fdatime(&timeptr, fd, 0);` |
| `:107-114` | `r < 0` -> "assume TTY": `S_IFCHR|0600`, `st_size = 0`, bei `exact` Zeit = `time(0)-2` |
| `:116-121` | sonst `S_IFREG|0644`, bei `exact` Zeit aus `Fdatime` (`__unixtime`) |
| `:124-126` | `oldplace = Fseek(0,fd,SEEK_CUR)`; `< 0` -> `S_IFIFO|0644`, `st_size = 1024` ("must be pipe") |
| `:130-141` | sonst `Fseek(END)` = `st_size`; `Fseek(0,SET)`; `Fread(fd, 2, &magic)`; `0x601a` oder `0x2321` -> `|0111`; `Fseek(oldplace,SET)` |
| `:146-154` | `st_dev = Dgetdrv()`, `getuid/getgid`, **`st_blksize = 1024`**, `st_blocks`, `st_ino = __inode++`, `st_nlink = 1` |

Der Kommentar `:73-74` nennt ausdruecklich den Bug: ungueltiger fd liefert eine
tty-Struktur. [R]

### 1.4 Wie ein umgelenktes Standard-Handle erkannt wird

Ausschliesslich ueber `isatty(fd)` (Fseek-Probe, `mintlib/isatty.c:28-48`:
Fcntl-TIOCGPGRP `-ENOSYS` -> `Fseek(1,fd,CUR) == 0` heisst Zeichengeraet; ein
Offset oder ein Fehler heisst Datei), danach (nur wenn `isatty` falsch) `Fdatime`.
Es gibt keinen `Finstat`/`Foutstat`-Pfad. Auf echtem TOS/EmuTOS liefert
`Fseek` auf ein per `Fforce` umgelenktes Handle die Dateiposition (EmuTOS
`bdos/bdosmain.c` Fseek-Geraeteblock endet in `return 0`, Datei geht durch
`ixlseek`; Beleg in `docs/analysis/libc-tty-2026-10-13.md` 1). [R, dort belegt]
`isatty` cached pro Handle (`__open_stat[]`, `isatty.c:23-24`); zurueckgesetzt
von `open/close/dup/dup2/pipe` und beim Start (`main.c:161-162`). [R]

### 1.5 Warum NewTOS ein eigenes `do_fstat.c` hat

* Commit `bbf45898` (2026-07-18, Erstport) legte `newtos-arch/do_fstat.c` an;
  Kommentar `do_fstat.c:1-31`: der volle `__do_fstat` ziehe Zeitumrechnung
  (`__unixtime`, `localtime`, 200-Hz-Uhr via `get_sysvar`), Besitzer
  (`getuid`/`getgid` -> passwd/group) und `isatty -> ioctl`; "exactly the tail
  that dragged the 6 unresolved symbols (get_sysvar, getpwuid, getgrent, setgrent,
  endgrent, ioctl)". `build-libc.sh:171` schliesst `mintlib/do_fstat.c` aus,
  `:259` baut den Shim. [R]
* Commit `0abc634a` (FS08 Teil 4, 2026-10-07) fuegte `NEWTOS_FILE_BLKSIZE` hinzu
  (Messungen `measurements/measurement-2026-10-07-fs08-pi-after/`, `...-stdiobuf/`,
  `docs/sdk/07-mintlib-limits.md:57-70`). [R fuer den Kommentar, Messungen nicht
  erneut gelesen]
* Der Shim klassifizierte 0..2 per Nummer, solange `isatty` ebenfalls ein Shim war
  (Fixbereich 0..2). Seit `66f861f5` (LIBC-TTY) ist `isatty` upstream; der Shim
  wurde dabei absichtlich nicht angefasst (`TASK/LIBC-TTY.md` Entscheidung 4). [R]
* [I] Die transitive Herkunft der 6 Symbole (insb. `getpwuid`/`getgrent` aus
  `getuid`) wurde nicht nachgebaut; `unix/getuid.c:12-19` liest nur `__uid`/
  `Pgetuid`. Welche der 6 Symbole `mintlib/do_fstat.c` heute tatsaechlich
  offenlaesst, ist **nicht verifiziert** (Link-Versuch noetig).

## 2. Wuerde upstream `__do_fstat` auf NewTOS laufen?

### 2.1 Quellspur der Aufrufe

| Aufruf | NewTOS | Beleg | Ergebnis |
|---|---|---|---|
| `Ffstat64` 0x15d | `id > MAX_FN` (0x57) und `!= SSYSTEM` (0x154) -> `EINVFN` | `tos-api/src/gemdos/mod.rs:151-156`, `ssystem.rs:31` | -32 -> weiter |
| `Fcntl(FSTAT64)`, `Fcntl(FSTAT)` (0x104) | `EINVFN` | `mod.rs:154-155` | -32; `r != 0`, also `-ENOSYS` zurueck -> TOS-Zweig |
| `isatty(fd)` | siehe `libc-tty-2026-10-13.md` 2.2 | `file.rs:231-246,914-934` | Geraet 1, Datei 0 |
| `Fdatime(fd,get)` | `ofd_of(handle)` loest ueber `resolve()` auch ein umgelenktes Std-Handle auf die Datei auf; Geraet/ungueltig -> `EIHNDL`; sonst Zeit+Datum aus Verzeichniseintrag | `file.rs:874-884`; EmuTOS `fsdir.c:684-692`, `fsmain.c:474-485` | ok bzw. `< 0` -> "TTY" |
| `Fseek` CUR/END/SET | wie `libc-tty` 2.2; Dateipfad `ixlseek` | `file.rs:914-934` | Position/Groesse |
| `Fread(fd,2,&magic)` | Dateipfad: **keine Modus-/Lesepruefung** (`o.mode` wird nur erfasst), liest ab `bytnum` | `file.rs:393-430` | liest auch auf Schreib-Handles, echter Medienzugriff |
| `Dgetdrv`, `getuid`, `getgid`, `__inode` | `getuid.c:12-19` -> `__uid` (nicht `Pgetuid`, da `__mint==0`) | `unix/getuid.c`, `mintlib/inode.c:5` | harmlos, aber Linkabhaengigkeit |

Fazit [R]: **funktional** laeuft der Upstream-Zweig auf NewTOS; ein umgelenktes
fd 1 wuerde `S_IFREG`, mit Groesse. Offen [unverifiziert]: ob alle sechs Symbole des
Erstports heute aufgeloest sind (siehe 1.5).

### 2.2 Warum trotzdem nicht uebernehmen

1. `st_blksize = 1024` (`:150`) hebelt die FS08-Regel aus (Faktor ~9 im Durchsatz,
   `docs/sdk/07-mintlib-limits.md:57-70`) fuer **jeden** `fopen`.
2. `Fread(fd, 2, ...)` (`:136`) plus 3 `Fseek` + `Fdatime` je Aufruf. `fstat`
   laeuft nicht nur beim `fopen`, sondern bei **jedem Text-Modus-Flush**
   (`sysd-stdio.c:210`) und je Standard-Stream einmal in `main.c:63`. Der Lese-Zugriff
   ist auf NewTOS ein echter Cache-/SD-Zugriff am Dateianfang. [R fuer Aufrufstellen;
   Kostengroesse I]
3. Exec-Bit aus Magic (`:137-139`): das lean-`do_fstat` setzt bewusst keins
   (`docs/sdk/07-mintlib-limits.md:363-365`).
4. `st_ino = __inode++` ist pro Aufruf wechselnd (nicht stabil), `st_dev =
   Dgetdrv()`: Tail, den der Shim absichtlich nicht hat.

## 3. Empfohlene Aenderung (nur `sdk/libc/newtos-arch/do_fstat.c`)

Idee in Worten (kein Code geschrieben):

* Konsolen-Zweig: `if (fd >= 0 && fd < NEWTOS_NUMSTD && __isatty(fd))` statt
  `fd >= 0 && fd <= 2`. Gleicher Cache wie `stdio_init.c:56` und `main.c:56,168,195`
  -> `fstat` und `isatty` koennen sich nicht widersprechen. Der Cache wird beim
  Start fuer 0..2 geleert und durch `main.c` sofort gefuellt, daher kostet der Aufruf
  in `__stdio_text_write` keinen Trap.
* Datei-Zweig unveraendert (Fseek CUR/END, Restore); `st_blksize` 32 KiB, wenn
  `fd >= 0 && end >= 0` (statt `fd >= NEWTOS_NUMSTD`). Die Zahl 6 wurde nur
  gebraucht, um Geraete-Handles 3..5 auszuschliessen, die `Fseek` mit 0
  beantworten; das uebernimmt jetzt `__isatty`. Negative Handles (BIOS-Geraete,
  `Fseek` -> 0) bleiben wie bisher BUFSIZ (`fd >= 0` in der Bedingung).
* Handles >= 6 **nicht** ueber `__isatty` leiten (Begruendung: `__open_stat` wird nur
  von libc-`open/close/dup/dup2` gepflegt; ein roh per `Fopen`/`Fclose` benutztes
  Handle kann einen veralteten `FH_ISATTY`-Eintrag erben und eine Datei als
  Zeichengeraet melden -> Zeilenpuffer, entferntes Seek. [I aus `close.c:14-30`,
  `open.c:343-347`, `isatty.c:23-24`]). Fuer 0..5 ist der Cache beim Start
  geleert.
* `exact`, Zeiten, Owner: unveraendert nicht aufgeloest.

Beobachtbare Verhaltensaenderungen:

| Fall | heute | nach Aenderung |
|---|---|---|
| fd 0..2 Konsole | CHR, 1024 | unveraendert (CHR, 1024, Zeilenpuffer, kein Seek) |
| fd 0..2 per Fforce auf Datei | CHR, 1024, **binaer** | `S_IFREG`, echte Groesse, 32 KiB, **Text** (siehe 4) |
| fd 3..5 Standardgeraet (AUX/PRN) | `S_IFREG`, Groesse 0, 1024 | `S_IFCHR`, 1024 (Seek-Probe -> Geraet) |
| fd 3..5 auf Datei umgelenkt | `S_IFREG`, 1024 | `S_IFREG`, 32 KiB |
| fd < 0, ungueltiger fd | unveraendert | unveraendert (Rueckgabe 0; EBADF waere eine eigene Entscheidung) |

## 4. Auswirkungen auf Programme

### 4.1 stdio-Pufferung
`stdio_init.c:52-81`: Zeilenpuffer + Seek-Entfernung nur wenn `S_ISCHR && __isatty`;
Puffergroesse immer `st_blksize`. `main.c:195` macht stdout bei `!isatty(1)` voll
gepuffert (unabhaengig von `fstat`). Heute: umgelenktes stdout -> voll gepuffert
mit 1024 B (CHR gemeldet, aber nicht tty, Seek bleibt). Danach 32 KiB, Seek bleibt.
Speicher: 32 KiB `malloc` je umgelenktem Standard-Stream (stdin/stdout/stderr
nur wenn tatsaechlich umgelenkt; stderr ist unbuffered, `main.c:193`). Die 8-KiB-
Chunkung im Text-Schreiber bleibt (`sysd-stdio.c:205-214`). [R]

### 4.2 Text-/Binaermodus und CR/LF
`main.c:38-72`: alle Std-Streams starten binaer; `!__mint && isatty(fd)` -> Text
(Konsole braucht CRLF). Sonst `fstat`: `S_ISREG` -> `__set_binmode(stream,
__default_mode__.__binary)`. `__default_mode__.__binary` ist 0 (Text), ausser
`UNIXMODE` enthaelt `b` (`main.c:126-130`) oder `__libc_enable_secure`
(`:115-117`). Heute (CHR gemeldet) wird der Zweig nie erreicht: Umlenkung =
binaer, kein CR. Danach: Text -> `__stdio_text_write` fuegt vor jedem `\n` ein
`\r` ein (`sysd-stdio.c:232-236`), der Text-Leser quetscht `\r\n` -> `\n`
(`:106-135`). Das ist dasselbe, was `fopen("f","w")` ohne `b` auf TOS tut
(`fopen.c:41`) und entspricht Upstream-TOS. **Binaerdaten durch stdout/stdin
brauchen dann `UNIXMODE=b` oder `setmode`/`_binmode`**; sichtbarer Unterschied
fuer Programme, die auf NewTOS bisher saubere LF-Dateien per Umlenkung erzeugen.
[R fuer Code, Auswirkung I]

### 4.3 isatty-Konsistenz
Heute: `isatty(1) == 0` (Probe) aber `fstat(1)` -> CHR: Programme, die `S_ISCHR`
statt `isatty` fragen, glauben an eine Konsole. Danach identisch (gleicher Cache).
Bekannte Upstream-Grenze bleibt: ein `Fforce` **im Programm** nach der ersten
`isatty`-Frage wird nicht gesehen (`libc-tty` 1); fuer vom Elternprozess vererbte
Umlenkung (Kind) gilt das nicht, da der Cache beim Start leer ist.

### 4.4 Vererbung der Umlenkung ans Kind
`docs/sdk/07-mintlib-limits.md:~368-379` behauptet "Redirection does not reach the
child yet". **Das ist veraltet**: `init_pd_files` (`tos-api/src/bdos/process/
loader.rs:773-806`, aufgerufen `:498,753`) vererbt Standard-Handles `> 0` per
`ixforce`; Commit `c2cd746b` (2026-10-06, PROC02) "children inherit forced
handles, Fforce counts uses". Doku-Korrektur noetig (nicht ausgefuehrt). Ob das
zur Laufzeit hier funktioniert, ist **nicht gemessen** (nur Quellspur).

### 4.5 Unsere Apps
Grep ueber `sdk/apps/{qed,teradesk,doom,apitest}` (Quellen, ohne Build-Ordner):
kein `fstat`/`isatty`/`fileno`/`S_ISREG`/`S_ISCHR` im Anwendungscode; alle laufen
nur durch `main.c`/`stdio_init.c`. Konsole: Ergebnis identisch zu heute (Zweig
`__isatty` wahr). TeraDesk setzt fuer Kinder `Fforce(2, Fdup(1))`
(`sdk/apps/teradesk/startprg.c:210-223`, Option "TOS_STDERR"): Handle 1 ist bei
Desktop-Start das Standardgeraet, die Dup ist ein BIOS-Geraet -> Kind sieht ein
Zeichengeraet, kein Unterschied. EmuCON: Kernel-Rust (`kernel/src/shell*.rs`),
Grep nach `Fforce`/`redirect`/`pipe` ohne Treffer -> keine Umlenkung, nicht
betroffen. DOOM: `system("... >/dev/null")` laeuft in einem Kind ohne Zenity;
die Umlenkung scheitert beim `Fcreate("null:")` (`07-mintlib-limits.md`), keine
fstat-Abhaengigkeit. Nicht gelaufen. [R Grep, I Aussage]

## 5. Testliste

### 5.1 SDK-Hosttests (`sdk/libc/tests/fstat-host*`)
`fstat-host-fs.c` braucht ein Umschalt-Modell: bisher liefert `Fseek` fuer
`h <= 2` immer 0 (Geraet). Neu: Funktion `fh_redirect(h, size)`, die ein
Standard-Handle (0..5) in die Dateitabelle haengt (Fseek -> Position), und
`fh_reset_cache()` (`__open_stat[]` leeren, wie `main.c:161`).
1. `fstat(0..2)` Konsole: `S_IFCHR`, `st_blksize 1024`, Zeilenpuffer, kein Seek
   (bestehende `small_case` bleiben gruen).
2. Pro Standard-Handle 0, 1, 2 nach `fh_redirect`+Cache-Reset: `S_IFREG`, echte
   Groesse, `st_blksize 32768`, Position unveraendert, `fh_probe_stream`: 32768,
   nicht Zeilenpuffer, seekbar.
3. Handles 3, 4, 5 als Geraet: `S_IFCHR`, 1024 (bisher "seekable file"-Fall
   `small_case(3, ...)` wird neu: 3 als Geraet CHR; Variante 3 umgelenkt REG 32K).
4. Cache: `fstat` nach `isatty` macht keinen weiteren Trap fuer 0..2 (Zaehler
   `fh_trap_count`); Konsistenz `isatty(fd) == S_ISCHR(fstat(fd))` fuer 0..5
   vor und nach Umlenkung+Reset.
5. Stale-Fall (dokumentierend): Cache `FH_ISATTY`, dann Umlenkung ohne Reset ->
   weiter CHR (Upstream-Grenze, als Befund-Test).
6. Unveraendert: fd 6, 7 (leer), 8 (REG, 32 KiB), 9 (nicht offen, 1024), -1 (1024).
7. Gegenprobe (Counter-Check wie `method/fstat-host-countercheck.sh`): mit dem
   alten Shim muessen 2 und 3 rot werden.
8. Optional: Text-/Binaermodus-Entscheidung von `main.c:init_stream_mode`
   (static) nachbilden: Konsole -> Text; REG -> `__default_mode__.__binary`.

### 5.2 QEMU-Sonde (Vorlage `stderra.c` / `stdoutdbg.c`, Eltern-Kind per `spawnl`)
Kind **muss** vom Elternteil umgelenkt gestartet werden (ein `Fforce` im selben
Prozess nach Start trifft den Cache, siehe 4.3).
* Eltern: `Fcreate` Datei `OUT.TXT`, `Fforce(1, fh)`, `spawnl(P_WAIT, "FSTATK.NAP",
  "FSTATK", ...)` (ARGV gesetzt, damit `main.c:168` kein `Fforce(2,-1)` macht),
  `Fforce(1, -1)` zurueck (nicht `Fclose` einer Dup, Use-Count-Hinweis
  `07-mintlib-limits.md`), `Fclose(fh)`, `Fopen` + `Fread` und Byte-Vergleich.
* Kind: meldet ueber `Cconws` (nicht stdio) `fstat(0..2)`-Mode, `st_blksize`,
  `isatty(0..2)`, `stdout->__bufsize`, `stdout->__mode.__binary`; dann `printf("a\nb\n")`.
  Erwartung: `fstat(1)` `S_IFREG`, `st_blksize 32768`, `bufsize 32768`, Datei
  `61 0D 0A 62 0D 0A` (Text-Default); mit `UNIXMODE=b` im Env: `61 0A 62 0A`.
  `fd 0` und `2` am Terminal: weiter `S_IFCHR`.
* stdin: Eltern `Fforce(0, fh)` auf Datei `x\r\ny\r\n`; Kind `fgets` -> `x\n`, `y\n`,
  danach EOF; `fstat(0)` `S_IFREG`.
* Kontrolle ohne Umlenkung (Konsole): `S_IFCHR`/1024 fuer 0..2, `isatty` 1 —
  unveraendert zu `measurement-2026-10-13-libc-tty`.
* Regressionsstichprobe: Desktop, TeraDesk, QED, Doom, APITEST (Start/Beenden,
  Pufferung, stderr), wie bei LIBC-TTY.
* Voraussetzung pruefen (Messung, nicht Lesen): Vererbung `p_uft` ans Kind
  (4.4); falls sie in der Laufzeit nicht ankommt, ist das ein Kernel-Fund und die
  Sonde nicht aussagekraeftig.

## 6. Offene Fragen an den Anwender

1. Text-Default fuer umgelenkte Std-Streams (LF->CRLF) wie Upstream-TOS
   uebernehmen, oder umgelenkte Streams bewusst binaer lassen (z. B. im Overlay
   `main.c`-Verhalten per `UNIXMODE=b` als NewTOS-Default)? Das ist Kontrakt-
   relevant fuer Programme, die LF-Dateien per `>` erzeugen.
2. fd 3..5 (AUX/PRN) werden von `S_IFREG` (Groesse 0) auf `S_IFCHR` korrigiert:
   einverstanden?
3. Ungueltiger Handle: weiter `0` + tty/Datei-Struktur (Upstream-Bug-Kommentar
   `do_fstat.c:73-74`) oder `-1/EBADF`? Der Shim lieferte nie -1; stdio und
   `main.c` ueberstehen -1 (`stdio_init.c:49-50`, `main.c:63-65`).
4. Handles >= 6, die Geraete sind (`open("con:")`): weiter als `S_IFREG` melden
   (Seek-Probe nicht benutzt, weil `__open_stat` bei Roh-`Fopen` veralten kann)
   oder `open()`-Pfad separat behandeln?
5. Doku-Korrektur `07-mintlib-limits.md` (Vererbung, "binary because fstat says
   CHR", `:188`) im selben Ticket?

## 7. Nicht verifiziert

* Kein Lauf (Host, QEMU, Pi). Alles Quelltextspur.
* Ob `mintlib/do_fstat.c` heute noch Symbole offenlaesst (`get_sysvar`, `getpwuid`,
  ...): nicht nachgebaut.
* Laufzeit-Vererbung der Umlenkung (nur `loader.rs`/Commit-Titel gelesen).
* EmuTOS-Quelle fuer `Fseek` auf umgelenkte Std-Handles nicht erneut gelesen; die
  Aussage stuetzt sich auf `libc-tty-2026-10-13.md` und `file.rs:914-934`.
* `time()`-Pfad von `exact` (nur relevant fuer den Upstream-Zweig) nicht verfolgt.
* Exakte Kosten des `Fread(2)` je `fstat` auf Pi/QEMU nicht gemessen (Groessenordnung
  Schluss aus `file.rs`, kein Messwert).

## 8. Out-of-scope observations

* `07-mintlib-limits.md:~368-379` (Vererbung) und `:188` (binaer wegen CHR) sind
  nach dieser Aenderung bzw. schon heute veraltet.
* `__stdio_text_write` ruft pro Flush `fstat` (`sysd-stdio.c:210`): jede Verteuerung
  von `__do_fstat` schlaegt dort durch (relevant fuer ein spaeteres Zeit-/Owner-Tail).
