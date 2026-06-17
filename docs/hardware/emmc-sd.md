# BCM2711 EMMC2 (microSD) — hardware reference

> Track-B reference per `docs/SOURCE-ANALYSIS-PROTOCOL.md`. Produced
> **before** the driver (Rule 6). Target: the **on-board microSD card
> slot** of the Raspberry Pi 4 — the card that holds the boot files —
> which is wired to the **EMMC2** controller. First test vehicle: QEMU
> `raspi4b` (emulates the SDHCI controller; debuggable).

## Which controller (important)

The RPi4 has three storage-ish paths; we target exactly one:

| Controller | Phys base | Connected to |
|------------|-----------|--------------|
| **EMMC2 (Arasan/iproc SDHCI)** | **`0xFE34_0000`** | **the microSD card slot** (boot card) ← us |
| sdhost (legacy BCM2835 EMMC) | `0xFE30_0000` | internal SDIO → Wi-Fi chip (not the slot) |
| xHCI (USB) | — | external USB storage (later, loadable driver) |

Confirmed by the official Linux device tree (`bcm2711-rpi-4-b.dts`:
`emmc2@7e340000` is the SD slot) and circle `include/circle/bcm2711.h`
(`ARM_EMMC2_BASE = ARM_IO_BASE + 0x340000` → `0xFE340000`). The
VideoCore firmware uses this card to load `start4.elf`/`kernel8.img`;
once our kernel runs the firmware is done, so we **re-initialise** the
controller and take it over (same physical card).

EMMC2's SD lines are dedicated (not muxed through the GPIO ALT function
selects the legacy sdhost needed), so no GPIO setup is required.

## Sources

| # | Source | Used for |
|---|--------|----------|
| 1 | BCM2711 ARM Peripherals datasheet §1.2 (address map) | EMMC2 base |
| 2 | BCM2835 ARM Peripherals datasheet §5 "External Mass Media Controller" | the EMMC register grouping (Arasan) — EMMC2 follows it |
| 3 | SD Host Controller Simplified Specification (SD Association) | register/bit semantics (EMMC2 = standard SDHCI) |
| 4 | SD Physical Layer Simplified Specification (SD Association) | the command/init sequence |
| 5 | circle (`rsta2/circle`, `addon/SDCard`) | verified-working RPi4 EMMC2 reference |

Exact register bits are validated empirically against QEMU `raspi4b`
during bring-up (QEMU implements the SDHCI standard).

---

## 1. Register map (32-bit accesses, offsets from `0xFE340000`)

The BCM groups the SDHCI registers into 32-bit words (datasheet §5).

| Off | Name | Contents |
|-----|------|----------|
| `0x00` | `ARG2` | argument 2 (ACMD23) |
| `0x04` | `BLKSIZECNT` | `[15:0]` block size, `[31:16]` block count |
| `0x08` | `ARG1` | command argument |
| `0x0C` | `CMDTM` | `[31:16]` command, `[15:0]` transfer mode |
| `0x10`..`0x1C` | `RESP0..3` | command response |
| `0x20` | `DATA` | buffer data port (PIO) |
| `0x24` | `STATUS` | present state (see below) |
| `0x28` | `CONTROL0` | host control 1 + power + block-gap |
| `0x2C` | `CONTROL1` | clock control + timeout + software reset |
| `0x30` | `INTERRUPT` | normal + error interrupt status |
| `0x34` | `IRPT_MASK` | interrupt status enable |
| `0x38` | `IRPT_EN` | interrupt signal enable |
| `0x3C` | `CONTROL2` | auto-CMD-err + host control 2 |
| `0x40` | `CAPABILITIES` | controller capabilities |
| `0xFC` | `SLOTISR_VER` | slot ISR + host-controller version |

### `STATUS` (present state) bits

```
0   CMD_INHIBIT     CMD line busy (cannot issue a command)
1   DAT_INHIBIT     DAT line busy
2   DAT_ACTIVE      data transfer active
8   WRITE_TRANSFER
9   READ_TRANSFER
10  BUFF_WRITE_EN   buffer ready for a PIO write
11  BUFF_READ_EN    buffer holds PIO read data
16  CARD_INSERTED
```

### `CONTROL1` (clock + reset) bits

```
0       CLK_INTLEN    internal clock enable
1       CLK_STABLE    internal clock stable (read)
2       CLK_EN        SD clock enable
6:7     CLK_FREQ_MS2  upper 2 bits of the 10-bit divider
8:15    CLK_FREQ8     lower 8 bits of the divider (SDCLK = base / (2*div))
16:19   DATA_TOUNIT   data timeout exponent
24      SRST_HC       reset the whole host controller
25      SRST_CMD      reset the command circuit
26      SRST_DATA     reset the data circuit
```

### `INTERRUPT` bits

```
0   CMD_DONE     command complete
1   DATA_DONE    transfer complete
4   WRITE_RDY    buffer write ready
5   READ_RDY     buffer read ready
15  ERR          an error bit (16..) is set
16+ CTO/CCRC/... command/data timeout & CRC errors
```

### `CMDTM` — command word (`[31:16]`)

```
29:24 CMD_INDEX      the SD command number
21    CMD_ISDATA     command involves a data transfer
17:16 CMD_RSPNS_TYPE 0=none, 1=136-bit, 2=48-bit, 3=48-bit-busy
19    CMD_CRCCHK_EN  check the response CRC
20    CMD_IXCHK_EN   check the response index
```

### `CMDTM` — transfer mode (`[15:0]`)

```
1   TM_BLKCNT_EN   block-count enable
4   TM_DAT_DIR     1 = card → host (read)
5   TM_MULTI_BLOCK multi-block transfer
```

