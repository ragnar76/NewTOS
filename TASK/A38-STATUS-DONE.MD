# A38 — Quittung

**Status: DONE** · 2026-09-12 · Der Ueberlauf ist geschlossen und der
Schreibzugriff ausserhalb **gemessen** verschwunden (nicht bloss "stuerzt nicht
mehr ab"). Beide Anwendungen laufen weiter. **Offen, ausdruecklich zur Vorlage
und nicht mitentschieden:** ob `RSC_CAP` steigt bzw. der Puffer dynamisch wird.

## Quelle

`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(`git -C references/emutos log -1` selbst bestaetigt), Zielprofil
Falcon030/TOS4, `make 512`. `CONF_WITH_COLOUR_ICONS` ist dort 1, spielt fuer
diesen Befund aber keine Rolle: `rs_readit`s Kurzleseabbruch steht **ausserhalb**
jeder `#if`-Klammer.

## Der Befund, selbst nachgeprueft

### EmuTOS (`aes/gemrslib.c:775-783`)

```c
rs_hdr = (RSHDR *)dos_alloc_anyram(rslsize);
if (!rs_hdr)
    return FALSE;
if (dos_lseek(fd, 0, 0x0L) < 0L)
    return FALSE;
if (dos_read(fd, rslsize, rs_hdr) != rslsize)
    return FALSE;           /* error or short read */
```

Erst **danach** laufen `fix_trindex`, `fix_cicons`, `fix_tedinfo_std` und die
fuenf `fix_nptrs`-Laeufe (`:790-805`), die durch jeden deklarierten Offset
**schreiben**. Eine kurz gelesene oder zu grosse Datei erreicht sie nie.

### NewTOS (vorher)

* `tos-api/src/bdos/fs/mod.rs:142` — `let want = (entry.size as usize).min(out.len());`
  kuerzt **still** auf die Puffergroesse und meldet nur, wie viel kopiert wurde.
* `tos-api/src/aes/resource.rs`, `rsrc_load` — prueft danach ausschliesslich
  `len < size_of::<NrscHeader>()`, **nie** `len` gegen `h.total_size`.
* `relocate_blob_core` / `relocate_blob_core_ilp32` — pruefen nur die Magic und
  laufen dann ueber `h.n_objects` Saetze, mit `o.ob_spec += tb` und den
  TEDINFO-/ICONBLK-/BITBLK-Zeigern hinter dem Gelesenen.

### Selbst getrieben, nicht uebernommen

1. Der SIGBUS des Berichts **reproduziert** (halbiertes `gem_fsel.nrsc`,
   2409 von 4819 Byte, passgenauer Puffer):
   `process didn't exit successfully: … (signal: 10, SIGBUS: access to undefined memory)`.
2. Mit Waechterbereich hinter dem Blob **quantifiziert**: der Lauf schreibt
   `+0..=+515` hinter das Blobende — **516 Byte Fremdspeicher**.

## Was geaendert wurde

Alles in `tos-api/src/aes/resource.rs`, ausser den drei Aufrufstellen.

| Stelle | Aenderung |
|---|---|
| `header_is_sound` (neu) | Prueft vor jedem Schreibzugriff: Magic, `len >= size_of::<NrscHeader>()`, **`total_size <= len`**, `total_size >= Header`, und dass jede deklarierte Tabelle (Baumindex, OBJECTs, ab v3 frstr/frimg, ab v5 TEDINFO/ICONBLK/BITBLK) vollstaendig in `total_size` liegt. Satzbreiten nach Aufrufer-ABI. |
| `record_fits` (neu) | Bindet jeden aus `ob_spec` **gefolgten** Substruktursatz an den Blob — EmuTOS braucht das nicht, weil es die vom Header abgegrenzten Arrays laeuft (`:799-805`), wir folgen dem Objektzeiger. |
| `relocate_blob_core`, `relocate_blob_core_ilp32` | Bekommen `len`, rufen `header_is_sound` statt der blossen Magic-Pruefung, und `record_fits` vor den drei Substrukturarmen (G_TEXT-Familie, G_IMAGE, G_ICON). |
| `relocate_and_register`, `relocate_blob` (beide `pub`) | Bekommen `len` = **tatsaechlich gelesene Bytes**. Die alte Sicherheitsbedingung ("mindestens `total_size` Bytes") war vom Aufrufer gar nicht einhaltbar — das *war* der Fehler. |
| `relocate_and_register_core` | Reicht `len` durch. |
| `rsrc_load` | Gibt `len` weiter (kommentiert, warum `len` die einzige ehrliche Aussage ueber vorhandene Bytes ist). |
| `load_into_user` | Statt des blossen Magic-Spaehers jetzt `header_is_sound` — eine aussichtslose Datei kostet keinen Pool-Block mehr. |
| `RSC_CAP` | `pub` + dokumentiert, damit der Test die Grenze nicht raten muss. |
| `tos-api/src/aes/fsel.rs` | `relocate_and_cache(buf, len)`; der `C:\GEMSYS\RSC\GEM.RSC`-Override lag mit nur **8 KiB** `GEM_CAP` im selben Fehler und wird jetzt abgelehnt (Rueckfall auf den eingebetteten Blob). |
| `kernel/src/selftest_objc.rs` | Uebergibt `buf.len()` (der Selbsttestblob fuellt seinen Puffer ganz). |

## Die zwei Auftragsfragen

**1. Laengenpruefung oder `RSC_CAP` mit?** Die Laengenpruefung reicht, und
`RSC_CAP` bleibt bei 64 KiB. EmuTOS' `dos_alloc_anyram` ist zwar unbegrenzt,
kennt aber **denselben Ausgang**: kein Speicher → `return FALSE` → `rsrc_load`
= 0 (`gemrslib.c:775-777`). Unsere feste Schranke verschiebt die Grenze nur
frueher; was nicht abweichen darf, ist die **Ehrlichkeit** der Absage. Eine
`.RSC` groesser als der Puffer wird jetzt abgelehnt statt still gekuerzt und
relokiert. **Anheben oder Dynamisieren ist eine Architekturaenderung und wird
hiermit vorgelegt, nicht miterledigt** — sie haengt an A41 (farbige Icons
brauchen mehr Platz) und betrifft ausserdem `GEM_CAP` (8 KiB) im
Fileselector-Override.

**2. Absage in `read_file` oder beim Aufrufer?** **Beim Aufrufer**, und zwar
im Relokationskern, wo die Schreibzugriffe stattfinden. `read_file` hat vier
weitere Leser, und **alle vier brauchen das Kuerzen**:

| Aufrufer | Puffer | Warum Kuerzen richtig ist |
|---|---|---|
| `aes/shell.rs:805,815,830` (`sh_find`, 3×) | kleiner `probe` | Reine Existenzprobe; eine Absage haette jede Datei groesser als die Probe unauffindbar gemacht — also gerade den B1-Pfad zerstoert, der frisch daneben liegt. |
| `aes/init.rs:270` (`process_inf1`) | `infbuf[..INF_SIZE]` | EmuTOS liest hier **absichtlich** nur einen Praefix: `dos_read(fd, IOBUFSIZE, infbuf)`, `n < 0 → n = 0` (`aes/geminit.c:771-773`). |
| `kernel/src/keymap.rs:313` (`KEYBOARD.KBD`) | `[u8; 1024]` | `load_kbd(&buf[..n])` validiert selbst; ein groesseres Layout faellt dort durch. |
| `kernel/src/keymap.rs:320` (`NEWDESK.INF`) | `[u8; 4096]` | Zeilensuche `#K` in einem Praefix, wie EmuTOS. |
| `aes/fsel.rs:173` (`GEM.RSC`) | `[u8; GEM_CAP]` | **Derselbe Fehler wie A38** — jetzt ueber `relocate_blob(buf, len)` mitgeschlossen. |

`read_file` bleibt daher unveraendert; jeder Aufrufer wurde einzeln gelesen,
nicht angenommen.

## Belege

### Hosttests

Neu: `tos-api/tests/aes_rsrc_short_blob.rs`, 6 Tests (die Datei existierte
nicht; vorher geprueft).

* Drei **end-to-end** ueber die echte AES-Dispatcher-Kette gegen ein FAT16-
  Abbild im Hostspeicher (`sh_find` → `read_file` → Relokation):
  `WHOLE.RSC` laedt (1), `HALF.RSC` wird abgelehnt (0), `HUGE.RSC`
  (`RSC_CAP + 4096`, Header sagt die Wahrheit) wird abgelehnt (0).
* Drei **direkt** mit 8-KiB-Waechterbereich hinter dem Blob: das halbierte
  Blob wird abgelehnt **und** weder dahinter noch darin geschrieben (Bytes
  danach identisch zum Eingang); das vollstaendige Blob relokiert und bleibt
  in sich; ein Header, dessen OBJECT-Array hinter dem Blob endet, wird
  abgelehnt.

Ohne den Fix (Pruefung von Hand abgeschaltet, von Hand zurueckgesetzt — kein
`git`): **4 von 6 rot**, die Waechtermeldung lautet
`the relocation wrote past the end of the blob, at +0..=+515`. Die zwei
Gegenproben (vollstaendige Resource) bleiben gruen — die Pruefung ist nicht zu
streng.

Angefasste bestehende Tests: `tos-api/tests/aes_rsrc_tedinfo_txtlen.rs`,
drei Aufrufe `relocate_and_register(p)` → `(p, RSC.len())`. Rein mechanisch
wegen der Signatur; kein Test schrieb den alten Zustand fest, keiner wurde
entfernt oder `#[ignore]`d.

`cargo run -p xtask -- test`: **vorher 472 / 0 / 1 ignoriert — nachher 478 / 0 / 1.**

### Build (jeweils frisch gebaut, Quellen vorher angefasst)

```
cargo run -p xtask -- build --board rpi4       0 errors   tos-api 64 warn, kernel 94 warn
cargo run -p xtask -- build --board qemu-virt  0 errors   tos-api 64 warn, kernel 94 warn
```

`tos-api` hatte **vor** der ersten Aenderung ebenfalls 64 Warnungen (im ersten
Reproduktionslauf abgelesen) — Delta 0. Clippy: `tos-api` 82, `kernel` 125 je
Board; **keine** Fundstelle liegt in neuem Code (geprueft ueber die
`file:line`-Liste; in `resource.rs` sind es dieselben fuenf
`Aespb is more private`-Meldungen wie vorher, nur um den Einschub verschoben).

### Die echten Anwendungen (QEMU `raspi4b`, headless, `#Z`-Autostart)

Kernel: frischer `--board rpi4`-Bau. Abbild: Kopie von `~/.nt-audit/sd.img`
in `$SCRATCH`, `NEWDESK.INF` je Lauf auf die eine `#Z`-Zeile gesetzt.

* **QED** (`C:\APPS\QED\QED.NAP` + `QED.RSC`, 41 190 B): Menueleiste
  `qed Datei Bearbeiten Fenster Suchen Spezial Optionen Shell` — die Texte
  kommen aus `qed.rsc`, also hat `rsrc_load` geladen. Kein Alarm.
* **TeraDesk** (`DESKTOP.NAP` + `desktop.rsc`, 39 043 B): Menueleiste
  `Desk File View Window Options`, Laufwerks-/Drucker-/Papierkorbsymbole.
  Kein Alarm, keine Panik.

Beide `.RSC` tragen `total_size == Dateigroesse` (nachgemessen; `mkrsc.py`
sichert das mit `assert len(blob) == total`), beide liegen unter 64 KiB — die
Pruefung sperrt sie nicht aus.

Kein QEMU-Prozess blieb zurueck (`pgrep -f 'qemu-system-aarch64|Hatari'` nach
jedem Lauf leer). Kein physisches Medium beschrieben, nichts ausgehaengt.

## Nicht angefasst

`tos-api/src/bdos/fs/mod.rs` (`read_file`), `aes/window.rs`, `aes/pd.rs`,
`kernel/src/arch/aarch64/exceptions.rs`, `tools/mkrsc.py`, `tosdump/`,
`measurement-*`. Keine Datei zurueckgesetzt, kein `git checkout/clean/stash/
restore/reset`, nichts committet. Die B1-/A37-Arbeit in `resource.rs` steht
unveraendert; meine Aenderung setzt darauf auf.

## Offen

1. **Architekturvorlage:** `RSC_CAP` 64 KiB / `GEM_CAP` 8 KiB anheben oder den
   Puffer dynamisch machen (→ A41).
2. **Messschiene:** `aes-r3-wind-state` gegen `EMUTOS14-AES-r3.BIN` ist der
   unabhaengige Gegenlauf des Testers. Strukturell kann er sich nicht
   verschieben — die r3-Tabelle ruft **keinen** `rsrc_*`-Opcode (0 Treffer fuer
   `rsrc` in `measurements/measurement-2026-09-12-full-regression/aes-r3-wind-state/AES.LOG`).
3. **Nicht getrieben:** der ILP32-Zweig (`relocate_blob_core_ilp32`) hat
   dieselbe Pruefung, ist aus einem Hosttest aber nicht erreichbar (er haengt
   am EL0-Prozesspfad). Heute laeuft kein ILP32-Programm.
4. **Ausserhalb des Auftrags, nicht angefasst:** ein `ob_spec` **ohne**
   Substruktur (G_STRING & Co.), der hinter den Blob zeigt, wird weiter
   relokiert — das ergibt einen wilden Zeiger, der spaeter *gelesen* wird;
   EmuTOS prueft dort ebenfalls nichts. A38 schliesst die Schreibzugriffe.
