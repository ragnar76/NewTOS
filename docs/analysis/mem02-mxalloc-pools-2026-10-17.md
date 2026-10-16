# MEM02 — `Mxalloc` und die Pools: Ist-Zustand und Ursache (Phase 1)

Datum: 2026-10-17 · Ticket: [`TASK/MEM02.md`](../../TASK/MEM02.md) ·
Belege: [`measurements/measurement-2026-10-17-mem02/RUNINFO.TXT`](../../measurements/measurement-2026-10-17-mem02/RUNINFO.TXT)

## Ergebnis in einem Satz

Auf dem heutigen Baum (HEAD `6b8b88cb`) beantwortet `Mxalloc` alle vier Modi aus
dem richtigen Pool, unter `memprot` an, aus und strikt, und die Ball-Demo meldet
alle neun Puffer als `/alt`. Der Befund vom 2026-09-28 war **kein Pool-Fehler**:
Die Blöcke kamen schon damals aus dem TT-RAM. Die Demo bekam aber die
Fenster-VA aus ADR-021 zurück, und die liegt ab 4 MiB, also im Zahlenbereich des
ST-RAM. ARCH03 Stufe 1 (`a9279fe9`, VA = PA) hat das beseitigt.

## 1. Gemessen (QEMU raspi4b, `serial=on`)

| Modus | HEAD memprot an/strict | HEAD memprot=off | c806a645 (vor ARCH03 1) |
|---|---|---|---|
| `Mxalloc(-1, MX_STRAM)` | 0x00a00000 (ST-Pool) | 0x00a00000 | 0x009ff000 |
| `Mxalloc(-1, MX_TTRAM)` | 0x37e76000 (TT-Pool) | 0x387e5d70 | 0x01c00000 (auf das Fenster geklemmt) |
| `Mxalloc(-1, MX_PREFSTRAM/PREFTTRAM)` | max = TT | max = TT | max |
| `MX_STRAM` 64K | 0x00400000 ST, EL0 r/w ok | 0x00400000 ST | 0x00400000 „ST“ |
| `MX_TTRAM` 64K | 0x0198a000 alt, ok | 0x0101a290 alt | 0x00410000 „ST“ |
| `MX_PREFSTRAM` 64K | 0x00410000 ST, ok | 0x00410000 ST | 0x00420000 „ST“ |
| `MX_PREFTTRAM` 64K | 0x0199a000 alt, ok | 0x0102a290 alt | 0x00430000 „ST“ |
| `MX_PREFTTRAM\|F_PROT_G` | alt, ok | alt | „ST“ |
| `MX_PREFTTRAM` 2025K | alt, ok | alt | „ST“ |
| `MX_STRAM` > ST-Größter | 0 | 0 | 0 |
| `MX_TTRAM` > TT-Größter | 0 | 0 | 0 |
| `MX_PREFSTRAM` > ST-Größter | alt (Rückfall) | alt | „ST“ |
| Ball-Demo, 9 Puffer | alle `/alt` | alle `/alt` | alle `/ST` |

phystop = 0x00e00000, ramtop = 0x39800000 (HEAD) bzw. 0x3be00000 (c806a645).
„ST“ in Anführungszeichen heißt: so klassifiziert, weil die Adresse unter
phystop liegt.

**Auch vor ARCH03 kam der Block aus dem TT-RAM.** Die Probe auf `c806a645` liest
`Mxalloc(-1, MX_STRAM)` vor und nach den Anforderungen: 0x009ff000 → 0x009dd000.
Die Differenz ist 0x22000 = 2 × (64 KiB + eine Seite Ausrichtungsreserve aus
`el0_alloc`). Das sind genau die Blöcke aus `MX_STRAM` und `MX_PREFSTRAM`. Die
vier TT- und PREFTT-Blöcke (zusammen etwa 2,2 MB) haben den ST-Pool nicht
berührt. Die „4,2 MB aus dem 10-MB-Pool“ des Tickets waren also nie
ST-Verbrauch.

## 2. Wo es aufhörte — die drei Stellen des Tickets

1. **Wird die Präferenz ausgewertet?** Ja, und zwar heute wie damals.
   - Unsere Seite: `tos-api/src/bdos/mem/traps.rs:313` maskiert den Modus
     (`mode & MX_MODEMASK`), `:318-345` liefert die Größe bei `-1`, `:389-408`
     wählt den Pool. `MX_PREFTTRAM` versucht `:400` zuerst `pmdalt` und danach
     `pmd`.
   - EmuTOS: `references/emutos/bdos/umem.c:241` (Maske), `:247-276` (`-1`,
     bei PREF der größere der beiden Pools), `:293-314` (Poolwahl; PREFTT
     `:308-314`).
   - Der Trap-Eintrag `tos-api/src/gemdos/memory.rs:69-75` reicht `amount`
     und `mode` unverändert weiter.
2. **Ist der TT-RAM-Pool angeschlossen?** Ja. `kernel/src/lib.rs:972-985`
   seedet `init::init(st, tt)`, und `pmdalt` ist der Pool, den
   `MX_TTRAM`/`MX_PREFTTRAM` nehmen. Im Lauf steht `[mm] BDOS pools: ST-RAM
   0x00400000+10485760 TT-RAM 0x0196f000+938020864`. Gegenstück in EmuTOS:
   `pmdalt` in `bdos/umem.c:36-37`, gefüllt über `xmaddalt` `:455ff`.
   `CONF_WITH_ALT_RAM` ist `1` (`include/config.h:879-884`), und im
   Falcon/TOS4-Profil ist das die Alt-RAM-Unterstützung (FastRAM). Nach
   `docs/api/_emutos-config.md:552` handelt es sich um eine HW-Zeile, die wir
   über unsere Pool-Aufteilung abbilden.
