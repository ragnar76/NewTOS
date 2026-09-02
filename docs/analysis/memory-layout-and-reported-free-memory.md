# Speicher-Layout von NewTOS und die Zahlen, die eine Anwendung sieht

> **Stand:** 2026-09-02, Branch `desk-rebuild`. Auslöser: TeraDesks Info-Box auf
> dem echten Pi 4 (4 GB) meldet `6209536 bytes ST-RAM` / `16486400 bytes TT-RAM`,
> und die Werte wandern zwischen Läufen.
>
> Jede Verhaltensaussage ist mit `file:line` belegt — unsere wie EmuTOS'.
> EmuTOS-Zitate gegen den gepinnten Klon in `references/emutos/`.

---

## 0. Ergebnis in fünf Sätzen

1. **Die ST-RAM-Zahl ist echt.** `6209536` ist exakt der freie ST-RAM-Pool, wie
   ihn der Kernel beim Boot einrichtet — die App bekommt hier die Wahrheit.
2. **Die TT-RAM-Zahl ist nicht der TT-RAM-Pool.** Sie ist der Rest des
   **16-MiB-EL0-Fensters** (ADR-021), das jeden Prozess von der physischen
   Speicherverwaltung trennt. Der TT-RAM-Pool selbst ist rund 60-mal so groß
   (~1 GiB in QEMU-raspi4b, real-Pi-Wert siehe §7).
3. **Beide Zahlen wandern aus verschiedenen Gründen**: ST-RAM springt in
   2-MiB-Stufen mit der Kernel-Image-Größe, TT-RAM sinkt monoton mit jeder
   Fenster-Abbildung des laufenden Prozesses (der Bump-Cursor wird bei `Mfree`
   nicht zurückgesetzt).
4. **Beide beschreiben strukturell das, was EmuTOS auch beschreibt** — der
   ST-RAM-Pool ist EmuTOS' `[end_os, phystop)` 1:1. Falsch waren dagegen drei
   Nachbarwerte im Sysvar-/OSHEADER-Bereich und die Rundung der `-1`-Abfrage;
   die sind in dieser Runde gefixt (§8).
5. **Von 4 GB nutzen wir höchstens den ersten DTB-Speicherbank-Eintrag.** Das
   ist ein echter, offener Hardware-Layer-Befund (§7.1), kein EmuTOS-Verstoß.

---

## 1. Die Kette, Ende zu Ende

TeraDesks Info-Box ruft `get_freemem()` auf
(`sdk/apps/teradesk/main.c:318-332`):

```c
*stsize = (long) Mxalloc(-1L, 0);   /* MX_STRAM */
*ttsize = (long) Mxalloc(-1L, 1);   /* MX_TTRAM */
```

Beide Aufrufe stehen wörtlich in den beiden committeten Traces:

```text
$ python3 tools/gdtrace.py tosdump/reference/TERADESK-GDTRACE-r2.BIN
  4   62.806   0  44 Mxalloc  len=-1 -> 6209536
  5   62.807   0  44 Mxalloc  len=-1 -> 16736256
  6   62.808   0  44 Mxalloc  len=65536 -> 4235264
```

(r1 identisch, nur früher: `6209536` / `16736256` / `4235264`.) Die Info-Box im
Bericht des Users zeigt `6209536` / `16486400` — dieselbe ST-Zahl, eine
kleinere TT-Zahl, weil sie **später** im Lauf abgefragt wurde (§6.2).

Der Weg dieser beiden Zahlen durch den Kernel:

| Schritt | Datei | Was passiert |
|---|---|---|
| 1 | `kernel/src/arch/aarch64/dtb.rs:50-130` | `parse_memory` liest den **ersten** `reg`-Eintrag des ersten `/memory@…`-Knotens |
| 2 | `kernel/src/bios/machine/layout.rs:280-320` | `discover` baut daraus `dram`, schneidet ST-RAM `[kernel_end, DRAM_START+14 MiB)` und TT-RAM `[DRAM_START+16 MiB, dram.end)` |
| 3 | `kernel/src/bios/machine/frames.rs:261-264` | Bitmap-Allokator übernimmt beide Extents |
| 4 | `kernel/src/lib.rs:483-511` | `bring_up_bdos_pools` fordert **alle** noch freien Frames als je einen zusammenhängenden Lauf an und übergibt `(start, len)` an BDOS |
| 5 | `tos-api/src/bdos/mem/init.rs:165-224` | `init` legt daraus je ein Seed-`MD` in `pmd` (ST) und `pmdalt` (TT) |
| 6 | `tos-api/src/bdos/mem/traps.rs:262-299` | `mxalloc(-1, mode)` = `pool::largest_free` des gewählten Pools — 1:1 EmuTOS `bdos/umem.c:243-278` |
| 7 | `tos-api/src/gemdos/memory.rs:83-95` | **EL0-Pfad**: das Ergebnis wird auf das Prozess-Fenster gedeckelt |

