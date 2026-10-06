# VFORKT-Absturz nach `exit()`-crt0 — Ursache

Stand 2026-10-06 · Diagnose zu `TASK/ARCH03-POSTE9-STATUS-UNFINISHED.MD` Punkt 2 · **Diagnose; Fix A seit 2026-10-06 umgesetzt (Abschnitt „Fix“), nichts committet** · Belege `measurements/measurement-2026-10-06-vforkt/` (`RUNINFO.TXT`, `serial/`, `method/`).

Pins: EmuTOS `1a7484ab` (`references/emutos`, Kürzel `ET`), FreeMiNT `f906416` (`references/freemint/sys`, Kürzel `FM`), mintlib 0.60.1 (`sdk/libc/mintlib-0.60.1.tar.xz`), NewTOS `desk-rebuild` HEAD `65b1078` + die nicht committeten Post-E9-Änderungen.

## Kurzfassung

Kein vfork-Kind besitzt oder belegt den Block, und kein `Pterm` gibt ihn frei. Die Vermutung im Statusbericht trifft also nicht zu. Der Elternprozess gibt den Block selbst frei, und zwar in mintlibs `tfork` (TOS-Zweig). Das Kind setzt in `startup` `b->p_env = parent->p_env` (`sdk/libc/overlay/mintlib/thread.c:67-68`, wörtlich so in mintlib 0.60.1). Wenn das Kind beendet ist, ruft der Elternprozess `Mfree(b->p_env)` auf (`thread.c:136`). Damit gibt er **seine eigene Umgebung** frei. Bei jedem weiteren vfork wiederholt sich dieses `Mfree` auf dieselbe Adresse. Inzwischen hat mintlibs `malloc` dort einen neuen 8-KiB-Block per `Malloc` geholt. Das dritte `Mfree` gibt deshalb den `malloc`-Block des Elternprozesses frei, während `_mchunk_free_list` noch hineinzeigt. Das erste `malloc` danach wandert in freien Poolspeicher. Mit dem `exit()`-crt0 ist das der stdio-Puffer `malloc(0x400)`.

Das ist ein Fehler im unveränderten mintlib-Code. Er tritt auf jedem TOS auf, auf dem mintlib den TOS-Zweig von `vfork` nimmt, also auch auf EmuTOS. Dort fällt er ohne MMU nur nicht auf. Die NewTOS-Speicherverwaltung (Besitz, `Pexec` 4, Regionen) ist nicht die Ursache. Der Schutz macht den Fehler nur sichtbar.

## 1. Der Block 0x01a57000: Herkunft, Besitz, Freigaben

Lauf `serial/vtc-on-s2all-serial.log` (Crash-Kernel + Speicherprotokoll, memprot=on). VFORKT hat die Basepage `0x01a8a000`, S2ALL `0x01a3f000`. Die Adressen sind über `method/sym.py` auf `post-e9/bin/VFORKT.NAP` aufgelöst (Laufzeitadresse − Basepage = ELF-Adresse).

