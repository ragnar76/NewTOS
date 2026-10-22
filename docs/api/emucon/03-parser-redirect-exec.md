# EmuCON2 — Hauptschleife, Parser, Umlenkung, Fremdprogramme

Quelle: `cli/cmdmain.c`, `cli/cmdparse.c`, `cli/cmdexec.c`, `cli/cmdutil.c`,
`cli/cmd.h`. Legende: [README](README.md).

## 1. Hauptschleife (`cmdmain.c:63-142`)

Initialisierung in dieser Reihenfolge (`:70-103`):

1. `_IDT`-Cookie lesen → `idt_value` (Standard `DEFAULT_DT_FORMAT` = 12h,
   Jahr/Monat/Tag, Trenner `/`, `cmd.h:121-122`) (`:70-71`).
2. `_VDO`-Cookie → `vdo_value` (Standard `_VDO_ST`) (`:73-74`).
3. `original_res = Getrez()`, wenn `vdo_value < _VDO_VIDEL` (mit
   `CONF_WITH_TT_SHIFTER`), sonst `-1` (`:75-80`); `current_res = original_res`.
4. `original_color3 = Setcolor(3, -1)` (`:81`).
5. `nflops_copy = Supexec(get_nflops)` (liest `*(WORD*)0x4a6`, `:83,333-336`);
   **im ROM-Bau ungenutzt** (einziger Verbraucher steht unter
   `#ifdef STANDALONE_CONSOLE`, `cmdparse.c:40-54`).
6. Ist `current_res == ST_LOW` (0): `change_res(ST_MEDIUM)` (`:88-89`).
7. Banner: `ESC E`, `ESC e`; dann
   `Welcome to EmuCON2 version <version>`, `Type HELP for builtin commands`,
   leere Zeile, jeweils mit `\r\n` (`:91-95`; `version` ist die EmuTOS-
   Versionszeichenkette, `include/version.h`, `version.mk:25-40`).
8. `linewrap = 0` (**ohne** `ESC w` zu senden), `dta = Fgetdta()`,
   `redir_name = ""`, `redir_handle = -1` (`:97-100`).
9. `init_cmdedit()`; bei `< 0` `warning: no history buffers` (`:102-103`).
10. `PATH=` aus der Umgebung → `path`-Befehl (`:105-118`, siehe 01 §3).

Dann (`:120-139`) die Schleife: `init_screen()`; innen `read_line` →
`save_history` → bei `rc < 0` (Ctrl-C) `continue` → `parse_line` → bei
`argc < 0` (Parserfehler) `continue` → `execute`; `rc < 0` = `exit`
(`change_res(original_res)`, Rueckkehr); `rc == 0` normal; `rc > 0` =
Aufloesungswechsel gewuenscht: innere Schleife verlassen,
`change_res(requested_res)`, aeussere Schleife (`init_screen` neu).

`execute(argc, argv, redir)` (`:151-178`):

1. `argc == 0` → 0 (leere Zeile).
2. `func = lookup_builtin(argc, argv)`.
3. `LOOKUP_EXIT` → -1.
4. `LOOKUP_ARGS` → `rc = WRONG_NUM_ARGS`.
5. sonst Builtin: `create_redir(redir)`, `strip_quotes(argc, argv)`,
   `rc = func(argc, argv)`, `close_redir()`; `rc == CHANGE_RES` → Rueckgabe 1
   (ohne Meldung).
6. sonst Fremdprogramm: `rc = exec_program(argc, argv, redir)` (ohne
   `strip_quotes`).
7. `errmsg(rc)` — **jeder** von 0 verschiedene `rc` erzeugt eine Zeile
   ([04 §2](04-builtins.md)), auch ein positiver Exit-Code eines Fremdprogramms
   (siehe 6).

## 2. Tokenizer (`cmdparse.c:118-161` `get_next_arg`)

* Trenner ist **nur das Leerzeichen** `' '` (kein Tab, kein anderes
  Whitespace; `:126-128,149`). Mehrfach-Leerzeichen werden uebersprungen.
* Ein Argument, das mit `"` beginnt, ist ein **Anfuehrungszeichen-Argument**:
  innerhalb gilt kein Trenner (`:135-148`). Das schliessende `"` **beendet
  das Argument nicht**: der Scan geht ohne Unterbrechung weiter, bis ein
  Leerzeichen ausserhalb der Anfuehrungszeichen kommt. Ein `"` mitten in einem
  Argument, das *nicht* mit `"` begann, ist `QUOTING_ERROR` (`:141-146`);
  ein nicht geschlossenes `"` ebenfalls (`:155-156`). Es gibt **kein
  Escaping**, keine einfachen Anfuehrungszeichen, keine `~`/`$`-Expansion.