Schritt 7 ist die Stelle, an der die TT-Zahl ihre Bedeutung verliert.

---

## 2. Was die Pool-Grenzen bestimmt

### 2.1 ST-RAM — `[__kernel_image_end, DRAM_START + 14 MiB)`

`layout.rs:52-62` legt einen **festen 16-MiB-Slot** an und nimmt die oberen
2 MiB heraus (Nachbau des Atari-Fensters `$E00000–$FFFFFF`), also 14 MiB
nutzbar. Der Pool beginnt am Ende des Kernel-Images (`layout.rs:308-313`).

Das ist **strukturell exakt EmuTOS**: `bmem_init` setzt
`membot = end_os = _end_os_stram` und `memtop = phystop`
(`references/emutos/bios/biosmem.c:66-72`), `getmpb` macht daraus das einzige
freie `MD` (`biosmem.c:134-151`). Der Unterschied ist nicht die Formel, sondern
wie viel unser „OS" davon belegt.

Budget auf `board-rpi4`, Release-Build vom 2026-09-02
(`llvm-nm`/`llvm-objdump` auf `target/aarch64-unknown-none-softfloat/release/kernel`;
Linker-Skript `kernel/src/arch/aarch64/link-rpi4.ld`):

```text
0x000000–0x080000    512 KiB  Pre-MMU-Boot-Stack (link-rpi4.ld:38-44) — unterhalb
                              des Images, gehört keinem Pool
0x080000–0x0F8998    483 KiB  .text
0x0F9000–0x107DB8     60 KiB  .rodata
0x107DB8–0x133118    173 KiB  .data
0x134000–0x255F50   1159 KiB  .bss (tatsächlicher Inhalt)
0x255F50–0x400000   1704 KiB  Padding auf ALIGN(2M) vor dem .dma-Block
0x400000–0x600000   2048 KiB  .dma  (PCIe/xHCI-Puffer, link-rpi4.ld:70-77)
0x600000–0x800000   2048 KiB  Stack-Guard (absichtlich ungemappt, link-rpi4.ld:81-85)
0x800000–0x814000     80 KiB  Kernel-Stack 64K + Emergency-Stack 16K
──────────────────────────────
0x814000  = __kernel_image_end = ST-RAM-Pool-Start
0xE00000  = ST-RAM-Pool-Ende (14-MiB-Kappung)
          = 6 209 536 Byte freier ST-RAM   ← genau TeraDesks Zahl
```

Bestätigt durch die Boot-Ausgabe (`kernel/src/lib.rs:349`):

```text
[mm]   kernel: 0x0000000000080000..0x0000000000814000 (7946240 bytes)
[mm]   ST-RAM: 0x0000000000814000..0x0000000000e00000 (6209536 bytes)
```

Von den 14 MiB gehen also 8,08 MiB an das OS, davon **3,66 MiB reines Padding
und Guard** (Adressraum, keine Daten).

### 2.2 TT-RAM — `[DRAM_START + 16 MiB, dram.end)`

Der Startwert 16 MiB ist Absicht und EmuTOS-treu: `TTRAM_START` ist dort
`0x01000000` (`references/emutos/include/tosvars.h:77`), und `altram_init` gibt
genau `[TTRAM_START, ramtop)` an den Pool (`bios/memory2.c:147-150`).

Vom oberen Ende zieht der Kernel ab, was er **vor** `bring_up_bdos_pools`
belegt:

| Verbraucher | Pool | Datei |
|---|---|---|
| Kernel-Heap, 2 MiB | TT-RAM | `kernel/src/bios/machine/heap.rs:323` |
| ramfb-Framebuffer (nur `qemu-virt`), 1920×1080×4 = 7,91 MiB | TT-RAM | `kernel/src/drivers/qemu_virt/ramfb.rs:153` |

Auf dem Pi liegt der Framebuffer im VideoCore-Speicher (Mailbox), nicht im
ARM-DRAM — deshalb kostet er dort **nichts**. Die Boot-Logs zeigen genau das:

```text
qemu-virt:  [mm] BDOS pools: TT-RAM 0x419e9000+258043904   (0x9E9000 = 2 MiB Heap + 7,91 MiB fb)
rpi4:       [mm] BDOS pools: TT-RAM 0x01200000+1054867456  (0x200000 = 2 MiB Heap)
```

---

## 3. Alle Wege, auf denen eine Anwendung Speicher „sieht"

