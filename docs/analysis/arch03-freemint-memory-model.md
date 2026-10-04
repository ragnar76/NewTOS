# ARCH03 Schritt 1 — Prozess- und Speichermodell: FreeMiNT, EmuTOS, NewTOS heute

Stand 2026-10-04 · Researcher-Auftrag zu `TASK/ARCH03.md` Schritt 1 · Nur Analyse, kein Code, keine Architekturentscheidung (das ist Schritt 2: `arch03-implementation-plan.md`).

Pins: FreeMiNT `f906416` (`references/freemint`), EmuTOS `1a7484ab` (`references/emutos`), NewTOS-Zweig `desk-rebuild`.

**Kennzeichnung.** **[gelesen]** = Quelle `file:line` in dieser Sitzung gelesen. **[Schluss]** = aus Gelesenem gefolgert (die Folgerung ist nicht selbst eine Quellzeile). **[Wissen]** = ARM-/Architekturwissen, das ich in dieser Sitzung **nicht** aus dem ARM ARM gelesen habe (es liegt nicht lokal vor, `references/arm-arm/` fehlt, und die Arm-Developer-Seiten liefern ohne JavaScript keinen Text) — vor Umsetzung Track B pruefen. **[Doku]** = Aussage aus einem Repo-Dokument (MEMORY.md, STATUS.md …), das ich gelesen, aber nicht gegen Emulator oder Hardware nachgemessen habe. **[offen]** = nicht geklaert.

Pfadkuerzel: `FM` = `references/freemint/sys`, `ET` = `references/emutos`, `NT` = `tos-api/src/bdos/process`, `KA` = `kernel/src/arch/aarch64`.

---

## 0. Kurzfassung (das Wichtigste vorweg)

1. **FreeMiNT hat genau einen Adressraum, Logisch = Physisch.** Jeder Prozess bekommt eine *eigene Seitentabelle*, die ueber **denselben** identitaetsgemappten Speicher liegt und je Seite nur die **Rechte** (`PROT_P/G/S/PR/I`) verschieden setzt (`FM/arch/mprot030.c:18`, `:76-80`). Zeiger sind prozessuebergreifend gueltig; ob man sie benutzen *darf*, entscheidet die Tabelle. **[gelesen]**
2. **Das ist die Voraussetzung fuer `Pexec 3/4/5/6/7`, LDG, SLB, Accessories, globalen `Mxalloc` und AES-Zeiger.** Alle diese Mechanismen reichen Zeiger von einem Prozess an einen anderen. NewTOS gibt jedem Prozess dagegen **VA 0 als Basepage** (`NT/loader.rs:170`) und bildet jeden Speicherblock mit **VA ≠ PA** in ein Fenster ab (`NT/usermem.rs:26`). Ein Zeiger gilt dort nur im Prozess, der ihn bekam. **[gelesen]**
3. **LDG laeuft auf FreeMiNT, weil `Pexec(4)` einen *Thread im Adressraum des Aufrufers* startet** (`FORK_SHAREVM`, `FM/k_exec.c:421`, `FM/proc_help.h:46`) — nicht wegen der Schutzrechte. Der Modul-Prozess schreibt per `OFFSETLDG` in eine Stack-Variable des Aufrufers (`Applications/ldg-src/ldg/src/devel/ldglib.c:134-137`, `ldgload.c:108`). Das setzt *denselben* Adressraum voraus, nicht nur dieselbe Adresszaehlung. **[gelesen]**
4. **FreeMiNTs Schutz ist standardmaessig weich.** Ein per `Pexec` erzeugtes Kind behaelt Zugriff auf die Regionen seines Elternprozesses, solange `KERN_MPFLAGS` Bit 0 (`MPF_STRICT`) nicht gesetzt ist; Default ist 0 (`FM/k_exec.c:799-806`, `FM/arch/mprot030.c:154`, `references/freemint/doc/examples/mint.cnf:132-137`). **[gelesen]**
5. **`memprot` ist an eine 68k-MMU gebunden und abschaltbar** (`no_mem_prot`, `FM/arch/mprot030.c:152`; 68000-Build: leere Stubs `FM/arch/mprot000.c:36-108`). Ohne Schutz bleiben Regionsverwaltung, Besitz, `Pexec`-Modi und Zeigergueltigkeit *unveraendert*; nur die Seitentabellen entfallen. Das ist der Hebel fuer eine Stufung: **zuerst gemeinsamer Adressraum, dann Rechte.** **[gelesen]**
6. **Eine Reihe von NewTOS-Annahmen „eigener Adressraum je Prozess“ ist fest verdrahtet** (Abschnitt C.9, 22 Stellen). Die harten Zwangspunkte fuer Schritt 2: (a) die Basepage traegt 32-Bit-Zeiger (`tos-types/src/basepage.rs:97-122`) und der Pool ist `i32`-adressiert (`tos-api/src/bdos/mem/init.rs:230`), (b) der Lader kann nur `ET_EXEC` bei fester Adresse (`NT/elf.rs:341`), (c) QUIRK-001 und ADR-010 verbieten auf der QEMU-Zielplattform bestimmte TTBR0/ASID/Aliasing-Muster (Abschnitt E.4). **[gelesen/Doku]**

---

## A. FreeMiNT (`references/freemint`, Pin `f906416`)

### A.1 Grundmodell: Regionen, zwei Karten, 8-KiB-Quantum

- **`MEMREGION`** beschreibt einen zusammenhaengenden Speicherbereich: `loc`, `len`, `links` (Anzahl Nutzer), `mflags`, `save`/`shadow` (fork), `next` (`FM/mint/mem.h:16-27`). **[gelesen]**
- **Zwei Karten** `core` (ST-RAM) und `alt` (TT-RAM) als einfach verkettete, adresssortierte Listen (`FM/memory.c:331-460`, `MMAP core = &_core_regions; MMAP alt = &_alt_regions;`). Eine dritte `swap` ist leer (`init_swap`, `FM/memory.c:459`). **[gelesen]**
- **Quantum.** Regionen werden auf `QUANTUM` gerundet; auf Maschinen mit MMU ist das 8 KiB (`FM/mint/mem.h:152`, `ROUND` in `:200-205`), auf 68000 ersatzweise 512 Byte (`:180`). **[gelesen]**
- **First-fit** mit Aufspaltung/Verschmelzen: `_get_region` (`FM/memory.c:686-799`), `free_region` (`:815-1000`). `get_region` liefert eine Region mit `links = 1` und ruft `mark_region(n, mode & PROT_PROTMODE, cmode)` (`:793`). **[gelesen]**
- **Der Prozess besitzt keinen Adressbereich, sondern eine Liste von Regionen**: `struct memspace { links; memflags; num_reg; mem[]; addr[]; page_table; pt_mem; txtsize; tp_ptr; tp_reg; base; }` (`FM/mint/proc.h:62-81`). `links` zaehlt Prozesse, die denselben `memspace` teilen (Threads). **[gelesen]**
- **Kernel-Speicher** liegt in denselben Karten: `init_mem` markiert alle belegten Regionen `PROT_S`, freie `PROT_I` (`FM/memory.c:145-160`); `rootproc` hat `memflags = F_PROT_S` (`FM/proc.c:138`). **[gelesen]**

### A.2 Besitz, `attach_region` / `detach_region`