| # | Ereignis | Prozess / Aufruf | Besitzer (`m_own`) |
|---|---|---|---|
| a | `alloc 0x01a57000 len=0x1000`. Das ist die **Umgebung** von VFORKT, angelegt beim `Pexec(0)` von S2ALL. | Lader, `run` = S2ALL | S2ALL. Danach gehört sie VFORKT (beim Freigeben gilt `own=0x1a8a000`). |
| b | Erstes vfork: `Pexec 5` → Kind-Basepage `0x401000` mit Umgebung `0x400000`, dann `Pexec 104` → `EINVFN`, dann `Pexec 4` | `tfork` | — |
| c | Das Kind ruft `Mfree(0x400000)` auf (eigene Pexec-5-Umgebung, `startup+0x28`). Danach `b->p_env = parent->p_env` = `0x01a57000` (`startup+0x2c..0x34`: `ldr w8,[x20,#0x2c]; str w8,[x19,#0x2c]`) und `Pterm(42)`. | Kind, Slot 2 | `0x400000` bleibt **belegt** (siehe Nebenbefund) |
| d | **`free 0x01a57000 len=0x1000`** durch `Mfree(0x01a57000)` → 0, ELR `tfork+0x1c8` (`ldr w0,[x19,#0x2c]; svc` = `Mfree(b->p_env)`) | **Elternprozess** VFORKT | VFORKT gibt seine eigene Umgebung frei |
| e | Zweites vfork: dasselbe `Mfree(0x01a57000)` → `EIMBA` (dort beginnt kein Block) | Elternprozess | — |
| f | **`alloc 0x01a57000 len=0x2000`**: `Malloc(0x2000)` → `0x01a57000`. Aufrufkette `main+0x23c` (`setenv("SHELL",…)`) → `setenv+0x140` (`realloc(NULL,…)`, `mintlib/setenv.c:101`) → `malloc+0xd4` → `sbrk+0x3c` (`unix/sbrk.c:59`, `_split_mem`) | VFORKT | VFORKT. Das ist der erste `malloc`-Block. First-fit legt ihn in das Loch aus der alten Umgebung und der freien Seite darüber. |
| g | Drittes vfork (`system()`, POSIX-Zweig): Das Kind holt per `_spawnve` `Malloc(0x46a)` → `0x405000`, ruft `Pexec(200)` auf (`EINVFN`), gibt `0x405000` per `Mfree` frei und beendet sich mit `Pterm(127)`. Kind-Umgebung wie in c. | Kind, Slot 2 | Das Kind besitzt nur `0x405000` und gibt ihn selbst frei |
| h | **`free 0x01a57000 len=0x2000`** durch `Mfree(0x01a57000)` → 0, ELR `tfork+0x1c8` | **Elternprozess** VFORKT | der eigene `malloc`-Block wird freigegeben |
| i | `main` kehrt zurück, `exit()` ruft `init_stream+0xdc` → `malloc(0x400)` auf (x0 = `0x400` = BUFSIZ) → `malloc+0x44` `ldr x19,[x10,#0x10]` = `q->size` mit `q` = `_mchunk_free_list.next` = `0x01a57000` (`mintlib/malloc.c:57-58`) | VFORKT | **Fault** `0x01a57010`, „free memory, read“ |

In der Regionstabelle (`tos-api/src/bdos/mem/region.rs`) hatte keiner der beiden Blöcke je einen weiteren Besitzer. Einen Region-Eintrag bekommen nur die Pexec-5-Basepage und -Umgebung des Kindes (`pexec.rs:386-390`, `region::attach`). Zum Zeitpunkt des Faults steht `0x01a57000` auf keiner Liste mehr, die Seite ist für alle ungültig (`memprot.rs:27`, I8).

Mit dem Produktkernel ohne Injektor (`serial/vt-on-s2all-serial.log`) ist die Folge identisch, ebenso Fault-Adresse und PC wie im Post-E9-Lauf.

**Sicherheit: hoch.** Jede Allokation und jede Freigabe ist mit Aufrufer protokolliert. Die Disassemblierung bestätigt `tfork`/`startup`/`malloc` (Abschnitt „Belege“).

## 2. Warum nur mit memprot=on (und nicht bei off/strict)

Der Fehler hat zwei Bedingungen. (1) Der `malloc`-Block muss an der Adresse der freigegebenen Umgebung landen, damit das dritte `Mfree` ihn trifft. (2) Der Zugriff auf freien Speicher muss auffallen.

