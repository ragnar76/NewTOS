# DESK44 — `.NTP` laeuft als GEM, soll aber Konsole mit Parametern sein

Status: **OFFEN** · Prioritaet: **P3** · Typ: Belegter Widerspruch im Baum — aus DESK43 U3, Anwenderentscheidung 2026-09-26

Abhaengigkeiten: —

## Der Widerspruch

Der Baum sagt zweierlei ueber dieselbe Endung:

* `kernel/src/desk/ftype.rs:82` bildet `.NTP` auf `Flavor::Params` ab, und
  `main::default_flavor` bildet `Params` auf **`AppFlavor::Ttp`** ab — also auf
  ein **Konsolen**programm mit Parameterzeile.
* `kernel/src/desk/main.rs:1059` und `ftype.rs:13` nennen als Atari-Analogon
  *„`.TTP` / `.GTP`"*, und `isgraf` behandelt `.NTP` heute als **GEM**
  (`#Y`/`.GTP`).

`.TTP` und `.GTP` tragen in EmuTOS **verschiedene** `AF_ISCRYS`
(`references/emutos/desk/deskapp.c:181-188`) — es kann also nur eines von
beiden sein.

## Die Entscheidung (Anwender 2026-09-26)

**`.NTP` ist wie `.TTP`: Konsole, mit Parametern.** Das passt zum Namen der
ADR-018-Familie — `.NCP` Konsole, `.NTP` Konsole *mit Parametern*, genau
analog `.TOS`/`.TTP`.

**Damit ist das heutige GEM-Verhalten der Fehler**, nicht der Eintrag in
`default_flavor`.

## Auftrag

1. `isgraf` (`kernel/src/desk/main.rs`, = `desksupp.c:876` +
   `deskapp.c:181-188`) fuer `.NTP` auf **Konsole** stellen, mit Zitat.
2. Die Doc-Stellen angleichen, die `.GTP` als Analogon nennen — **alle**
   Kopien suchen ([[justification-is-copied-to-several-places]]):
   `ftype.rs:13`, `main.rs:1059`, `docs/MEMORY.md:1039` (ADR-018) und was der
   Sweep sonst findet.
3. `docs/MEMORY.md` ADR-018 traegt die Entscheidung mit Datum.

## Abnahme

Ein `.NTP` startet ueber den Konsolenarm (VT52-Schirm, kein AES-Fenster), mit
der Parameterzeile aus dem Tail-Dialog. Gegenprobe: `.NAP` bleibt GEM,
`.NCP` bleibt Konsole ohne Dialog.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