* Rueckgabe: 1 = normal, 0 = Ende, -1 = Quoting-Fehler; Zeiger wird nur bei
  `>= 0` fortgeschrieben (`:116-117`). Der Tokenizer arbeitet **in-place** auf
  dem Eingabepuffer (Leerzeichen → NUL, `:149-152`).
* `parse_line` (`:21-79`) meldet `error in quoted field` (`:68`) bzw. das nie
  erreichte `error parsing line` (`:71`). Es gibt **keine** Obergrenze
  fuer die Zahl der Argumente: `argv[argc++] = temp` (`:58`) in
  `arglist[MAX_ARGS = 30]` (`cmdmain.c:44`, `cmd.h:105`) — **(Schluss)** bei mehr als
  30 Argumenten (die Zeile fasst bis ~198 Zeichen) laeuft das Feld ueber.
* **`strip_quotes`** (`cmdmain.c:207-220`, nur fuer Builtins): beginnt ein
  Argument mit `"`, wird das erste Zeichen uebersprungen und das **letzte
  Zeichen des Arguments** durch NUL ersetzt — unabhaengig davon, ob es ein
  `"` ist. `"abc"` → `abc`; `""` → leer; `"abc"def` → `abc"de`
  **(Schluss)**. Argumente ohne fuehrendes `"` bleiben unveraendert.
* Fuer **Fremdprogramme** bleiben die Anfuehrungszeichen im Kommandozeilentext
  erhalten (`strip_quotes` wird dort nicht aufgerufen, `cmdmain.c:173`).

## 3. Umlenkung `>` (`cmdparse.c:21-106`, `cmdmain.c:180-202`)

* **Nur `>` (Ausgabe), ueberschreibend.** `>>` (Anhaengen) und `<` gibt es
  nicht; `<` ist ein gewoehnliches Zeichen. Ein zweites `>` ausserhalb von
  Anfuehrungszeichen ergibt `more than one redirection` (`:28-31`) — also
  auch `>>`. Kein Handle-Praefix (`2>`), keine Pipe, keine
  Fehlerumlenkung.
* `get_redirect` (`:88-106`) zaehlt `>` ausserhalb von `"…"` (Umschalter
  `inquotes ^= 1` bei jedem `"`), ersetzt es durch ein Leerzeichen und merkt
  die Stelle des (letzten) `>`.
* Danach nimmt `parse_line` das **erste Token hinter dem `>`** als
  Dateinamen (`redir_name`, `:55-57`, Vergleich ueber Zeigeradresse
  `temp > redir_addr`); weitere Tokens sind normale Argumente. Die
  Reihenfolge `>` vor oder zwischen Argumenten ist also frei:
  `echo a >f b` ≡ `echo a b >f`.
  Kein Dateiname → `no filename for redirection` (`:61-64`).
* Dateiname wird per `strcpy` in `redir_name[MAXPATHLEN=128]` kopiert
  (`cmdmain.c:45`, `include/sysconf.h:51-55`) — **ohne Laengenpruefung**; die
  Zeile ist bis ~198 Zeichen lang (**Schluss**: Ueberlauf moeglich).
* **Builtins** (`cmdmain.c:166,180-193`): `create_redir` ruft
  `Fcreate(name, 0)`; bei Fehler `errmsg(rc)` und der Befehl laeuft ohne
  Umlenkung weiter (`redir_handle = -1`). Ausgabe geht ueber `output()`
  (`cmdint.c:1192-1197`), das bei `redir_handle >= 0` per `Fwrite(handle, …)`
  schreibt — **kein `Fforce`**. Nach dem Befehl `close_redir` (`Fclose`,
  `redir_name = ""`, `:195-202`).
* **Nicht umgelenkt** (gehen immer auf den Schirm, `Bconout(2)`):
  der Prompt, Fehlermeldungen (`errmsg`/`messagenl`), `Copying …`-Zeilen,
  Rueckfragen, `--More--`, `wrap`-Status, `path`-Anzeige, Banner.
* **Fremdprogramme** (`cmdexec.c:199-234`): `Fcreate(redir,0)`;
  `old_stdout = Fdup(1)`; `Fforce(1, handle)`; nach dem Programm
  `Fforce(1, old_stdout)`, `Fclose(old_stdout)`, `Fclose(handle)`; `redir[0] = 0`.
  Misslingt `Fforce`, wird zurueckgebaut (`:215-218`). Nur **Handle 1**.

