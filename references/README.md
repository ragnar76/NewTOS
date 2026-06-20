# References — NewTOS Source Material

This directory tracks the **source material** required by the
[Source Analysis Protocol](../docs/SOURCE-ANALYSIS-PROTOCOL.md). Each
contributor acquires the copyrighted/external sources independently;
nothing in this directory is redistributable as part of the NewTOS repo.

The directory itself is **gitignored** (`/references/`) except for this
README.

---

## Layout

```
references/
├── README.md                 # this file (committed)
├── compendium/               # Atari Compendium PDF (acquire separately)
├── profibuch/                # Atari Profibuch ST/STE (acquire separately)
├── tos-release-notes/        # Original Atari TOS release notes per version
├── atari-st-internals/       # Abacus / Data Becker book
├── bcm2711/                  # BCM2711 ARM Peripherals datasheet
├── rpi4-boot/                # Raspberry Pi 4 boot documentation
├── rpi-os-tutorials/         # Andre Richter's tutorials (git submodule recommended)
└── arm-arm/                  # ARM Architecture Reference Manual (ARMv8-A)
```

EmuTOS source lives under `references/emutos/` as a full upstream
checkout (see "EmuTOS source" below). It is the **primary** source for
Track A per the Source-Analysis-Protocol.

---

## EmuTOS source

- **Location:** `references/emutos/` (a full clone of
  https://github.com/emutos/emutos, default branch `master`).
- **Role:** Primary source for Track A (TOS API semantics). The TOS API
  spec in `docs/api/` is extracted from this tree during Phase 00.
- **Read-only:** the checkout is never modified by NewTOS work. It is
  referenced and quoted, never patched.
- **Not part of the Cargo workspace:** contains a separate Makefile-
  driven C build that we do not invoke. Documentation artifact only.
- **Versioning:** the SHA of the checkout is recorded in
  [docs/STATUS.md](../docs/STATUS.md) "Recent Decisions" each time it
  is updated, so spec entries stay reproducible.

To update the EmuTOS reference checkout:

```bash
(cd references/emutos && git fetch origin && git checkout origin/master)
git -C references/emutos rev-parse HEAD   # record this SHA in docs/STATUS.md
```

When citing EmuTOS in a spec entry or doc-comment, use the form:

```
EmuTOS @ <short-sha> — bdos/conout.c:bdos_2()
```

so that future readers can
`git -C references/emutos show <sha>:bdos/conout.c` and see exactly
what was being referenced.

---

## FreeMiNT source

- **Location:** `references/freemint/` (a clone of
  https://github.com/freemint/freemint).
- **Role:** **Secondary** Track-A source — used only for kernel
  **extensions that EmuTOS does not implement** (e.g. `Ssystem` /
  GEMDOS 0x154 and its cookie subfunctions, later MiNT syscalls). EmuTOS
  remains primary for everything it covers; FreeMiNT is consulted when
  EmuTOS lacks the function. Adopted 2026-06-20 so NewTOS can run the
  MiNT/GEM software base (see ADR-012 and the Source-Analysis-Protocol).
- **Read-only / not in the workspace:** same rules as EmuTOS — quoted,
  never patched, not built by us.
- **Versioning:** record the checkout SHA in `docs/STATUS.md` when used.
  Current checkout: **`f906416`** (cloned 2026-06-20).

Cite FreeMiNT in spec entries / doc-comments as:

```
FreeMiNT @ f906416 — sys/ssystem.c:s_system(), sys/cookie.c:get_cookie()
```

---

## Required-source checklist

Source-Analysis-Protocol step 1 demands that every task verify its
sources are present. The following table tracks acquisition status on
**this** development machine. Each contributor maintains their own copy.

| Source                        | Status (this machine) | Path                          |
|-------------------------------|-----------------------|-------------------------------|
| EmuTOS (master)               | present               | `emutos/`                     |
| Atari Compendium              | TODO                  | `compendium/`                 |
| Atari Profibuch ST/STE        | TODO                  | `profibuch/`                  |
| TOS Release Notes             | TODO                  | `tos-release-notes/`          |
| Atari ST Internals            | TODO (optional)       | `atari-st-internals/`         |
| BCM2711 ARM Peripherals       | TODO                  | `bcm2711/`                    |
| RPi 4 Boot Documentation      | TODO                  | `rpi4-boot/`                  |
| Andre Richter Tutorials       | TODO                  | `rpi-os-tutorials/`           |
| ARM ARM (ARMv8-A)             | TODO                  | `arm-arm/`                    |

Status meanings:

- **present** — file/directory exists and matches the expected layout
- **TODO** — must be acquired before tasks that depend on it begin
- **N/A** — not needed for any planned phase

---

## Acquisition pointers (no warranty, copyright respect)

- **Atari Compendium** — Dan Hollis, 1992. Long out of print; archived
  PDF scans circulate on Atari preservation sites (atariarchives.org,
  atari-forum.com). Personal-archive use only.
- **Atari Profibuch ST/STE** — Jankowski / Reschke (Sybex / Markt &
  Technik). Out of print; second-hand, German.
- **TOS Release Notes** — distributed with original Atari developer
  toolkits; mirrored by the Atari developer community.
- **Atari ST Internals** — Abacus / Data Becker. Out of print.
- **BCM2711 ARM Peripherals** — official PDF on
  https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf
- **RPi 4 Boot Documentation** —
  https://www.raspberrypi.com/documentation/computers/raspberry-pi.html
- **Andre Richter Tutorials** —
  https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials
- **ARM ARM (ARMv8-A)** — DDI 0487 from
  https://developer.arm.com/documentation/ddi0487

---

## Pre-flight check

The protocol mentions `cargo xtask check-references`. This subcommand is
**not yet implemented** (tracked under STATUS.md). Until it lands, the
acquisition table above is the authoritative manual checklist. Tasks
that require a missing source must be marked blocked in
`docs/STATUS.md`.
