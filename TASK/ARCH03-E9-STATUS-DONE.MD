# ARCH03 E9 — TPA = größter freier Block (EmuTOS) — Quittung

**Status: UMGESETZT · in QEMU auf beiden Boards in allen drei Schutzmodi
belegt · Abnahme durch den Tester offen · Pi offen (nur auf Ansage)** ·
2026-10-05 · Zweig `desk-rebuild`, Basis HEAD `0d09343`, Baum vorher sauber,
nichts committet · Belege
[measurements/measurement-2026-10-05-e9/](../measurements/measurement-2026-10-05-e9/)
(`RUNINFO.TXT`, `checks/`, `serial/`, `screendumps/`, `method/`, `bin/`).

Offen bleibt eine Entscheidung, keine Arbeit: die S2ALL/TSRPAR-Layoutprüfung
(unten, Punkt 4).

## Leitentscheidung

E9 → EmuTOS-Verhalten: `Pexec` 0/3 (und 5/7) geben als TPA den **größten
freien Block** des gewählten Pools; Programme schrumpfen ihn beim Start selbst.

## Was getan wurde

| Bereich | Datei / Funktion |
|---|---|
| TPA-Vergabe | `tos-api/src/bdos/process/arena.rs`: `TPA_SIZE` entfernt; `alloc_tpa(prgflags, needed)` = EmuTOS `alloc_tpa`; `choose_pool` (ST/TT mit `PF_TTRAMLOAD` und Bits-31-28-Gleichstand, `TPASIZE_QUANTUM` 128 KiB); `Tpa.len` |
| BDOS | `tos-api/src/bdos/mem/traps.rs`: `largest_free_block` (erster Block maximaler Länge = `ffit(ffit(-1))`), `mxalloc_largest` (ganzer Block, Vorderschlupf bis zur Seitengrenze zurück, `memprot` nachgeführt) |
| Lader | `tos-api/src/bdos/process/loader.rs`: `image_end_of` (= EmuTOS `needed`; ersetzt `validate_segment`/`USER_STACK_MIN`), `p_hitpa = B + len`, `sp_el0 = p_hitpa`; Nullen wie `kpgmld` (`PF_FASTLOAD`: nur Bild inkl. BSS, sonst ganze TPA); `create_basepage` (5/7) mit `needed = sizeof(PD)`; `LoadedImage::image_len` |
| Pexec | `tos-api/src/bdos/process/pexec.rs`: `LoadedChild::image_len`, Doku |
| Accessories | `kernel/src/accs.rs` `load_one_acc`: nach dem Laden `Mshrink(PD, image_len)` wie EmuTOS `pgmld`; Fehlschlag → nicht gestartet, Blöcke frei |
| Tabellenvorrat | `kernel/src/lib.rs`: + 2 × Poolregionen (zwei noch nicht geschrumpfte TPAs); `mmu.rs` `ARENA_MAX_TABLES` 4096 → 8192 |
| Selbsttests | `kernel/src/selftest_d3.rs`, `selftest_d7.rs`: Erwartung aus der echten TPA-Länge |
| libc-Start | `sdk/libc/crt0.S` + `sdk/libc/newtos-arch/heap_init.c` (`__libc_crtinit`): mintlib `_crtinit` (`_stksize`/`_heapbase`/`_initial_stack`, MINKEEP/MINFREE, `STACKSIZE`, `Mshrink(PD, m)`, Stapel bei `PD + m − 64`, `_PgmSize`, malloc über GEMDOS `Malloc` via `_split_mem`) und `_acc_main` (Accessory: `Malloc(_stksize)`-Stapel); Startstapel in der reservierten Lücke `PD+0x100..0x1000` |
| Plain-Start | `sdk/crt0.S`: `Mshrink(PD, Bild + 64 KiB)`, Stapel oben drin; Accessory → `Malloc(64 KiB)`-Stapel |
| Doom | `sdk/apps/build-doom.sh` / `doomgeneric_gem.c`: `DOOM_ARGS` `"-mb","6"`, MEMORY-Notiz neu; `build-doom-m68k.sh` Kommentar |
| Doku | `docs/api/gemdos/process/4b-pexec.md` (Abschnitt „TPA“), `docs/COMPATIBILITY.md`, `docs/api/structures/basepage.md`, `docs/sdk/01/05/08`, `sdk/newtos.ld`, Plan E9-Zeile + „E9“ unter „Stufe 3 wie gebaut“ |
| Tests | `tos-api/tests/arch03_e9_tpa.rs` (6) |
| Sonden | `method/tests/e9mem.c` (libc, startet sich selbst als Kind), `method/tests/e9plain.c` (Plain-crt0) |

## Accessory-Erkennung (offengelegt)

