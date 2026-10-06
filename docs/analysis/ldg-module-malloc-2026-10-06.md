# LDG-Modul-Heap (Befund B2): Ursache, Referenzverhalten, Behebungsorte

Stand 2026-10-06 · Auditor · Basis `desk-rebuild` HEAD `2d8cc9f`, Baum unverändert (nur diese Datei und der Messlauf neu) · keine Karte/Pi/TT.
Belege: [measurements/measurement-2026-10-06-ldg-b2/RUNINFO.TXT](../../measurements/measurement-2026-10-06-ldg-b2/RUNINFO.TXT) (dieser Lauf), [measurements/measurement-2026-10-06-ldg-m3/](../../measurements/measurement-2026-10-06-ldg-m3/RUNINFO.TXT) (Erstbefund).
Kennzeichnung: **[gelesen]** = Quelltext in dieser Sitzung gelesen · **[gemessen]** = im QEMU-Lauf dieser Sitzung belegt · **[Schluss]** = gefolgert · **[offen]** = ungeklärt.

Abkürzungen: `ET/` = `references/emutos/` (Commit `1a7484ab`), `FM/` = `references/freemint/`, `ML/` = `sdk/libc/mintlib/` (mintlib 0.60.1, die Quelle unserer libc), `LDG/` = `Applications/ldg-src/ldg/`, `LS/` = `references/libshare/orig-0.1.1/libshare-0.1.1/`, `RUN/` = `measurements/measurement-2026-10-06-ldg-b2/`.

## 1. Kurzfassung

1. **Die Kette des Workers stimmt, jetzt mit Aufrufpfad belegt [gemessen].** Instrumentierter Scratch-Kernel (nur serielle Ausgaben), Lauf `memprot=on` und `memprot=off`:
   * Während seines `Pexec(4)`-Laufs ruft **POPUP.NLD** `Malloc(0x2000)` → `0x1ad9000`, `run` = Modul-PD `0x1abb000`; Aufrufpfad aus der Rahmenzeigerkette: `_start+0x44 → exit+0x4c → fcloseall+0x64 → fclose+0x54 → __flshfp+0x9c → init_stream+0xe0 → malloc+0xd8 → sbrk` (`RUN/checks/b2-on-callchains.txt`, `RUN/serial/b2-on-serial.log:75`).
   * Beim `Pterm` des Moduls steht `_mchunk_free_list.next` = `0x1ad9000`; nach dem Pterm-Durchlauf ist der Block **nicht mehr belegt** (`b2-on-serial.log:76-77`). Kein `Mfree` auf diesen Block dazwischen.
   * Später ruft der Client `__popup → __get_bkgr → malloc`; `malloc+0x44` liest `q->size` des ersten Listenglieds `0x1ad9000` → Bus Error „free memory, read“, `far=0x1ad9010`, `x10=0x1ad9000`, `x8=&_mchunk_free_list`, `x9=VAL_BORDER` (`b2-on-serial.log:95-105`; Disassembly `malloc` 0x2c40/0x2c74).
   * **EX_LIB.NLD hat dieselbe hängende Freiliste** (`Malloc` `0x1aba000` aus demselben Pfad, nach Pterm frei, `free_list.next` = `0x1aba000`, `b2-on-serial.log:62-64`); es bleibt nur deshalb heil, weil es nie wieder `malloc` ruft.
   * **`memprot=off` (der EmuTOS-ähnliche Fall ohne Schutz):** der Client bekommt beim nächsten `malloc` des Moduls über `sbrk → Malloc(0x6000)` **genau den Block zurück, den die Modul-Freiliste noch referenziert** (`0x112b6c0` vorher Modul-Hunk, `b2-off-serial.log:75-76`, danach Client-`Malloc` → `0x112b6c0`, `:95`). Das Pop-up erscheint nie, das Programm steht (`RUN/screendumps/b2-off-timeout.png`). Mechanismus: Abschnitt 2.3.
