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
