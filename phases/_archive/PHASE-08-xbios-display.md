# PHASE-08: XBIOS Display

## Goal

Bring up an HDMI framebuffer on RPi4 (via the VideoCore mailbox interface)
and on QEMU virt (via virtio-gpu or a simulated framebuffer), and implement
the XBIOS display syscalls so source-ports can set up screens, palettes,
and resolution-equivalent modes.

## Inputs

- `CLAUDE.md`, Phases 01–07 complete
- `docs/api/xbios.md` — display-related entries written first
- Atari Compendium, XBIOS chapter, "Screen Functions"
- VideoCore mailbox property interface documentation
- BCM2711 datasheet
- EmuTOS: `bios/screen.c`, `bios/lineavars.h`

## Deliverables

### D1: Framebuffer HAL trait
`Framebuffer` trait: get/set buffer, geometry, format. Two impls:
mailbox-based for RPi4, virtio-gpu for QEMU virt.

### D2: Mode mapping
TOS modes (ST Low/Med/High, TT modes, Falcon modes) map to fixed RPi4
resolutions per a documented table:

| TOS mode      | Resolution | Bit depth | RPi4 framebuffer config       |
|---------------|------------|-----------|-------------------------------|
| ST Low        | 320×200    | 4 bpp     | scaled to 1280×800            |
| ST Medium     | 640×200    | 2 bpp     | scaled to 1280×800            |
| ST High       | 640×400    | 1 bpp     | scaled to 1280×800            |
| TT Low        | 320×480    | 8 bpp     | …                             |
| TT Medium     | 640×480    | 4 bpp     | …                             |
| TT High       | 1280×960   | 1 bpp     | native or 2× nearest neighbor |

Decision: rendering is software-bitmapped to a virtual framebuffer in TOS
format, then blitted (with palette lookup and integer scale) to the RPi
framebuffer. This preserves source-port semantics.

### D3: XBIOS display syscalls
- `Setscreen` (5)
- `Setpalette` (6), `Setcolor` (7)
- `Floprd` / `Flopwr` — error return (no floppy on RPi4), document
- `Logbase` (3), `Physbase` (2), `Getrez` (4)
- `Vsync` (37), `EsetShift` / `EsetPalette` / `EsetGray` / etc. for TT/
  Falcon modes (subset)

### D4: Compositor
Background task / IRQ handler that copies the virtual framebuffer to the
RPi framebuffer at vsync. This **is** preemptive, even if the rest of the
system isn't — it runs in interrupt context.

## Exit criteria

- **EC1**: A native ARM test program sets ST Low mode, fills the screen
  with a known palette pattern, and the result appears on HDMI within 100 ms.
- **EC2**: `Vsync` blocks until the next vertical retrace, with timing
  jitter < 1 ms.
- **EC3**: Mode change from ST Low to ST High preserves the kernel
  console output (compositor handles re-blit).
- **EC4**: All spec entries complete.

## Out of scope

- BLiTTER emulation (later, possibly via deferred software impl)
- Line-A primitives (Phase 12 with GEM)
- Hardware sprites (no analog on RPi4 framebuffer)
- Genlock, video DAC programming, monitor type detection

## Risks

- **R1**: VideoCore mailbox protocol is fiddly. Use a tested implementation
  pattern (Andre Richter has one) and cite it.
- **R2**: Frame buffer alignment. RPi VideoCore demands specific alignment;
  misalignment means mailbox call fails silently.
- **R3**: Mode switch flicker. Acceptable in this phase; document as known
  limitation.

## Claude Code instructions

- Display is the most "fun" phase and the most tempting place to add scope.
  Resist. Implement only the spec. Demos and effects come later, in user
  programs.
- Software bitmap rendering to a virtual framebuffer is the canonical
  approach. Do not be tempted to "use the GPU directly" for ST mode
  rendering — it breaks source-port compatibility.
