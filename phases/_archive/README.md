# Archived phase documents (pre-2026-05-03 phasing)

These phase docs were archived when the phase plan was restructured to
mirror the EmuTOS module layout (`bios/`, `bdos/`, `vdi/`, `aes/`,
`desk/`).

The current phase index lives in `phases/README.md`. The new docs map
back to these archived ones as follows:

| New phase                     | Replaces (archived)                          |
|-------------------------------|----------------------------------------------|
| 03 BIOS HW Layer              | PHASE-03-memory-management (D1–D5 portion)   |
| 04 BDOS Memory                | PHASE-03-memory-management (D6 portion)      |
| 05 High-Half Migration        | PHASE-03-memory-management ("Phase-03.5" item) |
| 06 BDOS Process               | PHASE-06-process-management                   |
| 07 BDOS Misc                  | (carved out of old 02, 06)                    |
| 08 BIOS Block Devices         | PHASE-04-block-storage + parts of 07          |
| 09 BDOS File System           | PHASE-05-gemdos-filesystem                    |
| 10 BIOS Screen                | PHASE-08-xbios-display                        |
| 11 XBIOS                      | PHASE-07-bios-xbios-devices (XBIOS portion)   |
| 12 VDI                        | PHASE-12-gem-aes-vdi (VDI portion)            |
| 13 AES                        | PHASE-12-gem-aes-vdi (AES portion)            |
| 14 Desktop                    | (new — was implicit in 12)                    |
| 15 First C Source Port        | PHASE-09-first-source-port                    |

**Removed entirely:** PHASE-10-m68k-emulator and PHASE-11-first-m68k-binary.
m68k userland emulation is a follow-on project, not part of this
codebase. Once NewTOS boots GEM applications natively, an m68k
emulator (if desired) becomes a separate repository that consumes
NewTOS as a host.

Files retained for historical context only. Do not implement against
them; the new docs in `phases/PHASE-XX-*.md` are authoritative.
