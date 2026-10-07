# FS09 — Block-Lese-Cache nach FreeMiNT `block_IO`

Stand 2026-10-07, Branch `desk-rebuild` @ `48b7f54`. Spezifikation **vor** dem Code.
Ticket: [`TASK/FS09.md`](../../TASK/FS09.md). Vorarbeit: FS08 Teil 1–4
([`fs08-sd-read-2026-10-07.md`](fs08-sd-read-2026-10-07.md),
[`measurement-2026-10-07-fs08-pi-after`](../../measurements/measurement-2026-10-07-fs08-pi-after/RUNINFO.TXT)).

Quelle: FreeMiNT @ `f906416` — `sys/block_IO.c`, `sys/block_IO.h`,
`sys/mint/block_IO.h`, `sys/cnf_mint.c`, `sys/fatfs.c`, `sys/bios.c`.
FreeMiNT ist GPL: übernommen wird **Verhalten**, kein Code. EmuTOS hat keinen
Block-Cache (nur die BDOS-Puffer `bdos/fsbuf.c`, NUMBUFS 2) — das hier ist eine
FreeMiNT-Erweiterung über EmuTOS hinaus (Anwenderentscheid 2026-10-07).

Kennzeichnung wie in FS08: **[gelesen]** mit Fundstelle, **[gemessen]**, **[Schluss]**.

---

## 1. Was FreeMiNT tut [gelesen]

### 1.1 Lage und Schnittstelle

- Der Cache sitzt **zwischen dem Dateisystem (xfs) und BIOS `Rwabs`/XHDI**:
  `bio_readin`/`bio_writeout` rufen `BIO_RWABS` (`block_IO.c:705-756`,
  Makro `mint/block_IO.h:206-207`), das je Laufwerk auf `rwabs_log`,
  `rwabs_log_lrec` oder `rwabs_xhdi` zeigt (`block_IO.c:388-499`).
- Pro Laufwerk ein `DI` mit eigener Hash-Tabelle (`HASHBITS 8` = 256 Köpfe,
  `:188`, `bio_hash` `:627-636`); Schlüssel ist die **Sektornummer des
  Laufwerks** (logisch, `rec << lshift`).
- Ein BIOS-`Rwabs` eines Programms läuft **am Cache vorbei** (`sys_b_rwabs`,
  `bios.c:159-213`, ruft direkt `callout(RWABS, …)`); FreeMiNT schützt sich
  dort nur mit `Dlock` (`dlockproc`, `bios.c:177-181`).

### 1.2 Speicher, Größe, Einheiten

- Vorgabe **128 KB** (`DEFAULT 128UL`, `:183`), Anteil für lineare Lesungen
  **5 %** (`DEFAULT_PERC 5UL`, `:184`), gesetzt in `init_block_IO` (`:1327-1371`).
- Einstellbar in `mint.cnf`: `CACHE=n` / `FS_CACHE_SIZE=n` (KB) und
  `PERCENTAGE=n` / `FS_CACHE_PERCENTAGE=n` (`cnf_mint.c:97,109,128-129,641-642,
  675-676`). `bio_set_cache_size` **legt dazu** (`cache.count += count`,
  `:1483`); `bio_set_percentage` rechnet `count * max_size * pct` (`:1503`),
  > 100 → `EBADARG` (`:1500-1501`).
- Speicher: `kmalloc` von `count` Blöcken zu `MIN_BLOCK = 32 KB` (`:201`,
  `:1394-1395`), jeder Block in **512-Byte-Stücke** (`CHUNK_SIZE`, `:202`)
  geteilt; ein `UNIT` belegt `ceil(size/512)` zusammenhängende Stücke in einem
  Block (`bio_unit_get`, `:1135-1314`).
- Einheitengröße = die Blockgröße, die das xfs anfragt: FAT/Wurzelverzeichnis
  je Sektor (`bio.read` mit `SECSIZE`, `fatfs.c:1245-1291`), Daten je Cluster
  (`CLUSTSIZE`, `fatfs.c:1294-1366`).

### 1.3 Ersetzung