2. **Der Kernel verhält sich wie EmuTOS.** EmuTOS stempelt jeden `Malloc` mit `run` (`ET/bdos/iumem.c:134`), setzt `run = p` beim Start eines `Pexec(4)`-Kinds (`ET/bdos/proc.c:575`), lässt bei `PE_GO` die Besitzverhältnisse von TPA/Umgebung unverändert (`:244-248`) und gibt beim `Pterm` alles frei, was dem Kind gehört (`:92-101,136-140`). NewTOS tut dasselbe (Abschnitt 4). **Unter EmuTOS + mintlib entsteht dieselbe hängende Freiliste**, nur ohne MMU-Fehler: still benutzter Freispeicher bzw. — wie der `memprot=off`-Lauf zeigt — Doppelvergabe desselben Blocks und eine zyklische Freiliste [Schluss, Abschnitt 2.3/3.1].
3. **FreeMiNT vermeidet es durch Thread-Semantik:** `Pexec(4)` = `fork_proc(FORK_SHAREVM|…)` (`FM/sys/k_exec.c:224-226,421`), das Kind teilt den `memspace` des Aufrufers (`FM/sys/k_fork.c:143-144`), `free_mem` beim Kind-Ende gibt nichts frei, solange der Aufrufer den Raum noch hält (`FM/sys/proc_help.c:228-229`).
4. **LDG selbst verbietet `malloc` in Modulen** und verweist auf `ldg_Malloc`/MEM.LDG (`LDG/guides/src/ldg.u:1047-1051`, `LDG/guides/src/mem.stg:44-52`); die LDG-Demo POPUP verletzt die eigene Regel (`LDG/demo/libs/popup.c:74`, Link `-lgem -lldg` ohne libshare, `popup.mint:8-10`). **libshare 0.1.1 ist das praktische Gegenmittel für mintlib-Module** und deckt genau diesen Fall: sein erster `malloc`-Block ist ein statisches 16-KiB-Feld im Modul-BSS (überlebt den `Pterm`, weil das TPA dem Client gehört), weiterer Speicher kommt über `ldg_Malloc` im Kontext des Rufers (`LS/malloc.c:47-133`, `LS/sbrk.c:60-89`).
5. **Empfehlung:** libshare-Allokator (malloc/free/realloc/calloc/sbrk) ins SDK portieren und in jedes `.NLD` linken (wie zViews `-lshare -lldg`). Kein Kernel- und kein libc-Eingriff. Begründung und Alternativen Abschnitt 6.

## 2. Verifizierte Ursachenkette

### 2.1 Statisch [gelesen]

| Schritt | Ort |
|---|---|
| Modul-Start: `crt0` → `__libc_crtinit` → `main` → `exit(main())` | `sdk/libc/crt0.S:57,89,91` |
| `__libc_crtinit`: Default `_stksize = MINKEEP` → `_split_mem = 1`, `_heapbase = NULL` → `malloc` holt Hunks per GEMDOS `Malloc` | `sdk/libc/newtos-arch/heap_init.c:167,174,200`; Vorbild `ML/mintlib/crtinit.c:180-215,227-247` |
| `main` von POPUP: `ldg_init(Ldg)` → `return 0` | `LDG/demo/libs/popup.c:310-316` |
| `ldg_init` schreibt nur `&lib` in die Client-Variable (`OFFSETLDG`), kehrt mit 0 zurück, kein `Ptermres` | `LDG/src/devel/ldglib.c:97-142` |
| `exit` → `__fcloseall` | `ML/stdlib/exit.c:45-57` |
| `__fcloseall` → `fclose` für jeden gültigen Strom (auch `stdout`) | `ML/stdio/fcloseall.c:27-36` |
| `fclose` eines Schreibstroms → `__flshfp(stream, EOF)`, danach `free(stream->__buffer)` | `ML/stdio/fclose.c:37-44` |
| `__flshfp` → `init_stream` → `malloc(BUFSIZ)` (1024), wenn noch kein Puffer | `ML/stdio/internals.c:536-539,55-80`; `BUFSIZ` `ML/include/stdio.h:243` |
| `malloc`: leere Freiliste → Hunk `MINHUNK` (8192, seitengerundet) per `__sbrk` → `Malloc` (weil `_split_mem`) | `ML/mintlib/malloc.c:62-90`; `ML/unix/sbrk.c:49-60` |
| Hunk wird mit Grenz-Chunk (`VAL_BORDER`) am Anfang in die **statische** Freiliste `_mchunk_free_list` gehängt; `free` legt den Puffer dorthin zurück | `ML/mintlib/malloc.c:93-108`; `ML/mintlib/malloc_int.h:5-19` |
| Callgraph POPUP.NLD: `malloc`-Rufer sind nur `__get_bkgr`, `init_stream` (+ ein unbenanntes Verzeichnis-Hilfsstück); `init_stream` ← `__flshfp` ← `fclose` ← `fcloseall` ← `exit` ← `_start`; `main`/`ldg_init` rufen kein `malloc` | `llvm-objdump -d` von `measurements/measurement-2026-10-06-ldg-m3/bin/LDGDEMO/LIBS/POPUP.NLD` (sha256 `7ef22e77…`) |

