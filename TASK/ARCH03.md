# ARCH03 — Prozess- und Speicherarchitektur nach FreeMiNT: ein gemeinsamer Adressraum mit Schutzrechten

Status: **IN ARBEIT** auf Zweig `arch03` · Prioritaet: **P1** — Umsetzung **vor der OFAM** begonnen (Anwender 2026-10-04: „leg los“) · Typ: Architektur — Anwenderentscheidung 2026-10-04

Abhaengigkeiten: — · blockiert spaeter: [MEM03](MEM03.md), [PROC01](PROC01.md), [PORT09](PORT09.md), LDG/SLB (zView, `docs/analysis/zview-ldg-2026-10-04.md`), Accessories/TSR, `kbdvec`-Haken fuer alle (KEYS02 D1)

## Anlass

Die zView/LDG-Analyse hat offengelegt: NewTOS gibt jedem Prozess einen
**eigenen** EL0-Adressraum (ADR-021, `USER_VA_BASE`, Fenster pro Prozess).
EmuTOS hat **einen** Adressraum ohne Schutz, FreeMiNT **einen** Adressraum mit
MMU-Schutzrechten je Prozess (`memprot`, PROT_P/G/S/PR/I). Viele
Atari-Mechanismen setzen den gemeinsamen Adressraum voraus: `Pexec` 3/5/6/7,
LDG, SLB, residente Programme, Accessories, Zeiger in AES-Nachrichten,
globaler `Mxalloc`-Speicher. Anwender: „es so wie bei FreeMiNT bauen“.

## Vorgehen (Anwender 2026-10-04)

1. **Analyse** (Researcher): FreeMiNTs Prozess- und Speichermodell aus dem
   Quelltext (`references/freemint`, Pin `f906416`), EmuTOS zum Vergleich,
   NewTOS-Ist-Zustand. → `docs/analysis/arch03-freemint-memory-model.md`
2. **Architektur** (Architekt): vollstaendige Implementierungsdefinition auf
   Basis der Analyse — ADR-Entwurf, Abbildung auf AArch64 (TTBR0 je Prozess,
   ASID, gleiche VA→PA, Rechte je Prozess), Ladeadressen/PIE, Loader,
   Speicherverwaltung, `Pexec` alle Modi, Migrationsschritte, Tests,
   Aufwand. → `docs/analysis/arch03-implementation-plan.md`
3. Freigabe durch den Anwender, Umsetzung nach der OFAM.

## Entscheidungen 2026-10-04 (Anwender: „leg los“)

* Umsetzung beginnt sofort, auf dem eigenen Zweig **`arch03`**. Der
  Vorfuehrstand ist als Tag **`ofam-baseline`** (16fdbc3) eingefroren; der
  Zweig `desk-rebuild` bekommt bis zur OFAM nur Fehlerbehebungen.
* E1–E18 aus `docs/analysis/arch03-implementation-plan.md` §6 gelten **wie
  empfohlen**, ausser **E8b** (PRGFLAGS als ELF-Notiz — eigenes Format), das
  dem Anwender in Stufe 0d vorgelegt wird.
* Stufen seriell, jede einzeln lauffaehig; auf der Pi nur auf Ansage.

## Entscheidungen 2026-10-05 (Anwender: „mach einfach weiter“ = Empfehlungen angenommen)

* **E8b freigegeben:** PRGFLAGS als ELF-Notiz `.note.newtos.prgflags`
  (Name „NewTOS“, Typ 1, `u32`), SDK-Standardwert, pro Programm
  ueberschreibbar; `newtos.ld` behaelt die Notiz mit `PT_NOTE`; Lader liest
  sie, sonst `PF_STANDARD`. Umsetzung in Stufe 2 (gebraucht von `Pexec` 7).
* **E6/E3:** ein `Pexec`-4-Kind behaelt seine eigene Seite 0 und sieht den
  uebrigen Speicher des Aufrufers (Stufe 1 so gebaut; Stufe 3: Wurzel des
  Kindes = Rechte-Tabellen des Aufrufers + eigene Seite 0).
* **Absturz-Spezifikation** `docs/api/bios/_exception-crash.md` freigegeben;
  **E11:** EmuTOS-Absturzbericht (kein FreeMiNT-Alert).

## Fortschritt (Stand 2026-10-05, Zweig `arch03`)

| Stufe | Stand | Quittung / Beleg |
|---|---|---|
| 0a Track B | erledigt | `docs/analysis/arch03-stage0a-armarm-verification.md` |
| 0b Spike | erledigt | `docs/analysis/arch03-stage0b-spike.md` |
| 0c Abbruch statt Halt | umgesetzt, QEMU belegt | [ARCH03-0c-STATUS-DONE](ARCH03-0c-STATUS-DONE.MD) |
| 0d PIE-Lader | umgesetzt, QEMU belegt | [ARCH03-0d-STATUS-DONE](ARCH03-0d-STATUS-DONE.MD) |
| 1 gemeinsamer Raum | umgesetzt, QEMU belegt | [ARCH03-1-STATUS-DONE](ARCH03-1-STATUS-DONE.MD) |
| 2 Pexec 5/6/7, Ptermres, Regionen | umgesetzt, QEMU belegt | [ARCH03-2-STATUS-DONE](ARCH03-2-STATUS-DONE.MD) |
| 3a Kernelzugriff | umgesetzt, QEMU belegt | [ARCH03-3a-STATUS-DONE](ARCH03-3a-STATUS-DONE.MD) |
| 3b Rechte je Prozess | umgesetzt, QEMU belegt; Offen 1/3/4/5 in 3c geschlossen, Offen 2 nicht | [ARCH03-3b-STATUS-UNFINISHED](ARCH03-3b-STATUS-UNFINISHED.MD) |
| 3c E10, Lader-Reihenfolge, E6/strict, F_KEEP, Tabellenmangel | umgesetzt, QEMU belegt; **S2ALL/TSRPAR unter Schutz an/strict weiter FAIL** (Ursache E9+E16, Entscheidung nötig) | [ARCH03-3c-STATUS-UNFINISHED](ARCH03-3c-STATUS-UNFINISHED.MD) |
| E9 TPA = größter Block (EmuTOS), Start-Mshrink wie mintlib | umgesetzt, QEMU belegt (ON/OFF/STRICT); TSRPAR-Layoutprüfung weiter Entscheidung | [ARCH03-E9-STATUS-DONE](ARCH03-E9-STATUS-DONE.MD) |
| 2b FreeMiNT-Modi 100+ | offen | — |
| 4 ASIDs | offen (optional) | — |

Abnahme durch den Tester und Pi-Läufe für 0c–3c offen; Pi nur auf Ansage.
Plan-Abschnitt „Stufe 3 wie gebaut“: `docs/analysis/arch03-implementation-plan.md`.

## Pi-Abnahme 2026-10-05

Anwender: kompletter Stand `ae097d0` (Kernel `ba6172a1`, alle Programme neu gebaut) laeuft auf der echten Pi 4; nur `BALL.NAP` (externes Projekt, noch ET_EXEC) wird erwartungsgemaess mit Fehler 35 abgelehnt. Kartensicherung des Vorfuehrstands: `sd-card-backup-20261005-ofam/` (gitignored).