* **on** (S2ALL): Pool-Raster eine Seite (E16). Die Umgebung ist eine Seite bei `0x01a57000`. Die Seite darüber (`0x01a58000`, bis zum TPA bei `0x01a8a000`) ist frei, denn OWNPARs `Malloc` dort wurde vorher freigegeben. Das Loch ist also mindestens 3 Seiten groß, und `Malloc(0x2000)` landet genau bei `0x01a57000` → (1) erfüllt. Freier Poolspeicher ist ungültig → (2) erfüllt → Fault.
* **off** (`serial/vtc-off-s2all-serial.log`): Raster 16 Byte. Die Umgebung ist 16 Byte groß (`0x010ae720`). Das Loch nach ihrer Freigabe reicht nicht für 8 KiB, also landet der Block bei `0x010ae780`. Das dritte `Mfree(0x010ae720)` liefert `EIMBA`, und der `malloc`-Block bleibt heil. Die Umgebung ist trotzdem freigegeben. Ohne Schutz wäre ein Zugriff auf freien Speicher ohnehin stumm geblieben.
* **strict** (`serial/vtc-strict-s2alls-serial.log`): Raster eine Seite. Das S2ALLS-Layout legt den TPA von VFORKT aber direkt hinter die Umgebung (`0x01a58000`). Das Loch ist eine Seite groß, der Block landet bei `0x01a84000`, das dritte `Mfree` liefert `EIMBA`. `strict` ändert nur die Regel für Kinder von `Pexec` 0/6 (E5), nicht `Pexec` 4. Der Unterschied liegt allein im Layout.

Der Schutz verursacht also nichts. Er bringt eine Freigabe ans Licht, die in allen drei Modi stattfindet (Schritt d steht in allen drei Logs). Ob die zweite, schädliche Freigabe (h) trifft, hängt vom Layout ab.

**Sicherheit: hoch** (alle drei Modi mit demselben Kernel protokolliert).

## 3. Warum das alte crt0 (Pterm) den Fehler verdeckt

`serial/vtc-on-s2all-oldcrt0-serial.log`: Die Speicherfolge a–h ist **identisch**, einschließlich `free 0x01a57000 len=0x2000` durch den Elternprozess. Danach kommt aber kein `malloc` mehr. Der letzte `malloc`-Aufruf von VFORKT liegt vor `system()` (`setenv`), und `check()`/`snprintf` benutzen Stack-Puffer. Das alte crt0 ruft nach `main` direkt `Pterm`. Die verwaiste Freiliste wird nie durchlaufen, S2ALL meldet PASS. Das neue crt0 ruft `exit()` → Stream-Abschluss → erster stdio-Puffer per `malloc(BUFSIZ)` → Durchlauf → Fault.

Das alte crt0 behebt also nichts. Ein Programm, das nach zwei vforks (oder nach `system()` in einem Programm mit Umgebung) noch `malloc` aufruft, stürzt auch damit ab, sofern das Layout passt.

Einzelstart per `#Z` (`serial/vtc-on-vforkt-alone*.log`): Der Kernel-Starter übergibt keine Umgebung (`EnvSpec::None`, `p_env = 0`, `kernel/src/launch.rs:71-80`). `tfork` ruft deshalb nur `Mfree(0)` auf → `EIMBA`, mit beiden crt0 harmlos. Die Notiz „Layout“ im Statusbericht bedeutet also genauer: **keine Umgebung**.

**Sicherheit: hoch.**

## 4. EmuTOS und FreeMiNT in der entsprechenden Lage

### EmuTOS (kein vfork; was mintlibs TOS-Zweig dort tut)

* Ein Prozess hat immer eine Umgebung: `alloc_env` kopiert bei `env == NULL` die von `run` (`ET/bdos/proc.c:413-432`), also gilt `p_env ≠ 0`. Bei `PE_LOADGO` gehören TPA und Umgebung dem Kind (`proc.c:305-307`).
* `Pexec 5`: Basepage und Umgebung gehören dem **Aufrufer** (`proc.c:229-230`, `set_owner(…, run)`).
* `Pexec 4`: Der Besitz bleibt unverändert (`proc.c:243-247`, nur `proc_go`). `proc_go` setzt `p_parent = run` (`:541`) und `run = p` (`:575`).
* Was das Kind per `Malloc` holt, gehört ihm (`ET/bdos/iumem.c:134`, `q->m_own = run`). Sein `Pterm` gibt genau das frei: `xterm` → `ixterm` → `free_all_owned(r, …)` (`proc.c:601-616`, `:92-101`, `:136-139`).
* `Mfree` prüft **keinen Besitzer** (`ET/bdos/umem.c:135-159`). Jeder Block, an dessen Startadresse aufgerufen wird, wird freigegeben.