### 2.2 Dynamisch [gemessen]

Scratch-Kernel = HEAD + M3-Eingabeinjektor + `RUN/method/apply-b2-instr.py` (Diff `RUN/method/b2-instr-exceptions.diff`: nur `kprintln` bei `Malloc/Mxalloc/Mfree/Mshrink/Pexec/Pterm` + Rahmenzeigerkette über `uaccess::get_user_u64`). Module unverändert aus dem M3-Lauf.

`memprot=on` (`RUN/serial/b2-on-serial.log`):

```
71 [b2] Pexec mode=0x4 x1=0x0 x2=0x1abb000 run=0x1a40000          <- Client startet POPUP
74 [b2] fn=0x4a ... a1=0x1abb000 a2=0x1d6c0 -> 0x0 run=0x1abb000    <- Mshrink des Moduls
75 [b2] fn=0x48 a0=0x2000 ... -> 0x1ad9000 run=0x1abb000 ...        <- Malloc des Moduls (exit-Pfad)
76 [b2] Pterm func=0x4c run=0x1abb000 free_list.next@POPUP=0x1ad9000
77 [b2]   after Pterm of 0x1abb000: block 0x1ad9000 still allocated = false
95 [crash] slot 0 Bus Error ... pc=0x1abdc74 far=Some(28151824)     <- 0x1ad9010
```

Symbolisiert (`RUN/checks/b2-on-callchains.txt`): `POPUP.NLD:sbrk+0x40` ← `malloc+0xd8` ← `init_stream+0xe0` ← `__flshfp+0x9c` ← `fclose+0x54` ← `fcloseall+0x64` ← `exit+0x4c` ← `_start+0x44`. Identisch für EX_LIB (`:62-64`).

Fehlerstelle: `pc 0x1abdc74` = POPUP-Basis `0x1abb000` + `0x2c74` = `malloc+0x44` `ldr x19,[x10,#0x10]` (`q->size`); `x10` stammt aus `ldr x10,[x8,#0x3d8]` = `_mchunk_free_list.next` (`_mchunk_free_list` = Link-VA `0x83d0`). Register im Crash: `x8=0x1ac33d0` (= `0x1abb000+0x83d0`), `x10=0x1ad9000`, `x9=0xb04d0abc` (`VAL_BORDER`) [gemessen + Disassembly].

### 2.3 Der Hänger unter `memprot=off` [gemessen + Schluss]

`RUN/serial/b2-off-serial.log`: Modul-Hunk `0x112b6c0` (`:75`), beim Pterm Kopf der Freiliste (`:76`); später `__get_bkgr → malloc(0x46c0)` im Client-Kontext → `sbrk(0x6000)` → GEMDOS `Malloc` liefert **`0x112b6c0`** (`:95`, Kette `RUN/checks/b2-off-callchains.txt`). Danach keine AES-Aufrufe mehr, Pop-up nie gezeichnet (`RUN/screendumps/b2-off-timeout.png`).

[Schluss, aus `ML/mintlib/malloc.c:54-108`]: Die Suche überspringt den alten Grenz-Chunk H und den zu kleinen Rest H+32, bekommt `q = H` von `sbrk`, sucht die Einfügestelle (`q > p->next` ist bei Gleichheit falsch → Einfügen am Kopf), schreibt den neuen Grenz-Chunk über den alten und setzt `q->next->next = p->next` = H — die Liste wird **zyklisch** (H → H+32 → H). Die Anforderung selbst gelingt noch; jeder weitere Listendurchlauf, der nichts Passendes findet, läuft endlos. Wo genau POPUP danach hängt, ist nicht instruktionsgenau verfolgt [offen]; der Zyklus ist aus dem Quelltext abgeleitet, nicht im Speicher abgelesen.