- **Besitz = Eintrag in `p_mem->mem[]`.** `attach_region(p, reg)` sucht einen freien Slot, traegt `reg`/`reg->loc` ein, `reg->links++`, und **ruft `mark_proc_region(p->p_mem, reg, PROT_P, pid)`** (`FM/memory.c:512-552`, Aufruf `:539`). Die Region kann dadurch **mehrere Besitzer** haben. **[gelesen]**
- `detach_region` / `detach_region_by_addr` entfernen den Eintrag, `links--`; bei 0 `free_region`, sonst `mark_proc_region(…, PROT_I, …)` fuer *diesen* Prozess (`FM/memory.c:557-660`). **[gelesen]**
- `free_region` ruft `mark_region(reg, PROT_I, 0)` — freigegebener Speicher wird fuer **alle** Prozesse ungueltig (`FM/memory.c:932`). **[gelesen]**
- `alloc_region(map, size, mode)` = `get_region` + `attach_region` an `curproc` (`FM/memory.c:1242-1280`). **[gelesen]**
- `M_KEEP` (`FM/mint/mem.h:45`): Region wird beim Prozessende **nicht** freigegeben, sondern an `rootproc` gehaengt (`FM/proc_help.c:243-258`); `Mfree` darf sie dann von jedem freigeben (`FM/dosmem.c:365-398`, Kommentar des Autors an `:368-374`: „I don't understand that“). **[gelesen]**

### A.3 Schutzmodi `PROT_P/G/S/PR/I` — Bedeutung

Definition: `FM/mint/mem.h:120-137`. Wirkung in `FM/arch/mprot030.c` (`other_dt/other_s/other_wp`, `:400-402`; `mark_region`, `:617-690`; `mark_proc_region`, `:695-770`):

| Modus | `F_PROT_*` (Mxalloc/PRGFLAGS, `mem.h:121-125`) | Besitzer (Prozess haelt Region in `mem[]`) | Nicht-Besitzer | Quelle |
|---|---|---|---|---|
| `PROT_P` (0) | `0x00` „no read or write“ | voller Zugriff (`dt=PAGE, s=0, wp=0`) | **ungueltig** (`other_dt[0]=0`) | `mprot030.c:400-402,653-680` |
| `PROT_G` (1) | `0x10` „any access OK“ | voll | **voll fuer alle** (`dt=1`), unabhaengig vom Besitz („everybody gets the same flags“) | `mprot030.c:656-659` |
| `PROT_S` (2) | `0x20` „any super access OK“ | voll | gueltig, aber `s=1` (nur Supervisor) | `mprot030.c:400-402` |
| `PROT_PR` (3) | `0x30` „any read OK, no write“ | voll | gueltig, `wp=1` (nur lesen) | `mprot030.c:400-402` |
| `PROT_I` (4) | `0x40` „invalid page“ | **ungueltig** fuer alle (auch `F_OS_SPECIAL` nicht) | ungueltig | `mprot030.c:656-659` |

- Die **globale Tabelle** `global_mode_table[]` haelt ein Byte je Seite (`mprot030.c:188`, `:629`) — der „Soll-Modus“ der Seite; die Seitentabelle jedes Prozesses ist davon abgeleitet. **[gelesen]**
- `mark_region` geht **ueber alle Prozesse** (`proclist`), ueberspringt `ZOMBIE_Q` und `TSR_Q` (`:651`), und setzt je Prozess „Besitzer-“ oder „Nicht-Besitzer-Rechte“ (`mprot030.c:645-690`). **[gelesen]**
- **`F_OS_SPECIAL`** (`mem.h:107`): der Prozess erhaelt Besitzerrechte auf **allen** Speicher (`mem_prot_special`, `mprot030.c:1122-1160`), und seine eigenen `PROT_P`-Regionen werden fuer alle anderen auf `PROT_S` gesetzt. Gesetzt fuer AES/Desktop (`GEM_memflags = … | F_PROT_S | F_OS_SPECIAL`, `FM/init.c:220`, Pexec(7) mit diesen Flags `:1237,1260`); Setzen zur Laufzeit nur `root` im Secure-Mode (`FM/procfs.c:1042-1054`). **[gelesen]**
- **`PROT_NOCHANGE`/„Modus eines vorhandenen Blocks aendern“** (`Mxalloc`-Bit 0x08) ist **auskommentiert** (`FM/dosmem.c:226-236` `# if 0`). Es gibt keinen Syscall, der den Modus einer bestehenden Region aendert (ausser `Ptermres`, A.10, und `Pexec`-Umgebung, A.8). **[gelesen]**
- **Vorrang:** Standardmodus einer Allokation ohne `F_PROTMODE`-Bits = die `F_PROTMODE`-Bits der **PRGFLAGS des aufrufenden Programms**; mit Bits: `(protmode >> 4) - 1` (`FM/dosmem.c:209-221`); `> PROT_MAX_MODE` → `PROT_P`. **[gelesen]**
- **VDI-Hack:** ein `Mxalloc` ohne Schutzbits aus dem ROM (`pc` in `0x00e00000..0x00efffff`) oder innerhalb des VDI wird auf `F_PROT_PR | F_KEEP` gesetzt (`FM/dosmem.c:180-204`) — Beleg, dass Atari-Systemcode mit Zeigerrueckgabe an Anwender Rechte brauchte. **[gelesen]**

### A.4 Seitentabellen je Prozess: Aufbau und Umschaltung (68030/68040)

**Aufbau (68030, `FM/arch/mprot030.c`):**
- Layout 4+4+4+7 Index-Bits, 8-KiB-Seite (`:38-46`, `:101-115`); 1 Tabelle je Prozess, **vollstaendig eigene Kopie** der oberen Ebenen *und* der Blattebene fuer alles RAM (`:76-82`); Groesse `page_table_size` ≈ 1 KiB je MiB RAM plus Kopf (`:229-231`, Berechnung `:294-302`). **[gelesen]**
- `init_page_table(proc, p_mem)` fuellt die Tabelle fuer einen neuen Prozess **als waere er Nicht-Besitzer jeder Seite** (`proto_page_type[global_mode_table[g]]`, `:208-216,915,976`), baut ROM-/I/O-Bereiche als „global“ ein (`:927-939`) und markiert **danach** alle Regionen, die er besitzt (`p_mem->mem[]` und `tp_reg`), als Besitzer (`mark_pages(…,1,0,0)`, `:1056-1067`). **[gelesen]**
- Der Anfang des physischen Speichers `0..membot` ist `S` (nur Supervisor); `membot..memtop` (TPA) ist `P`; Screen `G`; ROM `G`; I/O `G` (Hardware erledigt `S`) (`:24-36`). **[gelesen]**
- Seitentabellenspeicher selbst ist eine `PROT_S`-Region aus `core`/`alt` (`FM/proc_help.c:52-86`). **[gelesen]**

**Umschaltung beim Kontextwechsel:**
- `change_context` (`FM/arch/context.S:446-486`): `pflusha` (ganze ATC leeren), danach `pmove C_CRP(a0),%crp` + `pmove C_TC(a0),%tc` (68030) beziehungsweise `movec d0,urp` + `movec d0,srp` (68040/060, `:478-483`). Beim Speichern liest `save_context` den CRP aus (`:86-90,231-235`). **[gelesen]**
- **Kein Adressraum-Tag**, keine ASIDs: die 68k-MMUs haben keins; jeder Wechsel ist `pflusha` + Wurzelzeiger. **[Schluss]** (Es gibt in `FM/arch/context.S` keinen zweiten Pfad.)
- Cache-Behandlung: nach `mark_pages` `flush_mmu()` und `cpush(start,len)` (`mprot030.c:572-580`); Kommentar: 020/030 haben logische Caches und muessen bei MMU-Kontextwechsel geleert werden, 040 hat physische (`:577-582`). **[gelesen]**
- **Der Kernel laeuft immer unter der Seitentabelle des gerade aktiven Prozesses** (`mprot030.c:219-222`, Kommentar zu `init_tables`). Greift Kernelcode auf Speicher eines *anderen* Prozesses zu, nutzt er `prot_temp(loc,len,-1)` (Seiten voruebergehend fuer den aktuellen Prozess `G` machen, `:775-805`) und stellt danach den Cookie wieder her (z. B. `FM/slb.c:381-386`, `FM/memory.c:853,872`, `FM/procfs.c:752,816`). **[gelesen]**
- **040:** ein Seitentabellenbaum wird beim Start der Firmware **kopiert** (nicht neu gebaut), weil Falcon/Hades/Milan unterschiedliche Layouts haben; Template-Baeume je Modus beschleunigen `mark_pages` (`FM/arch/mprot040.c:1-120`, besonders Kopf „History 99/08/17“, „walk_or_copy_tree“); 4-KiB-Seiten sind moeglich (zwei Seiten je 8-KiB-Chunk). **[gelesen]**

### A.5 Was bei einer Schutzverletzung passiert

- 68k-Bus-Error → `sigbus()` (`FM/arch/sig_mach.c:493-502`): ist die Behandlung `SIG_DFL`, ruft es **`report_buserr()`**; danach `exception(SIGBUS)` (`SA_RESETHAND` + `raise`). Ohne Handler wird der Prozess beendet. **[gelesen]**
- `report_buserr` (`FM/arch/mprot.x:89-150`): liest `global_mode_table[addr>>13]`; ist die Seite `PROT_G` → „hardware“ (kein Schutzfehler), sonst Meldung „PROCESS … KILLED: MEMORY VIOLATION“ mit Typ (`private/global/super/readable/free/hardware`, `berr_msg`, `:71-79`), PC, Adresse, Basepage; als AES-Alert, wenn der Debug-Level es zulaesst. **[gelesen]**
- Mit gesetztem Handler laeuft der Prozess normal weiter; `bombs()` meldet bei `SIGBUS` ohne `no_mem_prot` nichts doppelt (`sig_mach.c:402-405`). **[gelesen]**
- `Mvalidate`/`Maccess` erlauben einem Programm, Zugriffsrechte vorab zu pruefen (`FM/dosmem.c:458-540`). **[gelesen]**

### A.6 Globaler und gemeinsamer Speicher, Supervisor-Speicher

- **Globaler Speicher** = Region mit Modus `PROT_G`: `Mxalloc(…, F_PROT_G)` oder Voreinstellung der PRGFLAGS. Der Kernel nutzt das fuer Cookie-Jar (`FM/cookie.c:162`) und das Keyboard-Layout inkl. Vektoren (`FM/keyboard.c:1524`); beide werden an `rootproc` gehaengt (`attach_region(rootproc, …)`). **[gelesen]**
- **Shared Memory (`u:\shm`)**: `Fcntl(SHMSETBLK, addr)` markiert die Region des Aufrufers `M_SHARED` und `links++` (`FM/shmfs.c:658-695`); `SHMGETBLK` macht `attach_region(curproc, m)` und liefert die Adresse (`:696-720`). Die Adresse ist in allen Teilnehmern **dieselbe**; jeder Teilnehmer wird Mit-Besitzer (`PROT_P` fuer alle Besitzer). Geforkte Regionen (`m->shadow`) sind nicht teilbar (`:683-687`). **[gelesen]**
- **SLB** (MagiC-Style Shared Libraries, `FM/slb.c`): die Bibliothek wird per `sys_pexec(106, …)` als **eigener Prozess** gestartet, ihr `init` laeuft, sie bleibt als Prozess resident (`slb.c:330-401`); der aufrufende Prozess bekommt per **`mark_proc_region(get_curproc()->p_mem, *mr, PROT_G, pid)`** Zugriff auf *alle* Regionen des Bibliotheksprozesses (`:581-591`), das Deskriptor-Objekt `PROT_PR` (`:606,629`). Aufrufe laufen in einem `user_things`-Trampolin im Aufrufer (`FM/arch/user_things.S:99-160`, `slb_exec`). Ein Prozess kann sich also *selbst* Rechte auf Regionen anderer Prozesse geben, ohne Besitzer zu sein. **[gelesen]**
- **Supervisor-Speicher** (`PROT_S`): von jedem Prozess im Supervisor-Modus nutzbar, aus dem User-Modus nicht. Auf 68k ist der Supervisor-Modus ein Modus *desselben* Prozesses (kein getrennter Kernel-Adressraum); darum konnten XaAES (Kernel-Modul/Thread) und Kernel gemeinsame Speicher unter einer Seitentabelle benutzen. **[Schluss]** (Beleg: Kernel-Threads erben `rootproc->p_mem` per `FORK_SHAREVM`, `FM/k_kthread.c:68-72`.)

### A.7 Prozesserzeugung: `Pexec` — alle Modi

Dispatcher `sys_pexec` (`FM/k_exec.c:168-660`). Moduskennung `mode & 0x7fff` (`:206`; Bit 15 = Tracing). Hilfsvariablen `mkbase/mkload/mkgo/mkwait/mkfree/overlay/thread`:

| Modus | Setzt (`k_exec.c`) | Wirkung (Besitz, Adressraum) |
|---|---|---|
| 0 `PE_LOADGO` | `:207-213` `mkwait mkload mkgo mkfree mkname` | laden, neuen Prozess starten (`fork_proc(FORK_SHAREEXT)` = **kopierter** `memspace`, `:421`), Elternprozess wartet; `mkfree`: Eltern **loest** Basepage/Env danach (Block `if (mkfree)`, `:586-590`) |
| 100 | `:210-213` (gleicher Fallthrough ohne `mkwait`) | wie 0, aber ohne Warten (Rueckgabe PID) |
| 200 | `:214-217` `mkload mkgo overlay` | **Overlay**: eigener Prozess wird durch das geladene Programm ersetzt (`p = curproc`, `b->p_parent = curproc->…base->p_parent`, `:411`) |
| 3 `PE_LOAD` | `:218-220` `mkload` | laden **in den `memspace` des Aufrufers**: Env und TPA werden per `alloc_region`/`attach_region` an `curproc` gehaengt (`create_env` `memory.c:1369`, `create_base` `:1427`, `load_region` `:1558`), Rueckgabe `base->loc` = **Zeiger auf die Basepage im Adressraum des Aufrufers** (`k_exec.c:654-656`); Aufrufer ist Besitzer |
| 4 | `:224-231` `mkwait mkgo thread=1` | **Thread**: `fork_proc(FORK_SHAREVM|FORK_SHAREEXT)` → Kind **teilt den `memspace` des Aufrufers** (`links++`, kein `init_page_table`, `k_fork.c:143-146,198-205`); `exec_region(p,base,thread=1)` laesst den Speicher stehen (Block `if (!thread)`, `k_exec.c:763`) |
| 104 | `:232-240` `mkgo thread=1` | wie 4, ohne Warten |
| 6 `PE_GOTHENFREE` | `:221-223` → faellt in 4, aber `thread = (mode == 4)` = 0 | Start des vorbereiteten Prozesses, **kopierter** `memspace`; `mkfree`: Eltern loest Basepage + Env |
| 106 | `:232-234` `mkfree` → `mkgo` | wie 6 ohne Warten; **von SLB benutzt** (`slb.c:331`) |
| 204 / 206 | `:243-259` `mkgo overlay` (+206: `mkfree` auskommentiert) | Overlay einer vorbereiteten Basepage |
| 5 `PE_BASEPAGE` | `:269-277` `mkbase` | Env + Basepage-Region anlegen (`create_env`, `create_base`), **an den Aufrufer angehaengt**, Rueckgabe `base->loc` (`create_base` initialisiert `p_lowtpa=b`, `p_hitpa`, `p_env`, `p_flags`, `memory.c:1521-1533`) |
| 7 | `:261-268` `flags = (long)p1`, `mkbase` | wie 5, mit PRGFLAGS-Wort |
| sonst | `:277-283` | `ENOSYS` |

Gemeinsame Regeln:
- **Modi 4/6/104/106/204/206 verlangen, dass die uebergebene Basepage `curproc` gehoert** (`addr2mem(get_curproc(), ptr_2)`, sonst `EFAULT` „memory not owned by parent“, `k_exec.c:378-387`). **[gelesen]**
- **`b->p_parent = curproc->p_mem->base`** — eine echte Zeigerverkettung der Basepages (`k_exec.c:420`, `:961`). **[gelesen]**
- **Eltern behaelt Zugriff auf die Kind-TPA bei Modus 3/4/5/7/100…**: Basepage und Env sind Regionen mit `links` ≥ 2, solange sie sowohl an den Eltern- als auch an den Kindprozess gehaengt sind (`exec_region` + `attach_region(p, env); attach_region(p, base)`, `:518-520`); nur `mkfree` loest die Eltern-Referenz (`:586-590`). **[gelesen]**
- **Env-Schutz:** nach dem Laden wird die Env-Region auf den Modus der PRGFLAGS des Kindes umgestellt (`mark_region(env, (flags & F_PROTMODE) >> F_PROTSHIFT, 0)`, `:398`). **[gelesen]**
- **TPA-Groesse:** `create_base` nimmt den **groessten freien Block** der gewaehlten Karte (`max_rsize`), abzueglich `KEEP_MEM`, begrenzt durch `initialmem` (Default 1024 KiB, `memory.c:1494-1499`), `F_SMALLTPA` (`:1494`), `maxmem` (`:1500`). PRGFLAGS-Bits waehlen ST/TT (`F_ALTLOAD` + `F_MINALT`, `:1442-1472`). **[gelesen]**
- **Lader:** GEMDOS-`.PRG` (`0x601a`) mit **Relokationstabelle**; `load_and_reloc` addiert `p_tbase/p_dbase/p_bbase` auf die abgelegten Adressen (`memory.c:1699-1815`). Das Programm laeuft also an der Adresse, die `create_base` zugeteilt hat — **Relokation zur Ladezeit ist die Voraussetzung dafuer, dass Identitaet VA = PA genuegt** (so auch ADR-021 „relocatable binaries“, `docs/MEMORY.md:1380`). **[gelesen]**

### A.8 Wer greift auf wessen Speicher zu — Eltern ↔ Kind, LDG

- **Eltern → Kind:** Bei Modus 3 liegt die Kind-TPA im `memspace` des Aufrufers (`PROT_P`, Besitzer = Aufrufer). Der Eltern-Prozess kann also lesen, schreiben **und ausfuehren** (kein NX auf 68k). **[gelesen]**
- **Kind → Eltern (Default):** Das Kind startet mit `copy_mem`: *alle* Regionen des Elternprozesses werden dem Kind-`memspace` zugerechnet (`links++`, `FM/proc_help.c:119-170`), `init_page_table` markiert sie als Besitz (`mprot030.c:1056-1067`). `exec_region` entfernt diese Zugehoerigkeit (`m->links--`, `k_exec.c:784-806`), **markiert die Seiten aber nur dann `PROT_I`, wenn `mem_prot_flags & MPF_STRICT`** (`:799-806`). Default 0 (`mprot030.c:154`; Doku: „strict … a process no longer has unrestricted access to its parent's address space“, `doc/ChangeLog.old/ChangeLog.#1.13-1.15.9:204-210`, „Some programs may refuse to run, so the default is 0“, `doc/examples/mint.cnf:132-137`). **Ergebnis: ein per `Pexec` gestartetes Kind darf standardmaessig in den Adressraum des Elternprozesses schreiben.** **[gelesen]; die Wirkung (Kind behaelt Seitenrechte, solange die Region des Elternprozesses noch lebt) ist [Schluss]** aus den Codestellen plus der Dokumentation.
- **LDG-Ablauf (Quelltext `Applications/ldg-src/ldg/src/devel/ldgload.c`):** `Pexec(5)` (Dummy-Basepage, um `p_env` zu lesen, `:73`), Umgebungsvariable `OFFSETLDG=<Adresse der Stack-Variable ldg>` (`:108`), `Pexec(3)` (Modul laden, `:137`), `ldg_cpush()` (`:148`), `Pexec(4)` (Modul-Thread starten, `:151`), `ldg_cpush()` (`:156`), `Pwait()` (`:158`); das Modul ruft `ldg_init`, das `*(long*)atol(getenv("OFFSETLDG")) = offset` schreibt (`ldglib.c:134-137`). Dank `Pexec(4)` = Thread im selben `memspace` ist die Stack-Variable des Aufrufers fuer das Modul beschreibbar. Spaeter ruft der Aufrufer Funktionen des Moduls **im eigenen Prozess** auf (Modul-TPA hat er per `Pexec(3)` besessen; `ldg_unload` gibt `p_env` und Basepage per `Mfree` frei, `ldgload.c:201-205`). **[gelesen]**

### A.9 Ptermres / TSR

- `sys_ptermres(save, code)` (`FM/k_exit.c:308-340`): verkleinert die Basepage-Region (`mem->mem[1]`, „should be the basepage“) auf `save` (`:322`); **setzt jede Region, die der Prozess allein besitzt (`links == 1`) und die `PROT_P` ist, auf `PROT_G`** (`:325-336`) („this means that more TSR's will 'do the right thing' without having to have prgflags set“); `terminate(p, code, TSR_Q)`. **[gelesen]**
- `terminate` mit `TSR_Q` gibt alles frei **ausser dem Speicher** (`FM/k_exit.c:62,131-134`: `free_mem` nur bei `ZOMBIE_Q`). Der `PROC` bleibt in `TSR_Q`, `mark_region` ueberspringt ihn (`mprot030.c:651`). **[gelesen]**

### A.10 fork / vfork / Threads

- `fork_proc1` (`FM/k_fork.c:56-215`): `p_mem = share_mem(p1)` bei `FORK_SHAREVM`, sonst `copy_mem(p1)` (`:143-146`); nach `copy_mem` wird `init_page_table(p2, p2->p_mem)` und der Trampolinbereich eingeblendet (`:198-205`). **[gelesen]**
- **`Pvfork`**: Kind erhaelt `copy_mem` (gleiche Regionen, eigene Seitentabelle), der Elternprozess **schlaeft**, bis das Kind exited oder ein Overlay-`Pexec` macht (`FM/k_fork.c:252-310`). Der Adressraum wird nicht gesichert. **[gelesen]**
- **`Pfork`**: ohne zweite Adresszaehlung **nicht** ohne weiteres moeglich — beide Prozesse sehen dieselben Adressen. FreeMiNT loest das mit **Schattenregionen** (`fork_region`, `FM/memory.c:1296-1360`; `sys_pfork`, `FM/k_fork.c:326-448`) und **vertauscht bei jedem Kontextwechsel den Inhalt** (`swap_in_curproc`, `FM/proc.c:376-410`, Aufruf `:636`). Das ist der teuerste Preis des gemeinsamen Adressraums. **[gelesen]**
- **Kernel-Threads** (`kthread_create`): `fork_proc1(rootproc, FORK_SHAREVM|FORK_SHARECWD|FORK_SHAREFILES|FORK_SHAREEXT)` (`FM/k_kthread.c:68-72`). **[gelesen]**
- **Trampolin** (`user_things`): je Prozess eine Region `tp_reg` (`PROT_P`, aus `alt`/`core`, kein fester VA) mit `terminateme`, `sig_return`, `pc_valid_return`, `slb_*`; die Zeigertabelle wird relokiert (`FM/proc_help.c:147-205`, `FM/arch/user_things.S:60-95`). Signalhandler laufen im User-Modus des Prozesses im selben Adressraum und kehren ueber `sig_return` → `Psigreturn` zurueck. **[gelesen; die Stapelmechanik von `handle_sig` selbst nicht gelesen]**

### A.11 Accessories, AES-Zeiger

- XaAES ist ein **Kernel-Modul** mit Kernel-Threads (`kthread_create`, `xaaes/src.km/init.c:680`, `k_main.c:1779`); es ruft Atari-Programme (Accessories, `.acc`-Endung `xaaes/src.km/xa_shel.c:61`) ueber `create_process` (`FM/k_exec.c:908-1040`, `xa_shel.c:634,684`): `create_env` + `load_region` + `fork_proc(0)` + `exec_region` + `attach_region`. Die Accessory ist ein eigener Prozess mit kopiertem `memspace`. **[gelesen]**
- **Wie das AES Zeiger des Clients liest/schreibt** (Menuebaeume, Objektbaeume, Nachrichtenpuffer, `shel_write`-Argumente): **[offen]** — ich habe `xaaes/src.km/*.c` nicht auf die Zugriffsmuster untersucht. **[Schluss]**, nicht gelesen: AES-Syscalls laufen im Supervisor-Modus *im Kontext des aufrufenden Prozesses* (gleiche Seitentabelle), daher sind Client-Zeiger direkt gueltig; AES-eigene Datenstrukturen liegen in `rootproc`-Speicher (`F_PROT_S`, `FM/proc.c:138`) und sind den Clients im User-Modus nicht zugaenglich. Die Zugriffsmuster Client → AES-Speicher sind zu pruefen (Schritt 2). **[offen]**

### A.12 Speicherlimits

`maxmem`, `maxdata`, `maxcore` je Prozess (`FM/mint/proc.h:157-159`); `recalc_maxmem` (`FM/memory.c:1858-1875`) leitet `maxmem` aus `maxdata + Programmgroesse` und `maxcore` ab; `_do_malloc` verweigert Allokationen jenseits `maxmem - memused` (`FM/dosmem.c:80-107`); `Malloc(-1)` meldet das Restkontingent (`:78-96`). Globale Defaults `initialmem` (`TPA_INITIALMEM`, `memory.c:1494`) und `forcefastload`. **[gelesen]**

### A.13 Verhalten bei abgeschaltetem Speicherschutz

- `no_mem_prot` (Bootmenue `MEM_PROT=NO`, `FM/bootmenu.c:207-219`): `init_tables` setzt `page_table_size = 0` und kehrt zurueck (`mprot030.c:242-246`); `mark_region`, `mark_proc_region`, `mark_pages`, `init_page_table`, `mem_prot_special` kehren sofort zurueck (`:505,626,702,843,1128`); `get_prot_mode` liefert `PROT_G` (`:609`); `prot_temp` gibt `-1` (`:779`); `change_context` ueberspringt den MMU-Wechsel (`context.S`-Bedingung `tst.w SYM(no_mem_prot)` vor `:478`); `init_page_table_ptr` legt keine Seitentabelle an (`FM/proc_help.c:52-60`); Warnung beim Boot (`FM/init.c:751-753`). **[gelesen]**
- 68000-Build (`WITH_MMU_SUPPORT` nicht gesetzt): `FM/arch/mprot000.c:36-108` — leere Funktionen mit demselben Namen. **[gelesen]**
- **Unveraendert bleiben:** Regionsverwaltung (`attach/detach/free_region`, `links`), `Mxalloc`-Modi (auch `F_PROT_*` werden geparst, haben aber keine Wirkung), `Pexec`-Semantik, Zeigerverkettung der Basepages, Besitz/`M_KEEP`/`Ptermres`. **[gelesen; der Schluss „alles andere identisch“ ist [Schluss]]**

### A.14 68k-spezifisch gegenueber allgemeinem Entwurf

| Aspekt | 68k-/Atari-spezifisch | Allgemeines Design (uebertragbar) |
|---|---|---|
| Seitentabellen | PMMU-Baeume 4+4+4+7 (030), URP/SRP (040/060), 8-KiB-Seiten, Layout je Maschine (`mprot030.c`, `mprot040.c`) | „eine Tabelle je Prozess, ueber derselben Identitaetsabbildung, nur Rechte verschieden“ |
| Umschalten | `pflusha` + `pmove %crp` / `movec urp,srp` (`context.S`) | Tabelle des Prozesses beim Kontextwechsel aktivieren |
| Supervisor-Modus | gleicher Adressraum, Modus-Bit `s` im Deskriptor | auf AArch64 EL1 ≠ User-Prozess; `PROT_S` bildet sich auf „EL1-only“ ab (E.3) |
| Caches | 020/030 logisch, nach Tabellenwechsel leeren; 040 physisch | AArch64: PIPT-D-Cache, I-Cache-Pflege bei Codeaenderung (E.5) |
| Programmformat | GEMDOS-PRG mit Relokationstabelle, Ladeadresse beliebig | Relokation beim Laden **oder** feste Adresse + eigener Raum |
| Quantum | 8 KiB (MMU-Seite) | beliebig (Seitengroesse des Ziels) |
| Rechte-Modell | `PROT_P/G/S/PR/I` (5 Modi), Besitz = Menge von Prozessen | uebertragbar, beschreibt Sichtbarkeit je Prozess |
| `F_OS_SPECIAL` | Hack, weil GEM historisch Zeiger zwischen allen Prozessen reicht (`mem_prot_special`) | allgemein: privilegierter Dienst mit Vollzugriff |
| `Pfork` per Inhaltstausch | Folge des festen Adressraums | auf AArch64 mit MMU unnoetig (kopieren oder COW), aber **nicht Teil der Atari-Pflichtmenge** |
| AES-Zeiger | Client-Zeiger direkt lesbar | **muss** in Schritt 2 neu entschieden werden |

---

## B. EmuTOS (`references/emutos`, Pin `1a7484ab`) — die Basislinie

- **Ein Adressraum, kein Schutz.** `run` ist der Zeiger auf die aktuelle `PD` (Basepage). Alle Prozesse teilen einen Adressraum; es gibt keine MMU-Nutzung. **[gelesen]** (`ET/bdos/proc.c:179-361`, `:536-578`)
- **Zwei Pools** `pmd` (ST) und `pmdalt` (TT), je `MPB` mit freier (`mp_mfl`) und belegter Liste (`mp_mal`); `MD` hat `m_start`, `m_length`, `m_own` (`ET/include/bdosdefs.h:114`, `ET/bdos/umem.c`, `ET/bdos/iumem.c`). `ffit` ist first-fit; der neue Block wird mit `q->m_own = run` gekennzeichnet (`ET/bdos/iumem.c:134`). **[gelesen]**
- **Besitz ist nur ein Aufraeum-Etikett**, keine Zugriffskontrolle: `set_owner(addr, p)` aendert `m_own` (`ET/bdos/umem.c:587-603`); `ixterm` → `free_all_owned(r, &pmd)` und `&pmdalt` gibt alles frei, was `m_own == r` ist (`ET/bdos/proc.c:92-101,110-141`). **[gelesen]**
- **`Mxalloc`** maskiert `mode &= MX_MODEMASK` (0x03), ignoriert also Schutzbits (`ET/bdos/umem.c:241`, `ET/include/bdosdefs.h:22`). `Malloc` waehlt TT, wenn `run->p_flags & PF_TTRAMMEM` (`ET/bdos/umem.c:116-129`). **[gelesen]**
- **`Pexec`** (`xexec`, `ET/bdos/proc.c:179-361`; Modi `ET/include/bdosdefs.h:25-32`):

| Modus | EmuTOS-Verhalten | Besitz |
|---|---|---|
| 0 `PE_LOADGO` | laden, relozieren, `proc_go` (`proc.c:249-251,358-359`) | Env + TPA gehoeren dem **Kind** (`owner = p`, `:305-307`) |
| 3 `PE_LOAD` | laden, **nicht starten**; Rueckgabe = Zeiger auf die `PD` (`return (long)cur_p`, `:360`) | Env + TPA gehoeren dem **Aufrufer** (`owner = run`) |
| 4 `PE_GO` | `proc_go((PD*)tail)` (`:244-248`) | unveraendert (Aufrufer bleibt Besitzer) |
| 5 `PE_BASEPAGE` | Env + Basepage anlegen, `alloc_tpa` nimmt den **groessten** freien Block (`:210-237,460-502`) | Aufrufer (`set_owner(p, run)`, `:229-230`) |
| 6 `PE_GOTHENFREE` | `set_owner(p,p); set_owner(p->p_env,p)`, dann wie 4 (`:238-248`) | geht auf das Kind ueber |
| 7 `PE_BASEPAGEFLAGS` | wie 5 mit Flags (`:213-237`) | Aufrufer |
| 50 `PE_RELOCATE` | nur NatFeats-Bootstrap (`:193-209`) | — |
| sonst (100+, 200+) | `EINVFN` (`:252-253`) | — |

- **`proc_go`** (`:536-578`): `p->p_parent = run` (echter Zeiger), legt den Startframe am **oberen Ende der TPA** ab (`p->p_hitpa - sizeof(gouser_stack)`), `run = p`, `gouser()`. **[gelesen]**
- **Ptermres** (`xtermres`, `:624-634`): `xsetblk(0, run, blkln)` (TPA verkleinern), `reserve_blocks(run, &pmd)` (+`pmdalt`) entfernt jeden Block mit `m_own == run` aus `mp_mal` (**der Speicher bleibt fuer immer belegt**, `:77-89`), dann `xterm(rc)`. **[gelesen]**
- **Accessories** (`ET/aes/geminit.c:236-257`, `ET/util/gemdos.c:29-47`): `pgmld` = `Pexec(PE_LOAD, …)` + `Mshrink(basepage, tlen+dlen+blen+0x100)`; anschliessend `pstart(gotopgm, …)`. Besitzer der Accessory-Speicherbloecke ist der AES-Prozess, der alle ACCs in **einem** Adressraum und ueber `gotopgm` (`ET/aes/gemasm.S:73-120`) startet. `free_accs` ist nur Kosmetik (`geminit.c:296-306`). **[gelesen]**
- **Programmstart durch die AES** (`sh_*`): `Pexec(PE_BASEPAGEFLAGS, PF_STANDARD, "", NULL)` + `dos_exec(PE_GOTHENFREE, …)` fuer das Desktop (`ET/aes/gemshlib.c:574-579`), `dos_exec(PE_LOADGO, …)` fuer Anwendungen (`:645`). **[gelesen]**
- **Programmformat:** nur GEMDOS-PRG (`kpgmhdrld`/`kpgmld`, `ET/bdos/kpgmld.c:33-120`), mit Relokation. Die PRGFLAGS-Schutzbits (`F_PROTMODE`) sind in EmuTOS ohne Wirkung. **[gelesen; die zweite Aussage ist [Schluss] aus `MX_MODEMASK` und dem Fehlen jeder Auswertung]**

---

## C. NewTOS heute (Zweig `desk-rebuild`)

### C.1 Ueberblick

Jeder Prozess hat einen **eigenen Slot** in `PROCESS_TABLE[MAX_PROCESSES = 16]` (`tos-api/src/bdos/process.rs:63,769`) mit **eigener statischer 4-Ebenen-Seitentabelle** `__USER_TABLES[slot]` = L0, L1, L2 und `USER_L3_COUNT = 16` L3-Tabellen (`NT/mmu.rs:130-152`) — 32 MiB mappbare Low-Half-VA je Slot. `TTBR0_EL1` wird bei jedem Wechsel auf die L0 des Slots gesetzt, `TTBR1_EL1` bleibt der Kernel. **[gelesen]**

### C.2 Adressraumaufteilung je Prozess

| VA-Bereich | Inhalt | Quelle |
|---|---|---|
| `[0, 0x100)` | Basepage (256 B) | `NT/loader.rs:13`, `tos-types/src/basepage.rs:95-135` |
| `[0x400, 0x670)` | System-/OSHEADER-/Cookie-Spiegel (`populate_sysvars`) | `NT/loader.rs:713-730,895` |
| `0x700…0xA40` | Upcall-Trampolin (`0x700`), G_USERDEF-Stub (`0x714`), PARMBLK (`0x730`), Return-Slot (`0x760`), Keytbl (`0x768`), Kbdvec-Tabelle (`0x9A0`), Kbdvec-Stubs (`0xA00`) | `NT/loader.rs:740-823` |
| `[0x1000, …)` | Programmbild (feste Adresse, `ET_EXEC`) | `NT/loader.rs:182-195,1356`, `sdk/newtos.ld:14-34` |
| `[…, USER_TPA_TOP = 4 MiB)` | BSS, freier TPA, Stack unten oben (min. 64 KiB) | `NT/loader.rs:170-182`, `NT/arena.rs:52` |
| `[4 MiB, 32 MiB)` | **EL0-Fenster** fuer `Malloc`/`Mxalloc`, `.RSC`, ADR-023-Strukturen, Framebuffer | `NT/usermem.rs:77-93` |

### C.3 Lader

- **Nur `ET_EXEC`** (feste Adressen); `ET_DYN` nicht angenommen (`NT/elf.rs:36,341-343`). Max. 8 `PT_LOAD` (`:181`). ELF32 wird geparst, aber abgelehnt (`NT/loader.rs`, `LoadError::Ilp32Unsupported`). **[gelesen]**
- Jedes `PT_LOAD` muss zwischen `RESERVED_LOW_END = 0x1000` und `USER_TPA_TOP - USER_STACK_MIN` liegen (`NT/loader.rs:1356-1372`); das SDK linkt bei `0x1000` (`sdk/newtos.ld`). **[gelesen]**
- Ablauf: TPA aus dem BDOS-Pool (`arena::alloc_tpa` = `mxalloc(TPA_SIZE + PAGE, MX_PREFTTRAM)`, `NT/arena.rs:104-121`), `set_owner(tpa.raw_start, tpa.base_kva)` (Besitzer-Cookie = **Kernel-VA** der TPA-Basis, `NT/loader.rs:425`), ELF streamen, ganzes TPA nullen, Basepage schreiben, `sync_code_cache` (`:542`), Seitentabellen bauen (`init_slot_tables`, `map_page` je 4-KiB-Seite, alle `UserRwExec`, `:546-560`), `seed_tpa` (`:572`). **[gelesen]**
- `bp.p_lowtpa = 0`, `bp.p_hitpa = USER_TPA_TOP`, `bp.p_xdta = &p_cmdlin`, **`bp.p_parent = 0`**, `bp.p_env = env_phys` (**eine physische Pool-Adresse, nicht im Kind abgebildet — MEM03**) (`NT/loader.rs:668-681`). **[gelesen]** EmuTOS setzt `p_parent = run` (`ET/bdos/proc.c:541`); NewTOS verfolgt den Elternprozess ueber `parent_slot` (`NT` Process-Feld, `process.rs:245`).
- `libc`-`malloc` laeuft aus dem freien TPA (`_heapbase = _end`, `sdk/libc/newtos-arch/heap_init.c:1-50`), nicht ueber GEMDOS. **[gelesen]**

### C.4 `usermem.rs` (ADR-021/-023)

- **Pro Slot** ein Blocktabelle `[Option<Block>; MAX_USER_BLOCKS = 32]` und ein **Bump-Cursor** (`NT/usermem.rs:93,167-187`). Block: `user_va`, `m_start` (Pool-Schluessel), `map_pa` (seitenausgerichtet, `≥ m_start`), `size`, `kind` (`Tpa/Heap/Rsc/Shared`), `prot`, `attr` (`:142-162`). **[gelesen]**
- `map()` (`:308-363`): berechnet `base_pa = page_round_up(m_start)`, nimmt `user_va = WINDOW_BASE + cursor`, mappt Seite fuer Seite per `mmu::map_page(slot, …)`, `UserRwExec` fuer `Heap`, `UserRwNx` fuer `Rsc`; `dsb ish; isb` (`publish_user_mappings`), kein TLBI. **VA ≠ PA** („offset-mapped“, `:26`). **[gelesen]**
- **Kein VA-Recycling**: `free()` laesst die Mappings stehen, der Cursor geht nie zurueck (`:596-612`, Kommentar „bump cursor never re-hands the VA“). `window_remaining` begrenzt `Malloc(-1)` (`:660-669`). Erschoepfung → `None` → `0`/ENSMEM (`:323-325`). **[gelesen]**
- `expose`/`expose_attr` (`:423-534`): veroeffentlicht eine Kernelstruktur (`'static`) per `PROT_PR`→`UserRoNx`, `PROT_G`→`UserRwNx`; `P/S/I` → `None` (kein Mapping). Idempotent je physischer Seite (`:479-487`). `MemAttr::Uncached` fuer den Framebuffer (`mmu.rs:184-197`). **[gelesen]**
- `resolve_kva(slot, user_va)` und `translate()` sind **slot-relativ** (`:546-591`). **[gelesen]**

### C.5 `Pexec` heute

- Unterstuetzt: `PE_LOADGO` (0), `PE_LOAD` (3), `PE_GO` (4). `PE_BASEPAGE`/`PE_GOTHENFREE`/`PE_BASEPAGEFLAGS` → `UnsupportedMode` → `EINVFN` (`NT/pexec.rs:168-181`). Modi 100+/200+ → `UnknownMode` → `EINVFN`. **[gelesen]**
- **`PE_LOAD` legt einen neuen Slot an** (eigene L0, eigener Adressraum ab VA 0) und liefert als Rueckgabewert einen **opaken Handle = `basepage_pa`** (`pexec.rs:75-81,264`); der Elternprozess **kann ihn nicht dereferenzieren** („a dereferenceable PD waits for the shared-address-space model“). `PE_GO(handle)` loest den Slot ueber `basepage_pa` auf (`:283-311`). **[gelesen]**
- Kernel-Handler `handle_pexec` (`KA/exceptions.rs:1684-1925`): liest `path`/`tail`/`env` mit `ldtrb` aus dem **TTBR0 des Aufrufers** (`:1690-1730`), schreibt den vollen Trap-Frame des Aufrufers in den Kind-Slot (`parent_frame`, `:1810-1830`), stempelt `parent_l0_pa = TTBR0_EL1` des Aufrufers (`:1832-1842`), setzt `RUN` auf das Kind, **nimmt den Elternprozess aus dem Run-Set** (`suspend_parent_for_child`, `:1877`) und wechselt TTBR0 (`restore_boot_ttbr0` + `kprintln` + `install_ttbr0(child.l0_pa)`, `:1900-1920`). **[gelesen]**
- `Pexec` ist **synchron**: der Elternprozess laeuft erst nach `Pterm` des Kindes weiter (Architektur wie EmuTOS `proc_go`/`xterm`). Echte Gleichzeitigkeit gibt es nur ueber `Pwait`/Scheduler-Wechsel und die AES-Blockierung (`evnt_*`, `sleep`) (`NT/sched.rs`, `docs/analysis/coop-kernel/*`). **[gelesen]**

### C.6 Prozesstabelle, Scheduler, TTBR0-Wechsel, TLB

- `Process`: `l0_pa`, `basepage_kva/pa`, `entry_va`, `sp_el0`, `parent_slot`, `parent_l0_pa`, `parent_frame`, `saved_frame`, Upcall-Stacks u. a. (`process.rs:199-440`). Die kooperative Auswahl `sched::pick_next` / `Switch { slot, fresh, l0_pa, entry_va }` (`NT/sched.rs:108`). **[gelesen]**
- **ASIDs: nicht benutzt.** `install_ttbr0` schreibt den nackten `l0_pa` (ASID-Feld 0) (`NT/mmu.rs:381-402`). Versuch mit ASID-Tag wedgt QEMU 11.0.0 cortex-a72 TCG (`NT/mmu.rs:366-374`, `docs/MEMORY.md:2087-2150` QUIRK-001) — **[Doku]**. Die Seitentabellen-Deskriptoren setzen **nicht** das `nG`-Bit (`NT/mmu.rs:91-111,200-212`: keine `D_NG`-Konstante). **[gelesen]**
- **TLB-Politik:** `install_ttbr0`: `dsb ish; msr ttbr0_el1; isb`, **kein TLBI** (`:381-402`); TLBI nur im Kernelkontext vor dem Install, per `restore_boot_ttbr0()` (setzt TTBR0 auf die leere Wurzel `EMPTY_L0` + `tlbi vmalle1`, `KA/user_entry.rs:101-117`). Jeder Wechsel laeuft: `restore_boot_ttbr0()`; `kprintln!` (QUIRK-001-Barriere: Device-Schreibverkehr zwischen TLBI und `eret`); `install_ttbr0()` (`KA/exceptions.rs:1056-1066,1377-1382,1910-1920`). Neue Fenster-Mappings nutzen **frische (vormals ungueltige) VAs**, damit kein TLBI noetig ist (`NT/usermem.rs:29-31,348-350`). **[gelesen]**
- `switch_ttbr0_kernel` (mit TLBI) ist nur fuer Selbsttests/EL1-Kontext vorgesehen (`NT/mmu.rs:566-580`). **[gelesen]**
- Terminate: `finish_terminate` (`KA/exceptions.rs:1239-1390`) → `release_slot` → `restore_boot_ttbr0()` → ggf. `install_ttbr0(parent_l0_pa)`; `release_slot` loescht Fenster-Zustand (`NT/pterm.rs:~215-235`). **[gelesen]**

### C.7 Kernel-Abbildung

- Kernel high-half (`TTBR1_EL1`, `0xFFFF_0000_0000_0000 + phys`), DRAM als 2-MiB-Bloecke Normal Cacheable, MMIO Device-nGnRnE (`KA/mmu.rs:1-60`); `TCR_EL1`: `T0SZ = T1SZ = 16` (48 Bit), 4-KiB-Granule, `IPS = 40 Bit`, Bit `A1` (22) = 0, Bit `AS` (36) = 0 (`KA/boot.rs:176-187`). **[gelesen]**
- `TTBR0_EL1` zeigt im Ruhezustand auf eine leere Wurzel (`EMPTY_L0`, ADR-014; `KA/mmu.rs:~690-735`), alle MMIO laeuft ueber die High-Half (`drivers::mmio_va`). **[gelesen]**
- `SCTLR_EL1` wird nur beim Boot geschrieben: `0x30D0_0800` initial, dann `M|C|I` (`KA/boot.rs:189-198`). **[gelesen]** (`UCI` etc. bleiben unverandert → siehe E.5.)
- BDOS-Pools: `m_start`/`m_length` sind `i32`; `init` liefert `AddressOutOfRange`, wenn der Pool nicht in positive 31 Bit passt (`tos-api/src/bdos/mem/init.rs:224-236`; Aufruf `kernel/src/lib.rs:767-775`). Frame-Pool-Claim im Boot (`bring_up_bdos_pools`, `lib.rs:742-775`). **[gelesen]**

### C.8 Wie Tabellen und Upcalls in den EL0-Raum kommen

- **Pro Prozess kopiert, nicht geteilt.** `populate_sysvars` (`NT/loader.rs:895`) schreibt Sysvar-Spiegel und Cookie-Jar-Spiegel in die Seite 0 jedes Prozesses; `refresh_cookie_mirrors()` laeuft ueber **alle Slots** und schreibt in jede TPA (`:1129-1138`). Upcall-Trampolin, G_USERDEF-Stub, Keytbl und Kbdvec-Tabelle werden beim Laden bzw. bei `Keytbl`/`Kbdvbase` per Kernel-Alias (`basepage_kva + offset`) in die TPA des **aktuellen** Prozesses geschrieben (`tos-api/src/xbios/keytbl.rs:219-223`, `xbios/kbdvecs.rs:149,199`). **[gelesen]**
- Der Kernel liefert Upcalls an feste EL0-Adressen: `deliver_upcall` (`KA/exceptions.rs:1395`) leitet den Frame so um, dass `eret` im Handler landet, **ohne `TTBR0` zu veraendern** (`docs/analysis/coop-kernel/30-aes-thread-design.md:693-705`: „Echte Luecke … sobald ein anderer Client … einen Repaint ausloest“). **[gelesen/Doku]**
- Ein Upcall-Handler-VA und seine PXN-Randbedingung: EL1 darf EL0-Seiten nie ausfuehren (`PXN` an allen Presets, `NT/mmu.rs:156-165,202-203`; `30-aes-thread-design.md:52-62`). **[gelesen]**

### C.9 Jede Stelle, an der „eigener Adressraum je Prozess“ vorausgesetzt wird

1. **Statische Seitentabellen je Slot** `__USER_TABLES[16]` (L0..L3), 32 MiB VA je Slot (`NT/mmu.rs:130-152`, `:87`).
2. **Alle Prozesse haben die Basepage an VA 0** (`USER_VA_BASE = 0`, `NT/loader.rs:170`; `pexec::go` setzt `basepage_va: USER_VA_BASE`, `NT/pexec.rs:290`; `apply_switch` `fresh`: `gpr[0] = USER_VA_BASE`, `KA/exceptions.rs:1072`).
3. **`ET_EXEC` bei fester Adresse `0x1000`** fuer alle Programme (`NT/elf.rs:341`, `NT/loader.rs:1356`, `sdk/newtos.ld`) — zwei gleichzeitig geladene Programme kollidieren in einem gemeinsamen Raum.
4. **Offset-Abbildung** `user_va ≠ pa` fuer Heap-/RSC-Bloecke (`NT/usermem.rs:26,308-363`) — prozessgebundene VA.
5. **Bump-Cursor-Fenster je Slot, kein VA-Recycling** (`NT/usermem.rs:596-612`); 28 MiB Fenster je Prozess (`:77-81`).
6. **Opake PD-Handles** statt Zeiger (`NT/pexec.rs:75-81,264,297-311`); `p_parent = 0` (`NT/loader.rs:677`).
7. **`PE_LOAD` = eigener Slot + eigener Raum** (`NT/pexec.rs:206-267`); Aufrufer kann das Kind weder lesen noch aufrufen.
8. **`Mshrink(basepage)`/`Mfree(0)` ueber `usermem::translate(user_va = 0)`** (Block 0 = TPA, `NT/usermem.rs:266-283`).
9. **Besitz-Cookie = `base_kva` der TPA** (`NT/loader.rs:425`) statt der user-sichtbaren PD-Adresse; `RUN` ist eine Kernel-Adresse (`process.rs:777`).
10. **TTBR0-Wechsel je Prozess mit QUIRK-001-Bracket** an vier Stellen (`KA/exceptions.rs:1056-1066`, `:1377-1382`, `:1910-1920`; `KA/user_entry.rs:101`).
11. **`parent_l0_pa`** pro Kind (`process.rs:298`) und Wiederherstellung beim `Pterm` (`KA/exceptions.rs:1370-1382`).
12. **Feste Seite-0-Tabellen je Prozess**: Cookie-Jar `0x5F0`, OSHEADER `0x5C0`, Trampolin `0x700`, USERDEF `0x714`, PARMBLK `0x730`, Return-Slot `0x760`, Keytbl `0x768`, Kbdvecs `0x9A0`, Stubs `0xA00` (`NT/loader.rs:713-827`).
13. **`refresh_cookie_mirrors`** schreibt die Cookie-Liste in jede TPA (`NT/loader.rs:1129`).
14. **Keytbl/Kbdvec schreiben in die TPA des aktuellen Prozesses** (`xbios/keytbl.rs:219`, `xbios/kbdvecs.rs:199`); `Kbdvbase` liefert eine Adresse im Prozessraum (`kbdvecs.rs:189`).
15. **Kernelzugriff auf Anwenderzeiger setzt das aktive TTBR0 voraus** (`ldtrb`/`ldtr`/`sttr`): `KA/exceptions.rs:1597-1650`, `tos-api/src/gemdos/console.rs:297,317`, `bios/vectors.rs:138,158`, `gemdos/ssystem.rs:209`, `bios/chardev.rs:161`.
16. **`Fsetdta`/DTA = `p_xdta` in der aktuellen Basepage** als 32-Bit-User-VA (`tos-api/src/bdos/fs/find.rs:70-98`).
17. **`Getbpb`/`Getmpb`/`Physbase`/`Logbase` veroeffentlichen je Prozess ein eigenes Fenster-Mapping** (`usermem::expose`, `bios/bpb.rs:299`, `bios/getmpb.rs:114`, `xbios/screen.rs:101`); `docs/COMPATIBILITY.md:216` nennt das ausdruecklich („a per-process window VA, not one address for all processes“).
18. **AES-Ressourcen**: `rsrc_load` mappt den Blob in das Fenster des *aufrufenden* Slots (`tos-api/src/aes/resource.rs:2365,2395`), `rsrc_gaddr` liefert diese App-VA; Bereinigung je Slot (`on_process_exit`).
19. **AES-Userdef-Upcall**: der Handler-VA ist im Raum des Baum-Besitzers; TTBR0 muss dessen sein (`30-aes-thread-design.md:693-705`).
20. **Umgebungsblock**: `p_env` = Pool-PA, im Kind nicht abgebildet (MEM03, `NT/loader.rs:1229-1305`, `:675-680`).
21. **`Ptermres`**: der Slot wird freigegeben (`release_slot`), damit gibt es **keine Seitentabelle mehr**, die den resident gehaltenen Pool-Block abbildet (`NT/pterm.rs:116-135,215-235`); die TPA bleibt als Pool-Block bestehen (Reserve), ist aber fuer niemanden mehr adressierbar. **[Schluss]**
22. **Zeigerbreite**: `BasePage`-Zeiger sind 32 Bit (`tos-types/src/basepage.rs:97-122`), `MD` ebenso (`bdos/mem/init.rs:226-236`) → jede gemeinsame VA, die in Basepage/MD/DTA/Cookie-Jar landet, muss in 31/32 Bit passen.

---

## D. Lueckentabelle

Kurzzeichen: **EM** = EmuTOS, **FM** = FreeMiNT, **NT** = NewTOS heute. Alle Quellenangaben stehen in A/B/C; hier nur Verweise (`§`).

| Mechanismus | EmuTOS | FreeMiNT | NewTOS heute | Was bricht |
|---|---|---|---|---|
| **Pexec 0** (LOADGO) | laden+starten, Env/TPA dem Kind (§B) | laden, Kind mit kopiertem `memspace`, Eltern wartet, `mkfree` loest Eltern-Referenz (§A.7) | geht (§C.5), eigener Slot+Raum | funktioniert; PD-Zeiger/`p_parent` nicht dereferenzierbar (`p_parent = 0`) |
| **Pexec 3** (LOAD) | liefert PD-Zeiger, Aufrufer Besitzer (§B) | laedt **in den Raum des Aufrufers**, Zeiger auf Basepage (§A.7) | neuer Slot, **opaker Handle** (§C.5) | Elternprogramme, die `p_tlen/p_dlen/p_blen` der PD lesen oder ins Kind springen/schreiben (Shell-Idiom, LDG, Overlays); `Mshrink` des Kinds |
| **Pexec 4** (GO) | `proc_go` im selben Raum (§B) | **Thread im Raum des Aufrufers** (`FORK_SHAREVM`) (§A.7) | startet geladenen Slot, Eltern pausiert (§C.5) | LDG `ldg_load`: Modul schreibt per Zeiger in den Stack des Aufrufers (§A.8); `Pexec(4)` als Thread-Erzeuger |
| **Pexec 5** (BASEPAGE) | Env+Basepage anlegen (§B) | wie EM, Region im Aufrufer (§A.7) | `EINVFN` (§C.5, PROC01) | LDG (`Pexec(5)` zum Env-Lesen), mintlib `vfork`/`system()` (PROC01), Pexec(5)+(4/6)-Idiom |
| **Pexec 6** (GOTHENFREE) | Besitz geht aufs Kind (§B) | wie 4, aber kopierter `memspace`, `mkfree` (§A.7) | `EINVFN` | AES-/Desktop-Startpfad (EmuTOS `gemshlib.c:574-579`), Shells |
| **Pexec 7** (BASEPAGEFLAGS) | wie 5 mit Flags (§B) | wie 5 (`F_PROTMODE` wird ausgewertet) (§A.7) | `EINVFN` | Programme, die PRGFLAGS (TT-RAM, Stack) vorgeben; FreeMiNT-Startpfad (`init.c:1237`) |
| **Pexec 100/104/106/200/204/206** | `EINVFN` (§B) | alle vorhanden (§A.7) | `EINVFN` | nur FreeMiNT-Programme (mintlib `Pexec`-Wrapper, `system()` POSIX-Zweig, SLB) |
| **Ptermres / TSR** | Bloecke mit `m_own==run` aus der Liste ausgetragen, bleiben belegt (§B) | Basepage verkleinern, Alleinbesitz-Regionen `PROT_P→PROT_G`, Prozess in `TSR_Q` (§A.9) | Pool-Bloecke reserviert, aber **Slot und Seitentabelle weg** (§C.9 Nr. 21) | Resident gehaltener Code/Vektoren (z. B. XBRA-Haken, Treiber) ist von keinem Prozess mehr erreichbar; jeder Zeiger darauf liefert Fault |
| **LDG** | n. a. (Bibliothek `ldg.prg` ist TSR + Cookie `LDGM`) | funktioniert: `Pexec 5/3/4` im Raum des Aufrufers, `OFFSETLDG` (§A.8) | geht nicht (`zview-ldg-2026-10-04.md` §4.1) | `ldg_load` komplett; User-Space-Lader-Alternative aus dem Zview-Bericht umgeht das Problem, ohne es zu loesen |
| **SLB** | n. a. (MagiC-Mechanismus) | eigener Bibliotheksprozess, Aufrufer bekommt `PROT_G` auf dessen Regionen, Trampolin `slb_exec` (§A.6) | nicht vorhanden | gesamte SLB-API (`Slbopen`/`Slbclose`/`Slbexec`) |
| **Accessories** | `Pexec(LOAD)`+`Mshrink`, Besitz AES, ein Raum (§B) | eigene Prozesse via `create_process` (§A.11) | `.NAC`-Prozesse als Scheduler-Peers mit eigenem TTBR0 (`kernel/src/accs.rs`, `process.rs`) | `menu_register`-Titelzeiger, `AC_OPEN`-Nachricht, `shel_write`/Programmzeiger zwischen ACC und Desktop (jeder Zeiger gilt nur im eigenen Raum) |
| **AES-Nachrichtenzeiger** (z. B. `WM_*` mit Baumzeiger, `MN_SELECTED`, `AC_*`, `evnt_*`-Puffer, Userdef) | alle Zeiger global gueltig | Zeiger gueltig dank gemeinsamer Adresse; Schutz via `F_OS_SPECIAL` fuer AES (§A.3) | Zeiger nur im eigenen Raum; Userdef braucht TTBR0 des Baumbesitzers (§C.9 Nr. 19) | alles, was Zeiger eines Clients in die Nachricht eines anderen packt; mehrere gleichzeitige Clients |
| **Globaler `Mxalloc`** (`F_PROT_G`, `F_KEEP`) | Bits maskiert (§B); jeder Block ist global | `PROT_G`-Region, `M_KEEP` (§A.3, A.2) | `mode &= MX_MODEMASK` (`tos-types/src/memory.rs:142`), Block nur im Fenster des Aufrufers (§C.4) | Programme, die `Mxalloc(F_PROT_G)` fuer Pufferfreigabe an andere Programme benutzen (z. B. Cookie-Werte, Treiberpuffer) |
| **Shm** (`u:\shm`, `SHMSETBLK/GETBLK`) | n. a. | Region `M_SHARED`, gleiche Adresse in allen Teilnehmern (§A.6) | nicht vorhanden; ADR-021 nennt das Luecke | alle SHM-Nutzer; FreeMiNT-Programme |
| **kbdvec fuer alle** (KEYS02 D1) | globale Tabelle im Systemspeicher (`Kbdvbase` liefert eine Adresse) | globale Region `PROT_G` (`FM/keyboard.c:1524`) | **je Prozess eine Kopie** in Seite 0 (`KBDVECS_AREA_VA = 0x9A0`, `NT/loader.rs:812`); Haken eines Prozesses wirken nicht auf andere | TSR/ACC, das `kbdvec` einhaengt, damit andere Programme es sehen; `Ikbdws`-Weiterreichen; ROM-Handler von EmuTOS-Code |
| **Env-Block / MEM03** | `p_env` = Pool-Zeiger im selben Raum | Env-Region im Besitz des Kinds (`create_env` + `attach_region`) | Pool-PA, nicht abgebildet (§C.9 Nr. 20) | jeder `Pexec` mit Env aus EL0, sobald das Kind `p_env` liest (MEM03/PORT09) |
| **vfork / `system()`** | `Pexec(5)+Pexec(4/6)` auf eigenem Code | `Pvfork` (Eltern schlaeft, Kind `copy_mem`) (§A.10) | nicht vorhanden; mintlib `system()` nur ueber SDK-Overlay | POSIX-`system()`/`vfork`-Nutzer; PROC01 |
| **Signalhandler / `etv_term` / `etv_critic`** | globale Vektoren, laufen im selben Raum | `user_things`-Trampolin je Prozess, Handler im Prozessraum (§A.10) | Software-Shadow-Vektoren (`bios/vectors.rs`), Upcalls an Handler-VA des **registrierenden** Slots mit Trampolin bei `0x700` (§C.8) | `etv_term` eines Programms, das in einem *anderen* Prozess zuschlaegt (Handler-VA im falschen Raum); Thunk fuer m68k |
| **`Fcntl`-/`Psigaction`-Handler, `Pause`** | nicht vorhanden | vorhanden | teilweise via Upcall-Schicht | nicht Teil von ARCH03 |
| **Prozess-Limits** (`maxmem`, `Psetlimit`) | nicht vorhanden | vorhanden (§A.12) | fest 4 MiB TPA + 28 MiB Fenster | kein Befund fuer Atari-Programme; FreeMiNT-Erweiterung |
| **`Mvalidate`/`Maccess`** | nicht vorhanden | vorhanden (§A.5) | nicht vorhanden | FreeMiNT-Programme, die Zeiger vorab pruefen |
| **PD-Zeiger/`Mshrink(basepage, …)`-Idiom** | Basepage = Zeiger in globalem Raum | `Mshrink(base, size)`, `size ≥ 256` (`FM/dosmem.c:408-455`) | `Mshrink(basepage@VA 0)` ueber Block 0 (§C.4), PD-Handle opak | Programme, die die Basepage eines **fremden** Prozesses lesen; Shell-Idiom |
| **Cookie-Jar fuer alle** | globale Tabelle (`_p_cookies`) | Region `PROT_G` aus `core` (`FM/cookie.c:162`) | Spiegel je Prozess, 15 Eintraege (`NT/loader.rs:723-730`), Refresh ueber alle Slots | Cookie-Werte, die auf Daten **ausserhalb** des eigenen Raums zeigen; Cookie, den ein Prozess als „globalen Dienst“ setzt und ein anderer nachschlaegt |
| **Framebuffer-Zeiger `Physbase/Logbase`** | physische Adresse global | n. a. | je Prozess ein UC-Mapping im Fenster (§C.4) | zwei Prozesse sehen verschiedene Zeiger auf denselben Schirm — kein Problem fuer Zeigervergleich im Prozess, aber nicht fuer Weitergabe |
| **Overlay-Programme** (Pexec 200) | n. a. | vorhanden | nicht vorhanden | FreeMiNT-Programme |

---

## E. AArch64-Fakten fuer den Architekten (Track B)

**Hinweis.** Das ARM ARM liegt nicht im Repo (`references/arm-arm/` fehlt). Ich habe fuer die PTE-Bitpositionen eine Querreferenz aus dem Linux-Quelltext gelesen (`arch/arm64/include/asm/pgtable-hwdef.h`; Linux ist laut Protokoll Querreferenz, **keine** Autoritaet) und die restlichen Aussagen als **[Wissen]** gekennzeichnet. Abschnittsnummern des ARM ARM nenne ich **nicht**, weil ich sie nicht nachgeschlagen habe; die Repo-Kommentare zitieren D5.10.1, B2.4.4, K11.5, D8.3 (unverifiziert hier).

### E.1 TTBR0 / TTBR1-Aufteilung

- Die VA wird anhand der oberen Bits getrennt: obere Haelfte → `TTBR1_EL1`, untere → `TTBR0_EL1`; Breite je Haelfte `64 - T0SZ`/`64 - T1SZ`. **[Wissen]**
- NewTOS: `T0SZ = T1SZ = 16` → 48-Bit-VA je Haelfte, 4-KiB-Granule, `IPS = 40`, `TG0 = 0b00`, `TG1 = 0b10` (`KA/boot.rs:170-187`). **[gelesen]**
- Folge fuer ARCH03: Ein gemeinsamer Adressraum fuer Anwender liegt in der Low-Half (TTBR0), Kernel bleibt high-half. Ein einziges, nie getauschtes TTBR0 ist der „ADR-010-Fall“; ein TTBR0 je Prozess ist der FreeMiNT-Fall (identische VA→PA, unterschiedliche Rechte). **[Schluss]**

### E.2 ASID

- Die ASID steht in `TTBR0_EL1[63:48]` (bei `TCR_EL1.A1 = 0`) bzw. `TTBR1_EL1`; sie ist 8 Bit breit, solange `TCR_EL1.AS = 0`, 16 Bit bei `AS = 1`; ob 16 Bit unterstuetzt werden, meldet `ID_AA64MMFR0_EL1.ASIDBits`. **[Wissen]**; die Linux-Konstanten `TCR_A1` und `TCR_ASID16` existieren (`pgtable-hwdef.h`), Bitpositionen dort aus `TCR_EL1_A1`/`TCR_EL1_AS` abgeleitet. Die NewTOS-Dokumente nennen dasselbe (`docs/MEMORY.md:2118-2123`). **[Doku]**
- NewTOS: `A1 = 0`, `AS = 0` (nicht gesetzt in `TCR_VALUE`, `KA/boot.rs:176-187`) → 8-Bit-ASID aus TTBR0; Prozesse laufen alle mit ASID 0 (`NT/mmu.rs:366-374`). **[gelesen]**
- **Offen:** ob der Cortex-A72 der Pi 4 16-Bit-ASIDs hat (`ASIDBits`) und was QEMU `raspi4b`/`virt` meldet. **[offen]**

### E.3 Seitenrechte: AP, UXN, PXN, nG

- Deskriptorbits (Linux-Querreferenz, `pgtable-hwdef.h`): `AP[1]` (EL0-Zugriff) Bit 6, `AP[2]` (nur lesen) Bit 7, `SH` Bits 9:8, `AF` Bit 10, `nG` Bit 11, `PXN` Bit 53, `UXN` Bit 54. NewTOS verwendet dieselben Positionen (`NT/mmu.rs:91-111`). **[gelesen]**
- Kombinationen, die NewTOS schon nutzt: `AP = 01` = EL1+EL0 lesen/schreiben; `AP = 11` = EL1+EL0 nur lesen; `AP = 00` = nur EL1 lesen/schreiben (Kernelabbildung) (`NT/mmu.rs:95-96`, `KA/mmu.rs` ff.). **[gelesen]** Alle Presets setzen `PXN` (`NT/mmu.rs:156-165`). **[gelesen]**
- **`PROT_*` → AArch64-Abbildung (Schluss, Entwurf fuer Schritt 2):** `PROT_P` Besitzer `AP=01`, Nicht-Besitzer *ungueltig*; `PROT_G` alle `AP=01`; `PROT_PR` Nicht-Besitzer `AP=11`; `PROT_S` Nicht-Besitzer *nur EL1* (`AP=00`) — beachte: auf AArch64 ist EL1 der Kernel, **nicht** der „Supervisor-Modus eines anderen Programms“; die FreeMiNT-Folge „AES im Supervisor-Modus liest Client-Speicher“ gilt nicht ohne weiteres. `PROT_I` ungueltig fuer alle. **[Schluss]**
- `nG` (not-global): Eintraege mit `nG = 1` gehoeren zu einer ASID, `nG = 0` gelten fuer alle ASIDs. NewTOS setzt `nG` heute nie. **[Wissen/gelesen]** (nicht gesetzt: `NT/mmu.rs:91-111`).
- **`SCTLR_EL1.WXN`/`PAN`/`UCI`:** `WXN` wird nicht gesetzt, `UCI` (EL0-Zugriff auf `DC CVAU`/`IC IVAU`) ebenfalls nicht (`SCTLR` `0x30D0_0800 | M|C|I`, `KA/boot.rs:189-198`); der Zview-Bericht vermerkt die Bedeutung von `UCI` als nicht aus dem ARM ARM gelesen (`docs/analysis/zview-ldg-2026-10-04.md` §4.1, Zeile „Cache-Sync“). **[offen/Wissen]**

### E.4 TLB-Pflege beim Prozesswechsel mit ASIDs

- Mit eindeutiger ASID je Prozess und `nG = 1` auf allen Anwendereintraegen genuegt beim Wechsel `msr ttbr0_el1` (neue ASID) + `isb`; ein TLBI ist nicht noetig, solange die ASID nicht wiederverwendet wurde. Wiederverwendung (Rollover) erfordert `TLBI ASIDE1IS` je ASID oder `TLBI VMALLE1IS`. **[Wissen]**
- Fehlt `nG` (wie heute), muessten bei VA-Aliasing (zwei Prozesse, gleiche VA, verschiedene PA) beim Wechsel alle Eintraege verworfen werden (`tlbi vmalle1`), sonst bleibt Fremdzustand im TLB (das ist die reale Fehlerquelle in ADR-014: staler 1-GiB-UXN-Block, Pi 4 ohne TLBI, `docs/MEMORY.md:456-492`). **[Doku/Schluss]**
- Bei **gleicher VA→PA fuer alle** Prozesse (FreeMiNT-Modell) und nur unterschiedlichen *Rechten* ist ein TLBI trotzdem noetig, sobald sich ein Recht **verengt** (Seite ungueltig oder schreibgeschuetzt wird) — veraltete weitere Rechte duerfen nicht im TLB bleiben; **verbreiterte** Rechte (ungueltig → gueltig) erzeugen nach ARM-Architekturregel nur dann kein Problem, wenn kein ungueltiger Eintrag gecacht wurde (ungueltige Eintraege werden nach Architektur nicht gecacht). **[Wissen]**
- **Beschraenkungen der Zielplattform (aus dem Repo):**
  1. `tlbi vmalle1` unmittelbar vor `eret` nach EL0 laesst QEMU 11.0.0 cortex-a72 TCG haengen (QUIRK-001); Workaround = UART-Schreibverkehr zwischen TLBI und TTBR0-Install. **[Doku]** (`docs/MEMORY.md:2087-2150`, `KA/exceptions.rs:1348-1380`)
  2. Ein **ASID-getaggter** `msr ttbr0_el1` haengt auf demselben Modell (`NT/mmu.rs:366-374`). **[Doku]**
  3. **Kernel-/User-Aliasing derselben DRAM-Seite mit verschiedenen Attributen** (TTBR1 EL1-Normal + TTBR0 EL0-Normal + PXN) haengt den ersten Kernel-Normal-Store nach dem TTBR0-Install (ADR-010 Stage 1, `docs/MEMORY.md:148-176`). Die geplante Identitaetsabbildung der DRAM-Pools in die Low-Half ist **genau dieses Muster**. **[Doku; offen, ob auf realer Pi-4-Hardware und neuerem QEMU anders]**
  4. Alle MMIO laeuft ueber die High-Half; TTBR0 ist im Ruhezustand leer (ADR-014). **[Doku]**
  5. Neue Fenster-Mappings nutzen frische (nie gemappte) VAs; sonst waere ein TLBI noetig (`NT/usermem.rs:29-31`). **[gelesen]**

### E.5 Caches

- Cortex-A72 hat PIPT-Datencaches; I-Cache siehe Kommentar `NT/mmu.rs:436-439` („Cortex-A72 caches are PIPT, so by-VA maintenance via any alias of the same PA affects the correct physical lines“). **[gelesen/Doku]**
- Geladener Code braucht `DC CVAU` + `DSB` + `IC IVAU` + `DSB` + `ISB` (im Kernel `sync_code_cache`, `NT/mmu.rs:445-489`); EL0 darf das nur bei `UCI = 1` (offen, E.3). In einem gemeinsamen Adressraum entfaellt die Pflege nicht: auch `Pexec(3)` + `Pexec(4)` und LDG laden Code, den ein anderer Prozess danach ausfuehrt (LDG ruft `ldg_cpush()` ausdruecklich, `ldgload.c:146-148,155-156`). **[Schluss]**
- **Mismatched-Attribute-Aliase** (Framebuffer: Kernel WB, Anwender Non-Cacheable) sind bekannt und mit `clean_invalidate_dcache_range` behandelt (`NT/mmu.rs:491-542`, `NT/usermem.rs:433-534`). Ein gemeinsamer Adressraum fuehrt keinen neuen Fall ein, solange alle Prozesse dasselbe Attribut je Seite sehen. **[Schluss]**
- Kosten der Seitentabellen je Prozess, Rechnung (**[Schluss]**, 4-KiB-Granule): ein L3 (4 KiB) deckt 2 MiB; N MiB Identitaetsabbildung je Prozess = N/2 L3-Seiten = ~2 KiB je MiB. FreeMiNT zahlt ca. 1 KiB je MiB (`mprot030.c:229-231`). Mit 16 Slots und 1 GiB abgebildetem Pool: 512 L3 × 4 KiB × 16 = 32 MiB nur fuer L3 — gegenueber heute 16 × 76 × 4 KiB ≈ 1,2 MiB (`NT/mmu.rs:69-70`). Gemeinsame L3-Tabellen mit abweichenden Rechten sind nur bei gleichen Rechten fuer den ganzen 2-MiB-Bereich moeglich.

### E.6 Weitere Zwangspunkte aus dem Repo

- **Pool-Adressen sind `i32`** (`bdos/mem/init.rs:226-236`); Pi 4 mit ≥ 2 GiB bricht `init` mit `AddressOutOfRange` — Folge: jede Identitaets-VA, die in `MD`/`BasePage`/`DTA` landet, muss in 31 Bit liegen. ADR-021 kennt das Problem als „Window kept well under 2 GiB so every pointer is a positive Long“ (`NT/usermem.rs:26-28`). **[gelesen]**
- **8-GiB-Pi-4:** Stage-2-Kernelmap deckt nur 4 GiB (`KA/mmu.rs:~30-35`). **[gelesen]**
- **ADR-010 ist bereits der Entwurf „ein gemeinsamer Adressraum“** (Status „Accepted … NOT YET IMPLEMENTED“, `docs/MEMORY.md:38-60`); es scheitert in QEMU an Wand 1 (TTBR0-Wechsel), Wand 2 (ASID-Tag), Wand 3 (Aliasing) (`:165-176`). ARCH03 muss das Verhaeltnis zu ADR-010 klaeren (ersetzen/erweitern). **[gelesen]**

---

## F. Offene Fragen an den Architekten (Schritt 2)

Reihenfolge nach Tragweite. Jeweils: was fehlt, damit entschieden werden kann.

1. **Welches Modell: ein TTBR0 fuer alle (ADR-010, kein Schutz) oder ein TTBR0 je Prozess mit identischer VA→PA und Rechten (FreeMiNT)?** Das hat unterschiedliche TLB-/Barrierekosten und verschiedene QEMU-Risiken (E.4 Nr. 1–3). Gibt es eine Stufung („zuerst einer, dann Rechte“) wie bei FreeMiNT mit `no_mem_prot` (A.13)?
2. **VA = PA ueberhaupt moeglich?** Bedingungen: Pool in 31 Bit (E.6), QEMU-Aliasing-Wand (E.4 Nr. 3), PXN-Verbot fuer EL1 auf EL0-Seiten (E.3). Falls nein: gemeinsame VA ≠ PA mit **fester Pool-VA-Abbildung**? (Dann zeigen `m_start`-Werte nicht mehr auf physische Adressen; `MD`-Format `Long` bleibt.)
3. **Ladeadressen.** `ET_EXEC` bei `0x1000` kollidiert bei zwei Programmen (C.9 Nr. 3). Relokation beim Laden (wie GEMDOS-PRG und FreeMiNT, A.7), PIE (`ET_DYN`, `R_AARCH64_RELATIVE`), oder feste Zuweisung je Programm? Was passiert mit bereits gebauten `.NAP`/`.NCP` (SDK `newtos.ld`)?
4. **TPA-Groesse.** FreeMiNT/EmuTOS geben den **groessten freien Block** (begrenzt durch `initialmem`, A.7); NewTOS fest 4 MiB. Mit gemeinsamem Raum muss `p_hitpa` stimmen; was heißt `Malloc(-1)`? Wie gross darf ein Fenster/Pool sein?
5. **Was wird aus `usermem.rs`?** Das Fenster (C.4) entfaellt bei VA = PA; `expose`-Pfad (ADR-023), `Kind::Shared`, `MemAttr::Uncached`, `seed_tpa`, `translate`/`free`/`resolve_kva` sind betroffen. `COMPATIBILITY.md:216` nennt „per-process window VA“ ausdruecklich als Abweichung — entfaellt sie?
6. **Besitzmodell.** `MEMREGION`-aehnlich (mehrere Besitzer, `links`) oder weiter EmuTOS-`m_own` (ein Besitzer)? FreeMiNT braucht Mehrfachbesitz fuer `Pexec 3/4`, `shm`, SLB. `Pexec(3)` im EmuTOS-Modell: Aufrufer ist Besitzer; was darf das Kind?
7. **Standardverhalten Kind→Eltern** (A.8). Soll NewTOS wie FreeMiNT-Default (`MPF_STRICT` aus) das Kind in den Eltern-Raum lassen? LDG braucht Kind-Schreiben in Elternraum **oder** `Pexec(4)` als Thread (A.7/A.8). Sollen `Pexec 4/104` als Thread (gleicher `memspace`) umgesetzt werden?
8. **`PROT_*` auf `Mxalloc`**: Standard aus PRGFLAGS (A.3) — NewTOS-PRGFLAGS im ELF? Wo stehen die (`p_flags` der Basepage: `PF_STANDARD`)? ADR-021 hatte die Bits ausdruecklich deferriert.
9. **AES-Zugriffsmuster** (A.11): wie liest XaAES Client-Speicher unter `PROT_P`? Das ist die Schluesselfrage fuer die NewTOS-AES (laeuft im Kernel). Hier bitte gezielt `xaaes/src.km/*.c` (Menues, Objektbaeume, Nachrichten, `shel_write`) lesen — das habe ich nicht getan.
10. **Kernel-Zugriff auf Anwenderzeiger.** Heute `ldtr*` unter aktivem Prozess-TTBR0 (C.9 Nr. 15). Mit gemeinsamem Raum und Rechten: Kernel-Zugriff auf *fremde* Prozessspeicher (FreeMiNT `prot_temp`, A.4) — AArch64-Aequivalent? (EL1 ueber TTBR0 mit EL0-Rechten kann eine Seite lesen, die dem Prozess `PROT_P` ist; nicht, wenn sie ihm ungueltig ist.)
11. **Fault-Behandlung.** FreeMiNT: Meldung + `SIGBUS` (A.5). NewTOS: Was bekommt der Anwender bei einer Verletzung (Data Abort lower EL → Prozess beenden mit Alert)? Welche Ausnahmeklasse genau (Translation vs. Permission)? Wie unterscheidet man „Hardware“ von „Schutz“ wie `report_buserr`?
12. **Cookie-Jar, Systemvariablen, `kbdvec`, `OSHEADER`.** Eine globale Kopie (FreeMiNT: `PROT_G`-Region, A.6) statt Spiegel je Prozess (C.9 Nr. 12–14)? Welche Adresse (Seite 0 ist fuer EL0 ein privates TPA-Stueck, ADR-021)? Fester Wert fuer `_p_cookies` (0x5A0) ist ABI.
13. **`p_parent`, `Mshrink`, PD-Zeiger**: Echte PD-Zeiger statt Handles (C.9 Nr. 6); `RUN`-Owner-Cookie und `PROCESS_TABLE`-Verhaeltnis (C.9 Nr. 9).
14. **Slot-Zahl und Seitentabellenbudget** (E.5): 16 Slots × ? GiB; gemeinsame L3-Tabellen?
15. **ASIDs auf QEMU nutzbar?** (E.2/E.4 Nr. 2) — Entscheidung: ASIDs nur auf Hardware, QEMU-Zielplattform mit TTBR0-Tausch + TLBI + Barriere? Dann entstehen zwei Codepfade.
16. **`Ptermres`/Residenz** (C.9 Nr. 21): muss ein resident gehaltener Prozess seine Seiten behalten (FreeMiNT: `TSR_Q`, `PROT_G`, A.9)?
17. **Signalpfad/Upcall** (C.8): Handler-VA im Raum des Registrierenden — mit gemeinsamem Raum entfaellt die TTBR0-Luecke (`30-aes-thread-design.md:693-705`); bleibt das Trampolin je Prozess bei `0x700` oder wird es global (FreeMiNT: je Prozess `tp_reg`, A.10)?
18. **m68k-Thunk (spaeter).** Ein m68k-Prozess braucht einen 32-Bit-Adressraum (4 GiB) — ist ein gemeinsamer Atari-Raum fuer den Emulator vorgesehen (ADR-018: „der spaetere Emulator soll .PRG/.TOS/.TTP uebernehmen“)? Beruehrt die VA-Wahl in Frage 2.
19. **Pfork** (A.10) ist keine Atari-Pflicht; FreeMiNT-Inhaltstausch soll nicht uebernommen werden? **[Entscheidung noetig]**
20. **`F_OS_SPECIAL`-Aequivalent:** Welche NewTOS-Komponenten brauchen Vollzugriff (Desktop in EL0, AES)? Und `desktop-must-run-in-user-scope` (MEMORY: Desktop soll nach EL0)?

---

## G. Selbstpruefung / Abgrenzung

- **Scope:** FreeMiNT, EmuTOS und NewTOS-Ist-Zustand des Prozess-/Speichermodells und die zugehoerigen AArch64-Fakten, wie im Ticket verlangt. Jeder Abschnitt zitiert `file:line` oder ist als [Schluss]/[Wissen]/[Doku]/[offen] markiert.
- **Nicht gelesen** (bewusst oder aus Zeitgruenden, im Text als [offen] markiert): XaAES-Zugriffsmuster (F.9), `handle_sig`-Stapelaufbau (A.10), `Fcntl(PCTXT)`, MiNT-`u:\proc`-Zugriffe, `mprot040.c` im Detail (nur Kopf und Struktur), Dateisystem-/DMA-Puffer in gemeinsamem Raum (XHDI, Block-IO), Mehrkernbetrieb.
- **Interne Details versus Vertrag** (`docs/COMPATIBILITY.md`): Vertrag sind die beobachtbaren Zeigereigenschaften (PD-Zeiger dereferenzierbar, `Pexec`-Rueckgabe, Gleichheit gleicher Zeiger ueber Prozesse, `Mxalloc`-Modi), die Rechte-Tabellen und `memflags` sind FreeMiNT-Erweiterungen; `MEMREGION`, `global_mode_table`, 8-KiB-Quantum, `pflusha`, Seitentabellenformate und Schattenregionen sind **intern**.

### Randbeobachtungen ausserhalb des Auftrags (je eine Zeile)

- `NT/loader.rs:677` setzt `p_parent = 0`, EmuTOS `proc_go` setzt `p_parent = run` (`ET/bdos/proc.c:541`); die Kind-Basepage zeigt also nicht auf die des Elternprozesses (Spec-Abweichung, unabhaengig von ARCH03 relevant).
- `NT/mmu.rs:140-142` Kommentar (`384 KiB bei USER_L3_COUNT = 3`) und `NT/mmu.rs:69-70` (1216 KiB) widersprechen sich; der Zahlenwert 1216 KiB gilt fuer 16 L3.
- `docs/ARCHITECTURE.md:93` sagt „Identity-mapped to physical RAM“ fuer den Kernelraum (richtig fuer `+ VA_OFFSET`), `NT/usermem.rs:26` nennt das User-Fenster „offset-mapped“ — die Begriffe sind in beiden Dokumenten korrekt, aber leicht zu verwechseln.
