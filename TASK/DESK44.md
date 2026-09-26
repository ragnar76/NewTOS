# DESK44 — `.NTP` laeuft als GEM, soll aber Konsole mit Parametern sein

Status: **BEHOBEN — HW-Gegenprobe offen** (2026-09-26) · Prioritaet: **P3** · Typ: Belegter Widerspruch im Baum — aus DESK43 U3, Anwenderentscheidung 2026-09-26

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

## Abschluss (2026-09-26)

`isgraf` gibt fuer `Flavor::Params` jetzt **`false`** zurueck, `App` allein
`true`. Beleg: `app_parse`s Fallthrough `case 'Y' → case 'G'` setzt
`AF_ISPARM|AF_ISCRYS`, `case 'P' → case 'F'` setzt **nur** `AF_ISPARM`
(`deskapp.c:310-322`, Tabelle `:181-188`); `isgraf` liest genau `AF_ISCRYS`
(`desksupp.c:876`).

**Kein zweiter Codeort noetig** — der Konsolenarm ist der regulaere Weg:
`pro_run` → `shel_write(SHW_EXEC, isgr=0)` → `sh_isgem = 0` → `sh_chgrf` →
`sh_toalpha`, gegen `gemshlib.c:513-523`.

Sweep nach `.GTP`-als-Analogon: **drei** Kopien, alle korrigiert
(`ftype.rs:13`, `main.rs:1059`, `docs/sdk/01-overview.md:47` — letztere sagte
zusaetzlich faelschlich „treated like `.NAP`"). Alle uebrigen `GTP`-Treffer
sind Aussagen **ueber EmuTOS** bzw. die klassische m68k-Familie und bleiben
richtig.

**Abnahme nur zur Haelfte einloesbar, und das ist eine alte Luecke, keine
neue:** `opn_appl` (der Tail-Dialog, `deskpro.c:71-121`) ist weiterhin nicht
gebaut. Der Doppelklick startet also mit **leerer** Parameterzeile — jetzt aber
auf dem Textschirm, wo er hingehoert. Gegenprobe im Code:
`inf::default_type_anodes` schrieb schon immer `#P *.NTP`, beide Seiten stimmen
jetzt ueberein.

**HW-Gegenprobe offen:** `.NTP` doppelklicken → `sh_isgem=0` im Serial und
Textschirm statt Desktop; `.NAP` bleibt GEM, `.NCP` bleibt Konsole ohne Dialog.