## 3. Referenzverhalten

### 3.1 EmuTOS + mintlib + LDG

* **Pexec(3) → TPA gehört dem Aufrufer**: `owner = (flag == PE_LOADGO) ? p : run; set_owner(p, owner); set_owner(env_ptr, owner)` (`ET/bdos/proc.c:302-307`) [gelesen].
* **Pexec(4) ändert keinen Besitz** (nur `PE_GOTHENFREE` übergibt TPA/Umgebung ans Kind, `:238-243`), `proc_go` setzt `p->p_parent = run` und `run = p` (`:541,575`) [gelesen].
* **Malloc des Kinds gehört dem Kind**: `ffit` → `q->m_own = run` (`ET/bdos/iumem.c:131-134`); `xmalloc` → `xmxalloc` (`ET/bdos/umem.c:115-128`) [gelesen].
* **Pterm** (`xterm`, `:601-618`) → `ixterm` → `free_all_owned(r, &pmd)` und `&pmdalt` (`:136-140`, Definition `:92-101`): jeder Block mit `m_own == Kind` wird frei; das TPA (Besitzer Client) bleibt [gelesen]. Freigabe erst durch `ldg_unload` → `Mfree(p_env)`, `Mfree(baspag)` (`LDG/src/devel/ldgload.c:201-213`) [gelesen].
* **mintlib-Start eines Pexec(3/4)-Programms**: `_crtinit` (`ML/mintlib/crtinit.c:154-257`): `_stksize` Default `MINKEEP` (`ML/mintlib/stksiz.c:18`) → `_split_mem = 1` (`:186-188`), `Mshrink` auf Image + 64 KiB Stack (`:212-247`), dann `_main` → `exit(main(…))` (`ML/mintlib/main.c:303`). `__libc_main` stellt `stdout` auf `_IOLBF` mit `NULL`-Puffer (`ML/mintlib/main.c:191-193`) — der Puffer entsteht also auch dort erst lazy per `malloc` [gelesen].
* **Folge [Schluss]:** Ein mit mintlib 0.60.1 gebautes LDG-Modul hinterlässt unter EmuTOS dieselbe Freiliste in einen beim `Pterm` freigegebenen Hunk. EmuTOS' `ffit` ist first-fit über die adresssortierte Freiliste (`ET/bdos/iumem.c:88-99`): der freigegebene Hunk direkt über dem geschrumpften Modul-TPA ist der niedrigste passende Freibereich und geht beim nächsten passenden `Malloc` an wen auch immer — genau das zeigt der `memprot=off`-Lauf mit dem NewTOS-Pool. Unter EmuTOS also **kein Absturz, sondern stiller Gebrauch freigegebenen Speichers, Doppelvergabe, ggf. Zyklus/Hänger**. Nicht auf EmuTOS/Hatari reproduziert [offen].
* Ob ältere mintlib-Stände (gcc-2.81-Ära, mit denen LDG-Demos ursprünglich gebaut wurden) oder Pure C beim `exit` ebenfalls einen stdio-Puffer per `malloc` holen, ist nicht gelesen [offen]. Pure-C-Bau der Demo: `LDG/demo/libs/popup.prj` (`pcstdlib.lib`).
* **LDG-Regel:** „Memory allocation with the standard C function or with the Malloc() GEMDOS function and exchange of these data with the client are forbiden … use the functions ldg_Malloc(), ldg_Realloc(), ldg_Calloc() and ldg_Free()“ (`LDG/guides/src/ldg.u:1047-1051`); MEM.LDG-Doku: „LDG-libraries cannot use directly the C malloc() function“ (`LDG/guides/src/mem.stg:44-52`) [gelesen].
* **`ldg_Malloc`** (`LDG/src/devel/ldgutil.c:120-159`): `Sversion` ≥ 0.19 → `Mxalloc(size, 3)`, mit `MiNT`/`MagX`-Cookie `Mxalloc(size, 3+8+32)` (32 = global); sonst `Malloc`. **Kein Besitz „im Namen“ des Clients** — der Besitzer ist schlicht `run` zum Aufrufzeitpunkt; ruft der Client die Modulfunktion, ist das der Client [gelesen + Schluss aus `ET/bdos/iumem.c:134`]. `ldg_Free` = `Mfree` (`:188-190`).