mintlib erkennt ein Accessory an den m68k-Einsprungregistern (`a0` =
Basepage, `p_parent == 0`, `startup/crt0.S:46-62`). NewTOS übergibt die
Basepage auf **jedem** Weg in `x0`, und vom Kernel gestartete Programme
(Desktop/AUTO/#Z) haben ebenfalls `p_parent == 0` — beides trägt nicht.
Gebaut ist die Erkennung am Zustand, den `pgmld` hinterlässt: das
`Mshrink(PD, Programm + Stapel)` des Starts kann den Block nicht vergrößern
(`EGSBF`) → `_acc_main`-Pfad. Kein neues ABI, nur GEMDOS-Semantik; bei
Bedarf durch eine andere Kennung ersetzen (Leitfrage).

## Quellen

- EmuTOS (`references/emutos`, Pin `1a7484ab`): `bdos/proc.c:34`
  (`TPASIZE_QUANTUM`), `:210-237` (5/7, `alloc_tpa(…, sizeof(PD), &max)`
  `:220`), `:281-299` (env vor TPA, `needed` `:291`), `:364-385`
  (`init_pd_fields`, `p_hitpa = p + max` `:374`), `:460-502` (`alloc_tpa`:
  `ffit(-1L)` `:466,476`, `PF_TTRAMLOAD` `:475`, Gleichstand `:481-484`,
  ganzer Block `:486,493`), `:544` (Stapel unter `p_hitpa`); `bdos/kpgmld.c:212-221`
  (`PF_FASTLOAD`); `util/gemdos.c:29-47` (`pgmld`: `Pexec(PE_LOAD)` +
  `Mshrink(tlen+dlen+blen+0x100)`); `aes/geminit.c:251-255`; `aes/gemasm.S:73-120`
  (`gotopgm`: `a0` = Basepage, kein Stapel).
- mintlib 0.60.1 (`sdk/libc/mintlib`): `mintlib/crtinit.c:6-25,112-149,154-267`
  (`_stksize`-Tabelle, `_acc_main`, `_crtinit`, Mshrink-Idiom), `mintlib/stksiz.h`
  (MINFREE 8 KiB, MINKEEP 64 KiB), `stksiz.c:18`, `heapbase.c:17`,
  `inistack.c:8`, `globals.c:37-43`, `unix/sbrk.c:54-73`, `mintlib/malloc.c:64-95`
  (Hunks 8–32 KiB über `Malloc`), `startup/crt0.S:46-76`.

## Prüfungen (Einzelheiten `RUNINFO.TXT`)

| | Basis 3c | E9 (e1 und e2) |
|---|---|---|
| Hosttests | 1129/0/1 | **1135/0/1** (+6) |
| Release frisch nach `cargo clean -p kernel -p tos-api …`, tos-api/kernel, beide Boards | 64/94 | **64/94** |
| Clippy tos-api/kernel, rpi4 und qemu-virt | 76/125 | **76/125** |
| Driftprobe | 31 | **31** |
| SDK-Hosttests | 13/0/0 | **13/0/0** |

QEMU e1, ON/OFF/STRICT je volle Serie (alle rc=0): Desktop, TeraDesk, QED,
APITEST (106/0/5), Accessory, KBDVEC+EmuCON — Bilder **bytegleich mit 3c**
in jedem Modus; Doom VERIFIED mit **6-MiB-Zone** („600000 allocated for zone“)
in jedem Modus; **TeraDesk startet QED** (F1-Taste, neue Fahrt) in jedem
Modus, Bild = direkt gestartetes QED; **EmuCON fünf Programme hintereinander**
(HELLO2, E9PLAIN PASS, E9MEM PASS + Kind, HELLO2, OLDEXEC Fehler 66, exit);
E9MEM/E9PLAIN PASS; CRASHTST, UACC3A, MPROT3B, E10USR PASS (raspi4b + virt);
virt-Selbsttests B1/D3/D3-PEXEC/D4-D5/D6/D7 PASS; virt-Desktop; desk3a
ON = OFF = STRICT bytegleich (E/F/H gegen 3c nur Dateizeit/-zahl); PORT10
42/44 wie 3c. **S2ALL:** OFF PASS; ON FAIL nur TSR; STRICT FAIL P567-6 + TSR
(wie 3c, korrektes strict-Verhalten).

### TPA / Malloc(-1) (raspi4b)

| | vorher (3c) | nachher (E9) |
|---|---|---|
| TPA eines Programms (`p_hitpa − p_lowtpa`) | 0x00400000 | 0x3a3c1000 ON/STRICT, 0x3ad69000 OFF (= ganzer größter TT-Block) |
| behalten nach Start | 4 MiB | 0x13b30 (libc, E9MEM) / 0x11680 (plain) |
| `Malloc(-1)` direkt nach Start | Pool − 4 MiB | 0x3a3ad000 ON / 0x3ad554d0 OFF |
| Kind-TPA (nach Mshrink des Elternteils) | 4 MiB | 0x39daa000 |
| S2ALL Ende `Malloc(-1)` | ON 0x39b76000, OFF 0x3a566590 | ON 0x3a385000, OFF 0x3ad408a0, STRICT 0x3a3ae000 |

## Offen / zur Entscheidung

1. **S2ALL/TSRPAR „TPA beyond blkln back in the pool (< 64 KiB kept)“** —
   weiter FAIL unter ON/STRICT (ON: größter TT-Block vorher 0x3a39a000,
   nachher 0x3a385000, Δ 0x15000), PASS unter OFF. Ursache jetzt genau
   gemessen: TSRRES behält nach dem crtinit-`Mshrink` Bild + 64-KiB-Stapel,
   sein `Mxalloc(64)` liegt (first fit) direkt dahinter; `Ptermres(blkln)`
   gibt `[PD+0x3000, PD+0x13000)` = genau den Stapel frei — als eigenen
   freien Block. Der Speicher **ist** zurück im Pool; die Prüfung misst den
   größten Block (Layout-Stellvertreter). Unter EmuTOS + mintlib entstünde
   dasselbe Loch. Prüfung **nicht** geändert — Vorschlag: 3c-Option (a),
   semantische Prüfung „freier Block an `PD + blkln`“.
2. **Sonden angepasst** (Spec geändert, nicht abgeschwächt): `p567.c`
   prüfte wörtlich „`p_hitpa == PD + 4 MiB (E9)`“ → jetzt „= größter
   ST-Block“; `ownkid.c` prüfte „`Mshrink(PD, p_hitpa − p_lowtpa)` als
   Mitbesitzer“ — nach dem Start-`Mshrink` ein Vergrößern (`EGSBF`, auch auf
   EmuTOS) → jetzt `Mshrink(PD, _PgmSize)`; geprüft bleibt die
   Besitzerregel. Beide nur in `measurements/…-e9/method/tests/`.
3. Accessory-Erkennung über `EGSBF` (oben) — Leitfrage, ob eine andere
   Kennung gewünscht ist.
4. GEMDOS-`Malloc`-Hunks der libc verbrauchen MDs (256 systemweit, EmuTOS
   ähnlich begrenzt); in allen Läufen keine Erschöpfung, aber bei großen
   Programmen mit vielen kleinen `malloc` beobachten.

## Für die Pi-Karte (nur auf Ansage)

Alle mit dem SDK gebauten Programme müssen ersetzt werden — ein altes
Accessory (alter crt0) läuft nach dem neuen Lader-`Mshrink` mit seinem Stapel
in freiem Speicher; alte Programme halten den ganzen Pool. Binärdateien
(SHA256 in `bin/SHA256SUMS`, `repo-binaries.sha256`):

| Karte | Quelle |
|---|---|
| `kernel8.img` | `measurements/measurement-2026-10-05-e9/bin/kernel8-e2.img` (`9d1a39b9…`) |
| `C:\Apps\QED\QED.NAP` (+ unverändert `qed.rsc`) | `sdk/apps/qed/build/QED.NAP` |
| `C:\Apps\TERADESK\DESKTOP.NAP` | `sdk/apps/teradesk/build/dist/DESKTOP.NAP` |
| `C:\DOOM\DOOM.NAP` | `sdk/apps/doom/build/DOOM.NAP` (6-MiB-Zone) |
| `C:\APITEST.NCP` | `sdk/apps/apitest/APITEST.NCP` |
| `C:\ACC\SYSACC.NAC` (und `C:\SYSACC2.NAC`, falls vorhanden) | `bin/SYSACC.NAC` |
| jedes weitere SDK-Programm auf der Karte (`SYSINFO.NCP`, `HELLO2.NCP`, `KBDVEC.NAP`, …) | gleichnamig in `bin/`; `TESTGEM.NAP`, `CHAINTST.NAP`, `CORE.NCP`/`COREVAL.NCP` unter `sdk/apps/…` |

Nicht neu gebaut: zView/HighWire (nicht im Baum gebaut), `BALL.NAP`
(extern, ET_EXEC, wird weiter mit 35 abgelehnt). `memprot=off`/`strict`
schaltet weiter über `cmdline.txt`.

## Beobachtungen außerhalb des Auftrags (unverändert)

- `libc.a` enthält `syscall__main.o` (mintlibs Host-Generator mit `main` und
  eigenem `_stksize`); bisher zieht der Linker `mintlib__stksiz.o`, aber ein
  doppeltes `_stksize` im Archiv ist ein Stolperstein.
- `free_accs` gibt TPA/Env der Accessories nicht frei (`release_slot` gibt
  keinen Speicher frei), anders als sein Kommentar sagt.
- `sdk/libc/crt0.S` ruft `main` und dann `Pterm` direkt, nicht `exit()` —
  `atexit`/stdio-Flush laufen nicht (vorbestehend).
