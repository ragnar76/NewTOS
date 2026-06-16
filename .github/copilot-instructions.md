# Copilot Instructions — NewTOS / PiRusTOS

Diese Datei wird von GitHub Copilot automatisch als Workspace-Kontext geladen.
Die **maßgeblichen Projektregeln** stehen in [CLAUDE.md](../CLAUDE.md) im
Repository-Root. Diese Datei dient als Einstiegspunkt und Kurzfassung.

## Pflichtlektüre vor jeder Aufgabe

1. [CLAUDE.md](../CLAUDE.md) — Projektregeln, kritische Regeln, Tech-Stack
2. [docs/REQUIREMENTS.md](../docs/REQUIREMENTS.md) — funktionale Anforderungen
3. [docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md) — Schichtenmodell, ABI
4. [docs/STATUS.md](../docs/STATUS.md) — aktueller Stand, offene ADRs
5. Aktuelles Phasendokument unter [phases/](../phases/)

## Kritische Regeln (Kurzfassung — Details in CLAUDE.md)

1. **Keine Stubs.** Kein `todo!()`, kein hardcodierter Rückgabewert, um den
   Build grün zu bekommen. Bei fehlender Abhängigkeit: stoppen, in
   `docs/STATUS.md` eintragen, Rückfrage stellen.
2. **Bei Unsicherheit STOPPEN und fragen.** TOS-API, m68k-ABI,
   BCM2711-Register sind vollständig dokumentiert — nicht raten.
3. **Debug-Tiefenlimit: drei Versuche.** Danach Diagnose schreiben, an User
   übergeben. Keine „nur noch ein Versuch"-Schleifen.
4. **Spec vor Code.** Jede TOS-API-Funktion braucht erst einen Eintrag in
   `docs/api/<group>.md`, dann Implementierung.
5. **Keine erfundene Hardware.** Register/Bits nur aus offizieller
   BCM2711-Doku, RPi-Foundation-Docs oder Andre Richters Tutorials.

## Sprache

- Design-Diskussionen, Erklärungen, Entscheidungen: **Deutsch**
- Code, Identifier, Kommentare, Commit-Messages, Doc-Comments: **Englisch**
- Dateinamen/Pfade: **Englisch**, klein, bindestrich-getrennt

## Tech-Stack (nicht verhandelbar)

- Rust nightly, `#![no_std]`, Target `aarch64-unknown-none-softfloat`
- Cargo-Workspace, Sub-Crates pro Architekturschicht
- `cortex-a`, `tock-registers`, `spin` / hand-rolled Sync-Primitives
- QEMU `aarch64-virt` als primäres Testtarget, RPi4 sekundär
- **Kein** C/C++, kein Linux/POSIX, kein `std`, kein x86

## Verzeichniskonventionen für Agent-Customization

- `.github/copilot-instructions.md` — diese Datei (Workspace-weit)
- `.github/instructions/*.instructions.md` — pfad-spezifische Regeln (`applyTo`)
- `.github/prompts/*.prompt.md` — wiederverwendbare Prompt-Templates
- `.github/chatmodes/*.chatmode.md` — spezialisierte Agent-Modi

Vollständige Source-Tree-Konventionen siehe [CLAUDE.md](../CLAUDE.md)
§ "Source Tree Conventions".
