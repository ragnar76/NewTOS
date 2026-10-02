# BCM2711 VideoCore mailbox + framebuffer — hardware reference

> Track-B reference per `docs/SOURCE-ANALYSIS-PROTOCOL.md` §"Track B".
> Produced **before** the driver (CLAUDE.md Rule 6: no invented
> hardware). Target: Raspberry Pi 4 (BCM2711), AArch64, low-peripheral
> mode. First test vehicle: QEMU `raspi4b` (debuggable), then real HW.
>
> **Status: implemented + verified on QEMU `raspi4b`** (640×480×32 FB
> allocated via the property interface; boot test pattern confirmed by
> QMP screendump). Drivers: `kernel/src/drivers/bcm2711/mailbox.rs` +
> `framebuffer.rs`. Run: `cargo xtask run --board rpi4 [--window]`.
> Two findings during bring-up, both folded into the notes below:
> (1) the firmware enters the kernel at **EL2** → boot does an EL2→EL1
> drop; (2) our XRGB8888 store needs pixel order **BGR**, not RGB. The
> framebuffer is mapped **Normal Non-Cacheable** (MAIR AttrIdx 2,
> `mmu::remap_normal_nc`) so the display DMA sees pixel writes on real
> HW without per-frame cache cleaning. Real-HW verification still
> pending (no serial; FB is the life sign).

## Sources