| Weg | Was sie bekommt | Wo |
|---|---|---|
| `Mxalloc(-1, MX_STRAM)` | freier ST-RAM-Pool, gedeckelt aufs EL0-Fenster | `gemdos/memory.rs:156-171` |
| `Mxalloc(-1, MX_TTRAM)` | **Fenster-Rest**, nicht der TT-Pool | dito |
| `Malloc(-1)` | wie `Mxalloc(-1, PF_TTRAMMEM ? MX_PREFTTRAM : MX_STRAM)` — 1:1 EmuTOS `bdos/umem.c:116-124` | `gemdos/memory.rs:185-212` |
| `Mxalloc(n)` / `Malloc(n)` | Block aus dem echten Pool, **in das Fenster gemappt**, Rückgabe = User-VA | `gemdos/memory.rs:83-120` |
| `Getmpb` (BIOS 0) | ein `MD` über `[stram_start, stram_end)` | `tos-api/src/bios/getmpb.rs:47-72` |
| `phystop`@`$42E`, `membot`@`$432`, `memtop`@`$436` | ST-RAM-Pool-Grenzen als **physische** Adressen | `bdos/process/loader.rs:886-888` |
| `end_os`@`$4FA`, `ramtop`@`$5A4`, `ramvalid`@`$5A8` | neu in dieser Runde, §8.1 | dito |
| OSHEADER `os_beg`/`os_end` | korrigiert in dieser Runde, §8.1 | dito |
| Basepage `p_lowtpa`/`p_hitpa` | **immer 4 MiB**, unabhängig von der Maschine | `loader.rs:636-637`, `bdos/process/arena.rs:52` |
| Boot-Splash „ST-RAM/TT-RAM" | Frame-Allokator-Totale, also die **echten** Pool-Größen | `kernel/src/initinfo.rs:142-144` |

Der Boot-Splash und die App widersprechen sich also systematisch — der Splash
sagt die Wahrheit über die Maschine, die App die Wahrheit über ihr Fenster.

### 3.1 Das EL0-Fenster (ADR-021), der Grund für die TT-Zahl

Klassisches TOS ist flach und ungeschützt; NewTOS führt Apps bei **EL0** hinter
`TTBR0_EL1` aus, während der EmuTOS-treue Pool physische Adressen liefert. ADR-021
legt deshalb einen dünnen Übersetzer darum: der Block kommt aus dem echten Pool,
wird in ein **Prozess-Fenster** gemappt, und die App bekommt die User-VA
(`tos-api/src/bdos/process/usermem.rs:1-48`).

Das Fenster ist genau 16 MiB groß:

```rust
// tos-api/src/bdos/process/usermem.rs:64-68
const WINDOW_BASE: u64 = super::arena::TPA_SIZE;                     // 4 MiB
const WINDOW_SIZE: u64 = mmu::USER_TPA_MAX_VA - super::arena::TPA_SIZE;
// mmu.rs:70,73,77: USER_L3_COUNT = 10 → USER_TPA_MAX_VA = 20 MiB → WINDOW_SIZE = 16 MiB
```

Und die `-1`-Abfrage wird darauf gedeckelt (`gemdos/memory.rs:85-95`). Die
Rechnung geht exakt auf:

```text
TeraDesk lädt zuerst desktop.rsc (39 043 B, sdk/apps/teradesk/build/dist/)
  → rsrc_load mappt page_up(39043) = 40 960 B ins Fenster
    (tos-api/src/aes/resource.rs:1005-1022; die Datei liest der Kernel über
     bdos::fs::read_file, resource.rs:952 — deshalb steht davor kein Fopen
     im GEMDOS-Trace)
  → cursor = 40 960
Mxalloc(-1, MX_TTRAM) = min(TT-Pool ≈ 1 GiB, 16 777 216 − 40 960) = 16 736 256  ✓ Trace
Mxalloc(-1, MX_STRAM) = min(6 209 536,       16 736 256)          =  6 209 536  ✓ Trace
```

Und die Folge davon im selben Trace: TeraDesk dimensioniert seinen globalen
Puffer aus genau dieser Zahl (`main.c:1268-1277`) —
`16736256/512000 = 32`, `>16 → ×2 = 64`, gekappt auf `[1,128]`, `×1024` =
**65536** — das ist Record 6. Mit der echten TT-RAM-Größe (~1 GiB) käme dort
`128 KiB` heraus. Das Fenster halbiert TeraDesks Arbeitspuffer.

> **Nach dem Fix aus §8.2** meldet die ST-Zeile `6 205 440` statt `6 209 536`
> (eine Seite weniger, dafür tatsächlich allozierbar); die TT-Zeile bleibt
> `16 736 256`, weil dort das Fenster bindet, und TeraDesks Pufferrechnung
> nimmt ohnehin das Maximum beider Werte. Die Traces oben sind der Zustand
> **vor** dem Fix.

---

## 4. Ist die ST-RAM-Zahl richtig?

