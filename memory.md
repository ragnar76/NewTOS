# NewTOS / PiRusTOS — Agent Memory

> **Zweck:** Persistenter Speicher für den Coding-Agent. Wird bei jedem
> Compact / Session-Restart als Erstes gelesen, um sofort den vollen
> Projektkontext zu haben. Wird vom Agent aktiv gepflegt.
>
> **Lese-Reihenfolge bei Session-Start:**
> 1. Diese Datei (`memory.md`)
> 2. [CLAUDE.md](CLAUDE.md) — verbindliche Projektregeln
> 3. [docs/SOURCE-ANALYSIS-PROTOCOL.md](docs/SOURCE-ANALYSIS-PROTOCOL.md) —
>    Pflicht-Vorgehen vor jeder Spec/Design/Code-Arbeit (zwei Tracks A+B,
>    Konkordanz vor Code)
> 4. [docs/STATUS.md](docs/STATUS.md) — aktueller Phasen-Stand
> 5. Aktuelles Phasendokument unter `phases/PHASE-XX-*.md`

---

## 1. Projekt-Kurzfassung

- **Name:** NewTOS (Repo: PiRusTOS)
- **Ziel:** Bit-exakte Reimplementierung der Atari-TOS-API (GEMDOS / BIOS /
  XBIOS) als nativer Bare-Metal-OS-Kern für AArch64 (Raspberry Pi 4).
- **Sprache:** Rust nightly + AArch64-Assembly. Kein C/C++. Kein `std`.
- **Targets:** QEMU `aarch64-virt` (primär für Entwicklung), RPi 4 Model B
  (sekundär, Deployment).
- **Referenz-Quellen (read-only, nicht im Build):**
  - `references/emutos/` — EmuTOS-Sourcen (upstream `emutos/emutos`
    master @ `1a7484ab`), **primäre Quelle** für Track A (Phase 00
    extrahiert daraus die TOS-API-Spec). Update-Verfahren in
    [`references/README.md`](references/README.md). Beim Zitieren Form
    `EmuTOS @ <sha> — <pfad>:<funktion>`.
  - `references/` — alle weiteren Quellen (Compendium, Profibuch, BCM2711-
    Datasheet etc.). Gitignored außer README; pro Maschine zu beschaffen.
  - Atari Compendium, Atari Profibuch, TOS Release Notes (siehe
    `references/README.md` Tabelle für Beschaffungs-Stand).
- **Lizenz:** MIT OR Apache-2.0 (ADR-002 ausstehend)

## 2. Kritische Regeln (NIE vergessen)

1. **Keine Stubs.** Kein `todo!()`, kein hardcodierter Return-Wert um den
   Build grün zu kriegen. Bei fehlender Abhängigkeit → STATUS.md "Blocked",
   User fragen.
2. **Bei Unsicherheit STOPPEN und fragen.** TOS-API/m68k-ABI/BCM2711 sind
   vollständig dokumentiert — nicht raten.
3. **Debug-Limit: drei Versuche.** Danach Diagnose schreiben, an User.
4. **Spec vor Code.** Jede TOS-API-Funktion erst in `docs/api/<group>.md`
   spezifizieren, dann implementieren. **Spec-Eintrag muss mit Source-Analysis
   beginnen** (Konkordanz-Tabelle, siehe `_template.md`).
5. **Source-Analysis-Protocol einhalten** (`docs/SOURCE-ANALYSIS-PROTOCOL.md`):
   Primary → Secondary → Konkordanz → Entscheidung → *dann* Code.
   Verstoss = Output wird verworfen.
6. **Keine erfundene Hardware.** Register/Bits nur aus offizieller
   BCM2711-Doku, RPi-Foundation, Andre-Richter-Tutorials.
7. **Sprache:** Diskussion auf Deutsch, Code/Identifier/Kommentare/Commits
   auf Englisch.

## 3. Workspace-Layout (Stand: aktuell)

