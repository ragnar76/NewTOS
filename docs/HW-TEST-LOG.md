# HW-TEST-LOG — Real Raspberry Pi 4 bring-up

> Manual hardware verification log + setup guide for booting NewTOS on
> a physical Raspberry Pi 4 (BCM2711) over a serial console. Closes
> PHASE-01 **EC2** once the boot banner appears on real hardware.
>
> Everything so far has only been verified in QEMU (`virt`,
> `cortex-a72`). This document is the checklist for the first metal boot.

---

## 1. What you need

- A Raspberry Pi 4 (any RAM size — the kernel currently uses the first
  1 GiB unless the firmware-supplied DTB reports more).
- A micro-SD card (any size; we only use a small FAT32 boot partition).
- A **3.3 V** USB-to-TTL serial adapter (e.g. CP2102 / FT232 / CH340).
  **Do not** use a 5 V adapter — the Pi's GPIO is 3.3 V and 5 V will
  damage it.
- A host terminal program at **115200 baud, 8N1, no flow control**
  (`screen /dev/tty.usbserial-XXXX 115200`, `minicom`, `picocom -b
  115200`, or PuTTY).

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
| _pending — awaiting USB-TTL adapter_ | | | | |