Ja, im Sinne des Vertrags: `Mxalloc(-1, MX_STRAM)` liefert
`pool::largest_free(pmd)` — dieselbe Semantik wie EmuTOS' `ffit(-1,&pmd)`
(`bdos/umem.c:247-251`), und der Pool ist beim App-Start unberührt (die TPA
kommt mit `MX_PREFTTRAM` aus dem TT-Pool, `bdos/process/arena.rs:106-110`),
also ist der Wert die volle Poolgröße.

Zwei Einschränkungen, beide unser Design, nicht EmuTOS':

* **14 MiB ist eine Kappung, keine Messung** (`layout.rs:55-62`). Auf einem
  echten TT/Falcon ist das die Hardware-Obergrenze, insofern Atari-treu.
* **8,08 MiB davon frisst das OS.** EmuTOS liegt im ROM; im ST-RAM steht nur
  seine BSS bis `__end_os_stram` (`references/emutos/emutos.ld:143-160`). Unser
  Kernel liegt dagegen **vollständig** im ST-RAM-Fenster — Image, Puffer, Stack
  und Guard. Das ist der Grund, warum von 14 MiB nur 5,9 MiB übrig bleiben, und
  der einzige Hebel, an dem §9.1 ansetzt.

## 5. Ist die TT-RAM-Zahl richtig?

**Nein — sie beantwortet eine andere Frage als die gestellte.** `Mxalloc(-1,
MX_TTRAM)` heißt „wie groß ist der größte freie TT-RAM-Block". Wir antworten
„wie viel Adressraum hat dein Fenster noch". Die Zahl ist als *Allokations­grenze*
ehrlich (mehr kann die App tatsächlich nicht bekommen), als *Maschinen­auskunft*
aber um den Faktor ~60 zu klein.

Bewertung nach der Projektregel: das ist **keine EmuTOS-Abweichung aus Bequemlichkeit**,
sondern die Folge der Speicherschutz-Realisierung — also Hardware-/Architekturebene,
die ausdrückliche Ausnahme. Sie ist in ADR-021 dokumentiert und in
`docs/api/gemdos/memory/44-mxalloc.md` als „the one deliberate, documented contract
adaptation" ausgewiesen. **Aber:** die 16 MiB sind kein Naturgesetz, sondern eine
Konsequenz von `USER_L3_COUNT = 10`, und diese Grenze ist heute der einzige Grund,
warum eine App auf einer 4-GB-Maschine nicht mehr als 16 MiB bekommen kann (§9).

---

## 6. Warum die Zahlen wandern

### 6.1 ST-RAM: die Kernel-Image-Größe, in 2-MiB-Stufen

Der Pool beginnt bei `__kernel_image_end`. Zwei `ALIGN(2M)` im Linker-Skript
(`link-rpi4.ld:70,73,81`) quantisieren diesen Wert: solange die BSS unter
`0x400000` bleibt, landet `.dma` bei `0x400000`, `.stack` bei `0x600000` und das
Image-Ende bei `0x814000` — **unabhängig davon, ob Debug oder Release** (beide
Builds vom 2026-09-02 zeigen `__kernel_image_end = 0xffff000000814000`).

Wächst die BSS über eine 2-MiB-Grenze, springen `.dma` und `.stack` mit, und der
freie ST-RAM fällt in einem Schritt um 2 MiB: `6 209 536 → 4 112 384`. Genau so
eine Änderung war ADR-021 selbst — `USER_L3_COUNT` 3→10 hat `__USER_TABLES` auf
851 968 Byte gebracht (`llvm-nm`, größtes BSS-Symbol überhaupt).

Aktueller Abstand zur nächsten Stufe: BSS endet bei `0x255F50`, die Grenze liegt
bei `0x400000` → **1,66 MiB Luft**. Danach kippt die Zahl.

### 6.2 TT-RAM: der Fenster-Cursor, monoton

`usermem::map` vergibt Fenster-VAs aus einem **Bump-Cursor** und
`usermem::free` gibt ihn **nicht** zurück (`usermem.rs:296-320`, dort auch als
Absicht dokumentiert: die stehengelassenen PTEs sparen den TLBI, QUIRK-001). Also:

```text
Prozessstart:            cursor =       0  → 16 777 216
nach desktop.rsc:        cursor =  40 960  → 16 736 256   (Trace, Record 5)
… weitere Allokationen …
Info-Box beim User:      cursor = 290 816  → 16 486 400   (71 Seiten)
```

Die 71 Seiten setzen sich nachvollziehbar zusammen: 10 (desktop.rsc) + 16
(der 64-KiB-Puffer aus Record 6) + 4 (icons.rsc, 13 269 B → Record 10) + 41
weitere Seiten aus TeraDesks Laufzeit. **Die Zahl kann im selben Lauf nur
fallen, nie steigen** — auch nach `Mfree`.

Das ist die Hauptursache für „warum ist das immer so unterschiedlich": die
TT-Zahl hängt davon ab, *wann* man die Info-Box öffnet und was die App vorher
getan hat.

