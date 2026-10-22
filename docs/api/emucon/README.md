# EmuCON2 — Spezifikation aus `references/emutos/cli/` (TASK/CON03, Schritt 1)

> **Stand:** 2026-10-22 · Quelle: EmuTOS @ `1a7484ab` (Pin laut `docs/MEMORY.md`),
> Verzeichnis `cli/` (`cmdmain.c`, `cmdint.c`, `cmdparse.c`, `cmdedit.c`,
> `cmdexec.c`, `cmdutil.c`, `cmdasm.S`, `cmd.h`, `clistub.h`) plus die
> Startpfade in `aes/gemshlib.c`, `bios/bios.c`, `bios/endrom.c`,
> `bios/initinfo.c`, `desk/deskmain.c`. Protokoll:
> `docs/SOURCE-ANALYSIS-PROTOCOL.md` (Track A).
>
> **Anwenderentscheid 2026-10-22** (`TASK/CON03.md`): EmuCON wird ein eigenes
> EL0-Programm (ADR-026-Muster wie NEWDESK), Befehlssatz 1:1 nach `cli/`,
> **nicht** in die Kernel-Konsole nachgeruestet.
>
> **Dies ist Dokumentation, kein Code.** Nichts hier ist freigegeben:
> die Entscheidungen in [06](06-mapping-and-decisions.md) §3 braucht der
> Anwender vor dem Bau (Regel 5).

## Dateien

| Datei | Inhalt |
|---|---|
| [01-startup-exit.md](01-startup-exit.md) | Wer startet EmuCON, Basepage/Umgebung, Beenden, Rueckkehr zum Desktop, NewTOS-Startpfad (shell_loop) |
| [02-line-editor.md](02-line-editor.md) | Prompt, Zeileneditor, Verlauf, Tab-Vervollstaendigung (`cmdedit.c`) |
| [03-parser-redirect-exec.md](03-parser-redirect-exec.md) | Tokenizer, Anfuehrungszeichen, `>`-Umlenkung, Jokerzeichen, Fremdprogramme (`cmdparse.c`, `cmdexec.c`, `cmdmain.c`) |
| [04-builtins.md](04-builtins.md) | Alle 21 Befehle (+ Laufwerkswechsel `X:`), Syntax, Ausgabe, GEMDOS-Aufrufe in Reihenfolge, Fehlertexte |
| [05-config-strings-hardware.md](05-config-strings-hardware.md) | `CONF_*`/`WITH_*`-Gating, Sprache, Hardwareanteile (Aufloesung, Cookies, VT52-Escapes, Line-A) |
| [06-mapping-and-decisions.md](06-mapping-and-decisions.md) | Abbildung EmuTOS-Funktion → `programs/emucon/…`, Unterschiede zu `kernel/src/shell.rs`, **Entscheidungen** und offene Fragen |

## Legende und Beleglage

* `file:line` ohne Zusatz = **gelesen** (Code in dieser Sitzung vor Augen).
  Pfade ohne Praefix sind relativ zu `references/emutos/`; NewTOS-Pfade sind
  repo-relativ.
* **(Schluss)** = aus gelesenem Code abgeleitet, aber nicht ausgefuehrt. Traegt
  die Aussage etwas, das der Entwickler nachpruefen sollte, steht es im
  Abschnitt „Pruefauftrag" der jeweiligen Datei.
* **OFFEN** = Quelle unklar oder eine NewTOS-Entscheidung; nichts wurde
  erfunden.
* Vertrag (muss gleich sein, Programme/Anwender sehen es) und Intern (frei,
  `docs/COMPATIBILITY.md`) sind je Abschnitt getrennt gekennzeichnet.

## Was EmuCON ist (Zusammenfassung, belegt)

* Zeilenorientierter Kommandointerpreter fuer den Textschirm, ca. 3,4 kLOC C in
  sieben Dateien (`cli/readme.txt:1-8`; Zeilenzahlen: `wc -l` ueber `cli/`).
  Eingebaut sind 21 Befehle (`cli/cmdint.c:140-163`), dazu `X:` fuer
  Laufwerkswechsel (`cmdint.c:174-176`).
* Bewusste Auslassungen laut Kopfkommentar: **keine Batchdateien, keine
  Eingabeumlenkung, keine Pipes** (`cmdmain.c:20-23`). Es gibt **kein `<`**, **kein
  `>>`**, **keine Umgebungsvariablen-Expansion**, **keine Befehlsverkettung**;
  der Task-Text nennt `>>`/`<` ausdruecklich als „falls vorhanden" — sie sind
  es nicht (`cmdparse.c:88-106`: ein zweites `>` ist ein Fehler).
* EmuCON ist **keine GEM-Anwendung** (`cmdmain.c:227-228`): kein `appl_init`, keine
  AES-/VDI-Aufrufe; es spricht nur GEMDOS/BIOS/XBIOS (`cmd.h:37-92`) und
  Line-A-Init zur Bildschirmgroesse (`cmdasm.S:51-72`).
* **Gegenstueck zu Atari-Dokumentation: keines.** EmuCON2 ist eine
  EmuTOS-Eigenentwicklung (`cli/readme.txt:3-5`); Compendium/Profibuch kennen
  sie nicht. Eine Gegenpruefung dort ist nicht moeglich und nicht noetig; die
  benutzten GEMDOS-Aufrufe (`Pexec`, `Fsfirst`, `Dfree` …) haben ihre eigenen
  Eintraege unter `docs/api/gemdos/`.