Damit läuft der TOS-Zweig von mintlib auf EmuTOS so ab: Das Kind gibt die Pexec-5-Umgebung des Elternprozesses frei (erlaubt, kein Besitztest) und übernimmt in `b->p_env` den Zeiger auf die Umgebung des Elternprozesses. Nach seinem `Pterm` gibt der Elternprozess mit `Mfree(b->p_env)` **seine eigene Umgebung** frei (`thread.c:136`). Jedes weitere vfork gibt dann den Block frei, der inzwischen an dieser Adresse beginnt. Das ist dieselbe Folge wie unter NewTOS. Ohne MMU bleibt der Speicher lesbar, und der Schaden zeigt sich erst, wenn jemand den Speicher neu vergibt. **[Schluss aus dem Quelltext; nicht auf EmuTOS/Hatari ausgeführt, VFORKT gibt es nicht als m68k-Programm.]**

Ein Kind aus `Pexec` 4 besitzt in EmuTOS also das, was es selbst per `Malloc` holt, und verliert es bei seinem `Pterm`. Dasselbe Modell hat NewTOS (`pexec.rs:331-336`). Für diesen Absturz spielt das aber keine Rolle: Das Kind holt hier nur `0x405000` (`_spawnve`) und gibt ihn selbst wieder frei.

### FreeMiNT

* Auf MiNT nimmt mintlib den TOS-Zweig gar nicht. `vfork` ruft `Pvfork` auf (`mintlib/unix/vfork.S:21-26`, `newtos-arch/vfork.S` `.Ltos`/`__mint`). `tfork` würde dort `Pexec(104)` nehmen (`thread.c:104`), und das `Mfree(b->p_env)` nach `PE_GO` liegt im `-ENOSYS`-Zweig, den MiNT nie erreicht.
* `sys_pvfork` forkt mit `FORK_SHAREEXT`, **ohne** `FORK_SHAREVM` (`FM/k_fork.c:252-260`). Das Kind bekommt mit `copy_mem` eine Kopie der Regionsliste, jede Region `links++` (`FM/proc_help.c:119-171`, `:171`). Den Speicher selbst teilen sich beide, denn der Elternprozess schläft. Was das Kind neu alloziert, hängt nur an ihm und wird bei seinem Ende abgelöst bzw. freigegeben. Das entspricht dem vfork-Vertrag, wonach das Kind nur `_exit`/`exec` aufrufen darf.
* `Pexec 104`/`106` (Thread) forkt mit `FORK_SHAREVM` (`FM/k_exec.c:415-421`) und teilt `p_mem` (`FM/k_fork.c:143-146`).
* `Mfree` löst nur den Aufrufer ab, und die Region wird frei, wenn `links` 0 erreicht (`FM/dosmem.c:331-358`, `detach_region_by_addr`). Den eigenen Basepage-Block freizugeben verweigert FreeMiNT (`:351-355`), die eigene Umgebung nicht. Liefe der TOS-Zweig von `tfork` auf MiNT, würde der Elternprozess (alleiniger Besitzer) seine Umgebung auch dort freigeben.

**Sicherheit:** EmuTOS-Besitzregeln hoch (Quelltext gelesen). „Auf EmuTOS derselbe latente Fehler“ mittel-hoch (Schluss, nicht ausgeführt). FreeMiNT mittel (Quelltext gelesen, kein Lauf).

## 5. Wege zu einer Behebung