### 6.3 TT-RAM (Pool): die Bildschirmauflösung — nur `qemu-virt`

`ramfb.rs:153` nimmt den Framebuffer aus dem TT-RAM-Frame-Pool. 1920×1080×4
kostet 7,91 MiB; eine andere Auflösung verschiebt den Pool-Start. Auf dem Pi
tritt das nicht auf (VideoCore-Speicher).

### 6.4 DRAM-Erkennung: DTB vs. Compile-Zeit-Default

`discover` nimmt den DTB-Wert nur, wenn `mem.start == DRAM_START` **und**
`mem.size >= 16 MiB` (`layout.rs:287-296`); sonst gilt der Board-Default
(1 GiB auf rpi4, `layout.rs:166-172`). Welcher Zweig gilt, steht in der ersten
Bootzeile: `[mm] DTB at 0x…` vs. `[mm] no DTB — using compiled-in DRAM size`
(`kernel/src/lib.rs:213-217`). Die auf der SD-Karte liegende
`bcm2711-rpi-4-b.dtb` enthält im `/memory@0`-Knoten `reg = <0x0 0x0 0x0>` —
Größe **null**; erst `start4.elf` patcht die echten Bänke hinein. Fällt dieses
Patchen je aus, greift lautlos der 1-GiB-Default.

---

## 7. Was von 4 GB tatsächlich ankommt

### 7.1 Offener Befund: nur die erste DTB-Speicherbank

`dtb::parse_memory` liest den ersten `reg`-Eintrag des ersten `/memory@…`
und **kehrt sofort zurück** (`kernel/src/arch/aarch64/dtb.rs:126-130`) — weitere
Einträge desselben `reg` und weitere `memory`-Knoten werden nie gesehen. Das ist
belegter Code, keine Vermutung.

Belegt ist auch, dass die Firmware den Knoten überhaupt erst füllt: die
`bcm2711-rpi-4-b.dtb` auf der Karte trägt `reg = <0x0 0x0 0x0>` (§6.4), wir
booten aber mit einer sinnvollen DRAM-Größe — also patcht `start4.elf` sie.

**Vermutung, HW-Lauf nötig (§10):** die Pi-4-Firmware meldet für Maschinen
> 1 GB mehrere Bänke, deren erste unterhalb 1 GiB endet; mit `gpu_mem=256`
(`boot/rpi4/config.txt:25`) also ≤ 768 MiB. Träfe das zu, verwalten wir auf
einer 4-GB-Maschine höchstens ~0,75 GiB und der Rest existiert für den Kernel
nicht. Die Bootzeile `[mm]   TT-RAM: …` sagt es eindeutig; bis dahin ist der
*Mechanismus* gesichert und die *Zahl* offen.

Das ist ein Hardware-Layer-Befund, keine
EmuTOS-Abweichung — EmuTOS' Äquivalent wäre, mehrere Regionen per `xmaddalt`
nachzumelden, genau wie es die Amiga-Portierung tut
(`references/emutos/bios/amiga.c:533-549`).

Der exakte Wert lässt sich ohne HW-Lauf nicht bestimmen (§10).

### 7.2 Zwei harte Obergrenzen darüber

* **Frame-Bitmap: 1 GiB pro Pool, mit `assert!`.**
  `frames.rs:43-44,131-136` — ein Pool mit mehr als 262 144 Frames bricht den
  Boot mit `panic("frames: pool too large for static bitmap")` ab. Auf dem Pi
  kann das heute nicht auslösen (Bank 0 < 1 GiB), auf `qemu-virt` mit `-m 2G`
  sofort. **Muss vor §7.1 fallen**, sonst tauscht man „RAM ungenutzt" gegen
  „bootet nicht".
* **`Long`-ABI: < 2 GiB pro Pool.**
  `tos-api/src/bdos/mem/init.rs:226-238` weist alles zurück, was nicht in einen positiven
  `i32` passt. Das ist **EmuTOS-konform**, nicht unsere Erfindung: EmuTOS
  begrenzt die TT-RAM-Erkennung ausdrücklich „to positive addresses to avoid
  shocking BDOS" (`bios/memory2.c:56-59`). Mehr als ~2 GiB kann ein
  TOS-`MD` nicht beschreiben — das ist die Decke des API-Vertrags, nicht ein Bug.

---

## 8. Was in dieser Runde geändert wurde

### 8.1 Speicher-Sysvars und OSHEADER (`bdos/process/loader.rs::populate_sysvars`)

Die Spec verlangt diese Felder ausdrücklich
(`docs/api/bios/_boot-init.md` §7c, Zeilen 403/418/419;
`docs/api/structures/osheader.md` §os_beg/§os_end), implementiert waren sie
nicht bzw. falsch:

| Feld | vorher | jetzt | EmuTOS-Beleg |
|---|---|---|---|
| OSHEADER `os_beg` (+0x08) | `stram_start` | `OSHEADER_VA` (= `sysbase`) | `bios/startup.S:88-89` (`_os_header`), `include/biosdefs.h:51` |
| OSHEADER `os_end` (+0x0C) | `stram_end` | `stram_start` | `bios/startup.S:90-91` (`__endvdibss`), `include/biosdefs.h:52` |
| `end_os` (`$4FA`) | **fehlte** | `stram_start` | `bios/biosmem.c:66` (`end_os = _end_os_stram`) |
| `ramtop` (`$5A4`) | **fehlte** | TT-RAM-Pool-Ende (0 ohne Alt-RAM) | `bios/memory2.c:115-122` |
| `ramvalid` (`$5A8`) | **fehlte** | `0x1357BD13` | `bios/memory2.c:124` (und `:129`), `include/tosvars.h:78` |

`os_end` war der schädlichste der drei: die klassische Rechnung
„freier ST-RAM = `memtop − os_end`" ergab damit **0**, weil beide Felder
denselben Wert (`stram_end`) trugen. Jetzt ergibt sie 6 209 536 — die volle
Poolgröße, also bis auf die eine Alignment-Seite aus §8.2 dasselbe, was
`Mxalloc(-1, MX_STRAM)` meldet.

`ramtop` brauchte einen neuen Accessor, gebaut als Spiegel des vorhandenen
`stram_range` (`tos-api/src/bdos/mem/init.rs`, neu: `ttram_range()`). Er wird
wie EmuTOS' `ramtop` **einmalig beim Boot** gesetzt und von einem späteren
`Maddalt` nicht verändert (`bios/memory2.c:92-127` tut dasselbe).

> **Risikohinweis, bewusst so gelassen:** `ramtop`@`$5A4` liegt direkt über
> `p_cookies`@`$5A0`. Eine LP64-App, die `*(void**)0x5A0` als **acht** Byte
> liest, sieht ab jetzt `(ramtop << 32) | jar`. Das ist die klassische,
> 32-bittige TOS-Belegung; die Lesebreite ist Sache der App/Toolchain (mintlib
> deklariert `ramtop` selbst als `unsigned long *` @`$5a4`,
> `Applications/mintlib-src/.../mint/sysvars.h:60`). TeraDesks **benutzter**
> Cookie-Pfad geht über `Setexc(0x5A0/4, -1)`
> (`sdk/apps/teradesk/library/utility/cookie.c:56`) und ist nicht betroffen;
> `install_cookie`, das den 8-Byte-Zeiger läse, wird von TeraDesk nirgends
> aufgerufen (geprüft).

### 8.2 `Mxalloc(-1)` meldete am EL0 eine nicht allozierbare Größe

`Mxalloc(-1)` existiert, damit ein Programm den größten Block erfragen und ihn
**dann nehmen** kann. Bei EmuTOS hält das: `ffit(-1,…)` gibt die Länge eines
echten freien `MD` zurück, und das `ffit(len,…)` eine Zeile später gelingt
(`bdos/umem.c:243-278`, `:305-327`). EmuTOS verlässt sich selbst darauf —
`alloc_tpa` misst mit `ffit(-1L,&pmd)` und alloziert direkt danach
`ffit(st_ram_size,&pmd)` **ohne Fehlerprüfung**, dereferenziert das Ergebnis
sofort (`bdos/proc.c:466,495-499`).

Am EL0 galt das nicht. Der Allokationspfad fordert **eine Seite mehr** vom Pool
an, damit die seitenausgerichtete Fensterbasis noch im 16-Byte-ausgerichteten
Poolblock liegt (`gemdos/memory.rs:101-104`), die Sonde meldete aber den rohen
Poolwert. Konkret, mit den echten Zahlen:

```text
Mxalloc(-1, MX_STRAM) → 6 209 536
Mxalloc(6 209 536, MX_STRAM) → verlangt 6 209 536 + 4096 vom Pool → ffit scheitert → 0
```

Neu (`gemdos/memory.rs::el0_largest_allocatable`):

```text
min( floor_page(pool_free − PAGE), window_remaining )
```

Damit ist die gemeldete Größe wieder eine, die der nächste Aufruf bekommt. Kosten:
die ST-Zahl sinkt um genau eine Seite (6 209 536 → 6 205 440); die
TT-Zahl bleibt unverändert, weil dort das Fenster bindet. Sechs Host-Tests in
`tos-api/src/gemdos/memory.rs` `mod tests` sichern die Eigenschaft, inklusive
eines Tests, der den alten `min(pool_free, window)` als nicht-allozierbar
nachweist.

Specs mitgezogen: `docs/api/gemdos/memory/44-mxalloc.md`,
`docs/api/gemdos/memory/48-malloc.md`.

### 8.3 Prüfung

