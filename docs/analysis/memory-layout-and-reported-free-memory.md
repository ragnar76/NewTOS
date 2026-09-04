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
| F6 | Nur die erste DTB-Speicherbank wird gelesen → ≥3 GiB einer 4-GB-Maschine ungenutzt | HW-Ebene | **teilweise** (§12): die Grenze kommt jetzt von der Firmware statt vom DTB und schließt GPU-Speicher aus; mehrere nicht zusammenhängende Bänke weiterhin offen |
| F7 | `frames`-Bitmap-`assert!` bricht den Boot bei Pool > 1 GiB ab | Robustheit | **gefixt** (§12.5): Pool wird gekappt und der Rest protokolliert; der zweite Deckel (`Long`-ABI) gleich mit |
| F8 | 8,08 MiB des 14-MiB-ST-RAM-Fensters gehen an das OS, davon 3,66 MiB Padding/Guard; Wert springt in 2-MiB-Stufen | Layout | offen, §9.1 |
| F9 | Boot-Splash zeigt Pool-Totale; EmuTOS' `initinfo` zeigt `phystop` bzw. `total_alt_ram()` (`bios/initinfo.c:277,279`) | kosmetische EmuTOS-Abweichung | offen, nicht angefasst (Screendump-Referenzen) |
| F10 | Basepage-TPA ist immer 4 MiB (`bdos/process/arena.rs:52`), unabhängig von der Maschine; EmuTOS' `alloc_tpa` gibt dem Kind den **ganzen** größten freien Block (`bdos/proc.c:460-501`) und lässt es selbst `Mshrink`en | EmuTOS-Abweichung | offen, gehört zu BB2/`Pexec` |


---

## 12. Nachtrag 2026-09-02: die Firmware fragen statt dem DTB glauben