- Zeitstempel `stat = c20ms` bei jedem Treffer und beim Einfügen
  (`bio_update_stat`, `:610-616`; `bio_hash_lookup` `:641-661`).
- Opferwahl: der erste unverriegelte 32-KB-Block mit genug freien Stücken,
  sonst der mit dem **ältesten** Zeitstempel (`:1172-1199`); darin die
  Stückfolge mit den geringsten Kosten (belegte Größe minus Alter, schmutzig
  doppelt, `:1224-1258`). Das ist ein **LRU-Näherungsverfahren** auf
  Blockebene. Verdrängte schmutzige Einheiten werden erst zurückgeschrieben
  (`bio_unit_remove` → `bio_wb_unit`, `:1121-1128`).

### 1.4 Lesen: `bio_l_read` (`:2071-2151`)

1. Je Block der Anfrage `bio_lookup`. **Treffer**: aus dem Cache kopieren
   (`:2095-2112`, `:2114-2117`).
2. **Fehlschläge** werden zu einem zusammenhängenden Lauf gesammelt
   (`tstart`/`tblocks`, `:2121-2125`) und mit **einem** `bio_readin` direkt in
   den Puffer des Aufrufers gelesen (`:2102`, `:2134`).
3. Nur bei Erfolg und nur wenn `size * 100 <= cache.percentage` werden die
   gelesenen Blöcke als Einheiten eingefügt (`bio_units_add`, `:2029-2069`,
   Grenze `:2034`). Lange lineare Lesungen verdrängen den Cache also nicht.
4. Ein Lesefehler bricht ab, nichts aus dem Lauf wird eingefügt (`:2108`).

### 1.5 Schreiben

- `bio_l_write` (`:2249-2262`): Anfrage **> `MIN_BLOCK` (32 KB)** →
  `bio_large_write` (`:2154-2207`): zuerst **alle** Einheiten im Bereich aus
  dem Cache entfernen (`:2163-2194`), dann direkt schreiben.
- Sonst `bio_small_write` (`:2210-2246`): je Block `bio_getunit` (legt eine
  Einheit an, falls keine da ist — *write-allocate*), Daten hineinkopieren,
  als geändert markieren (`bio_mark_modified` → Rückschreib-Warteschlange
  `bio_wbq_insert`, `:823-872`).
- Write-back-Warteschlange je Laufwerk, sortiert nach Sektor; `bio_wb_queue`
  fasst benachbarte Einheiten bis `WB_BUFFER = 64 KB` zu einem Schreibbefehl
  zusammen (`:186`, `:997-1050`).
- **Ohne `WB_ENABLE` für das Laufwerk** ruft das xfs nach jeder Operation
  `bio_SYNC_DRV` (`mint/block_IO.h:220-226`, `fatfs.c:8168`): die
  Warteschlange wird sofort geleert. FreeMiNTs **Vorgabe** ist also
  praktisch Durchschreiben; echtes Write-back nur auf Wunsch.

### 1.6 Invalidieren

- `bio_invalidate` (`:2411-2458`) wirft **alle** Einheiten eines Laufwerks
  weg, schmutzige ohne Rückschreiben.
- Aufgerufen über `bio_free_di` (`:1857-1874`) aus `fatfs_dskchng`
  (`fatfs.c:7738-7779`): ein erkannter Medienwechsel
  (`BIO_DSKCHNG` → `dskchng_bios` = `Mediach` ≠ 0, dann `Getbpb`,
  `block_IO.c:507-523`) gibt das `DI` frei und damit den Cache des Laufwerks.
  Ebenso `bio_set_lshift` bei geänderter Abbildung (`:1905-1925`).
- `Getbpb` **allein** invalidiert nichts; es ist der Wiedereinleser **nach**
  einem erkannten Wechsel.

### 1.7 Vorauslesen

`bio_pre_read` ist ein leerer Rumpf („not implemented", `:2271-2276`,
Kopfkommentar `:150`, `:159-160`). **FreeMiNT liest nicht voraus.**

---

## 2. NewTOS-Entwurf

### 2.1 Lage: unter BIOS `Rwabs`, ein Cache für das Blockgerät