* **§8.1** — der B1-Selftest (`kernel/src/selftest_sysvars.rs`) hat eine
  Gruppe **(e)** bekommen: neun Checks, die jedes Feld gegen die *lebenden*
  Pool-Extents prüfen statt gegen einen eingebackenen Wert (der mit jedem
  Kernel-Build driften würde), inklusive der Rechnung
  `memtop − end_os == Poolgröße`, die vor BB2 `0` ergab. Lauf auf
  `qemu-virt`: **18/18 OK, `=== B1 PASS ===`** (der Selftest ist wie gehabt
  nicht im Boot verdrahtet — er wurde für den Lauf temporär in
  `kernel/src/lib.rs` aktiviert und danach wieder entfernt).
* **§8.2** — sechs Host-Tests in `tos-api/src/gemdos/memory.rs` `mod tests`,
  laufen unter `cargo test -p tos-api --lib`.

---

## 9. Der Weg zu „ST-RAM 10 MB, TT-RAM der nutzbare Rest"

Der Wunsch trifft zwei getrennte Probleme; nur eines davon hat mit der MMU zu tun.

### 9.1 ST-RAM auf 10 MB — geht ohne MMU-Umbau

Naiv „ST-RAM-Slot auf 18 MiB vergrößern" wäre schlecht: der Pool endete bei
`0x1214000`, also **oberhalb** `TTRAM_START = 0x01000000`, und die Atari-typische
16-MiB-Grenze zwischen den Pools ginge verloren.

Der EmuTOS-förmige Weg ist der umgekehrte: **den ST-RAM-Fußabdruck des OS
verkleinern**, so wie EmuTOS im ST-RAM nur seine BSS hat. Konkret sind
`.dma` (2 MiB), der Stack-Guard (2 MiB) und der Kernel-Stack (80 KiB) — zusammen
mit dem Alignment-Padding **3,66 + 2,08 MiB** — die einzigen großen Posten, und
keiner davon muss unterhalb 14 MiB liegen. Verlegt man `.dma` und `.stack`
hinter die 16-MiB-Grenze, endet das Image bei ~`0x256000` und der ST-RAM-Pool
wird `0xE00000 − 0x256000` = **11,5 MiB** — mehr als die gewünschten 10 MB, bei
unverändertem `TTRAM_START`.

Was daran zu prüfen ist, bevor jemand es umsetzt:

* **`.dma` darf nicht blind wandern.** Der Block existiert, um für den
  nicht-kohärenten PCIe-Pfad des Pi 4 als Normal-NC umgemappt zu werden
  (`link-rpi4.ld:70-77`). Ob die VL805-/xHCI-DMA-Adressierung eine
  Lage-Einschränkung hat, ist eine BCM2711-Datenblatt-Frage — **nicht raten**.
* **Der Boot-Stub löscht die BSS vor der MMU** über `__bss_phys_start/end`
  (`link-rpi4.ld:99-100`); eine verschobene Sektion muss dort mitgezogen werden.
* **Verschobene Sektionen müssen reserviert werden.** `__kernel_image_end` ist
  heute die einzige Reservierung — die Pools beginnen einfach dahinter
  (`frames.rs:26-29`). Liegen `.dma`/`.stack` künftig *innerhalb* des
  TT-RAM-Bereichs, braucht der Frame-Allokator einen echten
  Reservierungsschritt, und `__kernel_image_end` darf nicht mehr das Ende der
  verschobenen Sektionen sein.
* Danach ist die Zahl auch **stabil**, weil sie nicht mehr an den zwei
  `ALIGN(2M)` hängt.

### 9.2 TT-RAM „der nutzbare Rest" — das ist die MMU-Herausforderung

Der Pool ist bereits „der Rest" (§2.2). Was fehlt, ist, dass die **App** ihn
erreichen kann, und genau da sitzt die Schwierigkeit, an die sich der User
erinnert.

Das Fenster größer machen heißt heute `USER_L3_COUNT` erhöhen, und das kostet
pro 2 MiB Fenster `MAX_PROCESSES × 4 KiB` = **64 KiB BSS**
(`tos-api/src/bdos/process/mmu.rs:64-77`, `MAX_PROCESSES = 16` in
`tos-api/src/bdos/process.rs:63`; heute 851 968 Byte für 20 MiB). Hochgerechnet:

| Fenster | L3-Tabellen | BSS | Wirkung auf den ST-RAM-Pool |
|---:|---:|---:|---|
| 16 MiB (heute) | 10 | 832 KiB | — |
| 64 MiB | 34 | 2,7 MiB | überschreitet die 2-MiB-Stufe → ST-RAM −2 MiB |
| 256 MiB | 130 | 10,4 MiB | ST-RAM-Pool wäre **weg** |