```
PiRusTOS/
├── CLAUDE.md                  # Projektregeln (Single Source of Truth)
├── memory.md                  # diese Datei
├── Cargo.toml                 # Workspace
├── rust-toolchain.toml        # nightly-2026-04-15
├── .cargo/config.toml         # cargo unstable: per-package-target
├── .github/                   # Copilot customization
│   ├── copilot-instructions.md
│   ├── instructions/, prompts/, chatmodes/, workflows/
├── docs/
│   ├── REQUIREMENTS.md, ARCHITECTURE.md, STATUS.md
│   └── api/                   # TOS-API-Spec, eine Datei pro Gruppe
├── phases/                    # PHASE-00 … PHASE-12
├── references/
│   ├── README.md              # Beschaffungs-Tracker
│   └── emutos/                # EmuTOS-Checkout (gitignored, read-only)
├── kernel/                    # Bare-Metal-Kern (forced-target aarch64-…)
│   ├── Cargo.toml, build.rs
│   └── src/
│       ├── lib.rs, main.rs, console.rs, panic.rs
│       ├── arch/aarch64/      # boot.rs, link-{rpi4,qemu-virt}.ld
│       ├── hal/uart.rs        # UART-Trait
│       └── drivers/{bcm2711,qemu_virt}/pl011.rs
├── tos-types/                 # geteilte Typen (DTA, BPB, Errorcodes)
└── tools/xtask/               # Host-Build-Driver
```

## 4. Tech-Stack-Entscheidungen

| Was | Entscheidung | Quelle |
|---|---|---|
| Toolchain | `nightly-2026-04-15` | `rust-toolchain.toml` |
| Target | `aarch64-unknown-none-softfloat` | `kernel/Cargo.toml` |
| Per-Package-Target | aktiv (`cargo-features = ["per-package-target"]`) | hält xtask host-builds parallel |
| Panic | `abort` in beiden Profilen | `Cargo.toml` |
| Sync-Primitiv | `spin` oder hand-rolled. **Kein** `parking_lot`, `lazy_static` | CLAUDE.md |
| Allocator | erst nach Phase 03 verfügbar | Phase-Plan |

## 5. Board-spezifika (Phase 01 verifiziert)

### RPi 4 (BCM2711, low-peripheral mode @ 0xFE00_0000)
- Kernel-Load: `0x80000`, Image-Name: `kernel8.img` (raw binary via objcopy)
- PL011 UART0: Base `0xFE20_1000`, GPIO 14/15 ALT0
- Baud 115200 @ 48 MHz Refclock: IBRD=26, FBRD=3
- GPIO-Pull-Cfg: `GPIO_PUP_PDN_CNTRL_REG0` @ `0xFE20_00E4` (BCM2711-spezifisch,
  **nicht** der alte BCM2837-`GPPUD`-Sequenz-Stil)

### QEMU virt (cortex-a72)
- Kernel-Load: `0x4008_0000` (RAM start `0x4000_0000` + 512 KiB Reserve für DTB/FW)
- PL011 UART: Base `0x0900_0000`, kein Pin-Mux nötig
- Baud 115200 @ 24 MHz Refclock: IBRD=13, FBRD=1

## 6. Commands (verifiziert lauffähig)

```bash
# Build
cargo xtask build --board qemu-virt          # ELF
cargo xtask build --board rpi4               # ELF + kernel8.img (raw)

# Run (QEMU 11.0.0, brew install qemu)
cargo xtask run                              # QEMU virt, Serial→stdout
                                             # Ctrl-A x zum Beenden

# Lint / Test
RUSTFLAGS="-D warnings" cargo build --package kernel \
  --no-default-features --features board-qemu-virt
cargo clippy --package kernel --no-default-features \
  --features board-{qemu-virt|rpi4} --lib --bins -- -D warnings
cargo xtask test                             # Host-tests (tos-types, xtask)
```

## 7. Phasen-Status (Snapshot)

