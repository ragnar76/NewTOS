# NewTOS SDK — developer documentation

This is the entry point for anyone who wants to build a C program for NewTOS
or port an existing TOS/GEM C application to it. Every statement in these
pages is taken from a file in this repository (cited as `path:line`) or from a
command that was actually run while writing them (the command and its output
are quoted). Where something could not be verified it is marked
**unverified**.

Repository revision the documentation was written against:
`49b2c2ce29458119ee064eb993efe1efff66bd0e` (branch `desk-rebuild`, 2026-09-17).

## Pages

| Page | What it answers |
|------|-----------------|
| [01-overview.md](01-overview.md) | What the toolchain is, what it produces (`.NCP` / `.NAP` / `.NTP`), what it cannot do |
| [02-prerequisites.md](02-prerequisites.md) | Which compiler, linker and host tools the scripts use, and where they look for them |
| [03-first-programs.md](03-first-programs.md) | Hello World (`.NCP`), a mintlib-linked program, and a minimal GEM application (`.NAP`) — built and run for this document |
| [04-porting-anatomy.md](04-porting-anatomy.md) | The overlay rule ("the app source stays pristine"), how libc/gemlib/cflib are built, how QED, TeraDesk and CoreMark are built, resource files |
| [05-abi-pitfalls.md](05-abi-pitfalls.md) | The trap ABI, and every LP64 / address-space / semantic trap that has bitten a port so far — with symptom and countermeasure |
| [06-image-and-testing.md](06-image-and-testing.md) | Getting a binary onto the SD image, `AUTO`, `NEWDESK.INF` `#Z`, running in QEMU and on a real Pi 4, screendumps, serial log |
| [07-mintlib-limits.md](07-mintlib-limits.md) | What the ported mintlib does not provide, derived from `sdk/libc/expected-failures.txt` and the test programs; `setjmp` and the libm (OpenLibm) since T07 |
| [08-porting-checklist.md](08-porting-checklist.md) | The short, tickable list for porting an application |
| [09-port-material.md](09-port-material.md) | What source material we hold, in which version, under which licence — and the age trap a frozen third-party library is |
| [10-ldg-modules.md](10-ldg-modules.md) | LDG run-time libraries as native `.NLD` modules: libldg, building a module, loading it from a client, what is not there (TSR, `ldg_callback`, search path) — TASK/LDG-M1; libshare 0.1.1 as the allocator of every module (licence unclear, §6) — TASK/LDG-M3-B2 |
| [11-codec-libraries.md](11-codec-libraries.md) | zlib, libpng, libjpeg (IJG), giflib, libexif as static NewTOS libraries for zView's codecs: versions, pins, how to link, which codec needs what, the LIBTEST.NCP round-trip test — TASK/LDG-M2 |
| [12-windom.md](12-windom.md) | WinDom 1.21.3 (+ zView's patch) as `libwindom1.a`: pin, flags, the 10 LP64/endianness overlays, the LP64 sweep, LDG's WinDom demo client, open problems (mouse clicks, module heap) — TASK/LDG-M3 |
| [13-zview.md](13-zview.md) | zView (LDG version) as `ZVIEW.NAP` + ten codec modules `CODECS\*.NLD`, without PDF: pin, build rules, the 12 overlays (endianness, two m68k-asm retargets, `.NLD`, PDF omission), samples, what is verified, open kernel blocker K1 (`graf_handle` workstation not open) — TASK/ZVIEW-M4 |

## Thirty-second version

```sh
# freestanding console program (no libc): sdk/build.sh
sdk/build.sh sdk/examples/hello.c /path/to/HELLO2.NCP

# libc-, gemlib- and cflib-linked programs: the per-app scripts
sdk/libc/build-libc.sh   [client.c] [out.NCP]     # builds libc.a, links client
sdk/gemlib/build-gemlib.sh [client.c] [out.NCP]   # builds gemlib.a, links client
sdk/cflib/build-cflib.sh   [client.c] [out.NCP]   # builds cflib.a, links client
sdk/libm/build-libm.sh     [client.c] [out.NCP]   # builds libm.a (OpenLibm), links client + libc
sdk/ldg/build-ldg.sh                               # libldg.a (LDG r133, tarball + overlay/)
sdk/libshare/build-libshare.sh                     # libshare.a (libshare 0.1.1, every .NLD links it; 10 §6)
sdk/ldg/build-ldg-demo.sh                          # EX_LIB.NLD (LDG demo module) + LDGTEST.NCP
sdk/windom/build-windom.sh                         # libwindom1.a (WinDom 1.21.3 + zView patch, 12)
sdk/ldg/build-ldg-client.sh                        # LDGDEMO.NAP (LDG's WinDom demo client) + POPUP.NLD
sdk/zlib/build-zlib.sh  sdk/libpng/build-libpng.sh  sdk/libjpeg/build-libjpeg.sh
sdk/giflib/build-giflib.sh  sdk/libexif/build-libexif.sh   # codec libraries (11)
sdk/examples/libtest/build-libtest.sh              # LIBTEST.NCP (their round-trip test)
sdk/apps/build-qed.sh                              # QED.NAP
sdk/apps/build-teradesk.sh                         # DESKTOP.NAP + dist/
sdk/apps/build-coremark.sh                         # CORE.NCP, COREVAL.NCP

# put everything on an SD image and boot it in QEMU
tools/make-sd-image.sh /tmp/sd.img 64
cargo xtask run --board rpi4 --sd /tmp/sd.img       # add --window to see the screen
```

The one rule that governs every port: **the application source is not
edited**. Every change lives in `sdk/overlays/<app>/` (or in the library
overlays `sdk/{libc,gemlib,cflib,libm,ldg,libshare,libjpeg,libexif,windom}/overlay/`) and is layered over the
pristine tree at build time. The only exception is genuine m68k assembly, which must be
retargeted. See [04-porting-anatomy.md](04-porting-anatomy.md).

## Where the scripts and sources live

```
sdk/
├── build.sh              freestanding .NCP builder (no libc)
├── crt0.S                startup for freestanding programs
├── newtos.ld             linker script for every native program
├── include/newtos.h      clean-room trap helpers (Cconws/Cconin/Pterm/...)
├── lib-stale.sh          library dependency chain + staleness test
├── libc/                 mintlib-0.60.1 port (tarball + overlay/ + newtos-arch/)
├── gemlib/               gemlib-0.44.0 port (tarball + overlay/)
├── cflib/                cflib-21 port (tarball + overlay/)
├── libm/                 OpenLibm 0.8.8 port (tarball + overlay/ + include/ math.h adapters), T07
├── ldg/                  LDG r133 devel library port (tarball + overlay/), .NLD demo + client, LDG-M1
├── zlib/ libpng/ libjpeg/ giflib/ libexif/   codec libraries for zView (release tarball + config overlay), LDG-M2
├── vendor-lib.sh         shared build steps of those five
├── overlays/{qed,teradesk,coremark}/   per-application overlays
├── apps/                 build scripts + vendored app trees (untracked)
└── examples/             example programs (examples/libtest/: the codec-library test, LDG-M2)
tools/make-sd-image.sh    builds the FAT16 SD image, installs the binaries
tools/mkrsc.py            converts classic .RSC files to the native format
```
