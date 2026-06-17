# HW-TEST-LOG — Real Raspberry Pi 4 bring-up

> Manual hardware verification log + setup guide for booting NewTOS on
> a physical Raspberry Pi 4 (BCM2711) over a serial console. Closes
> PHASE-01 **EC2** once the boot banner appears on real hardware.
>
> **First metal boot succeeded 2026-06-17** (graphics path: VideoCore
> framebuffer test pattern on HDMI; serial console not yet wired). The
> boot/MMU/heap path and the BCM2711 mailbox framebuffer run on real
> silicon. Serial-console verification (boot banner + self-tests over
> PL011) still pending the USB-TTL adapter — see §7.

---

## 1. What you need

- A Raspberry Pi 4 (any RAM size — the kernel currently uses the first
  1 GiB unless the firmware-supplied DTB reports more).
- A micro-SD card (any size; we only use a small FAT32 boot partition).
- A **3.3 V** USB-to-TTL serial adapter (e.g. CP2102 / FT232 / CH340).
  **Do not** drive the Pi's RX with 5 V signalling — the GPIO is 3.3 V
  and 5 V will damage it. If the adapter is level-selectable
  (e.g. a "3.3 / 5 V" cable), select **3.3 V**.
  - **In use here:** Waveshare USB-TO-TTL (D), **FT232RNL**, USB-A,
    SH1.0 3-pin (TXD / RXD / GND), 3.3 / 5 V selectable — use the
    **3.3 V** setting. FTDI wire convention: black = GND, yellow = the
    adapter's RXD, orange = the adapter's TXD (verify against the
    cable's own labels).
- A host terminal at **115200 baud, 8N1, no flow control**.
  - **macOS** (FT232R works with the built-in driver — no install on
    recent macOS):
    ```sh
    ls /dev/tty.usbserial-*                  # find the device
    screen /dev/tty.usbserial-XXXX 115200    # exit: Ctrl-A then K
    ```
  - Alternatives: `minicom`, `picocom -b 115200 /dev/tty.usbserial-XXXX`,
    or PuTTY.

## 2. Wiring (USB-TTL ↔ Pi 4 GPIO header)

The kernel drives **PL011 / UART0** on GPIO 14/15. Connect three wires
(TX↔RX crossed), sharing a common ground. **Leave the adapter's 5 V /
3.3 V power pin disconnected** — the Pi is powered separately.

| USB-TTL pin | Pi 4 GPIO (BCM) | Pi 4 header pin |
|-------------|-----------------|-----------------|
| RX  (adapter in)  | GPIO 14 — TXD0 | pin 8  |
| TX  (adapter out) | GPIO 15 — RXD0 | pin 10 |
| GND               | GND            | pin 6  |

(Adapter RX ← Pi TX, adapter TX → Pi RX. If you only see output but
typing does nothing, you likely swapped TX/RX or left GND off.)

## 3. SD-card layout (FAT32 boot partition)

Format the card with a single FAT32 partition and copy:

1. **Raspberry Pi 4 firmware** (from the official `raspberrypi/firmware`
   `boot/` folder): `start4.elf` and `fixup4.dat` (the minimum for a
   bare-metal 64-bit kernel; `bootcode.bin` is **not** used on the Pi 4).
2. **`config.txt`** — use [`boot/rpi4/config.txt`](../boot/rpi4/config.txt)
   from this repo. The `init_uart_clock=48000000` line is mandatory.
3. **`kernel8.img`** — built below.

That's it. No `cmdline.txt` is required for a bare-metal kernel.

## 4. Build `kernel8.img`

```sh
cargo xtask build --board rpi4 --release
# → produces ./kernel8.img at the workspace root (raw binary via objcopy)
```

Copy that `kernel8.img` to the SD card's boot partition.

## 5. Boot

1. Insert the SD card, connect the USB-TTL wires, open the host terminal
   at 115200 8N1.
2. Power the Pi.
3. **Expected first output** (within a second or two):

   ```
   RB
   NewTOS booting on rpi4
   Phase 02 reached.
   [mm] ...
   ```

   `RB` is the **pre-MMU boot heartbeat** — the boot stub writes it to
   the PL011 data register as its very first action, before any MMU /
   stack / BSS setup. Seeing `RB` confirms the firmware loaded
   `kernel8.img`, the CPU reached our entry point, and the serial path
   + baud are correct. The banner and self-test sequence follow (the
   same one QEMU prints: `[selftest p04] ... 10 passed`, `EC4/D3/D6/D7
   PASS`, `D3/D4D5 PEXEC PASS`, `EINVFN OK`, then the `[cconin]` and
   `RX echo` prompts — type a key to see it echoed).

## 6. Troubleshooting

| Symptom | Likely cause |
|---|---|
| **Nothing at all — not even `RB`** | The CPU never reached our entry, or serial is dead: wrong baud (must be 115200); TX/GND not connected or TX/RX swapped; `config.txt` missing (`enable_uart=1` / `arm_64bit=1`); firmware files (`start4.elf`/`fixup4.dat`) or `kernel8.img` missing/misnamed. |
| **`RB` then nothing** | Serial + firmware + entry are **fine** — a boot stage *after* the heartbeat faulted (MMU stage-1, BSS, stack, or the high-half jump). This narrows it to `boot.rs` / `mmu.rs`; capture exactly how far it got. |
| **Garbage / random bytes** | UART clock mismatch — confirm `init_uart_clock=48000000` is in `config.txt`. This is the #1 Pi 4 cause. |
| **Output but typing does nothing** | Adapter TX → Pi RX (GPIO 15) not connected, or TX/RX swapped. |
| **Banner then hangs / faults** | A real-hardware MMU / cache / boot-stage difference vs QEMU — capture the full log and the `[EXC]` line if any, and record it below. |

The pre-MMU `RB` heartbeat (boot stub) makes the silent-early-failure
case diagnosable: **no `RB`** = serial/firmware/entry problem; **`RB`
then silence** = a fault in our own early boot code.

---

## 7. Test log

Record each metal boot attempt here (date, firmware version, RAM size,
what appeared, any fault).

| Date | Pi model / RAM | Firmware | Result | Notes |
|------|----------------|----------|--------|-------|
| 2026-06-17 | RPi 4 Model B | `start4.elf`/`fixup4.dat` (card, 2026-04-17) | ✅ **First metal boot — VideoCore framebuffer test pattern visible on HDMI** (red / green+white-box / blue, 640×480 on a 1080p screen) | **Graphics path only** (no serial yet). Confirms on real silicon: EL2→EL1 drop (firmware enters at EL2), mailbox property-interface FB allocation against real VideoCore firmware, **Non-Cacheable FB mapping** (without it the display DMA would not see the cached pixels — the exact failure QEMU's TCG cannot reproduce), BGR pixel order, and the whole MMU/heap boot path. SD card written from this repo (`kernel8.img` + `init_uart_clock=48000000` added to an existing RPi4 `config.txt`). |
| 2026-06-17 | RPi 4 Model B | as above | ✅ **First full VDI frame on metal** — the Phase-12 VDI demo (16 GEM palette tiles via `vr_recfl` + line grid/border via `v_pline`, every pixel through `svc #2`) renders correctly on HDMI. | **Found + fixed a real HW-only cache bug en route:** the first attempt showed the filled tiles sharp but the **lines as dashes / missing** — dense `fill_rect` writes evict whole cache lines (visible), sparse single-pixel `put_pixel` writes stay stranded in untouched lines (invisible). The Non-Cacheable FB mapping was *not* effective on real silicon for these writes; QEMU's TCG has no cache so it could never surface there. Fix: `mmu::clean_dcache_range` + clean each drawn region (`dc cvac` to PoC) after `fill_rect`/`draw_line`. After the fix the grid lines are solid. The diagnostic signature (dense OK, sparse broken) pinpointed it without a serial console. |
| _pending_ | | | _serial console_ | Boot banner + self-tests over PL011 once the Waveshare FT232RNL USB-TTL (D) adapter is wired (ordered 2026-06-16). Heartbeat will read `RB2` (EL2 entry digit). |