```
GEMDOS fsio (direkt, usrio) ─┐
GEMDOS cache.rs (FAT/Daten) ─┼─> bios::bcache::device() ─> bcache ─> Host::block() (EMMC2)
BIOS Rwabs (log. + phys.)  ──┤
Getbpb / MBR-Scan  ──────────┘
```

- `tos-api/src/bios/bcache.rs`: **ein** Cache für das (einzige) Blockgerät,
  Schlüssel = **absolute Geräte-LBA** (512-Byte-Sektor).
- **Abweichung von FreeMiNT (benannt):** FreeMiNT sitzt über `Rwabs` und hat
  einen Cache je Laufwerk mit logischer Sektornummer; ein Programm-`Rwabs`
  geht vorbei. NewTOS legt den Cache **unter** alle Zugriffe auf das Gerät.
  Dadurch:
  - laufen GEMDOS-Direktübertragungen (`fsio.rs` `usrio`), die kleinen
    BDOS-Puffer, BIOS-`Rwabs` (logisch **und** physisch, `RW_NOTRANSLATE`),
    `Getbpb` und der MBR-Scan durch dieselbe Instanz;
  - ist logische und physische Adressierung **ein** Schlüsselraum — kein
    Alias-Problem (logischer Sektor s auf C: = LBA `start + s`, dieselbe
    Zeile wie ein physischer Zugriff auf `start + s`);
  - kann kein Schreibweg den Cache umgehen (FreeMiNT braucht dafür `Dlock`).
- `Host::block()` bleibt das rohe Gerät; alle fünf Aufrufer in `tos-api`
  (`bios/mod.rs` Rwabs, `bios/drive.rs` mount, `bios/bpb.rs` ×2,
  `bdos/fs/cache.rs`) gehen über `bios::bcache::device()`.

### 2.2 Einheit: 512 Byte, genau gelesen, kein Vorauslesen

- Gespeichert und verwaltet je **512-Byte-Sektor** — FreeMiNTs `CHUNK_SIZE`.
  Die Blockschicht kennt keine Cluster (sie liegt unter `Rwabs`), also gibt es
  keine xfs-Blockgröße, die man übernehmen könnte; der Sektor ist die einzige
  Einheit, die alle Aufrufer teilen.
- Fehlschläge werden wie in `bio_l_read` zu Läufen zusammengefasst und mit
  **einem** Gerätebefehl **genau** gelesen (kein Auffüllen auf 4–64 KB).
- Begründung aus den Pi-Messungen [gemessen, `fs08-pi-after/FSBENCH.LOG`]:
  ein Befehl kostet ~0,54–0,59 ms (L1 1 Sektor/Aufruf), Streaming
  ~10,5 MB/s ≈ 0,049 ms je KB (L1 128 Sektoren). Ein auf 4 KB aufgefüllter
  Fehlschlag kostet +0,17 ms (+30 %), auf 64 KB +3 ms (+550 %) — das lohnt nur,
  wenn die Nachbarn danach wirklich gelesen werden, also als **Vorauslesen**.
  FreeMiNT tut das nicht (§1.7); die Spezifikation gibt es nicht her →
  **kein Vorauslesen** in FS09. [Schluss] Größere Einheiten brächten sonst
  nur weniger Verwaltungsdaten (s. §2.4: 24 B je 512 B = 4,7 %), aber
  Verschnitt bei verstreuten Einzelsektoren (FAT, Verzeichnisse).
- Grob sind sequentielle Lesungen heute schon gebündelt: `fsio` liest ganze
  Sektoren als **einen** Auftrag, stdio seit FS08 Teil 4 in 32 KB.

### 2.3 Ersetzung: echtes LRU

Doppelt verkettete LRU-Liste über die Sektor-Plätze; Treffer → Kopf, Einfügen
→ Kopf, Opfer = Schwanz. FreeMiNTs Zeitstempel-Verfahren (§1.3) ist eine
Näherung an LRU auf 32-KB-Blöcken, nötig wegen variabler Einheitengrößen;
mit gleich großen Einheiten ist exaktes LRU einfacher und genauer.
Kein Platz ist je schmutzig (Durchschreiben, §2.5), also schreibt das
Verdrängen nie zurück.

