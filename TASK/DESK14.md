# DESK14 — `Read .INF file` ist nicht `app_read_inf`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Nebenbefund DESK03/DESK04

Abhaengigkeiten: [DESK03](DESK03.md) (erledigt), [DESK09](DESK09.md) (erledigt)

## Sachverhalt

`Options → Read .INF file` ist in EmuTOS `app_read_inf`
(`references/emutos/desk/deskapp.c:1492-1548`) und tut vier Dinge, die
`kernel/src/desk/main.rs`s `do_read_inf` nicht tut:

1. **Dateiauswahl.** EmuTOS fragt mit `fsel_exinput(path, fname, &button,
   desktop_str_addr(STRDINF))` (`:1505`), Vorgabe `INF_FILE_WILD` auf dem
   Startlaufwerk. NewTOS liest fest `NEWDESK.INF` und fragt nichts.
2. **Signaturpruefung.** EmuTOS verlangt `#R` **oder** `#E` als erste zwei
   Zeichen (`:1523-1526`); NewTOS prueft seit DESK03 nur noch das `#`.
3. **Rueckmeldung im Fehlerfall.** `EFILNF` → `fun_alert(1, STFILENF)` mit
   dem Dateinamen (`:1536-1538`), alles andere → `fun_alert(1, STINVINF)`
   = `[1][Illegal .INF file format.][  OK  ]` (`desk_rsc.c:2560`). NewTOS
   kehrt **still** zurueck — die Abweichung ist mit DESK03 entstanden und
   dort benannt worden: vorher setzte derselbe Pfad Ansicht, Sortierung und
   Hintergrund stillschweigend auf die Vorgaben zurueck.
4. **Neustart statt Nachziehen.** Bei Erfolg schreibt EmuTOS die Datei in den
   Shell-Puffer (`shel_get`/`shel_put`, `:1532-1533`) und **verlaesst den
   Desktop**, der daraufhin neu eintritt und die Daten wie beim Start durch
   `app_start` liest (`deskmain.c:918-920` und `:2127-2135`). NewTOS wendet
   sie im laufenden Desktop an (`load_and_apply` + `cnx_get_prefs` +
   Neuaufbau), was einen zweiten, EmuTOS unbekannten Anwendungspfad ist.

## Lead-Entscheidung zu Punkt 4 (2026-09-23): zurueckgestellt

Der Neustartpfad wird **jetzt nicht** gebaut. Er ist kein Detail dieses
Tickets, sondern der Umbau des Desktops von einem Kernelaufruf zu einem
Prozess, den `sh_main` starten und neu eintreten kann: `global::init` ist
idempotent, es gibt kein `win_start()` beim Wiedereintritt, `wm_init()`
wuerde die AES-Fenstertabelle leeren, und die Bruecke `cnx_put();
app_save(FALSE)` → `shel_get` (`deskmain.c:2152-2153`) fehlt. DESK06 hat
denselben Punkt getroffen und faehrt den Konsolenlauf deshalb verschachtelt
(`shell_loop::sh_run_pending_console`), benannt als Abweichung.

Der Umbau gehoert zum Umzug des Desktops in den Benutzerbereich (Notiz
`desktop-must-run-in-user-scope`) und ist vor dem Vorfuehrtermin zu riskant.
Bis dahin gilt: Nachziehen im laufenden Desktop, als Abweichung
dokumentiert — an **einer** Stelle, nicht in jedem betroffenen Ticket.

## Arbeit

Punkte 2 und 3 (Signaturpruefung `#R`/`#E`, `STFILENF`/`STINVINF`-Alerts),
dann Punkt 1 (`fsel_exinput`). Punkt 4 bleibt nach der Entscheidung oben
liegen; der bestehende Pfad bekommt den Verweis darauf im Quelltext.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