---

## 2. Controller init

1. **Reset:** set `CONTROL1.SRST_HC`; poll until it clears.
2. **Clock — identification (~400 kHz):** set `CONTROL1` =
   `CLK_INTLEN | (divider) | (DATA_TOUNIT=0xE)`, wait `CLK_STABLE`, then
   set `CLK_EN`. The base clock is the EMMC2 clock (≈ 100 MHz on RPi4 /
   reported via the mailbox `get clock rate` tag, clock id 1 = EMMC).
3. **Enable interrupt status** (`IRPT_MASK = 0xFFFFFFFF`) but **poll**
   `INTERRUPT` rather than taking IRQs in the bring-up driver.

After card identification, switch the clock to ~25 MHz (default speed).

> EMMC2 base clock: query the mailbox property `get clock rate`
> (tag `0x00030002`, clock id `1` = EMMC) — reuse the mailbox transport
> from `docs/hardware/mailbox-framebuffer.md`. QEMU returns a usable
> rate; on HW it is the real EMMC clock.

---

## 3. SD card init sequence (SD Phys. Layer spec)

Issue a command by: wait `!STATUS.CMD_INHIBIT`, write `ARG1`, write
`CMDTM`, wait `INTERRUPT.CMD_DONE` (check error bits), read `RESP0..3`.

| Cmd | Index | Arg | Resp | Purpose |
|-----|-------|-----|------|---------|
| GO_IDLE | CMD0 | 0 | none | reset card to idle |
| SEND_IF_COND | CMD8 | `0x1AA` | R7 | voltage check (echo `0xAA`); R7=`0x1AA` ⇒ SD v2 |
| APP_CMD | CMD55 | `RCA<<16` | R1 | prefix for an ACMD |
| SD_SEND_OP_COND | ACMD41 | `0x5100_0000` | R3 | `HCS`(bit30)+voltage; loop until OCR bit31=ready; OCR bit30=`CCS` ⇒ SDHC |
| ALL_SEND_CID | CMD2 | 0 | R2 | card identification |
| SEND_REL_ADDR | CMD3 | 0 | R6 | get the card's `RCA` |
| SELECT_CARD | CMD7 | `RCA<<16` | R1b | put card in transfer state |
| SET_BLOCKLEN | CMD16 | `512` | R1 | block length (SDSC only; SDHC fixed at 512) |

`CCS` (Card Capacity Status) decides addressing: **SDHC/SDXC use block
addressing** (arg = block number), **SDSC uses byte addressing** (arg =
block × 512).

---

## 4. Single-block read (CMD17)

1. `BLKSIZECNT = (1 << 16) | 512` (1 block, 512 bytes).
2. `ARG1 = block_addr` (block number for SDHC).
3. `CMDTM = (17 << 24) | RSPNS_48 | CRCCHK | IXCHK | ISDATA | TM_DAT_DIR`.
4. Wait `INTERRUPT.CMD_DONE`; check errors.
5. Wait `INTERRUPT.READ_RDY`; read 128 × 32-bit words from `DATA` into
   the buffer.
6. Wait `INTERRUPT.DATA_DONE`.

Multi-block (CMD18) and writes (CMD24/25) follow the same pattern with
`TM_MULTI_BLOCK`/`TM_BLKCNT_EN` and the opposite `TM_DAT_DIR`; added
after single-block read works.

---

## 5. NewTOS integration

- **`BlockDevice` HAL trait** (`kernel/src/hal/block.rs`), mirroring the
  `Framebuffer` trait: `block_count()`, `block_size()` (= 512),
  `read_blocks(lba, buf)`, `write_blocks(lba, buf)`.
- **Driver** `kernel/src/drivers/bcm2711/emmc.rs` (board-rpi4): the
  SDHCI controller + SD protocol above. The QEMU `raspi4b` `emmc`
  device is the dev vehicle — attach an SD image with `-drive
  file=sd.img,if=sd,format=raw`.
- **DMA / cache:** the bring-up path uses **PIO** (read words from the
  `DATA` register), so there is no DMA-coherency concern like the
  framebuffer. A later DMA path would need the same PoC clean/invalidate
  discipline.
- **BIOS layer (later):** `Rwabs`/`Mediach`/`Getbpb` on Trap 13 sit on
  top of `BlockDevice`; sector 0 read → FAT/MBR signature `0x55AA` at
  offset 510 is the bring-up exit check (EC1 reads via `Rwabs`).
- **QEMU vs HW base (verified).** QEMU `raspi4b` instantiates *two*
  `generic-sdhci` controllers (`0xFE300000` and `0xFE340000`) and wires
  its emulated SD card to the **legacy EMMC at `0xFE300000`** (that one
  is clocked at 200 MHz; the emmc2 clock reads 0 Hz). Real RPi4 has the
  microSD on **EMMC2 `0xFE340000`**. Both are standard SDHCI, so the
  driver `init()` **probes EMMC2 first then EMMC** and uses whichever has
  a responding card — works on QEMU and (expected) on HW with one binary.
- **Bus power matters:** after `SRST_HC`, set `CONTROL0` Power Control
  (`0x0F00` = SD bus power + 3.3 V) before issuing commands, or the card
  never responds (every response-command times out with `CTO`).
- **Bring-up status:** init + single-block read verified on QEMU
  `raspi4b` (reads a known sector 0 back byte-exact, `0x55AA` signature).
  Real-HW verification pending: there, the card is on EMMC2 and is the
  live boot card — read-only sector probes are safe; the EMMC2 base
  clock is already running from the firmware boot.