### Auffaelligkeit: veraltete Umlenkung (Schluss)

`redir_name` wird nur von `close_redir` (Builtin-Weg, `cmdmain.c:200`) und
`restore_stdout` (Fremdprogramm-Erfolgsweg, `cmdexec.c:223-232`) geleert.
Folgende Wege laufen **ohne** eines von beiden und lassen den Namen stehen:
`argc == 0` bei einer Zeile wie `>datei` (`cmdmain.c:156-157`);
`LOOKUP_ARGS` (`:163-164`); `exec_program`-Fruehabgaenge (`EPTHNF`,
`CMDLINE_LENGTH`, `EFILNF`, `Fcreate`-Fehler, `cmdexec.c:35-48`); ein
Parserfehler **nach** gefundenem Dateinamen (`cmdparse.c:55-57` vor `:67-69`).
Der naechste Befehl wird dann in diese Datei umgeleitet (der Name wird nur
ueberschrieben, wenn die naechste Zeile selbst ein `>` hat). Beispiel
(Schluss): `nosuch.prg >log` → `file not found`; danach schreibt `dir`
nach `log` statt auf den Schirm. **Nicht ausgefuehrt**, nur gelesen; Entscheidung
in [06 D7](06-mapping-and-decisions.md).

## 4. Jokerzeichen

EmuCON **expandiert selbst nichts**. `*`/`?` gehen unveraendert an GEMDOS
(`Fsfirst` u. a.). Wo Muster wirken, steht in [04](04-builtins.md):
`cat`, `more`, `cp`, `mv`, `rm`, `ls`, Tab-Vervollstaendigung; `has_wildcard`
(`cmdutil.c:257-266`, prueft `?` und `*`) wird an **genau zwei** Stellen
benutzt (Suche `has_wildcard` ueber `cli/`): Joker im Programmnamen →
`EPTHNF` (`exec_program`, `cmdexec.c:35-36`) und bei `rm` die Rueckfrage
`Delete ALL matching files` (`cmdint.c:525`). `cp`/`mv` pruefen das Ziel mit
`check_path_component`, das bei `?`/`*` `EPTHNF` liefert (`cmdint.c:1145-1148`).
Ein Fremdprogramm bekommt Joker unveraendert im
Kommandozeilentext (kein ARGV).

## 5. Befehlsaufloesung (`cmdint.c:167-205` `lookup_builtin`)

Reihenfolge:

1. **Laufwerkswechsel:** `argc == 1`, `strlen(argv[0]) == 2` und
   `argv[0][1] == ':'` → `run_setdrv` (`:174-176`; vor der Tabelle).
2. Tabellensuche mit `strequal` = **ohne** Gross-/Kleinschreibung
   (`cmdutil.c:294-313`), auf `name` oder `synonym` (`:181-187`). Nicht
   gefunden → `NULL` = Fremdprogramm.
3. **`-h`:** genau `argc == 2` und `argv[1]` = `-h` → es wird `help <befehl>`
   ausgefuehrt (`:194-198`; `argv[1] = argv[0]; argv[0] = "help"`). Wirkt
   **vor** der Argumentzahlpruefung und fuer jeden Befehl, auch `exit`
   (`exit -h` zeigt die Hilfe).
4. Argumentzahl: `argc-1` ausserhalb `[minargs, maxargs]` → `LOOKUP_ARGS`
   (`:200-202`).

Das gilt auch fuer `exit` (`LOOKUP_EXIT`, `maxargs 0`): `exit foo` →
`wrong number of arguments`.

## 6. Fremdprogramme (`cmdexec.c:29-58` `exec_program`)

Reihenfolge:

1. `has_wildcard(argv[0])` → `EPTHNF` (`path not found`) (`:35-36`).
2. `build_cmdline(cmdline, argc, argv)` (`:84-105`): ab `argv[1]`, durch
   **ein** Leerzeichen getrennt (Quotes bleiben drin); Zaehlung `len`;
   hoechstens **125 Zeichen** (`MAXCMDLINE`, `cmd.h:98-99`); ueberlaenger →
   `CMDLINE_LENGTH` (`cmdline too long`). Format: `cmdline[0] = len`,
   Zeichen ab `[1]`, NUL-terminiert. Es gibt **kein `ARGV`** (keine
   Umgebungsvariable, kein 127er-Ueberlauf).