### 3.2 libshare 0.1.1

* Ersetzt `malloc/free/realloc/calloc/sbrk/chdir`; `libshare_dummy` zieht alle Objekte (`LS/libshare.c:21-30`); `libshare_init/exit` sind praktisch leer (`:36-58`) [gelesen].
* **`malloc`**: der erste Hunk ist **`static_mblock[16384]`** im BSS (`LS/malloc.c:50-52,120-126`), erst danach `sbrk` [gelesen].
* **`sbrk`**: ohne `_heapbase` → `ldg_Malloc(n)` (`LS/sbrk.c:68-75`) [gelesen].
* **Wirkung auf B2 [Schluss]:** Der exit-Puffer (1024 B) landet im statischen Block, der im Modul-TPA liegt (Besitzer Client) und den Modul-`Pterm` überlebt; die Freiliste zeigt danach nur in lebenden Speicher. Spätere Hunks kommen per `ldg_Malloc` im Kontext des Rufers (Client) und leben bis zu dessen Ende. **libshare ist damit genau das Gegenmittel für B2** — mit Grenze: Was ein Modul während seines eigenen Laufs (`main`/`exit`) über 16 KiB hinaus anfordert, geht wieder per `ldg_Malloc` mit `run` = Modul und hängt danach ebenso.
* Der Autorentext nennt als Zweck „make LDG compiled against mintlib shareable“ (`LS/README`); die Teilbarkeit (MiNT, mehrere Clients) ist der erklärte, der Pterm-Schutz der mitgelieferte Effekt [Schluss]. Die Aussage in `docs/analysis/libshare-2026-10-06.md` §4 („Zweck … unter NewTOS **nicht deckungsgleich**“) ist damit widerlegt: unter NewTOS (EmuTOS-Besitzmodell) ist der Zweck deckungsgleich.

### 3.3 FreeMiNT

* `Pexec` Modus 4: `mkwait = mkgo = 1; thread = (mode == 4)` (`FM/sys/k_exec.c:224-226`); Kind per `fork_proc(thread ? (FORK_SHAREVM | FORK_SHAREEXT) : FORK_SHAREEXT)` (`:421`); TPA/Umgebung zusätzlich an das Kind gehängt (`:519-520`) [gelesen].
* `FORK_SHAREVM` → `p2->p_mem = share_mem(p1)` (`FM/sys/k_fork.c:143-144`) [gelesen].
* Kind-Ende: `free_mem` → `if (--p_mem->links > 0) return;` (`FM/sys/proc_help.c:228-229`) — **nichts wird frei**, solange der Client lebt; `M_KEEP`-Regionen gehen sonst an `rootproc` (`:255-261`) [gelesen].
* **Folge [Schluss]:** Unter FreeMiNT tritt B2 nicht auf; die Modul-`Malloc`s des Ladelaufs gehören dem gemeinsamen Speicherraum und werden mit dem Client frei. Problem unter MiNT ist ein anderes (Teilen eines Moduls zwischen Clients bei Speicherschutz) — das adressieren `ldg_Malloc` mit Global-Flag und libshare.

## 4. NewTOS-Implementierung