### 2.4 Speicher und Größe

- **Herkunft:** ein eigener Bereich `block_cache` in der OS-Reserve am oberen
  DRAM-Ende (`kernel/src/bios/machine/layout.rs` `discover`), neben Kern-Heap
  und (nur qemu-virt) Framebuffer — außerhalb von ST-RAM und TT-RAM, nie in
  einem Pool, nicht über `Malloc` erreichbar, nicht in die Benutzertabellen
  eingetragen (`map_pools` bildet nur die Pools ab). Gleiches Muster wie
  EmuTOS `balloc_stram(size, TRUE)` (`bios/biosmem.c:88-131`). Der
  Framebuffer liegt auf der Pi bei der VideoCore (außerhalb `dram`), auf
  qemu-virt in seinem eigenen Reserveteil — keine Überlappung.
- Der Kern-Heap (2 MiB) ist zu klein und nicht wachsend; daher eigener Bereich.
  Verwaltungsdaten (je Platz 24 B + Hash-Kopf ≤ 8 B) liegen **im** Bereich,
  der Rest sind 512-B-Datenplätze.
- **Vorgabe 32 MiB.** Begründung: FreeMiNTs 128 KB sind auf einem 4-MB-ST
  ~3 % des RAMs; dasselbe Verhältnis zu den ~950 MiB, die NewTOS auf der Pi
  verwaltet, ergibt ~30 MiB. 32 MiB fassen den FSBENCH-Datensatz (22 MB),
  ein DOOM-WAD (4–14 MB), zView mit Codecs (~1,5 MB), Desktop/QED und alle
  Metadaten. TT-RAM wird um 32 MiB kleiner (~3,4 %).
- **Anteil lineare Lesungen: Vorgabe 5 %** wie FreeMiNT (§1.4): ein Lauf von
  Fehlschlägen wird nur eingefügt, wenn `Lauf · 100 ≤ pct · Kapazität`
  (32 MiB → 1,6 MB je Lauf). Programme (`Pexec` liest ein Programm als einen
  Auftrag, ZVIEW.NAP ~0,5 MB), Codecs, stdio-Füllungen (32 KB) und FSBENCH-
  Lesungen bis 64 KB passen; ein riesiges einzelnes `Fread` (Kopieren mit
  großem Puffer) verdrängt den Cache nicht.
- **Einstellung per Boot-Argument** (`cmdline.txt` bzw. QEMU `-append`):
  `bcache=<KiB>` (0 = aus) und `bcachepct=<0..100>`. Kein `NEWDESK.INF`-
  Eintrag: der Bereich wird ausgeschnitten, **bevor** die Pools entstehen
  und bevor die Karte gelesen ist; eine Zeile auf der Karte käme zu spät, und
  ein späteres Wachsen müsste aus TT-RAM nehmen (FreeMiNT kann `CACHE=`
  nachträglich draufpacken, weil sein Cache aus `kmalloc` kommt). Wie
  `memprot=`/`emmc=`/`cpuclk=`.
- Obergrenze 256 MiB; `discover` gibt dem Cache höchstens die Hälfte dessen,
  was sonst TT-RAM wäre. Ungültiger Wert → Vorgabe, die Bootzeile sagt es.
- Nur `board-rpi4` (das einzige Board mit Blockgerät); auf qemu-virt wird nichts
  reserviert.

### 2.5 Schreiben: durchschreiben (HW03), dann den Cache angleichen

- **Schreiben geht immer zuerst auf die Karte.** Erst wenn das Gerät den
  Auftrag mit `Ok` bestätigt, wird der Cache angeglichen. Kein Write-back
  (Ticket-Entscheid 2; entspricht FreeMiNTs Vorgabe ohne `WB_ENABLE`, §1.5).
- Auftrag **≤ 32 KB** (FreeMiNT `MIN_BLOCK`): jeder Sektor wird eingefügt oder
  überschrieben (write-allocate wie `bio_small_write`).