> **Wahrheitsquelle:** [docs/STATUS.md](docs/STATUS.md) — bei Diskrepanz
> gilt STATUS.md.

| Phase | Titel | Status |
|---|---|---|
| 00 | EmuTOS Source Analysis | **pending** — Anweisung wird vom User erstellt; vor Vorliegen kein Start |
| 01 | Boot + UART | **abgeschlossen auf QEMU** (D1–D7, EC1, EC3, EC4 ✅; EC2 RPi4-HW pending) |
| 02 | GEMDOS Console I/O | blockiert auf Phase 00 |
| 03 | Memory Management | nicht begonnen |
| … | (siehe STATUS.md) | nicht begonnen |

## 8. Offene Punkte / Blocker

- **EC2 (RPi4-Hardware-Boot):** SD-Karte mit RPi-Firmware + `kernel8.img`,
  USB-TTL-Adapter auf GPIO 14/15. Manueller Schritt.
- **Source-Analysis-Backfill (gemdos.md):** Bestehende Einträge für
  `Cconin`, `Cconout`, `Cconws` enthalten noch keine `## Source Analysis`-
  Sektion mit Konkordanz-Tabelle. Vor Beginn von Phase 02 nachpflegen.
- **References-Beschaffung:** Atari Compendium, Profibuch, BCM2711-
  Datasheet, ARM ARM, RPi-Boot-Doc fehlen unter `references/`. Pflege-
  Tabelle in [`references/README.md`](references/README.md). Tasks, die
  diese Quellen brauchen, müssen bis Beschaffung blockiert werden.
- **`cargo xtask check-references` fehlt:** Pre-Flight-Check vom Protokoll
  gefordert, noch nicht implementiert.
- **`docs/SOURCE-ERRATA.md` fehlt:** Im Protokoll referenziert, aber nicht
  angelegt. Erst bei erstem Errata-Fund nötig.
- **Git-Init:** Repo hat noch keinen Initial-Commit. User entscheidet
  Zeitpunkt.
- **ADRs offen:** ADR-001 (Native ARM PRG-Format), ADR-002 (Lizenz),
  ADR-003…007 — siehe ARCHITECTURE.md §"Key ADRs".

## 9. Wiederkehrende Stolperfallen (Lessons Learned)

| Problem | Lösung |
|---|---|
| `rust-lld: unknown argument '-nostartfiles'` | `rust-lld` kennt das Flag nicht. Einfach weglassen — `_start` wird über das Linker-Skript via `ENTRY()` gesetzt. |
| `feature(naked_functions)` Warnung | seit Rust 1.88 stabil, Feature-Flag entfernen. Stattdessen `core::arch::naked_asm!` + `#[unsafe(naked)]`. |
| Modul fehlt obwohl Verzeichnis da ist | Rust braucht `mod.rs` (oder `<modul>.rs`) in jedem Zwischenebenen-Verzeichnis. |
| `missing_docs` Lint trifft auch `#[cfg]`-gates | Jeder `cfg`-Zweig braucht eigenes `///`-Doc, oder Lint lokal lockern. |
| BCM2711 GPIO-Pull abweichend von BCM2837 | Nicht den GPPUD-Clock-Sequenz-Trick nutzen — neuer Chip hat dedizierte `GPIO_PUP_PDN_CNTRL_REGn` Register. |

## 10. Pflege dieser Datei

- **Nach jeder erledigten Deliverable / EC:** Abschnitt 7 + ggf. 9 updaten.
- **Bei jeder Tech-Stack-/ADR-Entscheidung:** Abschnitt 4 + STATUS.md.
- **Bei jedem neuen Verzeichnis / Crate:** Abschnitt 3.
- **Bei jedem Stolperstein, der mehr als 5 Min. gekostet hat:** Abschnitt 9.
- **Vor Phasen-Wechsel:** kompletten Snapshot durchsehen, abgelaufene
  Notizen entfernen.
