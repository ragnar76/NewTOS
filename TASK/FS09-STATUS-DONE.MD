# FS09 — Quittung (FERTIG, QEMU): Block-Lese-Cache nach FreeMiNT `block_IO`

Datum 2026-10-07 · Basis `desk-rebuild` `48b7f54` (+ `3ec1983`, nur Doku) + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-fs09](../measurements/measurement-2026-10-07-fs09/RUNINFO.TXT)
· Ticket: [FS09](FS09.md) · Spezifikation: [`docs/analysis/fs09-block-cache-2026-10-07.md`](../docs/analysis/fs09-block-cache-2026-10-07.md)

## Auftrag

Entscheid des Anwenders 2026-10-07: großer **Lese**-Cache (einige MB, Größe
einstellbar wie FreeMiNT), LRU; Schreiben bleibt Durchschreiben (HW03);
Mediach/Laufwerkswechsel invalidiert; FreeMiNT `block_IO.c` als
Verhaltensvorbild (GPL — nachgebaut, nicht kopiert).

## Spezifikation (zuerst)

* `docs/analysis/fs09-block-cache-2026-10-07.md`: FreeMiNT-Verhalten mit
  Fundstellen (`block_IO.c` `DEFAULT`/`DEFAULT_PERC` :183-184, `MIN_BLOCK`
  :201, `CHUNK_SIZE` :202, `bio_l_read` :2071-2151, `bio_units_add` :2029-2069,
  `bio_large_write`/`bio_small_write`/`bio_l_write` :2154-2262, `bio_pre_read`
  leer :2271-2276, `bio_invalidate` :2411-2458; `cnf_mint.c` `CACHE=`/
  `FS_CACHE_SIZE=`/`PERCENTAGE=`; `fatfs.c` `fatfs_dskchng`; `bios.c`
  `sys_b_rwabs`) und der NewTOS-Entwurf.
* Notizen in `docs/api/bios/04-rwabs.md`, `07-getbpb.md`, `09-mediach.md`.

## Entscheidungen

| Frage | Entscheid | Grund |
|---|---|---|
| Lage | **unter** allen Kartenzugriffen (`bios::bcache::device()` statt `Host::block()`), Schlüssel = absolute LBA | GEMDOS-Direktübertragungen, BDOS-Puffer, `Rwabs` logisch+physisch, `Getbpb`, MBR-Scan teilen eine kohärente Kopie; kein Schreibweg am Cache vorbei. FreeMiNT liegt über `Rwabs` (benannte Abweichung) |
| Einheit | 512-B-Sektor, Fehlschlag-Läufe genau gelesen, ein Befehl je Lauf | FreeMiNT `CHUNK_SIZE`; Blockschicht kennt keine Cluster. Auffüllen auf 4–64 KB wäre Vorauslesen (+0,17 ms bzw. +3 ms je Fehlschlag bei 0,55 ms/Befehl, 10,5 MB/s) |
| Vorauslesen | **keins** | FreeMiNT `bio_pre_read` ist leer — die Spezifikation gibt es nicht her |
| Ersetzung | exaktes LRU | FreeMiNT nähert LRU über Zeitstempel auf 32-KB-Blöcken an (variable Einheiten); bei festen Einheiten ist exakt einfacher |
| Größe | **32 MiB** Vorgabe, `bcache=<KiB>` (0 = aus, ≤ 256 MiB, 2-MiB-Schritte) | FreeMiNTs 128 KB ≈ 3 % eines 4-MB-ST → ~30 MiB der ~950 MiB auf der Pi; fasst FSBENCH (22 MB), WAD, zView+Codecs |
| Anteil lineare Lesungen | **5 %** Vorgabe, `bcachepct=` | FreeMiNT `DEFAULT_PERC`; ein Riesen-`Fread` fegt den Cache nicht leer |
| Einstellung | Boot-Argument (`cmdline.txt`), kein `NEWDESK.INF` | Speicher wird vor den Pools und vor dem ersten Kartenzugriff ausgeschnitten |
| Speicher | eigener Bereich unten in der OS-Reserve am DRAM-Ende, außerhalb ST/TT-RAM, nicht in Benutzertabellen; Heap/Framebuffer behalten ihre Adressen | Muster `balloc_stram`; Framebuffer-Regel unberührt (Pi: VideoCore; qemu-virt: kein Cache) |
| Schreiben | Karte zuerst; bestätigt ≤ 32 KiB → Sektoren einfügen/überschreiben; > 32 KiB → Bereich vorher verwerfen; Fehler → Bereich verwerfen | HW03, FreeMiNT `bio_small_write`/`bio_large_write`; nach abgebrochenem Mehrblock-Schreiben ist die Karte die Wahrheit |
| Invalidieren | `bios::mount()`; `Mediach` ist konstant 0; `Getbpb` invalidiert nicht | wie FreeMiNT (Invalidierung folgt einem erkannten Wechsel) |

