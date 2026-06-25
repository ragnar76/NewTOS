# auto-test — a NewTOS AUTO-folder test program

A minimal AArch64 program that demonstrates the `\AUTO\*.PRG` boot path:
it prints a message via `Cconws`, waits ~5 seconds (300 `Vsync` frames at the
~60 Hz host frame clock), then `Pterm`s so the boot sequence continues to the
desktop.

## Build

```sh
python3 build.py
```

This assembles [`auto_test.s`](auto_test.s) with the Homebrew AArch64 binutils
(`aarch64-elf-as` / `-ld` / `-objcopy`) and wraps the code in a NewTOS-loadable
ELF (`AUTOTEST.PRG`): one `ET_EXEC` ELF64 with a single RWX `PT_LOAD` at
vaddr `0x100`, entry `0x100` — the shape the NewTOS program loader expects
(BasePage in the first 256 bytes, code/entry at `0x100`, one 4-KiB page).

## Deploy

Copy `AUTOTEST.PRG` into the boot drive's `\AUTO\` folder (the FAT16 `C:`
partition). On macOS, strip the AppleDouble metadata afterwards so no stray
`._AUTOTEST.PRG` is left on the FAT volume:

```sh
cp AUTOTEST.PRG "/Volumes/BOOT/AUTO/"
dot_clean "/Volumes/BOOT"
```

## ABI

NewTOS syscall ABI: `x8` = function number, `x0..` = arguments.
GEMDOS = `svc #1`, XBIOS = `svc #14`. A syscall clobbers only `x0` (the
result); `x1..x30` are restored from the trap frame.