| Teil | Ort | Befund |
|---|---|---|
| libc-Start | `sdk/libc/crt0.S:57,89,91`; `sdk/libc/newtos-arch/heap_init.c:135,167,174,200` | mintlib-`_crtinit`-Regeln, `_split_mem = 1`, `Mshrink` auf Image+64 KiB, dann `exit(main())` wie `ML/mintlib/main.c:303` [gelesen]. Kein Unterschied zum Original, der B2 erklärt |
| `Pexec(4)` | `tos-api/src/bdos/process/pexec.rs:352-422` | Kind wird **weiterer** Besitzer von TPA/Umgebung (`region::attach`, `:391`), Client bleibt Besitzer; Kind läuft im Speicherraum des Clients (`space_pd`, `:403-410`) — das betrifft nur **Rechte** (`tos-api/src/bdos/process/memprot.rs:30-35`), nicht den Besitz neuer Blöcke [gelesen] |
| `run` = Kind | `kernel/src/arch/aarch64/exceptions.rs:2052` (`set_current(pd_of(child))`) | wie `ET/bdos/proc.c:575` [gelesen] |
| Besitzstempel | `tos-api/src/bdos/mem/traps.rs:92-106` (`current_owner` = `run`) | wie `ET/bdos/iumem.c:134` [gelesen]; im Lauf: Modul-`Malloc` mit `run=0x1abb000` [gemessen] |
| Pterm-Freigabe | `tos-api/src/bdos/process/pterm.rs:207`; `tos-api/src/bdos/mem/free_all_owned.rs:41-50`; `tos-api/src/bdos/mem/region.rs:374-420` | gibt frei, was nur dem Sterbenden gehört — bei einem Besitzer = EmuTOS `free_all_owned`. Modul-TPA bleibt (Client hält es), Modul-Hunk wird frei [gelesen + gemessen `b2-on-serial.log:77`] |
| Speicherschutz | `memprot=on`: freigegebene Blöcke sind unzugänglich | macht den stillen EmuTOS-Fehler zum sofortigen Bus Error — **besser beobachtbar, nicht falsch** |

**Urteil Kernel:** EmuTOS-konform; kein Kernel-Defekt in B2. Der Defekt liegt in der Kombination „mintlib-`malloc` mit statischer Freiliste“ + „`exit` alloziert im eigenen Prozesskontext“ + „Modul lebt nach seinem `Pterm` weiter“ — derselbe latente Fehler wie auf dem Original.

## 5. Nebenbefunde zu bestehenden Dokumenten (nicht behoben)

* `docs/sdk/10-ldg-modules.md:97` („No libshare — Not needed by plain modules“): falsch für jedes Modul, das nach dem Laden `malloc` ruft (POPUP) — genau diese Module brauchen libshare.
* `docs/sdk/10-ldg-modules.md:102` („`EX_LIB.NLD` does not `malloc` and is unaffected“): EX_LIBs Freiliste hängt ebenso (`b2-on-serial.log:63-64`); „unaffected“ gilt nur, solange es nie `malloc` ruft.
* `docs/analysis/libshare-2026-10-06.md` §4 [Schluss] „nicht deckungsgleich … eigener Heap“: widerlegt (Abschnitt 3.2).
* `docs/analysis/ldg-after-arch03-2026-10-06.md` Zeile 117 („Modul-eigene `Malloc`s … werden frei — wie im Original … **ok**“): für den Kernel richtig, die Folge für die Modul-Freiliste fehlt.

## 6. Behebungsoptionen

