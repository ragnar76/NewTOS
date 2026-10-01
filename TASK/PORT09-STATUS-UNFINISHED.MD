# PORT09 — Quittung: GESTOPPT vor der Umsetzung, die Umgebung erreicht das Kind nicht

Datum 2026-10-02 · Basis `desk-rebuild`, HEAD `e91a99e`, Baum vorher sauber
· Belege [measurements/measurement-2026-10-02-port09/](../measurements/measurement-2026-10-02-port09/)

**UNFINISHED heisst hier: Schritt 1 (Konvention aus den Quellen) ist erledigt,
Schritt 2 (`cmdargs.c` lernt ARGV, Anwenderentscheidung 2026-10-02) ist
bewusst nicht umgesetzt.** Grund: ein Kind kann seinen `p_env` heute nicht
lesen. Gemessen, nicht vermutet. Die Arbeitsanweisung verlangte, in diesem
Fall vor jeder Aenderung an `kernel/` oder `tos-api/` anzuhalten.

## Schritt 1 — was `cl.length == 127` zusagt, aus den Quellen

| Quelle | Befund |
|---|---|
| `references/freemint/doc/programmer/atari/ARGV.txt:95-142,296-314` | Atari-Spezifikation (Pratt, 1990). `ARGV=` ist die **letzte** Variable; Wert beliebig. Danach folgen NUL-getrennt argv[0] (= Pexec-Pfadname), dann die Argumente, dann ein abschliessendes NUL. Laengenbyte 127 im Schwanz **validiert** die Umgebungsargumente; der Schwanz traegt zusaetzlich so viel, wie hineinpasst. Das Kind kappt seine Umgebung am `A` von `ARGV`. |
| ebd. `:350-377` (ARGSTART.S) | Kappung **vor** der 127-Pruefung; ohne 127 → normaler Schwanz-Parser. |
| `references/freemint/doc/programmer/atari/ARGV-blank.txt:12-34` | Erweiterung `ARGV=NULL:i,j,...` fuer leere Argumente (in argv als `" "` uebertragen). |
| `sdk/libc/mintlib/mintlib/crtinit.c:61-70,298-327,460-520` | mintlibs Umsetzung (unser Vertrag fuer portierte Programme): `STRICTLY_COMPATIBLE_WITH_STANDARD` ist **aus** (`#if 0`) — ARGV wird auch ohne Laengenbyte 127 akzeptiert (Grund: gulam); `NULL:`-Liste wird ausgewertet; argv[0] = Pfadname setzt `program_invocation_name`. Das ist die vorhandene Implementierung, die nach „port: reuse" wiederzuverwenden ist. |
| `sdk/apps/teradesk/environm.c:169-252`, `startprg.c:543-567,650-655,312` | Aufrufer: `make_argv_env` baut genau dieses Format (`ARGV=\0<prog>\0<args>\0\0`, Anfuehrungen entfernt), haengt es an die Umgebung, setzt `cl.length = 127` und ruft `Pexec(0, name, &cl, buildenv)`. |
| `references/emutos` | **Kein** ARGV — weder BDOS noch Desktop noch AES (`grep` findet nur `tools/mkheader.awk`). `bdos/proc.c:379-380` kopiert den Schwanz bis `PDCLSIZE` oder NUL. |

## Die Pruefung, die zum Halt fuehrte

QEMU `raspi4b`, headless, `#Z`-Autorun, frisches `make-sd-image.sh`-Abbild,
Kern `cargo xtask build --board rpi4 --release` (unveraendert). Die Probe
`method/envpar.c` macht TeraDesks Einzelstart im Kleinen: `Pexec(0, kind,
tail[0]=127, env)` mit 14 Argumenten (196 Zeichen, eines mit Leerzeichen).

* `serial/envkid.log`: Kind meldet `p_env=0x209b000`, der erste Lesezugriff
  darauf ist ein **Data Abort aus EL0** (ESR `0x92000006`, Translation Fault
  Level 2, `FAR == p_env`); die Maschine kommt nicht zurueck.
* Ursache im Quelltext: der Lader schreibt die **physische Pool-Adresse** des
  Umgebungsblocks in den BasePage (`tos-api/src/bdos/process/loader.rs:502-507,675`)
  und bildet ihn nie in den EL0-Raum des Kindes ab — anders als Malloc/Mxalloc
  (`usermem::map`, ADR-021; `usermem.rs:4-8` beschreibt genau diese Falle).
  Der Selbsttest `kernel/src/selftest_pexec.rs:378-397` prueft den Block ueber
  den Kern-Alias und sieht das deshalb nicht.
* Betroffen: jedes `Pexec` aus EL0 mit Umgebung **und** ohne (`env == NULL` →
  `EnvSpec::Inherit`, ebenfalls Pool-PA). Unbetroffen: Starts aus unserem
  Desktop, AUTO, Bootlader (`EnvSpec::None`, `p_env = 0`).
* `serial/argvtest.log` (Ist-Zustand): `ARGVTEST.NAP` bekommt argc 12 —
  zehn ganze Argumente plus das abgeschnittene `"kilo-el"`; vier Argumente
  fehlen, das mit Leerzeichen ebenfalls.

**Warum keine libc-Aenderung:** ein ARGV-faehiger `__libc_parseargs` muss
`p_env` lesen. Jedes von TeraDesk gestartete libc-Programm (QED
eingeschlossen) bekaeme damit beim Start denselben Abort wie ENVKID.

## Was offen bleibt

1. **Kernfehler (eigenes Ticket noetig):** der Umgebungsblock muss in den
   EL0-Raum des Kindes abgebildet und `p_env` als Nutzer-VA gestempelt werden;
   `Inherit` liest heute `(*run).p_env` als PA — der Umbau muss das mitziehen.
2. **Dann PORT09 Schritt 2:** `cmdargs.c` um den Umgebungsteil von
   `parseargs()` erweitern (mintlibs Semantik oben, inkl. `NULL:`), Hosttests,
   Abnahme mit `method/envpar.c` → `ARGVTEST.NAP` (Soll: argc 15, argv[0] =
   `C:\ARGVTEST.NAP`, `"mike thirteen has blanks"` als ein Argument).

## Geaendert

Nichts unter `kernel/`, `tos-api/`, `sdk/`. Neu: diese Datei und das
Messverzeichnis. Nebenwirkung: `lib_ensure` hat das unversionierte
Bauprodukt `sdk/gemlib/gemlib.a` als veraltet neu archiviert.
Basislinien nicht neu gefahren (keine Rust-Datei angefasst); Release-Build
rpi4 zum Messen: 64/94 Warnungen, unveraendert.