## Was geändert ist

* neu `tos-api/src/bios/bcache.rs`; `tos-api/src/bios/mod.rs` (Modul,
  `mount()` verwirft, `Rwabs` über `device()`), `bios/bpb.rs`, `bios/drive.rs`,
  `bdos/fs/cache.rs` (über `device()`).
* `kernel/src/bios/machine/layout.rs` (`discover(.., block_cache_bytes)`,
  `MemoryMap::block_cache`), `kernel/src/bootargs.rs` (`bcache=`,
  `bcachepct=`, `numeric_setting` + Test), `kernel/src/lib.rs`
  (`bcache_switch`, `install_block_cache`, Bootzeile `[bcache]`).
* Tests neu: `tos-api/tests/bios_bcache.rs` (16), `tos-api/tests/gemdos_fs_bcache.rs` (5).

## Messwerte (QEMU raspi4b, memprot an)

| Messung | Ergebnis |
|---|---|
| FSBENCH Lesekommandos kalt / warm, Cache aus | 75 711 / 75 721 (gesamt 151 492 = FS08 Teil 4) |
| … 8 MiB / 16 MiB | 72 717 / 72 727 · 46 642 / 46 655 |
| … **32 MiB (Vorgabe)** | **45 878 / 0** (10 gezählte = Protokoll auf C:) |
| FSBENCH 100 Lesungen, D:\FSBENCH, fsck | crc ok, 10/10, sauber — alle Läufe |
| zView zweimal starten, Lesekommandos 1./2. Start | aus 296 / 291 · **an 104 / 0** · 2 MiB 104 / 0 |
| GUI aus → an: deskw / qedw / teraw / zvw | 192→117 · 286→40 · 284→66 · 566→307; Schreibkommandos gleich; HOSTCHECK PASS; fsck sauber |
| Screendumps an/aus/2 MiB | 37/40 bytegleich, 3 nur Dump-Zeitpunkt (Mauszeiger, Menü) |
| FSWRITE an / aus / 2 MiB | PASS, HOSTCHECK PASS, fsck sauber |
| APITEST / S2ALL | 106/0/5 / PASS |
| Hosttests | **1227/0/1** (+22) |
| Release-Bau rpi4 / qemu-virt | 64/94 / 64/94 |
| clippy rpi4 / qemu-virt | 76/125 / 76/125 |
| `desk-drift-probe.sh` / SDK-Hosttests | 31 / 18/0/0 |

Ist der Cache kleiner als der Datensatz, hilft er beim zweiten FSBENCH-
Durchgang kaum: gleiche Lesereihenfolge = schlechtester Fall für LRU.

## Pi-Paket (nicht kopiert)

`measurements/measurement-2026-10-07-fs09/bin/kernel8.img` (sha256
`706bab02…`), `LIESMICH.TXT` (FSBENCH kalt/warm, zView zweiter Start,
Einstellen/Abschalten per `cmdline.txt`), `SHA256SUMS`.

## Offen

* Echte Pi 4: FSBENCH kalt gegen warm, Gegenprobe `bcache=0`, zView zweimal.
* TT-RAM ist um die Cachegröße kleiner (Startschirm, `Mxalloc(-1)`) — gewollt,
  aber sichtbar.
* `Mediach` meldet nie einen Wechsel; eine im Betrieb getauschte Karte bleibt
  nicht unterstützt (wie vorher). Kommt je ein Wechselpfad, gehört dort
  `bcache::invalidate_all()` hin.
* `Getbpb` invalidiert nicht (FreeMiNT-Verhalten) — falls der Lead „Getbpb
  liest neu von der Karte“ anders meinte, ist das eine Zeile.
* `docs/STATUS.md`-Eintrag beim Commit (Lead).