| # | Ort | Was | Referenzgetreu? | Bewertung |
|---|---|---|---|---|
| **A** | **SDK: libshare-Port, in jedes `.NLD` gelinkt** (`sdk/ldg/build-ldg*.sh`) | libshare-Allokator (statischer 16-KiB-Erstblock, Wachstum über `ldg_Malloc` im Rufer-Kontext), Link-Reihenfolge vor `libc.a` | **Ja** — das ist das Werkzeug, das die Referenz-Toolchain für mintlib-LDGs benutzt (zView: `LIB = -lshare -lldg`, `docs/analysis/libshare-2026-10-06.md` §4); Verhalten auf EmuTOS, MiNT und NewTOS gleich | **Empfohlen.** Behebt B2 für POPUP und alle Codecs; M4 braucht libshare ohnehin. Grenzen/Arbeitspunkte: (1) Lizenz offen (`libshare-2026-10-06.md` §1 Nr. 5) — sonst Clean-Room mit identischer Semantik; (2) LP64-Prüfung (`struct mem_chunk`, `(long)q == -1`); (3) `posix_memalign` in `libc.a` ruft `__malloc` direkt (`llvm-nm`: `mintlib__posix_memalign.o U __malloc`), würde den mintlib-Allokator mitziehen — im Port mitersetzen oder ausschließen; (4) > 16 KiB im eigenen Modul-Lauf bleibt ein (referenzgleicher) Restfall, dokumentieren |
| B | libc: `_stksize < -1` / `_initial_stack` für Module (eigener Heap im TPA, `ML/mintlib/crtinit.c:203-206`) | Heap im Modul-TPA statt GEMDOS `Malloc` | mintlib-Mechanismus, aber **funktioniert nicht für Module**: `HeapAlloc` verweigert, sobald der aktuelle Stack unter `_heapbase + sz` liegt (`ML/unix/sbrk.c:30-46`) — Modulfunktionen laufen auf dem **Client**-Stack (hier `0x1a91780` < Modul-Heap `0x1ac…`), also schlägt jedes `malloc` aus Client-Kontext fehl [Schluss] | Verworfen |
| C | libc: `fclose`/`__flshfp` ohne Puffer-`malloc`, wenn nichts zu schreiben ist | Abweichung von mintlib-stdio (Overlay) | Nein (mintlib tut es) | Behebt nur den exit-Pfad; jedes Modul, das in `main` `malloc`t, bleibt kaputt; verändert alle Programme. Nicht empfohlen |
| D | libc-Start für Module: nach `main` `_exit` statt `exit` | überspringt stdio-Close | Nein (`ML/mintlib/main.c:303`) | Gleiche Lücke wie C, plus fehlende atexit/Flush-Semantik. Nicht empfohlen |
| E | libldg (`ldg_init`) | — | — | Kein Hebel: `ldg_init` läuft **vor** `exit`; der Fehler entsteht danach. Nur dokumentarisch (Modulautoren: `ldg_Malloc` statt `malloc`, `ldg.u:1047-1051`) |
| F | Kernel: `Pexec(4)`-Kind-Allokationen gehören dem Aufrufer (FreeMiNT `FORK_SHAREVM`, `FM/sys/proc_help.c:228-229`) | Besitzmodell ändern | FreeMiNT ja, **EmuTOS nein** (`ET/bdos/iumem.c:134`, `ET/bdos/proc.c:136-140`) | Beobachtbare Abweichung von EmuTOS für jedes `Pexec(3)+(4)`-Programm (Speicher des Kinds bliebe bis zum Ende des Elternteils belegt). Widerspricht „EmuTOS-Basis zuerst“; allenfalls Teil des späteren FreeMiNT-Ausbaus. Nicht jetzt |

**Empfehlung: A.** Gründe: (1) Es ist das Mittel, das die Referenzwelt für genau diese Bauart (mintlib-gebaute LDG-Module) bereitstellt, und es verhält sich auf EmuTOS, MiNT und NewTOS gleich; (2) kein Eingriff in Kernel oder libc, die beide referenzgetreu sind; (3) Besitzer der Modul-Hunks wird der Client — identisch zu dem, was mintlib-`malloc` im Client-Kontext ohnehin täte, also über die API nicht unterscheidbar; (4) M4 (zView-Codecs) braucht libshare sowieso. Das Hinzulinken in POPUP.NLD ist eine Toolchain-Maßnahme, die Quelle bleibt unverändert. Vor der Umsetzung: Lizenzentscheidung (Lead).

## 7. Was getrieben wurde, was offen ist

Getrieben [gemessen]: Ladepfad beider Module mit `Pexec 5/3/4`, Modul-`Malloc` im exit-Pfad mit vollständiger Rufkette, Pterm-Freigabe (Block danach nicht belegt), Freilisten-Kopf beim Pterm, Fehlzugriff (`memprot=on`), Blockwiedervergabe an den Client (`memprot=off`), Hänger ohne Pop-up.

Offen: (a) EmuTOS/Hatari-Gegenlauf mit einem mintlib-gebauten m68k-POPUP.LDG (würde die [Schluss]-Aussage 3.1 belegen); (b) instruktionsgenauer Ort des Hängers unter `memprot=off` (Zyklus aus Quelltext abgeleitet); (c) Verhalten älterer mintlib-Stände / Pure C beim `exit`; (d) MagiC nicht betrachtet; (e) libshare-Lizenz.