| # | Source | Used for |
|---|--------|----------|
| 1 | BCM2711 ARM Peripherals datasheet (datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf), §1.2 address map / §1.3 mailboxes | peripheral base, mailbox region |
| 2 | raspberrypi/firmware wiki — [Mailboxes](https://github.com/raspberrypi/firmware/wiki/Mailboxes) | register offsets |
| 3 | raspberrypi/firmware wiki — [Accessing mailboxes](https://github.com/raspberrypi/firmware/wiki/Accessing-mailboxes) | status bits, read/write procedure, channel encoding |
| 4 | raspberrypi/firmware wiki — [Mailbox property interface](https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface) | channel 8 protocol, buffer format, tag IDs + value layouts |
| 5 | `references/circle` (in-tree, pinned) — `include/circle/bcmpropertytags.h`, `lib/bcmframebuffer.cpp`, `lib/2dgraphics.cpp`, `addon/linux/raspberrypi-firmware.h` | virtual-offset + vsync tag ids and value layouts, the double-buffer sequence (§3b), clock-rate + temperature tag ids and value layouts (§3d), command-line tag (§3e, also `lib/koptions.cpp`, `doc/cmdline.txt`) |

Cross-ref (verified-pattern, tertiary): rpi bare-metal FB tutorials
(valvers part 5), circle. **Linux is cross-reference only**, not
authority (Rule 6).

---

## 1. Mailbox transport

### Base address

Peripheral base in **low-peripheral mode** = `0xFE00_0000` (datasheet
§1.2.4; same base the PL011 driver uses). The mailbox block sits at
offset `0xB880`:

```
MBOX_BASE = 0xFE00_B880
```

> Legacy VC bus address is `0x7E00_B880`; in 64-bit low-peripheral mode
> the ARM sees it at `0xFE00_B880`. (QEMU `raspi4b`: verify by reading
> back the buffer address under GDB — see §5.)

### Registers (offsets from `MBOX_BASE`) — source #2

Two mailboxes, each an 8-deep FIFO of 32-bit words:

| Mailbox | Read | Peek | Sender | Status | Config |
|---------|------|------|--------|--------|--------|
| 0 (VC→ARM) | `0x00` | `0x10` | `0x14` | `0x18` | `0x1C` |
| 1 (ARM→VC) | `0x20`*(Write)* | `0x30` | `0x34` | `0x38` | `0x3C` |

- **ARM reads** responses from **mailbox 0** `Read` (`0x00`), polling
  mailbox 0 `Status` (`0x18`).
- **ARM writes** requests to **mailbox 1** `Write` (`0x20`), polling
  mailbox 1 `Status` (`0x38`).

### Status bits — source #3

```
MAIL_FULL  = 0x8000_0000   # set in a Write mailbox's status when full
MAIL_EMPTY = 0x4000_0000   # set in a Read  mailbox's status when empty
```

### Channel encoding — source #3

The low **4 bits** of a mailbox word are the channel; the upper **28
bits** are the payload. For the property interface the payload is a
16-byte-aligned buffer address, whose low 4 bits are therefore already
0 and hold the channel directly:

```
write word = (buffer_addr & ~0xF) | channel
read  word: channel = word & 0xF ; payload = word & ~0xF
```

> ⚠ The firmware-wiki pseudo-code shows `(data << 4) | channel` /
> `data >>= 4`. That convention is for a 28-bit *opaque* payload. For
> the **property channel the payload is an address** and the wiki
> property page states "only the upper 28 bits of the address can be
> passed" — i.e. the address is 16-byte aligned and we use
> `(addr & ~0xF) | channel`, **no shift**. Using a shift here would
> corrupt the address. This is the convention all bare-metal FB drivers
> use; do not "fix" it to a shift.

### Procedure

```
mbox_write(channel, addr):
    while status1(0x38) & MAIL_FULL: spin
    dsb                                   # ordering before VC reads it
    write1(0x20) = (addr & ~0xF) | channel

mbox_read(channel) -> payload:
    loop:
        while status0(0x18) & MAIL_EMPTY: spin
        dmb
        word = read0(0x00)
        if (word & 0xF) == channel: return word & ~0xF
        # else: response for another channel, discard & retry
```

Channel for the property/tags interface: **8** (source #4,
"Channel 8: Request from ARM for response by VC").

---

## 2. Property message buffer (channel 8) — source #4

A single 16-byte-aligned `u32` array. Layout:

```
[0] u32  total buffer size in bytes (including this header + end tag + pad)
[1] u32  request/response code:
           request  = 0x0000_0000
           success  = 0x8000_0000   (response: request processed OK)
           error    = 0x8000_0001
--- one or more tags ---
[ ] u32  tag id
[ ] u32  value buffer size in bytes
[ ] u32  tag request/response code:
           request: 0
           response: 0x8000_0000 | (response value length in bytes)
[ ] ...  value buffer (tag-specific, padded to 32-bit)
--- end ---
[ ] u32  end tag = 0x0000_0000
[ ] ...  pad to a multiple of 16 bytes
```

The buffer is **both** input and output: the VC overwrites the value
buffers in place and sets bit 31 of each tag's code. Always check
buffer word [1] == `0x8000_0000` before trusting the results.

---

## 3. Framebuffer allocation tags — source #4

Tag IDs and exact value-buffer layouts:

| Tag id | Name | Request value | Response value |
|--------|------|---------------|----------------|
| `0x0004_0003` | get physical (display) w/h | (none, 0 B) | `u32 width, u32 height` (8 B) |
| `0x0004_8003` | set physical (display) w/h | `u32 width, u32 height` (8 B) | same (8 B) |
| `0x0004_8004` | set virtual (buffer) w/h | `u32 width, u32 height` (8 B) | same (8 B) |
| `0x0004_8005` | set depth | `u32 bpp` (4 B) | `u32 bpp` (4 B) |
| `0x0004_8006` | set pixel order | `u32 state` (4 B) | `u32 state` (4 B) |
| `0x0004_8007` | set alpha mode | `u32 state` (4 B) | `u32 state` (4 B) — *"may not be the same as the request so it must be checked"* |
| `0x0004_0007` | get alpha mode | (none, 0 B) | `u32 state` (4 B) |
| `0x0004_0001` | allocate framebuffer | `u32 alignment` (4 B) | `u32 base, u32 size` (8 B) |
| `0x0004_0008` | get pitch | (none, 0 B) | `u32 bytes_per_line` (4 B) |

Pixel order `state`: **`0x0 = BGR`, `0x1 = RGB`**.

Alpha mode `state` (source #4, *"Get alpha mode"*, verbatim):

> * 0x0: Alpha channel enabled (0 = fully opaque)
> * 0x1: Alpha channel reversed (0 = fully transparent)
> * 0x2: Alpha channel ignored

(Both ids corroborated by the firmware's own header carried in-tree,
`references/circle/addon/linux/raspberrypi-firmware.h:81,102` —
`RPI_FIRMWARE_FRAMEBUFFER_GET_ALPHA_MODE = 0x00040007`,
`RPI_FIRMWARE_FRAMEBUFFER_SET_ALPHA_MODE = 0x00048007`.) Why NewTOS
sends it: §3c.

A standard allocation sends all seven tags in one message (the VC
processes them in order, so the geometry/depth/order/alpha mode are set
before `allocate` runs):

```
set physical  W x H
set virtual   W x H        (= physical for a non-scrolled FB)
set depth     32           (bpp; 32-bit XRGB for bring-up)
set pixel order 0x0        (BGR — see note)
set alpha mode 0x2         (ignored — see §3c)
allocate      alignment=16 -> base, size
get pitch                  -> bytes_per_line
```

> **Pixel order (verified on raspi4b):** our pixel store is XRGB8888
> packed as `0xFF_RR_GG_BB`, i.e. little-endian memory bytes
> `[B, G, R, X]` (blue lowest). The VC's **BGR** order (`0x0`) reads the
> lowest byte as blue → matches. Selecting RGB (`0x1`) swaps red/blue
> (a QMP screendump showed the red bar as blue). Use **BGR**.

For `allocate`, pass the alignment (e.g. 16 or a page = 4096) in the
request value; the VC returns the **bus base address** and the byte
size.

---

## 3b. Virtual offset — the page flip — and the vsync tag

The framebuffer's **virtual** size (tag `0x0004_8004`) may be larger than
its **physical** (displayed) size (tag `0x0004_8003`). The display engine
then scans out a `physical`-sized window inside the larger buffer, and a
second tag moves that window:

| Tag id | Name | Request value | Response value |
|--------|------|---------------|----------------|
| `0x0004_8009` | set virtual offset | `u32 offset_x, u32 offset_y` (8 B) | same, echoed (8 B) |
| `0x0004_0009` | get virtual offset | (none, 0 B) | `u32 offset_x, u32 offset_y` (8 B) |
| `0x0004_800E` | wait for vsync | (none, 0 B) | (none) |

**Source (in-tree, verifiable):** `references/circle` — a bare-metal RPi
library that drives exactly this interface:

- `references/circle/include/circle/bcmpropertytags.h:65`
  `#define PROPTAG_SET_VIRTUAL_OFFSET 0x00048009`
- `references/circle/include/circle/bcmpropertytags.h:282-288`
  `struct TPropertyTagVirtualOffset { TPropertyTag Tag; u32 nOffsetX; u32 nOffsetY; } PACKED;`
- `references/circle/lib/bcmframebuffer.cpp:33` — the init-tag table entry
  `{{PROPTAG_SET_VIRTUAL_OFFSET, 8, 8}, 0, 0}`, i.e. **value buffer 8 bytes,
  request length 8 bytes**
- `references/circle/lib/bcmframebuffer.cpp:386-399` — `SetVirtualOffset`,
  which sends the tag and then **checks the firmware echoed the offsets
  back**; a firmware that could not honour the request answers with the
  offsets it kept
- `references/circle/include/circle/bcmpropertytags.h:67`
  `#define PROPTAG_WAIT_FOR_VSYNC 0x0004800E`, used by
  `CBcmFrameBuffer::WaitForVerticalSync` (`lib/bcmframebuffer.cpp:405-410`)
  with an empty value buffer
- Corroboration from a second in-tree copy of the firmware's own header:
  `references/circle/addon/linux/raspberrypi-firmware.h:83,103` —
  `RPI_FIRMWARE_FRAMEBUFFER_GET_VIRTUAL_OFFSET = 0x00040009`,
  `RPI_FIRMWARE_FRAMEBUFFER_SET_VIRTUAL_OFFSET = 0x00048009`

**How double buffering is built from it** (`references/circle/lib/
2dgraphics.cpp:540-552`): allocate with `virtual height = 2 × physical
height`, draw into the half that is not shown, then
`SetVirtualOffset(0, swapped ? height : 0)`. Circle calls
`WaitForVerticalSync()` immediately before the swap.

**NewTOS use.** `kernel/src/drivers/bcm2711/framebuffer.rs` asks for
`virtual height = 2 × physical` at boot (`build_request(w, h, pages)`) and
implements the flip as `set_virtual_offset(0, page × height)`. That is the
hardware underneath XBIOS `Setscreen`'s `physLoc` — the VideoCore's
stand-in for the Atari video-base register EmuTOS' `atari_setphys` writes
(`references/emutos/bios/screen.c:939-947`). The firmware may refuse the
larger allocation (it echoes the virtual height it accepted, and the
allocate tag reports the byte size it granted), so the driver believes those
two numbers rather than the number it asked for, and falls back to a
single-page framebuffer if the double-height allocation fails outright.

**Observed on QEMU `raspi4b`** (QEMU 11, 2026-09-05): the emulated
VideoCore accepts `virtual 640×960` for a `physical 640×480` display,
returns pitch 2560 and a 2400 KiB buffer (= exactly two screens), and
`0x0004_8009` moves the scan-out — verified by a QMP screendump pair around
an XBIOS `Setscreen` from an EL0 program.

`0x0004_800E` (wait for vsync) is **documented here but not yet used**:
NewTOS' `Vsync` (XBIOS 0x25) paces on the ARM generic timer instead, see
`docs/api/xbios/25-vsync.md`.

---

## 3c. Alpha mode — why NewTOS sets "ignored" (2026-09-21)

The framebuffer is 32 bits per pixel, but only 24 of them are colour. The
top byte of every pixel (`0xXX_RR_GG_BB`, memory bytes `[B, G, R, X]`)
belongs to the VDI: it holds the Falcon screen word's `OVERLAY_BIT`
(EmuTOS `vdi/vdi_fill.c:26`), replicated into all eight bits — `0x00` on a
pen-drawn pixel, `0xFF` on a cleared or complemented one (the contract in
`kernel/src/hal/framebuffer.rs`, module doc; V11/V12, 2026-09-21). The
display must show the same colour for both values.

None of the Track-B sources say what the VideoCore does with that byte at
depth 32 when no alpha mode has been set — Circle, for one, writes `0xFF`
there as "alpha" when it packs a palette entry
(`references/circle/lib/bcmframebuffer.cpp:137`), which is consistent with a
firmware that *may* read it. Before 2026-09-21 the kernel happened to write
`0xFF` on every pixel and the real Pi 4 displayed it; after the overlay-bit
change both values occur on one screen, and with mode `0x0` ("enabled,
0 = fully opaque") the byte would read as opacity. Rather than assume a
default, the driver sends the documented **set alpha mode = `0x2` (alpha
channel ignored)** in the allocate message (`build_request`, between set
pixel order and allocate), reads the tag's echoed state (the wiki: *"the
response may not be the same as the request so it must be checked"*), then
asks **get alpha mode** (`0x0004_0007`) and logs one line at boot:

```
[fb] alpha mode = 2
```

A firmware that does not implement the tag leaves the tag's response bit
clear (§3a: *"Unknown tags are ignored (the response bit will not be
set)"*) — the allocation still succeeds, and the line says so
(`... (set tag unanswered)` / `= not reported (get tag unanswered; ...)`)
instead of printing a number the firmware never wrote. An echoed state
other than `2` is printed as `(set tag echoed N, asked 2)`.

**Observed on QEMU `raspi4b`** (QEMU 11, 2026-09-21): the emulated
VideoCore answers both tags — `0x0004_8007` comes back with its response
bit set and the state `2` echoed, `0x0004_0007` reports `2` — so the boot
log reads `[fb] alpha mode = 2`, and the allocation is unchanged
(`640x480 pitch 2560 virtual height 960 -> 2 screen page(s), 2400 KiB`).
The desktop screendump is byte-identical to the one taken without the tag
(`measurements/measurement-2026-09-21-v11-v12/screendumps/desk-after-idle.png`), as it
must be: QEMU never scanned the X byte out. The real-Pi acceptance is
item 30 of `docs/api/desktop/_hw-test-2026-09-17.md`.

---

## 3a. Memory tags — how much DRAM does the ARM get? — source #4

The same property channel answers the "how much RAM is there" question.
Two complementary tags, both in the *Hardware* group of the firmware
*Mailbox property interface* page (source #4):

| Tag id | Name | Request value | Response value |
|--------|------|---------------|----------------|
| `0x0001_0005` | Get ARM memory | (none, 0 B) | `u32 base, u32 size` (8 B) |
| `0x0001_0006` | Get VC memory | (none, 0 B) | `u32 base, u32 size` (8 B) |

Both entries carry the same verbatim footnote in the reference:

> *"Future formats may specify multiple base+size combinations."*

Three consequences, all of them properties of the documented interface,
not assumptions:

1. **The tag reports the split, not the fitted total.** "ARM memory" is
   the share the firmware hands to the CPU; "VC memory" is the GPU's
   share (`gpu_mem` in `config.txt`). The ARM share is exactly what an OS
   may allocate from — the VC share holds the framebuffer, codec buffers
   and firmware housekeeping and must never be handed to `Malloc`.
2. **A response may be longer than 8 bytes.** The tag's code word is
   `b31 | value length in bytes` (§2), so the response length — not a
   fixed 8 — says how many `(base, size)` pairs came back. A reader must
   therefore offer a value buffer sized for several pairs and honour the
   returned length. (The reference also warns: *"Response length
   indicates the desired length even when it is longer than the buffer
   size filled"* — so clamp to the buffer you provided.)
3. **A single pair cannot describe more than 4 GiB**, both fields being
   `u32`. On a machine with more RAM than one pair can express, the
   answer is necessarily either several pairs or a truncated view.

Unanswered tags are how a mailbox that does not implement a tag presents
itself: *"Unknown tags are ignored (the response bit will not be set)"*
(§2). Checking the buffer's overall response code is therefore **not**
enough for a query — the tag's own `b31` must be tested as well.

**Observed on QEMU `raspi4b`** (QEMU 11, `qemu-system-aarch64 -M raspi4b`,
2026-09-02): one pair, `ARM = 0x0000_0000 + 960 MiB`,
`VC = 0x3C00_0000 + 64 MiB` — sum exactly 1 GiB — and the answer does
**not** change when the machine is given `-m 2G`. The emulated firmware
therefore describes only the legacy low-1-GiB window. Whether the real
Pi-4 firmware answers the same way on a 4 GB board, or with several
pairs, is a hardware question; the kernel prints every pair it receives
(`kernel/src/lib.rs::firmware_dram`) so one serial log settles it.

Driver: `kernel/src/drivers/bcm2711/mailbox.rs::get_memory`, one message
carrying both tags.

---

## 3d. System-info tags — clock rate and temperature — sources #4, #5

The same property channel answers "how fast is the CPU running" and "how
warm is the die". Both tags live in the *Clocks* / *Voltage* group of the
firmware *Mailbox property interface* page (source #4); the pinned
in-tree `references/circle` (source #5) carries the same ids and the value
structs verbatim, which is what fixes the request layout:

| Tag id | Name | Request value | Response value |
|--------|------|---------------|----------------|
| `0x0003_0002` | Get clock rate | `u32 clock id` (4 B) | `u32 clock id, u32 rate in Hz` (8 B) |
| `0x0003_0006` | Get temperature | `u32 sensor id` (4 B) | `u32 sensor id, u32 value in 1/1000 °C` (8 B) |

Ids, from `references/circle/include/circle/bcmpropertytags.h`:

| Constant | Value | Line |
|----------|------:|------|
| `PROPTAG_GET_CLOCK_RATE` | `0x00030002` | `:39` |
| `PROPTAG_GET_TEMPERATURE` | `0x00030006` | `:41` |
| `CLOCK_ID_ARM` (the CPU core clock) | 3 | `:184` |
| `TEMPERATURE_ID` (the only SoC sensor) | 0 | `:196` |

The request shape — id in, 4 request-parameter bytes, 8 value bytes back —
is `references/circle/lib/cputhrottle.cpp:394-418`
(`Tags.GetTag(nTagId, &TagClockRate, sizeof TagClockRate, 4)`; `sizeof` is
tag header + 8 value bytes, the trailing `4` is the request parameter
length). Both tags therefore share one buffer shape.

Two rules a reader must honour, both from the interface itself (§2 and
§3a): an unanswered tag comes back with its `b31` **clear** — that is how a
mailbox which does not implement the tag presents itself — and the tag's own
code word carries the value length, so a response shorter than the 8 bytes
the reply format requires is not an answer either.

One further rule is taken from the reference implementation rather than the
interface text: **a clock rate of 0 means "no rate for this clock"**, not
"0 Hz". Circle checks exactly that before falling back to another source
(`references/circle/lib/machineinfo.cpp:459-471`:
`Tags.GetTag(…) && TagClockRate.nRate != 0`). NewTOS reports it as
"no answer". A temperature of 0 is *not* filtered — 0.000 °C is a
temperature, and circle does not filter it either
(`lib/cputhrottle.cpp:408-418` returns `nValue` unchanged).

**Observed on QEMU `raspi4b`** (QEMU 11.0.2, `qemu-system-aarch64 -M
raspi4b`, 2026-09-24): the emulator's firmware model answers **both** tags,
with `rate = 700 000 000` Hz for clock id 3 and `value = 25 000` m°C for
sensor id 0 — fixed numbers of the model, identical across boots, not a
measurement. They are reported faithfully as what the firmware said; only a
real Pi 4 shows a clock that moves with the governor and a die that warms
under load.

Driver: `kernel/src/drivers/bcm2711/mailbox.rs::get_clock_rate` /
`::get_temperature`, both over the shared `query_id_value` helper. Consumer:
GEMDOS `Ssystem` modes 2000/2001 through `Host::cpu_clock_hz` /
`Host::cpu_temperature_mdeg` (`docs/api/gemdos/154-ssystem.md`).

---

## 3e. Command-line tag — `cmdline.txt` reaches the kernel — source #5

SER01 (2026-10-02) switches the serial output with an option in
`cmdline.txt` on the boot partition. The kernel reads that text through the
property channel, the way the pinned in-tree `references/circle` (source #5)
reads its own kernel options:

| Item | Value | Where |
|------|-------|-------|
| Tag id `PROPTAG_GET_COMMAND_LINE` | `0x0005_0001` | `include/circle/bcmpropertytags.h:72`; same id `RPI_FIRMWARE_GET_COMMAND_LINE` in `addon/linux/raspberrypi-firmware.h:113` |
| Value buffer | 2048-byte string (`TPropertyTagCommandLine::String[2048]`) | `bcmpropertytags.h:305-310` |
| Request | no parameter bytes | `lib/koptions.cpp:53` (`GetTag(…, sizeof m_TagCommandLine)`, default `nRequestParmSize = 0`, `bcmpropertytags.h:321`), `lib/bcmpropertytags.cpp:49-71` |
| Reply | value length = string length, **no NUL**; length 0 = no answer; a length that fills the whole buffer is rejected | `bcmpropertytags.cpp:64-68`, `koptions.cpp:58-61` |
| Text format | options on one line, separated by a space | `doc/cmdline.txt:3-4` |

That the firmware fills this tag from `cmdline.txt` is what circle's
documentation states (`doc/cmdline.txt:3`: *"Create a file cmdline.txt on the
SD(HC) card used for booting"*) and what `CKernelOptions` relies on
(`lib/koptions.cpp:29-61`). The device tree's `/chosen/bootargs` is **not**
used: circle never reads it, no local source documents what the Pi firmware
writes there, and QEMU `raspi4b` started with `-kernel` hands over no device
tree at all (x0 = `0x100`, no FDT magic: every raspi4b serial log says
`[mm] device tree: no usable /memory node (blob invalid or absent)`).

**Observed on QEMU `raspi4b`** (QEMU 11.0.2, 2026-10-02,
`measurements/measurement-2026-10-02-ser01/`): the firmware model answers the
tag with the `-append` string — `-append serial=on` switches the output on,
no `-append` leaves it off. That makes `-append` the emulator's stand-in for
`cmdline.txt`.

Driver: `kernel/src/drivers/bcm2711/mailbox.rs::with_command_line`.
Consumer: `kernel/src/console.rs::configure_serial_tx` with the token parser
`kernel/src/bootargs.rs`.

---

## 4. Framebuffer bus → ARM address conversion

The `allocate` tag returns a **VC bus address**, not an ARM physical
address. The VC firmware's mini-MMU maps ARM phys → bus, and the **top
2 bits select caching** (source: search of BCM2711 addressing notes;
valvers part 5). To get the ARM-visible physical address, clear the
upper bits:

```
fb_phys = bus_addr & 0x3FFF_FFFF
```

(`0x3FFF_FFFF` = low 30 bits; the masked-off `0xC000_0000` is the
"uncached alias" selector.) On the RPi4 the framebuffer lives in the
first GiB, so the masked address is directly mappable.

> On QEMU `raspi4b` the returned address may already be ARM-physical
> (no `0xC...` alias). **Do not hardcode the mask result — apply the
> mask (it is idempotent for addresses already < 0x4000_0000) and then
> verify the actual returned value under GDB** (§5).

---

## 5. QEMU `raspi4b` test notes

- `qemu-system-aarch64 -M raspi4b -kernel kernel8.img` (QEMU ≥ 7;
  ours is 11). The machine models the BCM2711 mailbox; the property FB
  path is emulated but may be **incomplete** — treat the GDB-observed
  mailbox transaction (not just a visible image) as the primary
  success signal.
- Breakpoint after `mbox_read` returns and inspect: buffer word [1] ==
  `0x8000_0000`, the `allocate` response base/size, and the `get pitch`
  value. A non-zero base + plausible pitch (`width * 4`, possibly
  rounded up) means the transaction worked even if QEMU does not paint
  a window.
- Then write a test pattern (R/G/B/white quadrants, as the ramfb
  bring-up did) and confirm via QEMU's display or a screendump.
- Only after the QEMU transaction is verified do we move to real HW,
  where the framebuffer is itself the first life sign (no serial).

---

## 6. NewTOS integration

- New module `kernel/src/drivers/bcm2711/mailbox.rs` (transport) and
  the FB allocation in `kernel/src/drivers/bcm2711/framebuffer.rs`,
  implementing the existing [`Framebuffer`](../../kernel/src/hal/framebuffer.rs)
  trait (same `put_pixel`/`fill_rect`/`clear`/`rgb` surface as ramfb).
- Gated behind the `board-rpi4` feature (mirrors `qemu_virt/ramfb.rs`
  under `board-qemu-virt`). Per **ADR-011**, the resulting framebuffer
  is the BCM2711 physical screen workstation's device.
- The mailbox buffer is in cacheable DRAM; `mailbox::call_property`
  does an explicit clean (before) + invalidate (after) by VA to PoC
  around the transaction so the VC sees the request and we see the
  reply. (QEMU is lax here; real HW is not.)
- **Framebuffer memory is mapped Normal Non-Cacheable** (`mmu::
  remap_normal_nc`, MAIR AttrIdx 2) after allocation. The display
  controller is a separate DMA master reading DRAM beyond the ARM
  caches, so a Write-Back FB would strand drawn pixels in dirty cache
  lines (invisible on real HW; QEMU's TCG has no cache and hides it).
  NC removes the coherency problem with no per-frame `dc cvac`. The
  2-MiB-block remap uses break-before-make (ARM ARM K11.5).
- 32-bit depth (XRGB) for bring-up; the VDI palette screen
  (8-bit) is a later concern — note in
  [`STATUS.md`](../STATUS.md) "Blocked"/scope if VDI needs 8-bit.