Das ist die Rückkopplung: mehr Fenster frisst genau den ST-RAM, den §9.1
gewinnen soll. Der Ausweg ist nicht „mehr L3-Tabellen", sondern **2-MiB-Blöcke
direkt in der L2** für große Fensterabbildungen — eine L2-Block-Deskriptor-Variante
kostet pro 2 MiB einen Eintrag und **null** zusätzliche Tabellen, verlangt aber
2-MiB-ausgerichtete Poolblöcke und eine Erweiterung von `usermem::map`. Das ist
eine echte Design-Entscheidung (ADR-Ebene), nicht ein Konstantendreh — deshalb
hier nur benannt, nicht gemacht.

Zweitens muss dann §6.2 mit: ein Fenster, dessen Cursor nie zurückläuft, ist
egal bei 16 MiB und eine Leckstelle bei 1 GiB. Die Freigabe von Fenster-VAs
verlangt einen TLBI genau auf dem Pfad, der QEMU festfährt (QUIRK-001,
`usermem.rs:302-306`) — auch das gehört in dieselbe Entscheidung.

---

## 10. Was ich nicht bestimmen konnte

**Wie viel DRAM der echte Pi 4 meldet.** Alle vorliegenden `[mm]`-Logs stammen
aus QEMU (`raspi4b`: `TT-RAM … 0x40000000`, also 1 GiB). Der Wert der echten
Maschine steht ausschließlich in ihrer eigenen Bootausgabe. §7.1 ist deshalb als
Mechanismus belegt, in der Zahl aber offen.

**Ein Lauf auf dem Pi klärt es vollständig** (Serial 115200, USB-TTL wie in
`docs/HW-TEST-LOG.md`); zu protokollieren sind die ersten Sekunden, konkret
diese fünf Zeilen:

```text
[mm] DTB at 0x…            ← oder „no DTB — using compiled-in DRAM size"
[mm]   kernel: …           ← bestätigt __kernel_image_end dieses Builds
[mm]   ST-RAM: …
[mm]   TT-RAM: 0x01000000..0x????????   ← die gesuchte Zahl: dram.end
[mm] BDOS pools: …
```

Ist `TT-RAM`-Ende `0x30000000` (768 MiB), bestätigt das die
`gpu_mem=256`-Rechnung; ist es `0x40000000`, greift der Compile-Zeit-Default und
der DTB wurde verworfen. Beides ändert nichts an dem, was TeraDesk anzeigt (das
Fenster bindet), wohl aber an §7.1 und an der Reihenfolge von §7.2.

---

## 11. Befundliste

| # | Befund | Ebene | Status |
|---|---|---|---|
| F1 | `os_beg`/`os_end` trugen Pool-Grenzen statt OS-Grenzen; „`memtop − os_end`" ergab 0 | EmuTOS-Abweichung | **gefixt** (§8.1) |
| F2 | `end_os`/`ramtop`/`ramvalid` wurden nie geschrieben — `ramtop = 0` heißt „kein Alt-RAM", obwohl `Mxalloc(-1,1)` welchen meldet | EmuTOS-Abweichung | **gefixt** (§8.1) |
| F3 | EL0-`Mxalloc(-1)` meldete eine um eine Seite zu große, nicht allozierbare Größe | Vertragsbruch | **gefixt** (§8.2) |
| F4 | TT-RAM-Meldung = Fenster-Rest, nicht Poolgröße; App-Decke 16 MiB | HW-/MMU-Ebene, ADR-021 | offen, §9.2 |
| F5 | Fenster-Cursor läuft nie zurück; Zahl sinkt monoton, `Mfree` gibt VA nicht frei | HW-/MMU-Ebene, ADR-021 (dort als Deferral notiert) | offen, §9.2 |
| F6 | Nur die erste DTB-Speicherbank wird gelesen → ≥3 GiB einer 4-GB-Maschine ungenutzt | HW-Ebene | offen, §7.1 |
| F7 | `frames`-Bitmap-`assert!` bricht den Boot bei Pool > 1 GiB ab | Robustheit | offen, §7.2 — **Vorbedingung für F6** |
| F8 | 8,08 MiB des 14-MiB-ST-RAM-Fensters gehen an das OS, davon 3,66 MiB Padding/Guard; Wert springt in 2-MiB-Stufen | Layout | offen, §9.1 |
| F9 | Boot-Splash zeigt Pool-Totale; EmuTOS' `initinfo` zeigt `phystop` bzw. `total_alt_ram()` (`bios/initinfo.c:277,279`) | kosmetische EmuTOS-Abweichung | offen, nicht angefasst (Screendump-Referenzen) |
| F10 | Basepage-TPA ist immer 4 MiB (`bdos/process/arena.rs:52`), unabhängig von der Maschine; EmuTOS' `alloc_tpa` gibt dem Kind den **ganzen** größten freien Block (`bdos/proc.c:460-501`) und lässt es selbst `Mshrink`en | EmuTOS-Abweichung | offen, gehört zu BB2/`Pexec` |