> Auftrag: die offene Frage aus §10 („wie viel DRAM meldet der echte Pi 4?")
> nicht raten, sondern die Raspberry-Pi-Firmware selbst fragen. Umgesetzt in
> `kernel/src/drivers/bcm2711/mailbox.rs::get_memory` (:268),
> `kernel/src/lib.rs::firmware_dram` (:373) und
> `kernel/src/bios/machine/layout.rs::discover` (:373).

### 12.1 Die Primärquelle und was sie sagt

Firmware-Dokumentation *Mailbox property interface* (raspberrypi/firmware
Wiki, Quelle #4 in `docs/hardware/mailbox-framebuffer.md`; dort neu als
**§3a** aufgenommen). Wörtlich:

```text
#### Get ARM memory          #### Get VC memory
 * Tag: 0x00010005            * Tag: 0x00010006
 * Request: Length: 0         * Request: Length: 0
 * Response:                  * Response:
   * Length: 8                  * Length: 8
   * Value:                     * Value:
     * u32: base address          * u32: base address
     * u32: size in bytes         * u32: size in bytes

Future formats may specify multiple base+size combinations.
```

Drei Aussagen, die daraus **folgen** (nicht angenommen sind):

1. Der Tag meldet die **Aufteilung**, nicht den bestückten Gesamtspeicher:
   „ARM memory" ist der Anteil, den die Firmware der CPU gibt, „VC memory"
   der GPU-Anteil (`gpu_mem`). Genau der ARM-Anteil ist die Zahl, die die
   Pool-Obergrenze bestimmen darf — der VC-Anteil enthält u. a. den
   Framebuffer.
2. Die Antwort kann **länger als 8 Byte** sein; das Längenfeld im
   Tag-Code-Wort (`b31 | Länge`) sagt, wie viele Paare kamen. Unser Puffer
   bietet acht Paare an und liest so viele, wie die Firmware füllt.
3. Ein einzelnes Paar kann wegen der `u32`-Felder **maximal 4 GiB**
   beschreiben.

Ebenfalls aus derselben Quelle: *„Unknown tags are ignored (the response
bit will not be set)"* — deshalb prüft `get_memory` das `b31` **des Tags**,
nicht nur den Gesamt-Antwortcode. Eine Mailbox ohne diesen Tag liefert
damit sauber `None` statt einer erfundenen Zahl.

### 12.2 Was jetzt die Pool-Grenze bestimmt

`discover` nimmt die erste Auskunft, die plausibel ist (`plausible`,
layout.rs:348 — Start muss `board::DRAM_START` sein, Größe ≥ 16 MiB):

| Reihenfolge | Quelle | Log-Kennzeichnung |
|---|---|---|
| 1 | Firmware-Mailbox (`0x0001_0005`) | `source: firmware` |
| 2 | DTB `/memory@…`, erste Bank (wie bisher) | `source: device tree` |
| 3 | Compile-Zeit-Konstante `board::DRAM_SIZE` | `source: compiled-in board default` |

Meldet die Firmware mehrere Bänke, wird eine **direkt anschließende** Bank
angehängt; eine nicht-zusammenhängende Bank wird protokolliert und bleibt
ungenutzt (sie bräuchte eine zweite Region im Frame-Allokator, EmuTOS'
Äquivalent wäre `xmaddalt` — F6, weiterhin offen).

`board-qemu-virt` hat keine VideoCore-Mailbox; dort schreibt der Kernel
`firmware: no VideoCore mailbox on this board — device tree decides` und
der DTB-Pfad gilt unverändert (`lib.rs:417`).

### 12.3 Messung: QEMU `raspi4b`

```text
[mm] firmware: ARM memory bank 0: 0x0000000000000000 + 1006632960 bytes (960 MiB)
[mm] firmware: VC memory: 0x000000003c000000 + 67108864 bytes (64 MiB)
[mm] device tree: no usable /memory node (blob invalid or absent)
[mm]   DRAM  : 0x0000000000000000..0x000000003c000000 (1006632960 bytes) — source: firmware
[mm]   TT-RAM: 0x0000000001000000..0x000000003c000000 (989855744 bytes)
```

ARM + VC = **exakt 1 GiB**, und die Antwort ändert sich **nicht**, wenn die
Maschine mit `-m 2G` läuft. Die emulierte Firmware beschreibt also nur das
alte 1-GiB-Fenster. Ob die echte Pi-4-Firmware auf einem 4-GB-Board
genauso antwortet oder mehrere Paare liefert, ist damit **nicht**
entschieden — aber jede Antwort steht jetzt wörtlich im Bootlog (§12.6).

### 12.4 Der eigentliche Gewinn: wir hatten GPU-Speicher im TT-RAM-Pool

Vorher (HEAD `be955c0`, gleicher QEMU-Lauf) griff auf `raspi4b` der
1-GiB-Compile-Default, weil der DTB unbrauchbar ist:

```text
[mm]   TT-RAM: 0x0000000001000000..0x0000000040000000   (vorher)
[mm]   TT-RAM: 0x0000000001000000..0x000000003c000000   (jetzt)
```

Der Bereich `0x3c000000..0x40000000`, den der alte Pool mitverwaltete, ist
**genau der VC-Anteil**, den dieselbe Firmware meldet — und dort liegt der
Framebuffer: eine temporäre Sonde in `framebuffer.rs` (Ausgabe
`[fbprobe] bus=0x3c100000 phys=0x3c100000 size=1228800`, Sonde danach
zurückgebaut) zeigt ihn bei `0x3C10_0000..0x3C23_2800`, also mitten im
alten Pool. `Malloc` konnte diese Seiten vergeben. Das ist mit der
Firmware-Auskunft strukturell erledigt, nicht nur zufällig.

Kehrseite auf dem echten Pi: mit `gpu_mem=256` (`boot/rpi4/config.txt:25`)
ist der ARM-Anteil um 256 MiB kleiner als der bisherige 1-GiB-Default. Das
ist die korrekte Zahl, kein Verlust — wer den Speicher zurück will, senkt
`gpu_mem` (Raspberry-Pi-Doku: auf dem Pi 4 ist `76` das empfohlene
Maximum, weil die 3D-Einheit eine eigene MMU hat und dynamisch aus dem
OS-Speicher alloziert).

### 12.5 F7 erledigt — und der zweite Deckel gleich mit

§7.2 nennt zwei harte Obergrenzen über einer wahrheitsgemäßen Auskunft.
Beide brechen jetzt den Boot **nicht** mehr ab, sondern kappen den Pool und
protokollieren den Rest (`layout.rs:396-410`):

* **Frame-Bitmap**, `POOL_MAX_BYTES` = 1 GiB pro Pool (layout.rs:78; die
  bisherige Konstante in `frames.rs:51` leitet sich jetzt daraus ab, eine
  Quelle statt zweier). Der `assert!` in `frames::init` bleibt als
  Invariantenprüfung stehen, ist aber nicht mehr der Mechanismus.
* **`Long`-ABI**, `TOS_MD_MAX_END` = `0x7FFF_F000` (layout.rs:93). Ein
  TOS-`MD` speichert `m_start`/`m_length` als vorzeichenbehaftete 32 Bit
  (`tos-api/src/bdos/mem/init.rs::seed_md`), also muss der Pool unter
  `0x8000_0000` enden. **Das ist EmuTOS-Verhalten, kein Notnagel:** EmuTOS
  bricht seine Alt-RAM-Erkennung aus demselben Grund ab — *„We artificially
  limit TT-RAM detection to positive addresses to avoid shocking BDOS"*
  (`references/emutos/bios/memory2.c:56-59`) — und bootet weiter.

Nachweis auf `qemu-virt` mit `-m 2G` (dort ist der DTB die Quelle und
meldet die vollen 2 GiB):

```text
vorher:  [PANIC] frames: pool too large for static bitmap
           at kernel/src/bios/machine/frames.rs:133:9
jetzt:   [mm]   DRAM  : 0x40000000..0x7ffff000 (1073737728 bytes) — source: device tree
         [mm]   1073745920 bytes above 0x000000007ffff000 NOT managed
                (limits: 1073741824 bytes per pool bitmap,
                 TOS descriptor end < 0x7ffff000)
         …
         [desk] desktop up
```

(Mit nur der Bitmap-Kappung endete der Pool bei `0x8100_0000` und der Boot
starb eine Stufe später in `bdos::mem::init` mit `AddressOutOfRange` — der
zweite Deckel ist also nicht theoretisch.)

Der Standardlauf ist unverändert: `qemu-virt -m 272M` liefert weiterhin
`BDOS pools: ST-RAM 0x40814000+6209536 TT-RAM 0x419e9000+258043904`, exakt
die Zahlen aus §2.2.

### 12.6 Was der Bootlog auf dem echten Pi zeigen wird

Die fünf Zeilen aus §10 sind durch diese hier ersetzt bzw. ergänzt; sie
stehen unmittelbar nach `Phase 02 reached.`:

```text
[mm] DTB at 0x…                       ← oder „no DTB — using compiled-in DRAM size"
[mm] firmware: ARM memory bank 0: 0x… + N bytes (M MiB)   ← die gesuchte Zahl
[mm] firmware: ARM memory bank 1: …                        ← nur falls mehrere
[mm] firmware: VC memory: 0x… + N bytes (M MiB)
[mm] device tree: /memory first bank: 0x… + N bytes (M MiB)  ← zweite Meinung
[mm]   DRAM  : 0x…..0x… (N bytes) — source: firmware|device tree|compiled-in board default
[mm]   … NOT managed (…)              ← nur falls die Antwort größer ist als ein Pool
[mm]   TT-RAM: 0x01000000..0x…
```

Damit beantwortet **ein** Lauf drei Fragen auf einmal:

1. Wie groß ist der ARM-Anteil wirklich (erwartet mit `gpu_mem=256`:
   `0x30000000` = 768 MiB — **Erwartung, nicht Messung**).
2. Meldet die Firmware auf einem 4-GB-Board mehrere Bänke? Dann steht
   `bank 1` im Log, und F6 bekommt eine belastbare Grundlage.
3. Patcht `start4.elf` den DTB tatsächlich (§6.4)? Die `device tree:`-Zeile
   sagt es direkt.

### 12.7 Was weiterhin offen ist

* **F6 bleibt offen.** Wenn die Firmware nur den ARM-Anteil unterhalb 1 GiB
  meldet, sind ≥ 3 GB einer 4-GB-Maschine weiter unsichtbar. Der Weg dahin
  ist nicht „noch eine Konstante", sondern eine **zweite, nicht
  zusammenhängende Region** im Frame-Allokator plus ein zweites Seed-`MD`
  (EmuTOS: `xmaddalt`) — und darüber liegt ohnehin `TOS_MD_MAX_END`, d. h.
  mehr als knapp 2 GiB kann ein TOS-`MD` nie beschreiben.
* **Die App sieht davon nichts.** Die 16-MiB-EL0-Fenstergrenze (F4/F5,
  §9.2) ist von dieser Änderung unberührt und bleibt die Zahl, die
  `Mxalloc(-1, MX_TTRAM)` an eine Anwendung liefert.

---

## 13. Nachtrag 2026-09-04: warum der Boot-Schirm erst 5 MiB und dann 3 MiB meldete

> Auslöser: zwei Läufe auf der echten Pi 4 am selben Tag melden **5 MiB**
> bzw. **3 MiB** ST-RAM bei unverändert 752 MiB TT-RAM. Der User vermutete,
> „irgendwas ist in ST-RAM und TT-RAM eingeblendet". Die Vermutung ist für
> `board-rpi4` widerlegt und für `board-qemu-virt` bestätigt (§13.4).

### 13.1 Was die Zahl auf dem Boot-Schirm wirklich ist

`kernel/src/initinfo.rs:225-227` zeigt
`frames::stats(pool).total_bytes()` — also `PoolStats.total × 4096`
(`kernel/src/bios/machine/frames.rs:97-99`). Das ist

* die **Gesamtgröße des Pool-Extents**, nicht der freie Anteil,
* **einmalig** in `frames::init` aus `layout::discover` gesetzt und danach
  unveränderlich (`frames.rs:268-271`) — kein Laufzeit-Schnappschuss,
* **inklusive** allem, was der Kernel selbst schon daraus genommen hat: der
  2-MiB-Heap (`heap.rs:323`) und auf `qemu-virt` der 7,91-MiB-Framebuffer
  (`ramfb.rs:153`) liegen **innerhalb** dieser Zahl, sind aber für kein
  Programm allozierbar.

Damit ist auch beantwortet, ob die **Zahl der USB-Geräte** etwas damit zu tun
hat: nein. `total` ändert sich nie, nur `used` — und die xHCI-Puffer sind
ohnehin `#[link_section = ".dma"]`-Statics im Kernel-Abbild
(`kernel/src/drivers/usb/xhci/mod.rs:83,144,158,160`), also weder Pool noch
laufzeitabhängig. 1 oder 4 Geräte kosten exakt dasselbe.

**EmuTOS zeigt an dieser Stelle etwas anderes:** `stramsize = (long)phystop`
und `altramsize = total_alt_ram()`
(`references/emutos/bios/initinfo.c:277,279`, gedruckt `:337-342`) — also die
**ST-RAM-Größe der Maschine** (eine Konstante) und die Summe aller Alt-RAM-`MD`s
(`bdos/umem.c:520-538`). Unsere Zahl ist der Pool, EmuTOS' Zahl ist `phystop`.
Diese Abweichung bleibt offen (F9, §13.6) — sie ist eine sichtbare
Produktentscheidung, keine Mechanik.

### 13.2 Die Rundung log — und ist gefixt

`size_str` rundete mit `bytes >> 20` **ab**. `6 209 536` wurde „5 MiB",
`4 112 384` wurde „3 MiB"; in Wahrheit sind das 5,92 und 3,92 MiB. Der
Sprung sah dadurch nach 2 MiB **und** einer krummen Grenze aus.

EmuTOS' `cprintf_bytesize` (`bios/initinfo.c:243-260`) rundet nicht ab: **MB
nur bei exaktem MiB-Vielfachen, sonst KB.** `size_str` ist darauf angeglichen
(2026-09-04). Der Boot-Schirm meldet jetzt `6064 KB` bzw. `944 MB` statt
`5 MiB` / `944 MiB` — die beiden Läufe des Users hätten damit `6064 KB` und
`4016 KB` gezeigt, also die Änderung sofort sichtbar gemacht.

### 13.3 Die Ursache des Sprungs: `bench::DST` schob die BSS über `ALIGN(2M)`

Gemessen, nicht vermutet. Für jeden Kandidatenstand wurde
`cargo build -p kernel --no-default-features --features board-rpi4 --release`
in einem separaten Worktree gebaut und `rust-nm` / `rust-objcopy` ausgewertet:

| Commit | Zeit | `kernel8.img` | `__dma_start` | `__kernel_image_end` | ST-RAM-Pool | Schirm (alte Rundung) |
|---|---|---:|---|---|---:|---|
| `d39a091` | 09-02 08:57 | 733 300 | `0x400000` | `0x814000` | 6 209 536 | 5 MiB |
| `afe0bf4` | 09-02 17:33 | **733 464** | `0x400000` | `0x814000` | 6 209 536 | **5 MiB** |
| `0883613` | 09-02 20:08 | **733 464** | `0x400000` | `0x814000` | 6 209 536 | **5 MiB** |
| `be955c0` | 09-02 20:53 | **733 464** | `0x400000` | `0x814000` | 6 209 536 | **5 MiB** |
| `c063980` | 09-02 21:37 | 741 704 | `0x400000` | `0x814000` | 6 209 536 | 5 MiB |
| **`49c3938`** | **09-03 23:54** | 766 504 | **`0x600000`** | **`0xa14000`** | **4 112 384** | **3 MiB** |
| `3c85c52`…`4d5e031` | 09-04 00:46…11:47 | 766 504 | `0x600000` | `0xa14000` | 4 112 384 | 3 MiB |
| `04aaf16`…`4deb146` | 09-04 16:52…18:27 | **770 600** | `0x600000` | `0xa14000` | 4 112 384 | **3 MiB** |

Die beiden vom User genannten Abbildgrößen sind **auf das Byte** eindeutig:
733 464 → ein Stand aus `afe0bf4`…`be955c0`, 770 600 → `04aaf16`…`4deb146`
(die Datei `kernel8.img` im Arbeitsbaum war exakt diese 770 600 Byte).
Der ST-RAM-Pool ist in beiden Fällen `0xE00000 − __kernel_image_end`.

**Die Stufe liegt bei `49c3938`** („feat(aes): run the AES shell loop,
launching the desktop via #Z"). Der BSS-Symbolvergleich `c063980` → `49c3938`
(Summe aller BSS-Symbolgrößen 1 475 693 → 3 573 275) zeigt genau **ein** neues
Symbol von Belang:

```text
+ 0x200000 (2 MiB)  kernel::bench::DST
```

Der Rest der Differenz sind 430 Byte AES-/Shell-Kleinkram. Der Grund, warum
`DST` neu ist, obwohl `kernel/src/bench.rs` seit `a3c4329` (2026-06-19)
existiert: in `c063980` enthält das ELF **kein einziges** `kernel::shell`-
oder `kernel::bench`-Symbol — der ganze Konsolen-Zweig war unerreichbar und
wurde weggeräumt. `49c3938` legte `kernel/src/shell_loop.rs` an, das
`shell::run_console` ruft; damit wurde `bench::run` erreichbar und sein
2-MiB-Puffer nicht mehr entfernbar.

Weil `link-rpi4.ld:70,73,81` zweimal `ALIGN(2M)` erzwingt, kostet das nicht
2 MiB BSS, sondern verschiebt `.dma` und `.stack` um eine volle 2-MiB-Stufe:
`__kernel_image_end` `0x814000` → `0xa14000`, ST-RAM-Pool −2 097 152 Byte.
Das ist §6.1 in freier Wildbahn.

> **Nebenbefund im selben Code, mit gemessen:** weil `SRC` nie geschrieben
> wurde, konnte der Optimierer alle Lesezugriffe zu `0` falten, `SRC` ganz
> entfernen und die 64 Kopierdurchläufe zu einem `memset` zusammenziehen. Das
> Disassemblat von `bench::run` (Release, `4deb146`) hatte für den Lesetest
> zwei **direkt aufeinanderfolgende** `mrs CNTPCT_EL0` — die gemeldete
> Lese- und Kopierbandbreite war erfunden. Mit den Laufzeitpuffern laufen
> alle drei Schleifen wieder.

### 13.4 Die Einblendungs-Hypothese, Fund für Fund

| # | Fund | Board | Im Pool? | Beleg |
|---|---|---|---|---|
| E1 | **ramfb-Framebuffer 1920×1080×4 = 8 294 400 B aus `Pool::Ttram`** | `qemu-virt` | **ja** — `0x41200000..0x419E9000` | `kernel/src/drivers/qemu_virt/ramfb.rs:153` |
| E2 | Kernel-Heap 2 MiB aus `Pool::Ttram` | beide | **ja** — `0x41000000..0x41200000` (virt) / `0x01000000..0x01200000` (rpi4) | `kernel/src/bios/machine/heap.rs:323` |
| E3 | VideoCore-Framebuffer | `rpi4` | **nein** (§13.5) | `drivers/bcm2711/framebuffer.rs:276`, `layout.rs:388-392` |
| E4 | `.dma`, 2 MiB (xHCI, Device-nGnRnE) | beide | nein, aber **im ST-RAM-Fenster** `0x400000..0x600000` | `link-rpi4.ld:70-77`, `drivers/usb/mod.rs:84-99` |
| E5 | Stack-Guard 2 MiB + Kernel-Stack 64 K + Notfall-Stack 16 K | beide | nein, aber im ST-RAM-Fenster `0x600000..0x814000` | `link-rpi4.ld:81-93` |
| E6 | `bench::DST`, 2 MiB BSS | beide | nein, aber im ST-RAM-Fenster | **behoben 2026-09-04** |
| E7 | Alignment-Padding zwischen BSS-Ende und `.dma` | beide | nein, aber im ST-RAM-Fenster (nach E6: 1,63 MiB) | `link-rpi4.ld:70` |
| E8 | Pre-MMU-Boot-Stack 512 KiB unterhalb des Abbilds | beide | nein | `link-rpi4.ld:38-44` |

Ein **Laufzeit**-Verbraucher des ST-RAM-Pools existiert nicht: die einzigen
Aufrufer von `frames::alloc_frame`/`alloc_frames_contig` im ganzen Baum sind
`lib.rs:575` (Smoke-Test, gibt sofort zurück), `lib.rs:644`
(`bring_up_bdos_pools`), `ramfb.rs:153` und `heap.rs:323` — die letzten beiden
ausschließlich aus `Pool::Ttram`. Der Grep des Users war vollständig.

E1 ist die Bestätigung seiner Vermutung, aber nur auf `qemu-virt`: dort liegt
ein Framebuffer **im TT-RAM-Pool**. Er ist zwar dauerhaft als `used` markiert
und damit nicht `Malloc`-bar, zählt aber in der Boot-Schirm-Zahl mit und
verletzt die Vorgabe „im TT-RAM darf kein Framebuffer sein" wörtlich.
EmuTOS' eigenes Vorgehen wäre `balloc_stram(size, TRUE)` — Bildschirmspeicher
**oben** vom RAM abziehen und `memtop` senken, damit er nie Teil des Pools
wird (`bios/biosmem.c:88-131`). Umsetzung: §13.6, Entscheidung offen.

### 13.5 Wie viel DRAM die echte Pi 4 meldet — aus dem Boot-Schirm des Users abgeleitet

§10 und §12.6 hatten das als offen geführt (Konsolenzeilen, die der User ohne
serielle Leitung nicht sehen kann). Die auf **seinem Schirm** stehende
TT-RAM-Zahl reicht aber:

```text
TT-RAM-Extent = [DRAM_START + 16 MiB, dram.end)
angezeigt (alte Abrundung) 752 MiB  ⇒  len ∈ [788 529 152, 789 577 728)
                                    ⇒  dram.end ∈ [0x30000000, 0x30100000)
```

Von den drei möglichen Quellen passt genau eine:

| Quelle | `dram.end` | TT-RAM-Extent | angezeigt |
|---|---|---:|---|
| Firmware `0x0001_0005`, `gpu_mem=256` → ARM = 768 MiB | `0x30000000` | 788 529 152 | **752 MiB** ✓ |
| DTB, erste Bank einer 4-GB-Pi (typ. `0x3b400000`) | `0x3b400000` | 976 175 104 | 930 MiB ✗ |
| Compile-Default `board::DRAM_SIZE` = 1 GiB | `0x40000000` | 1 056 964 608 | 1008 MiB ✗ |

**Also:** die echte Pi 4 nimmt den Firmware-Zweig, die Firmware meldet
**768 MiB ARM-Speicher** (= 1024 − `gpu_mem=256`, `boot/rpi4/config.txt:25`),
und der VideoCore-Anteil `0x30000000..0x40000000` — in dem der Framebuffer
liegt — ist **außerhalb** beider Pools. Das ist genau die Wirkung von
`c063980`. Damit ist E3 belegt und F6 in der Zahl beantwortet: auf einer
4-GB-Maschine verwalten wir heute 768 MiB; die restlichen ~3,25 GB sind
unsichtbar (weiterhin offen, §12.7).

Der eine Rest an Unsicherheit: jede Quelle mit `dram.end` innerhalb von
1 MiB oberhalb `0x30000000` ergäbe dieselbe Anzeige. `0x30000000` ist der
einzige Wert in diesem Fenster, der eine Bedeutung hat.

### 13.6 Was 2026-09-04 geändert wurde — und was ausdrücklich nicht

**Geändert:**

1. `kernel/src/bench.rs` — die beiden 2-MiB-Puffer kommen nicht mehr aus der
   Kernel-BSS (und damit aus dem ST-RAM-Fenster), sondern zur Laufzeit aus dem
   **TT-RAM**-Pool (`Mxalloc(SIZE, MX_TTRAM)` / `Mfree` über
   `bdos::mem::traps`, Kernel-Alias `pa + VA_OFFSET` wie in
   `aes::resource::rsrc_load`). `__kernel_image_end` fällt damit zurück auf
   `0x814000`, der ST-RAM-Pool auf **6 209 536 Byte**. Auf beiden Boards
   verifiziert.
2. `kernel/src/initinfo.rs::size_str` — EmuTOS' `cprintf_bytesize`-Regel
   (§13.2).

**Nicht geändert, bewusst — das sind Lead-/ADR-Entscheidungen:**

* **F9 / die Bedeutung der Zahl** (Pool-Extent statt `phystop` +
  `total_alt_ram()`). Auf `phystop` umzustellen macht die ST-RAM-Zeile zu
  einer Konstanten (14 MB) — EmuTOS-treu und „fest", aber sie verschweigt
  dann, dass davon nur 5,92 MiB frei sind. Das ist eine sichtbare
  Produktentscheidung, keine Mechanik.
* **E1 (Framebuffer im TT-RAM-Pool auf `qemu-virt`)** und **E2 (Heap im
  TT-RAM-Pool)**. Der EmuTOS-förmige Weg ist, beides *vor* der Pool-Bildung
  oben aus `dram` herauszuschneiden (`balloc_stram(…, TRUE)`-Äquivalent) statt
  es aus dem Pool zu allozieren. Das ist ein Eingriff in `layout::discover` +
  die Reihenfolge der Boot-Bring-up-Schritte.
* **Der eigentliche Wunsch „ST-RAM fest auf 10 MB".** Er ist mit dem heutigen
  Layout nicht erreichbar, siehe §13.7.

### 13.7 Warum „ST-RAM = feste 10 MB" eine ADR-Entscheidung ist

Nach dem Fix aus §13.6 ist der ST-RAM-Pool wieder 5,92 MiB — aber immer noch
**ein Rest, kein Wert**. Er hängt an `__kernel_image_end`, und der springt in
2-MiB-Stufen. Aktueller Abstand zur nächsten Stufe: BSS-Inhalt endet bei
`0x25f598`, die Grenze liegt bei `0x400000` → **1,63 MiB Luft**.

Damit `Mxalloc(-1, MX_STRAM)` verlässlich 10 MB liefert, muss `membot` eine
**Konstante** werden — bei `phystop = 0xE00000` also `membot = 0x400000`. Was
heute unterhalb `0x400000` liegen müsste, tut es nicht:

```text
0x080000..0x13d000   756 KiB  .text/.rodata/.data
0x13d000..0x25f598  1,13 MiB  .bss (nach dem bench-Fix)
0x25f598..0x400000  1,63 MiB  ALIGN(2M)-Padding
0x400000..0x600000     2 MiB  .dma          ← muss raus
0x600000..0x800000     2 MiB  Stack-Guard   ← muss raus
0x800000..0x814000    80 KiB  Stacks        ← muss raus
```

Ohne `.dma` und `.stack` endet das Abbild bei ~`0x260000` und passt mit
1,6 MiB Reserve unter eine feste `membot = 0x400000` — ST-RAM wäre dann
**exakt 10 MiB, immer**. Die drei offenen Punkte sind aber echte
Entscheidungen, keine Konstantendreher:

1. **Wohin mit `.dma` (2 MiB)?** Naheliegend wäre das ohnehin ausgesparte
   Atari-Hardware-Register-Fenster `0xE00000..0x1000000` (`STRAM_IO_RESERVE`,
   `layout.rs:60-62`): exakt 2 MiB, 2-MiB-ausgerichtet, gehört keinem Pool.
   Ob die VL805-/BCM2711-PCIe-Inbound-Fenster eine Lagebeschränkung haben, ist
   eine **Datenblattfrage** — nicht raten (Projektregel 6).
2. **Wohin mit Guard + Stack (2,08 MiB)?** Über die 16-MiB-Grenze. Dann
   brauchen sowohl der Frame-Allokator einen echten **Reservierungsschritt**
   (heute gibt es keinen: die Pools beginnen einfach hinter
   `__kernel_image_end`, `frames.rs:23-29`) als auch `boot.s` ein angepasstes
   `__bss_phys_start/end`.
   *Oder*: der 2-MiB-Guard schrumpft auf eine Seite — dafür müsste das
   TTBR1-Fenster im unteren Bereich eine **L3-Tabelle** bekommen, denn der
   Kernel bildet DRAM heute ausschließlich in 2-MiB-L2-Blöcken ab
   (`mmu.rs:100,159-165`). Genau das ist die „MMU-Herausforderung", an die
   sich der User erinnert.
3. **Was ist dann `phystop`?** Bleibt `0xE00000` (14 MiB); die 10 MB sind
   `memtop − membot`. Das ist EmuTOS-Form: `phystop` = Maschine,
   `[membot, memtop)` = Pool (`bios/biosmem.c:66-72`).

Kosten/Nutzen: +4,08 MiB ST-RAM und — wichtiger — eine Zahl, die nicht mehr
mit jedem Kernel-Build wandert. Berührte Dateien: beide Linkerskripte,
`boot.s`, `frames.rs`, `layout.rs`, `mmu.rs`, `drivers/usb/mod.rs`.
**Vorgelegt, nicht umgesetzt.**

---

## 14. Nachtrag 2026-09-04 (zweite Runde): die Aufteilung, wie ein TOS-System sie braucht

> Entscheidung des Users nach §13: **erst** E1/E2 (nichts Fremdes im
> TT-RAM-Pool), **dann** der feste ST-RAM mit **10 MB**. Beides ist
> umgesetzt; F9 wird am Ende beantwortet.

### 14.1 Das Ergebnis, beide Boards

```text
board-rpi4 (physisch)                    board-qemu-virt = +0x4000_0000
──────────────────────────────────────────────────────────────────────
0x000000..0x080000    512 KiB  Pre-MMU-Boot-Stack
0x080000..0x260000   ~1,9 MiB  .text .rodata .data .bss
0x261000..0x262000      4 KiB  Stack-Guard  (L3-Seite, ungemappt)
0x262000..0x276000     80 KiB  Kernel-Stack 64K + Notfall-Stack 16K
0x276000..0x400000   1,54 MiB  Reserve — Überschreitung = LINKER-FEHLER
────────────────────────────────────────── membot = STRAM_OS_RESERVE = 4 MiB
0x400000..0xE00000     10 MiB  ST-RAM-Pool  ← KONSTANT
────────────────────────────────────────── memtop = phystop = 14 MiB
0xE00000..0x1000000     2 MiB  .dma  (Atari-$E00000-Registerfenster)
────────────────────────────────────────── TTRAM_START = 16 MiB
0x1000000..pool_top            TT-RAM-Pool
pool_top..dram.end             OS-Reserve: Kernel-Heap 2 MiB [+ ramfb]
```

Gemessen (QEMU, Release):

| | vorher | nachher |
|---|---:|---:|
| Kernel-Abbild (rpi4) | 7 946 240 B | **2 056 192 B** |
| ST-RAM-Pool | 6 209 536 B (schwankend) | **10 485 760 B (konstant)** |
| TT-RAM-Pool-Extent (virt) | 268 435 456 B | 258 043 904 B |
| BDOS-TT-RAM-Start (virt) | `0x419e9000` | **`0x41000000` = TTRAM_START** |
| Boot-Schirm ST-RAM | „5 MiB" / „3 MiB" | **„10 MB"** |

### 14.2 E1/E2 — Framebuffer und Heap oben abgeschnitten, nicht aus dem Pool

`layout::discover` nimmt jetzt **vor** der Pool-Bildung eine OS-Reserve vom
oberen Ende von `dram` (`MemoryMap::os_reserve` → `kernel_heap` +
`framebuffer`). Das ist EmuTOS' `balloc_stram(size, TRUE)`: es senkt
`memtop` unter `phystop`, und `getmpb` veröffentlicht nur `[membot, memtop)`
(`bios/biosmem.c:88-131,134-151`) — der Bildschirmspeicher liegt im RAM der
Maschine, aber nie im Pool und nie in einer `Malloc`-Antwort.

Die Reserve bleibt **innerhalb** `map.dram`, wird also weiter Normal-Cacheable
gemappt; sie ist nur kein Poolbestandteil mehr. `heap::init(map)` und
`ramfb::init(map)` nehmen ihre Bereiche aus der Karte statt
`alloc_frames_contig` zu rufen. Zwei `const`-Assertions koppeln
`heap::HEAP_BYTES` an `layout::KERNEL_HEAP_RESERVE` und `ramfb::FB_BYTES`
an `layout::FRAMEBUFFER_RESERVE`, damit die Seiten nicht auseinanderlaufen.

Auf `board-rpi4` ist `FRAMEBUFFER_RESERVE = 0` — der VideoCore besitzt den
Framebuffer und nimmt ihn aus **seinem** SDRAM-Anteil, der laut
BCM2711-Datenblatt §1.2.3 *„mapped in from `0x0_4000_0000` downwards"* liegt
und von `gpu_mem` bemessen wird. Damit ist E3 **primärquellenbelegt** und
nicht mehr nur aus der 752-MiB-Anzeige gerechnet (§13.5).

Nebenwirkung, ausdrücklich gewollt: der TT-RAM-**Pool** beginnt jetzt exakt
bei `TTRAM_START = 0x0100_0000`, wie EmuTOS' `altram_init`
(`bios/memory2.c:147-150`, `include/tosvars.h:77`) — vorher lag er 2 MiB
höher, weil der Heap davorstand.

### 14.3 Der feste ST-RAM: `membot` ist eine Konstante

`STRAM_OS_RESERVE = 4 MiB`, `stram = [DRAM_START + 4 MiB, DRAM_START + 14 MiB)`,
also `STRAM_POOL_BYTES = 10 MiB` — unabhängig von Build-Profil, Kernel-Größe
und angeschlossener Peripherie. Damit das aufgeht, mussten drei Dinge aus dem
unteren 4-MiB-Budget:

**`.dma` (2 MiB) → `$E00000..$FFFFFF`.** Das ist `STRAM_IO_RESERVE`, das
Atari-Hardware-Register-Fenster, das ohnehin aus **beiden** Pools ausgespart
ist. Ein dauerhaft gehaltener Hardware-Puffer gehört genau dorthin, und es
kostet den 10-MiB-Pool nichts. Der Block verlässt `.bss`, deshalb löscht ihn
der Boot-Stub jetzt über ein zweites `__dma_phys_*`-Paar; die Zusicherung
„jedes nullinitialisierte Static ist null, bevor Rust läuft" bleibt exakt
erhalten.

**Der Stack-Guard wird eine Seite statt eines 2-MiB-Blocks.** Der
Kernel-Mapping bestand ausschließlich aus 2-MiB-L2-Blöcken, und ein
ungemapptes Loch kann nicht feiner sein als die Deskriptor-Granularität —
das war der *ganze* Grund für den 2-MiB-Guard und, zusammen mit dem
2-MiB-ausgerichteten Stack, für über 4 MiB Verbrauch im 14-MiB-Fenster.
`mmu::refine_stack_block` gibt genau dem **einen** Block, der den Guard
enthält, eine L3-Tabelle (ein Static, 4 KiB) und stanzt eine Seite aus.
`remap_l2_attrs` weigert sich seither, einen Table-Deskriptor zu
überschreiben — sonst könnte ein späterer Remap den Kernel-Stack
abhängen.

**Der Kernel-Stack** folgt direkt auf `.bss` und passt damit unter das Budget.

**Überschreiten ist ein Linker-Fehler, keine stille Verkleinerung.** Beide
Skripte haben
`ASSERT(__kernel_image_end - VA_OFFSET <= STRAM_OS_RESERVE_PHYS, …)`.
Mit einer Scratch-Änderung auf 2 MiB geprüft:

```text
rust-lld: error: kernel image + stack overruns STRAM_OS_RESERVE (membot)
          -- see bios/machine/layout.rs
```

Aktuelle Reserve: Release `0x400000 − 0x276000` = **1,54 MiB**,
Debug **1,43 MiB**.

### 14.4 Die Datenblattfrage zu `.dma` — und warum daraus kein Rateschluss wurde

Das BCM2711-Datenblatt (`references/datasheets/RP-008248-DS-1`) dokumentiert
**kein** PCIe-Inbound-Fenster: null Treffer für `RC_BAR`, `inbound`,
`root complex`, `SCB0`, `0xFD50`. Es beschreibt nur die *ausgehende*
PCIe-Range in der Adresskarte (§1.2.1/§1.2.2, `0x6_0000_0000`, „covers 8GB").

Die erreichbare Region ist deshalb keine feste Hardware-Eigenschaft, sondern
das, was **unser eigener Treiber** in RC_BAR2 programmiert:
CPU-physisch `[0, rc_size)`, wobei `rc_size` die auf eine Zweierpotenz
gerundete DTB-`dma-ranges`-Größe ist, bzw. 1 GiB, wenn `dma-ranges` fehlt
(`kernel/src/drivers/usb/pcie_rpi4.rs:22,476-490`). Alt (`0x600000`) und neu
(`0xE00000`) liegen im selben, lückenlosen Fenster.

Statt das anzunehmen, **prüft der Treiber es jetzt**: `pcie_rpi4` vergleicht
die gelinkte `.dma`-Ausdehnung mit dem gerade programmierten Fenster,
protokolliert das Ergebnis und schaltet USB ab, statt in unerreichbaren
Speicher zu DMAen. Damit bleibt keine Annahme übrig — die Maschine sagt es.

### 14.5 Nachweise

* **Guard wirklich page-granular:** Scratch-Sonde, ein `read_volatile` auf
  `__stack_guard_start` nach `exceptions::init()`:
  `ESR=0x96000007` → EC=0x25 Data Abort, **DFSC=0b000111 = Translation
  Fault Level 3**, `FAR=0xffff00004025d000` = `__stack_guard_start`. Genau
  der L3-Seitenfehler, den die Verfeinerung erzeugen soll. Sonde danach von
  Hand entfernt.
* **`.dma` am neuen Ort funktioniert:** auf `qemu-virt` enumeriert der
  xHCI-Stack Tastatur und Maus (`HID configured: slot 1/2`) — das verlangt
  genullte DCBAA-, Command- und Event-Ringe im verschobenen Block.
* **Beide Boards booten bis zum Desktop:** raspi4b mit gemountetem C: (20
  Einträge im Fenster), virt mit Tastatur+Maus. 0 PANIC. Debug-Build
  ebenfalls (dort greifen die neuen `debug_assert!`s: Poolgröße, Guard-Größe,
  `img_end <= membot`, kein Table-Deskriptor im Remap).
* **Abbildgröße:** `kernel8.img` bleibt 770 600 Byte — der NOLOAD-`.dma` bei
  hoher LMA bläht das Rohabbild nicht auf.

### 14.6 F9 — was davon übrig bleibt

Der **Schaden** von F9 ist weg: die ST-RAM-Zahl auf dem Boot-Schirm ist jetzt
eine Konstante, die sich weder mit dem Kernel noch mit der Peripherie ändert,
und weil 10 MiB ein exaktes MiB-Vielfaches ist, druckt EmuTOS'
`cprintf_bytesize`-Regel sie sauber als **„10 MB"**.

Der **nominelle** Unterschied bleibt: EmuTOS zeigt `phystop` (= die ST-RAM-Größe
der Maschine, bei uns 14 MB), wir zeigen den Pool (10 MB). Beides sind jetzt
Konstanten. Meine Einschätzung: der Pool ist die nützlichere Zahl — 14 MB
würde 4 MB bewerben, die kein Programm bekommt —, aber es ist eine sichtbare
Produktentscheidung und keine Mechanik, deshalb steht sie hier statt im Code.
**Dem Lead vorgelegt, nicht im Alleingang entschieden.**

### 14.7 Was ohne echten Pi-4-Boot offen bleibt

* Der **Device-nGnRnE-Remap des verschobenen `.dma`-Blocks**
  (`usb::map_dma_coherent` → `mmu::remap_device_ngnrne(0xE00000, 2 MiB)`)
  läuft nur, wenn ein VL805 gefunden wird — QEMU `raspi4b` modelliert keinen.
  Der Pfad ist also **real-Pi-4-only**. Block-Index-Arithmetik geprüft
  (i=0, j=7; der verfeinerte Guard-Block ist j=1, also kein Konflikt), und
  der neue `is_table`-Schutz in `remap_l2_attrs` würde eine Verwechslung
  fangen.
* Die neue `[pcie] .dma block … inside the …-byte inbound window`-Zeile
  erscheint erst auf echter Hardware; sie ist Konsolenausgabe (der User hat
  keine serielle Leitung), also ein Tester-/Lead-Nachweis, kein User-Nachweis.
* F6 (≥3 GB einer 4-GB-Maschine unverwaltet) ist unverändert offen; §13.5
  und §14.2 haben nur die *Zahl* geklärt (768 MiB ARM-Anteil).
