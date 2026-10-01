# Welche der uebrigen 20 abweichenden mint/-Strukturen erreichen heute eine gebaute Anwendung?

Stand 2026-09-17, nach dem `_DISKINFO`-Fix. Groessen selbst nachgemessen
(`04-struct-sizes-both-toolchains.txt`, `char probe_T[sizeof(T)]` mit
m68k-atari-mint-gcc -m68000 und clang aarch64, identischer Kopfstuecktext).

Gebaute Anwendungen: TeraDesk (48 .c), QED, APITEST, GEMTEST, CHAINTEST,
CoreMark. Gebundene Bibliotheken: gemlib.a, libc.a, cflib.a. "Wird gelesen"
heisst: eine Uebersetzungseinheit, die im Bild landet, greift auf Felder zu.
Bei Bibliotheksobjekten ist zusaetzlich geprueft, ob der Linker das Archivglied
ueberhaupt zieht (`sdk/apps/{qed,teradesk}/build/why.txt`).

## WIRD GELESEN (3)

| Struktur | m68k -> a64 | Leser | Heute richtig? |
|---|---|---|---|
| `OSHEADER` | 48 -> 96 | TeraDesk `library/utility/gettos.c` (`get_tosversion`: `_sysbase->os_version`); APITEST T04 (gleiches Feld ueber rohen Offset) | **nur zufaellig.** `os_entry`/`os_version` sind zwei `unsigned short` auf 0 und 2 -- in beiden ABIs. Der Kern legt eine 48-Byte-Fassung nach `0x5C0` (`tos-api/src/bdos/process/loader.rs`, `OSHEADER_VA`). Der erste Zeiger ist `reseth` auf m68k+0x04, LP64+0x08; **ab dort** ist alles verschoben, nicht erst ab `os_beg` |
| `MFDB` | 20 -> 24 | TeraDesk `screen.c`, `resource.c`, `library/xdialog/xdutil.c`; QED ueber cflib `cicon.o`, `asciitab.o`, `backgrd.o` (alle drei im Link) | **ja.** Die Anwendungen sehen gemlibs `mt_gem.h`-Fassung (durch `__MFDB__` gegen `mint/linea.h` verriegelt, layoutgleich). `fd_addr` ist ein echter Zeiger, laesst sich also nicht auf 32 Bit pinnen; der Kern verzweigt deshalb nach ABI (`tos-api/src/vdi/raster.rs:109/117`) |
| `_KEYTAB` | 28 -> 56 | TeraDesk `library/xdialog/xdevent.c` (`((_KEYTAB *)Keytbl(-1,-1,-1))->unshift[scan]`); QED ueber cflib `nkccinit.o` (`nkc_init` kopiert `unshift`/`shift`/`caps`) | **ja.** `tos-api/src/xbios/keytbl.rs` schreibt jedes Zeigerfeld in der **Breite des Aufrufers** (`put_ptr`, `Abi::Lp64` -> 8 Byte) in den prozesseigenen KEYTBL-Bereich. Rest-Unterschied ohne Wirkung: Feld 7 heisst bei mintlib `altgr`, bei EmuTOS `dead`+`features`; gelesen werden nur 0..2 |

## WIRD NICHT GELESEN (17)

| Struktur | m68k -> a64 | Warum sie heute niemanden erreicht |
|---|---|---|
| `_MD` | 16 -> 32 | `Getmpb` ist implementiert, aber keine gebaute Anwendung benutzt das mint-typedef. APITEST erklaert eigens `MDX`/`MPBX` mit `sizeof == 24`, weil die Fassung des Kerns (ADR-023, `tos-types/src/memory.rs`) `{Zeiger 8, Long 4, Long 4, Zeiger 8}` = **24** ist -- weder m68k-16 noch LP64-32. Blosses Verbreitern wuerde hier **nicht** genuegen |
| `_MPB` | 12 -> 24 | wie `_MD`; drei Zeiger, LP64-24 passt zufaellig zur Kernfassung, aber niemand liest sie ueber das mint-typedef |
| `_KBDVECS` | 38 -> 80 | keine Anwendung und keine Bibliothek ruft `Kbdvbase`. (Getrennt davon latent: `tos-api/src/xbios/system.rs` gibt die **Kernadresse** von `KBDVECS` heraus -- EL0 kann sie gar nicht lesen -- und die Kernfassung ist 9x`usize` = 72) |
| `_IOREC` | 14 -> 24 | `Iorec` (XBIOS 0x0E) ist nicht implementiert (Dispatcher gibt die Funktionsnummer zurueck); niemand ruft es |
| `_MAPTAB` | 24 -> 48 | nur ueber `Bconmap`, das `0x2c` ("kein umlegbarer Port") liefert und nie eine Tabelle herausgibt; kein Aufrufer |
| `_BCONMAP` | 6 -> 16 | wie `_MAPTAB` |
| `_PBDEF` | 30 -> 48 | `Prtblk` ist nicht implementiert; kein Aufrufer |
| `_DSPBLOCK` | 10 -> 24 | kein DSP, keine `Dsp_*`-Aufrufe irgendwo im SDK |
| `META_INFO_1` | 16 -> 32 | MetaDOS ist nicht implementiert; kein `Metainit`-Aufrufer |
| `META_INFO_2` | 10 -> 24 | wie `META_INFO_1` |
| `HDINFO` | 84 -> 160 | erreichbar nur ueber `pun_ptr`@0x516; kein Aufrufer im SDK |
| `BBPB` | 76 -> 104 | Line-A. Einziger Leser ist mintlib `mintlib/linea.c`; das Archivglied wird von keinem Anwendungs-Link gezogen (in beiden `why.txt` nicht vorhanden) |
| `SDDB` | 14 -> 24 | wie `BBPB` |
| `__LINEA` | 122 -> 176 | wie `BBPB` (`mintlib/linea.c`, `mintlib/alglobal.c`); NewTOS hat kein Line-A |
| `__FONT` | 88 -> 112 | wie `__LINEA` |
| `MCF_COOKIE` | 30 -> 40 | ColdFire-Cookie; einziger Leser ist mintlib `unix/sysinfo.c`, nicht gezogen |
| `_CCONLINE` | 258 -> 257 | die einzige **Schrumpfung**, und sie ist reine m68k-Randauffuellung: `maxlen`+`actuallen`+`buffer[255]` = 257, m68k-gcc rundet auf gerade Groesse. Der Cconrs-Puffer wird byteweise adressiert, die Gesamtgroesse steht in `maxlen` -- 257 statt 258 aendert nichts. Ausserdem ruft niemand `Cconrs` |

Zusaetzlich, weil es zum reparierten Fall gehoert: mintlib `unix/statfs.c`
liest `_DISKINFO` (`b_secsiz * b_clsiz`, `b_total`, `b_free`). Das Archivglied
wird von keiner gebauten Anwendung gezogen, der Fix wirkt dort also nur latent.
