# ACC01 — Quittung

**Status: ERLEDIGT — Lebenszyklus extrahiert, keine Produktcode-Aenderung** · 2026-09-24

## Was getan wurde

`docs/api/aes/accessories.md` (neu) geschrieben: der vollstaendige `.ACC`-
Lebenszyklus aus der gepinnten EmuTOS-Quelle (`1a7484aba1bfba23e2c0899e243b6d8177fe41df`),
jede Zeile gegen den tatsaechlichen Quelltext gelesen, nicht aus dem Ticket
uebernommen.

**Ticket-Zitate zuerst gegen die Quelle geprueft** (Abschnitt 0 der Spec,
Tabellenform). Ergebnis: kein Fehltreffer (keine Adresse zeigt auf die
falsche Funktion), aber sieben Bereiche sind off-by-1..5 — durchgehend
Doc-Kommentar- oder Leerzeilen-Versatz an der Funktionsgrenze, z. B.
`count_accs` (`geminit.c:264`, nicht `:265`), `run_accs_and_desktop` (endet
`:750`, nicht `:745`), `wait_for_accs` (Signatur `:599`, nicht `:588` — dort
liegt der Doc-Kommentar). Korrigierte Zeilen durchgehend verwendet.

## Die zwei zusaetzlich verlangten Punkte

**Gating.** Kein `CONF_WITH_*` schaltet den `.ACC`-Lebenszyklus selbst ab —
er ist in jedem EmuTOS-Build unconditional code. `TARGET_192`
(`config.h:210-401`) hat keinen Effekt auf `TARGET_512` (kein eigener
`#ifdef TARGET_512`-Block existiert); alle inhaltlich benachbarten Flags
(`CONF_WITH_MENU_EXTENSION`, `_3D_OBJECTS`, `_BACKGROUNDS`,
`_LOADABLE_CURSORS`, `_PCGEM`) fallen fuer `TARGET_512` auf ihren
generischen TOS4-Default (`1`). **Voll an in unserem Referenzbuild** —
plain gesagt, wie gefordert.

**Erweiterungsfrage.** Zwei Lead-Entscheidungen kamen per Zwischennachricht
waehrend der Bearbeitung: `.NAC` als Name, dann die Zwei-Orte-Suche
(`\ACC\` zuerst, dann Root, Dedupe nach Dateiname). Beide als Vorgabe
uebernommen und **die Kosten ausgearbeitet**, nicht selbst empfohlen:

- `count_accs`'s Muster (`geminit.c:272`) hat **keine** bestehende NewTOS-
  Entsprechung zum Aendern — kein Scan existiert heute (Grep ueber
  `kernel/src/`/`tos-api/src/` bestaetigt); ACC02 schreibt ihn neu.
- Nichts downstream (Laden, `menu_register`, `AC_OPEN`/`AC_CLOSE`) haengt
  am Extensions-String — nur der Scan selbst.
- `NUM_ACCS=6` ist laut `sysconf.h:37-40` bereits in EmuTOS ein
  **gemeinsamer** Lade- **und** Registrierungs-Pool; die Zwei-Orte-Suche
  muss diesen als **einen** Pool mit stillem Abbruch bei 6 behandeln
  (EmuTOS-Beleg fuer den Abbruch-Mechanismus selbst, NewTOS-eigene
  Folgerung fuer den geteilten Pool ueber zwei Orte — klar getrennt in der
  Spec markiert).
- Ein klassisches `.ACC` waere so oder so nicht lauffaehig — belegt (nicht
  nur erwartet) ueber `gotopgm` (`gemasm.S:73-131`), das direkt in
  m68k-Maschinencode springt.
- Die leere `ACC\`-Testordner-Attrappe auf der Karte
  (`tools/make-sd-image.sh:59`) verwechselt den heutigen EmuTOS-Root-Scan
  **nicht** — Name- *und* Attribut-Mismatch nachvollzogen
  (`bdos/fsdir.c:737-796`/`:1802-1845`).
- `kernel/src/desk/ftype.rs:59-72` und `docs/MEMORY.md` ADR-018
  (`:1004-1061`, Stand 2026-07-16) enthalten **noch kein** `.NAC` — als
  Luecke fuer ACC02 benannt, nicht als Diskrepanz dieser Extraktion
  gewertet.

## Weitere verlangte Punkte

- **`msg[3]`/`msg[4]`-Asymmetrie bestaetigt**, unabhaengig aus `ap_sendmsg`
  (`gemwmlib.c:851-862`) und seinen zwei Aufrufstellen (`gemctrl.c:337-350`
  fuer `AC_OPEN`, `gemmnlib.c:740-758` fuer `AC_CLOSE`) hergeleitet — traf
  sich mit `sysacc.c`s eigenen Zitaten, wurde nicht von dort uebernommen.
- **`sysacc.c` (ACC06) geprueft**: `AC_OPEN`/`AC_CLOSE`-Vertrag **richtig**,
  inklusive Asymmetrie; Lebenszyklus-Form korrekt (kein `appl_exit`/`Pterm`).
  Keine Korrektur an der Datei noetig.
- **`menu_register` (`tos-api/src/aes/menu.rs:1090-1130`)**: erfuellt nur
  die Slot-Buchhaltung (Titel-Pointer + `pid`, stabile Slot-ID); postet
  weder in die `THEDESK`-Drop-down-Liste noch `AC_OPEN` — steht so auch
  bereits im eigenen Code-Kommentar, hier unabhaengig bestaetigt.

## NewTOS-Luecken → ACC02–ACC05

Fuenf Punkte (Abschnitt 8 der Spec): kein Discovery/Load-Pfad, keine
Accessory-Slots in der AESPD-Tabelle, `AC_OPEN` nie gepostet, `AC_CLOSE`
nie gepostet, `sysacc.c` blockiert heute fuer immer in `evnt_mesag`
(bestaetigt als ehrlicher dokumentierter Zustand, kein Defekt).

## Pruefung

Keine Tests noetig — reine Quellenanalyse/Dokumentation. Kein Produktcode
angefasst (`kernel/`, `tos-api/`, `sdk/` unveraendert). `docs/STATUS.md`
Recent-Decisions-Eintrag 2026-09-24 gesetzt; `TASK/ACC01.md` Status auf
ERLEDIGT + `## Abschluss` ergaenzt.

## Offen

Fuenf konkrete Punkte fuer ACC02–ACC05 (siehe oben). Keine offenen Fragen
aus diesem Ticket selbst — alle vier vom Lead gestellten Zwischenfragen
(Zwei-Orte-Reihenfolge, `NUM_ACCS`-Budget, `ACC\`-Verwechslungsgefahr,
`ftype::classify`-Notiz) sind in `docs/api/aes/accessories.md` Abschnitt 7
beantwortet.