3. **Kommt der Block im EL0-Raum an?** Heute ja, als derselbe Wert (VA = PA).
   EL0 schreibt und liest jeden Block, auch unter `memprot=strict`. **Hier lag
   die Ursache des alten Befunds.** Auf `c806a645` hat
   `tos-api/src/gemdos/memory.rs:90-128` (`el0_alloc`) den Poolblock `raw`
   korrekt aus dem verlangten Pool geholt (`:113`). Bei `:119` wurde er aber per
   `usermem::map` in das Prozessfenster eingeblendet, und zurück kam
   `user_va = WINDOW_BASE + cursor` (`tos-api/src/bdos/process/usermem.rs:326`).
   Dabei ist `WINDOW_BASE = arena::TPA_SIZE = 4 MiB` (`:77`), und das Fenster
   reicht bis `USER_L3_COUNT × 2 MiB = 32 MiB` (`mmu.rs:80`). Jede VA
   zwischen 0x00400000 und 0x00e00000 liegt unter phystop. Die Ball-Demo
   (`ball.c` `ram_of`) klassifiziert über phystop, deshalb las sie „ST“.
   EmuTOS hat kein solches Fenster. Dort gibt `xmxalloc` `m->m_start` zurück
   (`bdos/umem.c:323-327`), und genau das tut heute auch unser Pfad
   (`traps.rs:410-428`, `memory.rs:69-75`).

Seit `a9279fe9` (2026-10-05, „one shared address space for all programs (ARCH03
stage 1)“) gibt es das Fenster nicht mehr. Der Pi-Lauf vom 2026-09-28 mit Kern
`09f3e928` liegt davor.

## 3. Vorschlag

**Keine Produktcode-Änderung nötig.** Eine „Reparatur“ hätte am Pool nichts zu
reparieren. Die Poolwahl entspricht `xmxalloc` Zeile für Zeile, und der
Rückgabewert ist seit ARCH03 Stufe 1 der Poolblock selbst.

Was bleibt, ist das Festnageln:

1. **Host-Tests** (Entwürfe liegen bei
   `measurements/measurement-2026-10-17-mem02/method/host-tests/`, auf einer
   Scratch-Kopie von HEAD gefahren: 3/3 und 1/1 ok):
   - `tos-api/tests/mem02_mxalloc_modes.rs`: Ein EL0-Aufrufer läuft über den
     GEMDOS-Trap-Eintrag. Geprüft werden die Größenabfragen (`-1` je Modus,
     PREF = max, Modus 4 → maskiert), alle vier Modi plus PREFTT|`F_PROT_G`
     (Adresse im jeweiligen Pool) sowie Verweigerung (`MX_STRAM`/`MX_TTRAM`
     über dem Größten → 0) und Rückfall (`MX_PREFSTRAM` → TT).
   - `tos-api/tests/mem02_mxalloc_no_alt.rs`: eigenes Binary, ohne TT-Pool
     (`init(st, 0, 0, 0)`). `MX_TTRAM` liefert 0 und kein ST-RAM,
     `Mxalloc(-1, MX_TTRAM)` liefert 0, `MX_PREFTTRAM` liefert ST. Das ist die
     Abnahmeforderung des Tickets („`MX_TTRAM` … muss fehlschlagen“).
   - **Rot vorher:** auf `c806a645` hätte der Modi-Test bei den Adressprüfungen
     versagt, weil alle EL0-Blöcke Fenster-VAs ab 4 MiB waren. In QEMU gezeigt
     hat das die Probe (`pre-arch03`: FAIL). Den Host-Test selbst habe ich auf
     dem alten Stand nicht gefahren, dort fehlen die heutigen Test-Hilfen.
     **Grün nachher:** HEAD.
   - Teilabdeckung gibt es schon:
     `tos-api/tests/arch03_shared_space.rs::an_el0_malloc_returns_the_pool_block_itself`
     prüft `MX_STRAM`, `MX_TTRAM` und `Malloc` mit `PF_TTRAMMEM`, aber nicht
     die PREF-Modi, keine Verweigerung, keinen Rückfall und keinen Fall ohne
     Alt-RAM.
2. **QEMU-Probe:** `MXPROBE.NCP` (`method/tests/mxprobe.c`) als
   AUTO-Konsolenprogramm. PASS auf HEAD unter on/off/strict, FAIL auf
   `c806a645`.
3. **Abnahme auf der Pi** (Lead/Anwender): dieselbe Ball-Demo auf der Karte.
   Erwartet wird bei allen neun Puffern `/alt` (die Demo schreibt „alt“,
   nicht „TT“, wie das Ticket vermutet).

## Nebenbefunde (nicht angefasst)

- Die Ball-Demo nennt alternate RAM „alt“. Die Abnahmezeile im Ticket („`/TT`
  statt `/ST`“) sollte deshalb „`/alt`“ heißen.
- Der Absatz „Warum es mehr ist“ in `TASK/MEM02.md` und die Voraussetzung in
  `TASK/PORT06.md` §3 beruhen auf der Lesart „ST-RAM verbraucht“. Die ist
  widerlegt, und beide Stellen müssten berichtigt werden. Begründungen stehen
  selten nur an einer Stelle.