- Auftrag **> 32 KB**: alle Sektoren des Bereichs werden entfernt
  (wie `bio_large_write`); spätere Lesungen holen sie von der Karte.
- **Schreibfehler:** alle Sektoren des Bereichs werden entfernt — was nach
  einem abgebrochenen Mehrblock-Schreiben auf der Karte steht, ist unbekannt;
  die nächste Lesung fragt die Karte. Der Fehlercode geht unverändert nach oben
  (HW03, `bios/mod.rs` `rwabs`).
- Teilüberlappung ist kein Sonderfall: die Einheit ist der Sektor; jeder
  geschriebene Sektor wird einzeln angeglichen, Nachbarn bleiben.

### 2.6 Lesen

Wie `bio_l_read` (§1.4): Treffer kopieren, Fehlschlag-Läufe mit einem
Gerätebefehl direkt in den Puffer des Aufrufers, danach (Erfolg + 5-%-Regel)
einfügen. Lesefehler: Rückgabe des Codes, **nichts** aus dem Lauf wird
eingefügt; was davor als Treffer kopiert wurde, bleibt im Puffer (wie vorher
ein Teilergebnis des einen Gerätebefehls).

### 2.7 Invalidieren

- `bios::mount()` (Partitionen neu einlesen: Boot, `selftest-emmc`) wirft den
  **ganzen** Cache weg, bevor der MBR gelesen wird — das ist NewTOS' „neues
  Medium / neue Abbildung" (FreeMiNT `bio_set_lshift`/`bio_free_di`).
- **Mediach:** Die Karte gilt als nicht wechselbar, `Mediach` liefert immer 0
  (`docs/api/bios/09-mediach.md`); es gibt keinen erkannten Wechsel und damit
  keinen Auslöser. Sobald `Mediach` je einen Wechsel meldet, gehört dorthin
  dasselbe Wegwerfen (wie `fatfs_dskchng`). Eine Karte im laufenden Betrieb zu
  ziehen und eine andere zu stecken ist weiterhin nicht unterstützt.
- **Getbpb** invalidiert nicht (wie FreeMiNT, §1.6): es liest den Bootsektor
  durch den Cache; weil jeder Schreibzugriff auf die Karte durch den Cache
  geht, ist der Cache-Inhalt gleich dem Karteninhalt.
- Wer am Cache vorbei auf die Karte schreibt (nur der Selbsttest
  `selftest-emmc`, der danach `bios::mount()` ruft), muss danach invalidieren.

### 2.8 Rwabs-Flags

`RW_NOTRANSLATE` (Bit 3) ändert nur die LBA-Berechnung, nicht den Cache (§2.1).
`RW_NORETRIES` (Bit 2) und `RW_NOMEDIACH` (Bit 1) wirken auf Wiederholung bzw.
Wechselprüfung im Treiber; ein Treffer hat weder Fehler noch Wechsel — sie
ändern am Cache nichts. Ein Lesen mit Fehler wird nie aus dem Cache beantwortet,
der Fehlercode kommt vom Gerät.

### 2.9 Beobachtbarkeit

Über die API unsichtbar (gleiche Bytes, gleiche Codes) — außer der Zeit und
dem um die Cachegröße kleineren TT-RAM. Bootzeile `[bcache] …` (Größe, Plätze,
Anteil, Quelle der Einstellung); Zähler (Treffer, Fehlschläge, Gerätebefehle,
eingefügt, verdrängt, entfernt) in `bcache::stats()`.

---

## 3. Prüfpunkte (Hosttests)

Treffer/Fehlschlag, LRU-Verdrängungsreihenfolge, Lauf-Zusammenfassung zu einem
Befehl, große Anfrage teilweise im Cache (Lücken = genau die Fehlschläge),
5-%-Regel, Durchschreiben mit Teilüberlappung, Schreibfehler entfernt, Lesefehler
fügt nichts ein, Invalidieren durch `bios::mount`, logisch/physisch dieselbe
Zeile, GEMDOS-Lesen/Schreiben byte-genau mit Cache, zufällige Lese-/Schreibfolge
gegen ein Modell (Inhalt und Karte byte-gleich).
