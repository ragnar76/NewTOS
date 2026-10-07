# FS08-FAT — Quittung (FERTIG, QEMU): FAT/GEMDOS-Lese- und Schreibweg wie EmuTOS

Datum 2026-10-07 · Basis `desk-rebuild` `003da28` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-fat-curcl](../measurements/measurement-2026-10-07-fat-curcl/RUNINFO.TXT)
· Ticket: [FS08](FS08.md), Teil 2 („FAT wie EmuTOS“). Teil 1 (Treiber): [FS08-EMMC](FS08-EMMC-STATUS-DONE.MD).

## Auftrag

Entscheidung des Anwenders 2026-10-07: der FAT/GEMDOS-Lesepfad 1:1 wie EmuTOS —
aktueller Cluster je offener Datei, getrennte FAT- und Datenpuffer mit EmuTOS'
Ersetzungsstrategie, ganze Sektoren eines `Fread`/`Fwrite` direkt in/aus dem
Puffer des Programms als ein Blockauftrag, nur angebrochene Sektoren über den
Puffer. Schreiben korrekt und zurückgeschrieben, fsck sauber.

## Spezifikation (zuerst)

* `docs/api/gemdos/_state.md` §3 neu „Replacement policy and FAT mirroring“
  (`getbcb`/`flush`, `bdos/fsbuf.c:114-202`, `:85-105`) und neu §3a „The file
  transfer engine `xrw`“ (`bdos/fsio.c:33-426`, `bdos/fsfat.c:23-293`), jeweils
  mit NewTOS-Anmerkung (8 statt 2 Puffer je Liste; Rückschreiben am Ende jedes
  `Fwrite` bleibt, HW03).
* `docs/api/gemdos/file/3f-fread.md`, `40-fwrite.md`, `42-fseek.md`: FS08-Notizen.

## Was geändert ist

* `tos-api/src/bdos/fs/fsio.rs` (neu): `Stream` mit `o_strtcl`/`o_fileln`/
  `O_DIRTY` und `o_bytnum`/`o_curcl`/`o_currec`/`o_curbyt`; `addit`, `nextcl`,
  `usrio`, `xrw_recs`, `xrw`, `ixlseek` wie EmuTOS.
* `tos-api/src/bdos/fs/cache.rs`: zwei Listen (FAT / Daten), je 8 Puffer, MRU-
  Ordnung, Opfer = letzter ungültiger sonst LRU, FAT-Puffer einmal gehalten und
  in jede FAT-Kopie zurückgeschrieben; `direct_read`/`direct_write` = `usrio`
  (überlappende Datenpuffer erst zurückschreiben, dann verwerfen). HW03-
  Semantik (fehlgeschlagenes Rückschreiben behält den Sektor) unverändert.
* `tos-api/src/bdos/fs/fat.rs`: FAT über die FAT-Liste, `findfree`/`findfree16`
  wie EmuTOS, `fat_lookups`-Zähler.
* `tos-api/src/bdos/fs/file.rs`: `Fread`/`Fwrite`/`Fseek` über die Engine.
* `tos-api/src/bdos/fs/mod.rs`: Programmlader (`FileImage::read_at`) und
  `read_file` über dieselbe Engine.
* Tests: `tests/fatclus/` (neu), `tests/gemdos_fs_curcl.rs` (7, neu),
  `tests/gemdos_fs_chain_steps.rs` (3, neu); `gemdos_fs_device_error.rs`:
  Hilfsfunktion dreht jetzt beide Listen um (Assertions unverändert).

## Messwerte (QEMU raspi4b, memprot an)

| Messung | Vorher (HEAD) | Nachher |
|---|---|---|
| FSBENCH gelesene Blöcke (Nutzlast 548 152) | 1 221 462 (2,23×) | **548 939 (1,0014×)** |
| FSBENCH Lesekommandos | 1 176 324 | **195 769** |
| FSBENCH 100 Lesungen / D:\FSBENCH / fsck | crc ok / OK / sauber | **crc ok / OK / sauber** |
| FSWRITE (neu, Schreibweg) + Host-Modell + fsck | PASS/PASS/sauber | **PASS/PASS/sauber** |
| FSWRITE Blöcke gelesen / geschrieben | 280 451 / 25 750 | **50 521 / 18 562** |
| EmuDESK kopieren/verschieben/löschen, Ordner kopieren/löschen | Host-Prüfung PASS | **PASS**, Dumps bytegleich |
| QED neu, sichern unter, ändern, sichern | PASS | **PASS**, Dumps bytegleich |
| TeraDesk Datei- und Ordnerkopie | PASS | **PASS**, Dumps bytegleich |
| zView BIG.JPG (2,2 MB) | dekodiert | **dekodiert**, Dumps bytegleich |
| APITEST / S2ALL | 106/0/5 / PASS | **106/0/5 / PASS** |
| fsck C: D: nach jedem Lauf | sauber | **sauber** |
| Hosttests | 1185/0/1 | **1195/0/1** (+10; 4 davon rot auf HEAD) |
| Release-Bau rpi4 / qemu-virt | 64/94 | **64/94** |
| clippy rpi4 / qemu-virt | 76/125 | **76/125** |
| `desk-drift-probe.sh` / SDK-Hosttests | 31 / 16/0/0 | **31 / 16/0/0** |

QEMU-Zeiten sind nicht aussagekräftig (keine Kartenlatenz modelliert).

## Pi-Paket (nicht kopiert)

`measurements/measurement-2026-10-07-fat-curcl/bin/`: `kernel8.img`
(sha256 `28f56fb3…`), `LIESMICH.TXT` (Vergleichslauf FSBENCH), `SHA256SUMS`.

## Offen

* Echte Pi 4: FSBENCH-Vergleichslauf (Paket oben).
* `Fwrite` schreibt den Puffer weiter am Ende jedes Aufrufs zurück (HW03),
  strenger als EmuTOS — bewusst beibehalten, in `_state.md` §3a benannt.
* Verzeichnisleser (`dir.rs` `dir_nth_sector`) läuft die Kette eines
  Unterverzeichnisses weiter je Sektor vom Anfang — nicht Teil dieses Auftrags.
* `docs/STATUS.md`-Eintrag beim Commit (Lead).