| Option | Grundlage | Bewertung |
|---|---|---|
| **A. `tfork` im SDK-Overlay korrigieren** (`sdk/libc/overlay/mintlib/thread.c`). Den Pexec-5-Umgebungszeiger **vor** `Pexec(PE_GO)` sichern und nach dem Kind diesen gesicherten Zeiger freigeben, nicht das von `startup` überschriebene `b->p_env`. Gleiches gilt für den Fehlerzweig `:142`. | Das ist ein Fehler im unveränderten mintlib-Code. Die Behebung gehört in die Werkzeugkette (`port-target-app-unmodified`, `port-reuse-dont-rewrite`: mintlib ist Werkzeugkette, `thread.c` liegt schon im Overlay). EmuTOS: Der Elternprozess gibt nur noch den Block frei, den sein `Pexec 5` angelegt hat (`proc.c:229-230`, ihm gehörig). Ist der schon vom Kind freigegeben (EmuTOS, kein Besitztest), liefert `Mfree` `EIMBA`, harmlos. | **Empfohlen.** Das behebt die Ursache auf jedem TOS. Ohne Kerneländerung verschwindet nebenbei auch das Seitenleck je vfork unter NewTOS (Nebenbefund), weil dann der Elternprozess als `m_own` die Pexec-5-Umgebung freigibt. Vorher prüfen, ob eine neuere mintlib-Version das schon geändert hat (keine im Repo; nicht ausgedacht). |
| B. Kernel: dem Aufrufer `Mfree` der eigenen `p_env` verweigern | Keine. FreeMiNT verweigert nur die eigene Basepage (`FM/dosmem.c:351-355`), EmuTOS gar nichts (`umem.c:135-159`). | Abgelehnt: erfundene Regel, weicht von EmuTOS ab. |
| C. „`Malloc` eines Pexec-4-Kindes gehört dem Raum“ (Option a des Statusberichts) | FreeMiNT `FORK_SHAREVM` (`k_fork.c:143-146`), E6 | **Behebt diesen Absturz nicht.** Kein Kind-`Malloc` ist beteiligt, der Block gehört dem Elternprozess, und dieser gibt ihn selbst frei. Höchstens eine eigene Designfrage. |
| D. crt0 zurück auf `Pterm` (Option c des Statusberichts) | — | Verdeckt den Fehler nur, siehe Abschnitt 3. Die Umgebung und der `malloc`-Block werden weiter freigegeben, während sie in Gebrauch sind. Zusätzlich fehlen die atexit-Funktionen und der stdio-Flush. |
| E. Kernel-Starter gibt eine Umgebung mit (wie EmuTOS `alloc_env`) | `proc.c:413-432` | Keine Behebung. Damit tritt der Fehler auch beim Einzelstart auf. Eigene EmuTOS-Abweichung (`launch.rs:71-80`), siehe unten. |

**Empfehlung: A.** Danach als Prüfung S2ALL unter on/off/strict und VFORKT einzeln. Erwartung: Das Protokoll zeigt kein `Mfree` auf die Umgebung des Elternprozesses mehr, der Elternprozess gibt die Pexec-5-Umgebung frei, und VFORKT meldet einen ST-Delta von 0.

## Fix (Option A, umgesetzt 2026-10-06)

`sdk/libc/overlay/mintlib/thread.c` `tfork`: Vor dem Start des Kindes wird `b->p_env` (die Umgebung aus `Pexec(PE_CBASEPAGE)`) in `child_env` gesichert. Nach `Pexec(PE_GO)` und im Zweig `pid < 0` gibt der Elternprozess `child_env` frei statt `b->p_env`. Kommentar im Overlay mit Herkunft (mintlib 0.60.1, beide Stellen `Mfree(b->p_env)`). Eine neuere mintlib mit anderer Lösung liegt weder im Repo noch unter `references/`. Keine Kerneländerung.

Belege `measurements/measurement-2026-10-06-vforkt-fix/` (`RUNINFO.TXT`):