3. `find_executable(path, argv[0])` (`:131-171`): Name kopieren; letzten `.`
   merken (zurueckgesetzt nach `:`/`\`); fehlt ein Punkt, `".*"` anhaengen;
   `Fsfirst(name, 0x07)` und `Fsnext` (0x07 = RO|HIDDEN|SYSTEM, **keine
   Ordner**); fuer jeden Treffer `program_extension(dta)`: erster Treffer mit
   Programmendung gewinnt, die Endung wird aus dem **Verzeichniseintrag**
   hinter den Punkt geschrieben (`strcpy(dot+1, q)`, Schreibweise wie im
   Dateisystem). Es entscheidet also die Verzeichnisreihenfolge, wenn
   `FOO.PRG` und `FOO.TOS` beide existieren.
4. Nicht gefunden: `check_user_path` (`:112-126`): jede Komponente von
   `user_path` (Trenner `;` oder `,`, `get_path_component`
   `cmdutil.c:230-255`), `\` anhaengen wenn nicht schon da (`add_to_path`,
   `:63-77`), derselbe `find_executable`. Nichts → `EFILNF`
   (`file not found`). Der Pfad-Suchlauf findet auch absolute
   Angaben bei jedem Fehlversuch (**Schluss**, harmlos).
5. `redirect_stdout(redir)` (siehe 3); Fehler → `rc` zurueck.
6. `is_graphical(path)` (`:176-194`): die letzten drei Zeichen vor dem
   Ende (`.` an Position -4) gleich `app`/`gtp`/`prg` (Gross/Klein egal) →
   `Cursconf(0, 0)` (Cursor aus).
7. `rc = Pexec(0, path, cmdline, NULL)` (GEMDOS 0x4b, Modus 0 = laden+starten;
   Umgebung `NULL` = Kopie).
8. `Cursconf(1, 0)` **immer** (Cursor an) — auch bei Programmen ohne
   Cursor-Aus (`:53`).
9. `restore_stdout(redir)`; Rueckgabe `rc`.

**Anzeige des Rueckgabewerts.** `execute` uebergibt den Rueckgabewert von
`Pexec` an `errmsg` (`cmdmain.c:173-175`). `Pexec` (Modus 0) liefert den
Pterm-Code des Kindes, auf `UWORD` verkuerzt (`bdos/proc.c:600-616`:
`run->p_dreg[0] = rc`, `xterm(UWORD rc)`), oder einen negativen Fehlercode.
Folgen (**Schluss** aus `cmdutil.c:55-112` + `proc.c`): ein Programm, das mit
Code `0` endet, bleibt still; jeder andere Code, auch ein positiver, erzeugt
`error code <N>` (z. B. `error code 1`, bei `Pterm(-1)` `error code 65535`);
bekannte negative Codes haben ihren Text (`file not found`, `not enough memory`).

Ctrl-C waehrend des Programms behandelt GEMDOS (`conbrk`), nicht EmuCON.
EmuCON stellt den Schirm nach einem Grafikprogramm **nicht** wieder her
(es gibt keinen Redraw ausser `ESC E` bei `cls`/Aufloesungswechsel).

## 7. Programmendungen (`cmdutil.c:271-287`, `cmdexec.c:176-194`)

`program_extension(dta)`: kein Ordner (`d_attrib & 0x10`) und irgendein
`.`, hinter dem der **Rest des Namens** (case-insensitiv) genau `app`, `gtp`,
`prg`, `tos` oder `ttp` ist. `is_graphical`: `app`, `gtp`, `prg`. Die
Menge ist EmuTOS-/Atari-Konvention; **NewTOS' native Programme heissen
`.NAP`/`.NCP`/`.NTP`** (ADR-018, `docs/MEMORY.md:1006-1060`,
`programs/desktop/src/ftype.rs:11-13,76-95`); die Zuordnung ist eine
Entscheidung ([06 D6](06-mapping-and-decisions.md)).

## 8. Pruefauftrag (Entwickler)

* Stale `redir_name` (Abschnitt 3), `arglist`-Ueberlauf, `redir_name`/
  `user_path`-Ueberlauf: in Rust nicht 1:1 darstellbar (Index-/Laengenpruefung
  erzwingt eine Entscheidung) — Anwender entscheidet ([06 D7](06-mapping-and-decisions.md)).
* Die Rueckgabe „positiver Exit-Code → `error code N`" setzt voraus, dass
  `Pexec` aus EL0 den Pterm-Code zero-extended liefert; PROC05 sagt das fuer
  `run_program_with_tail_rc` (Kernel-Aufrufer); fuer ein `Pexec` aus EL0 ist es **OFFEN** (`kernel/src/launch.rs:111-121`, Kommentar zu
  `run_program_with_tail_rc`; `bdos/proc.c:601,616`); im Test gegen NewTOS zu bestaetigen.