* S2ALL unter on: **PASS** (Produkt- und Crash-Kernel). off: PASS, strict (S2ALLS): PASS. VFORKT allein per `#Z` in allen drei Modi: PASS.
* Speicherprotokoll (`serial/vt-vf-on-s2all-serial.log`): Pro vfork gibt der Elternprozess jetzt `0x400000` frei (die Pexec-5-Umgebung) und danach die Basepage. Ein `Mfree` auf seine Umgebung `0x01a57000` gibt es nicht mehr. Sie wird erst beim `Pterm` von VFORKT frei. Der `malloc`-Block aus `setenv` landet bei `0x01a58000` und bleibt bis zum `Pterm` belegt.
* Das Seitenleck je vfork (Nebenbefund 1) ist auf der mintlib-Seite weg. VFORKT meldet „ST largest block before 10485760 after 10485760“ (vorher Delta 8192). Die Kernel-Abweichung bleibt bestehen: `Mfree` eines weiteren Besitzers löst nur ab. Deshalb liefert auch das `Mfree` des Elternprozesses auf `0x400000` 0, unter EmuTOS wäre es `EIMBA`.
* Gegenprobe ohne Abweichung: Hosttests 1138/0/1, Warnungen 64/94, Clippy 76/125, Drift 31, SDK-Hosttests 13/0/0. Die Post-E9-Reihe ist in allen drei Modi wie p1. Ausnahme ist ein einmaliger KBDVEC-Ausreißer unter strict (Injektor-Timing, zweimal wiederholt PASS).

## Nebenbefunde (unverändert, nicht Teil des Auftrags)

* **Ein Seitenleck je vfork unter NewTOS.** Das Kind ruft `Mfree` auf die Pexec-5-Umgebung auf, die seinem Elternprozess gehört und an der es selbst nur als weiterer Besitzer hängt (`pexec.rs:390`). `release_as_caller` → `region::detach` → `Kept` → `E_OK` (`tos-api/src/gemdos/memory.rs:141-161`), der Block bleibt belegt. EmuTOS gibt ihn frei (`umem.c:135-159`). Belege: `free 0x00400000/0x401000/0x402000` erscheinen erst beim `Pterm` von VFORKT. VFORKTs „ST largest block before 10485760 after 10477568“ (Delta 8192) sind genau diese zwei Seiten. Option A beseitigt das Leck auf der mintlib-Seite. Die Kernel-Abweichung (FreeMiNT-Ablösung statt EmuTOS-Freigabe bei `Mfree` eines weiteren Besitzers) bleibt eine eigene Frage.
* Kernel-Starts erhalten keine Umgebung (`p_env = 0`, `launch.rs:71-80`). EmuTOS gibt jedem Prozess eine (`proc.c:413-432`).
* `S2ALL`/`VFORKT` prüfen „children's TPAs went back“ mit einer Toleranz von 64 KiB und verdecken so das Leck.

## Belege

* `measurements/measurement-2026-10-06-vforkt/RUNINFO.TXT`: Läufe, Kernel- und Programm-Hashes.
* `serial/vtc-on-s2all-serial.log` (Abschnitt 1), `vt-on-s2all-serial.log` (Produktkernel + Protokoll), `vtc-on-s2all-oldcrt0-serial.log` (3), `vtc-off-s2all-serial.log`, `vtc-strict-s2alls-serial.log` (2), `vtc-on-vforkt-alone*.log` (3).
* `method/vt-instrumentation.py` (nur Scratch-Kopien), `method/build-vforkt.sh` (VFORKT mit wählbarem crt0, mit exit-crt0 bytegleich zu `post-e9/bin/VFORKT.NAP`), `method/sym.py`, `method/runs-vforkt.sh`.
* Disassemblierung `post-e9/bin/VFORKT.NAP`:
  * `tfork+0x1a8..0x1d4`: `ldr w0,[x19,#0x2c]; mov w8,#0x49; svc #1` (Mfree `b->p_env`), dann `mov x0,x19; svc #1` (Mfree `b`).
  * `startup+0x1c..0x34`: `ldr w0,[x19,#0x2c]; svc` (Mfree eigene Umgebung), danach `ldr w8,[x20,#0x2c]; str w8,[x19,#0x2c]` mit `x20` = `p_parent`.
  * `malloc+0x44`: `ldr x19,[x10,#0x10]` (`q->size`).
  * `init_stream+0xdc`: `bl malloc`.
  * `setenv+0x140`: `bl realloc`.
* mintlib 0.60.1 pristine `mintlib/thread.c:58-59,127,133` = Overlay `:67-68,136,142` (der Overlay ändert nur Casts, Diff in dieser Sitzung geprüft).
