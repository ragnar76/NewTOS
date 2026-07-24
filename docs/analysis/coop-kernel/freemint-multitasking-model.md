# FreeMiNT-Multitasking-Modell — Source-Extraktion für den NewTOS-Kernel-Vergleich

**Scope:** Ausschließlich FreeMiNT (`references/freemint/`, gepinnter Clone
`freemint/freemint` @ `f906416a9387ed68876c738cfc5c30037b294fc0`, verifiziert via
`git log -1` in `references/freemint`). Ziel: die Scheduling-/Signal-/IPC-/
Speicherschutz-Mechanik extrahieren, damit der Lead "NewTOS-Kernel
EmuTOS-kooperativ vs. FreeMiNT-präemptiv+Signale" beziffern kann.

Jede strukturelle Aussage ist mit `datei:zeile` aus dem gepinnten Clone belegt.
Wo ich vom Lesen zur Interpretation übergehe, ist das explizit als "Befund"
vs. "Einordnung" markiert. EmuTOS-Aussagen sind bewusst auf das Nötigste
beschränkt und mit Verweis auf das bereits etablierte Projektwissen
(`docs/MEMORY.md`) markiert — die EmuTOS-Extraktion selbst ist Aufgabe des
parallelen EmuTOS-Researchers, nicht dieses Dokuments.

---

## 1. Der präemptive Scheduler

### 1.1 Datenstrukturen

`struct proc` (der Prozess-Deskriptor) — `references/freemint/sys/mint/proc.h:119-289`.
Relevante Felder:

- `sysstack` (proc.h:121) und `ctxt[PROC_CTXTS]` (proc.h:122) — **müssen** laut
  Kommentar die ersten beiden Felder sein (Assembler-Offsets in `syscall.S`/
  `context.S` sind hart codiert, s.u.).
- `pid, ppid, pgrp` (proc.h:127).
- `pri` (proc.h:138, Basis-Priorität) und `curpri` (proc.h:164, aktuelle,
  gealterte Priorität) — getrennte Felder für Basis- vs. Laufzeit-Priorität.
- `wait_q` (proc.h:139) — die Queue, auf der der Prozess gerade sitzt (Zustand,
  s. 1.3).
- `wait_cond` (proc.h:146) — die Bedingung, auf die gewartet wird (ein
  beliebiger `long`, typischerweise eine Funktionsadresse oder ID — dient als
  Schlüssel für `sleep()`/`wake()`, s. 1.2).
- `slices` (proc.h:222-224) — Zähler "wie oft wird dieser Prozess noch
  übersprungen, bevor er wieder dran ist" (Aging-Mechanismus, s.u.).
- `p_mem, p_cred, p_fd, p_cwd, p_sigacts, p_limits, p_ext` (proc.h:186-192) —
  sharbare Teilstrukturen (Adressraum, Credentials, offene Dateien, cwd,
  Signal-Actions, Limits, Erweiterungen). `fork()`-Varianten teilen oder
  kopieren diese selektiv über Flags (s. 2.2).
- `sigpending`, `p_sigmask`, `nsigs` (proc.h:235-238) — Signal-Zustand.
- `q_prev`, `q_next`, `gl_next` (proc.h:262-265) — Verkettung in der
  Warteschlange bzw. globalen Prozessliste.
- `stack[STKSIZE+4]` (proc.h:288) mit `STKSIZE = ISTKSIZE + SSTKSIZE`
  (proc.h:89-91, 8 KiB Interrupt- + 16 KiB Supervisor-Stack) — der
  **Kernel-Stack** dieses Prozesses; jeder Prozess hat seinen eigenen.

`CONTEXT` (`struct context`) — `references/freemint/sys/mint/arch/context.h:41-82`:
`regs[15]` (D0-D7/A0-A6), `usp`, `sr`, `pc`, `ssp`, `term_vec`, FPU-State
(`fstate`, `fregs`, `fctrl`), und **`crp`/`tc`** (context.h:70-71) — die
gesicherten MMU-Root-Pointer-/Control-Register für Speicherschutz (s. 5).
`PROC_CTXTS = 2` (context.h:84): jeder Prozess hat zwei Kontext-Slots,
`SYSCALL` (0, der beim Trap-Eintritt gebaute Kontext) und `CURRENT` (1, der
beim Scheduler-Wechsel gesicherte Kontext) — context.h:85-86.

### 1.2 Die Kern-Primitiven: `sleep()`/`wake()`/`preempt()`

Alle Blockier-Mechanismen in FreeMiNT (IPC, I/O-Wait, Signal-Stop,
Zeitscheiben-Verzicht) laufen über **eine** Primitive:

- `sleep(que, cond)` — `sys/proc.c:490-658`. Legt `curproc` auf Queue `que`
  mit Bedingung `cond`, sucht den nächsten lauffähigen Prozess in `READY_Q`
  (FIFO-Liste mit Aging, s.u.), sichert den aktuellen Kontext via
  `save_context()` (proc.c:631) und schaltet mit `change_context()`
  (proc.c:654) auf den neuen Prozess um (**echter** Kontextwechsel inkl.
  MMU-Root-Pointer, s. 1.4/5). Rückgabewert: `1`, wenn seit dem letzten Sleep
  keine Signale eingetroffen sind, sonst `0` (proc.c:490-491, 553, 639).
- `wake(que, cond)` (proc.c:703-716) / `iwake(...)` (proc.c:754-774, IRQ-sichere
  Variante) — verschiebt alle Prozesse auf `que` mit passender `wait_cond`
  von `que` nach `READY_Q` (`do_wake`, proc.c:665-701). Weckt **nicht** selbst
  um; der geweckte Prozess läuft erst beim nächsten `sleep()`-Aufruf eines
  anderen Prozesses oder beim nächsten präemptiven Wechsel.
- `preempt()` (proc.c:351-368) — Aufgerufen "by the vbl routine and/or the
  trap handlers" (Kommentar proc.c:345-347). Bestraft den unterbrochenen
  Prozess (`curpri -= 1`, proc.c:363-364, außer bei aktivem Bconout-Puffer)
  und ruft `sleep(READY_Q, curproc->wait_cond)` (proc.c:367) — Präemption ist
  intern nichts anderes als ein erzwungenes `sleep()` auf die Ready-Queue.
- `add_q`/`rm_q` (proc.c:299-342) — FIFO-Verkettung auf einer der
  `NUM_QUEUES = 8` Warteschlangen (`sysq[]`, proc.c:56).

**Befund:** Es gibt **keine** Interrupt-Stack-basierte Präemption *innerhalb*
einer Syscall-Ausführung — der Kontextwechsel selbst geschieht immer
synchron innerhalb von `sleep()`, aufgerufen entweder freiwillig (IPC/IO) oder
von `preempt()` (Timer/Trap-Exit, s. 1.3).

### 1.3 Prozesszustände (Queues)

`references/freemint/sys/mint/proc.h:296-305`:

```
CURPROC_Q = 0   READY_Q = 1   WAIT_Q = 2   IO_Q = 3
ZOMBIE_Q  = 4   TSR_Q   = 5   STOP_Q  = 6   SELECT_Q = 7
```

Menschenlesbare Namen bestätigt in `DUMPPROC()`:
`references/freemint/sys/proc.c:815-822` (`"run", "ready", "wait", "iowait",
"zombie", "tsr", "stop", "select"`). `CURPROC_Q` (0) ist kein eigenständiger
Wartezustand, sondern der Wert, den `rm_q()` einem Prozess zuweist, der
gerade von keiner Liste (mehr) verkettet ist — insbesondere der laufende
Prozess selbst (proc.c:340). Der Zustandsübergang ist strikt: ein Prozess ist
entweder `CURPROC_Q` (läuft/nicht verkettet) oder exakt einer der sieben
Queues zugeordnet (`add_q`/`rm_q`, proc.c:299-342, mit `assert`s, die
Doppelverkettung verhindern).

### 1.4 Zeitscheiben / Priorität

- `time_slice` (proc.c:65, Default `2`) — konfigurierbar über `mint.cnf`
  (`KERN_SLICES`/`SLICES`, `sys/cnf_mint.c:656,689`) oder zur Laufzeit via
  `Ssystem()` (`sys/ssystem.c:400-401`, Bereich 0-20).
- `proc_clock` (proc.c:54, `unsigned short`) — der laufende Countdown "Ticks
  bis zur nächsten Präemptions-Prüfung". Ein Tick = **20 ms**, belegt durch
  den Kommentar in `remaining_proc_time()`
  (`proc.c:226-234`, `proc_ms = proc_clock; proc_ms *= 20;`) und durch die
  4:1-Untersetzung der 5-ms-Timer-ISR zur 20-ms-"VBL"-Routine
  (`sys/arch/intr.S:77-85`, `vblcnt` zählt 4 Interrupts). **Korrektur der
  Aufgabenstellung:** Es existiert kein Makro namens `TIMESLICE` im
  FreeMiNT-Quelltext — die Zeitscheibengröße ist die Variable `time_slice`
  (in Einheiten von 20-ms-Ticks), nicht ein fester Compile-Time-Wert.
- `SLICES(pri)` — `sys/proc.h:20`: `((pri) >= 0) ? 0 : -(pri)`. Eine
  **negative** `curpri` bedeutet "so oft in der Ready-Queue überspringen,
  bevor dieser Prozess drankommt" — das eigentliche Prioritätsschema ist ein
  Skip-Counter, keine klassische Mehrfach-Prioritätsliste.
- Priorisierung/Aging: `reset_priorities()` (proc.c:242-255, 1×/Sekunde
  aufgerufen — Kommentar proc.c:238-240) setzt `curpri` auf `pri` zurück,
  damit Prozesse mit niedriger Priorität nicht verhungern. `add_q()`
  (proc.c:315-318) "belohnt" einen Prozess beim Verlassen einer Nicht-Ready-
  Queue durch Zurücksetzen von `curpri`/`slices`. `preempt()` (proc.c:363-364)
  "bestraft" per Dekrement von `curpri` (bis `MIN_NICE = -20`,
  `sys/mint/proc.h:166`).
- `MIN_NICE=-20`, `MAX_NICE=20` (mint/proc.h:166-167).

### 1.5 Auswahl-Algorithmus in `sleep()`

`sys/proc.c:591-628`: Traversiert die `READY_Q`-FIFO-Liste; für jeden
Kandidaten mit `slices > 0` wird `slices` dekrementiert und der nächste
Kandidat geprüft; der erste mit `slices <= 0` wird ausgewählt (`rm_q`,
proc.c:628). Das ist **Round-Robin mit Skip-basiertem Priority-Aging**, keine
klassische Prioritäts-Bucket-Queue. Deadlock-Fallback: wenn `READY_Q` leer
ist, werden zunächst `SELECT_Q`-Schläfer geweckt (`wake(SELECT_Q, (long) nap)`,
proc.c:579), sonst wird `rootproc` (PID 0) zwangsweise ready gemacht
(proc.c:583-588), "just so we have someone to charge time to" (Kommentar
proc.c:576-577).

### 1.6 Der Kontextwechsel-Mechanismus (Assembler)

`references/freemint/sys/arch/context.S` definiert vier Routinen (Header-
Kommentar context.S:13-47 zitiert die Semantik):

- `build_context` (context.S:74-217) — baut einen `CONTEXT` aus dem
  Exception-Stack-Frame eines Traps (aufgerufen z.B. beim Syscall-Eintritt,
  `sys/arch/syscall.S:481`).
- `save_context` (context.S:219-303) — sichert den *aufrufenden* Kontext
  (klassisches `setjmp`-Muster, Rückgabe `0` beim ersten Durchlauf,
  Kommentar context.S:23-33).
- `restore_context` (context.S:305-442) — springt in einen zuvor
  gesicherten Kontext **innerhalb desselben Prozesses** zurück (kein
  MMU-Flush).
- `change_context` (context.S:445-592) — wie `restore_context`, aber für
  einen **anderen** Prozess: flusht die ATC (`pflusha`, context.S:484/480)
  und lädt `crp`/`tc` aus dem Zielkontext (context.S:483-487 auf 030,
  context.S:478-482 auf 040/060) — das ist der Speicherschutz-Kontextwechsel
  (Analogon zu einem `TTBR0_EL1`-Switch + TLBI auf AArch64).

**Befund:** Nur bei aktivem `WITH_MMU_SUPPORT` und `!no_mem_prot`
(s. Abschnitt 5) wird das MMU-Root-Register tatsächlich umgeschaltet;
andernfalls ist der m68k-Adressraum flach (wie EmuTOS).

### 1.7 Wo Präemption tatsächlich ausgelöst wird

Zwei unabhängige Trigger-Pfade, **beide** münden in `preempt()`:

**(a) Asynchron aus der Timer-ISR** — `sys/arch/intr.S:245-467` (`mint_vbl`,
läuft alle 20 ms, s. 1.4). Ablauf (intr.S:381-452):
1. `tst.w proc_clock; beq.s L_expired` (intr.S:381-382) — nur wenn die
   Zeitscheibe abgelaufen ist.
2. `btst #5,(sp); bne.s L_out` (intr.S:410-411) — **nur wenn der
   unterbrochene Code im User-Modus lief**; supervisor-mode Code wird von
   der Timer-ISR *nicht* präemptiert.
3. Floppy-Lock- und `in_kernel`-Prüfung (intr.S:413-424) — bei aktivem
   Floppy-Zugriff oder laufender Kernel-Operation kein Wechsel.
4. `build_context` auf den `SYSCALL`-Kontext-Slot (intr.S:427-431), dann
   `jsr preempt` (intr.S:452) — der unterbrochene User-Kontext wird wie ein
   Syscall-Eintritt behandelt und danach über `restore_context` (intr.S:466)
   fortgesetzt (entweder derselbe oder — nach dem in `preempt()`
   ausgelösten `sleep()` — ein anderer Prozess).

**(b) Synchron am Syscall-Austritt** — `sys/arch/syscall.S:583-598`: Nach
jedem GEMDOS/BIOS/XBIOS-Aufruf wird geprüft `tst.w proc_clock` (syscall.S:585)
und `btst #13,d0` (Supervisor-Rückkehr? syscall.S:587-588, dann kein
Wechsel) sowie das Floppy-Lock (syscall.S:589-590); ist die Zeitscheibe
abgelaufen und der Rücksprung geht in den User-Modus, wird `jsr preempt`
(syscall.S:597) direkt beim Trap-Austritt ausgeführt.

**Einordnung:** FreeMiNT ist damit *echt* präemptiv für User-Mode-Code (Trigger
a, 20-ms-Granularität, unabhängig davon ob der User-Prozess gerade in einer
Rechenschleife hängt), aber Kernel-Mode-Code (laufende Syscalls) wird nie
mitten in der Ausführung unterbrochen — Präemption für Kernel-Code ist an
Syscall-Grenzen gebunden (Trigger b). Das ist ein **Nicht-präemptiver
Kernel + präemptiver User-Space**-Hybrid, nicht vollständig symmetrische
Präemption wie z.B. ein moderner Linux-Kernel mit `CONFIG_PREEMPT`.

---

## 2. Prozess-/Kontextmodell — Erzeugung

### 2.1 `fork_proc1()` — `references/freemint/sys/k_fork.c:55-236`

Kopiert `struct proc` per Flat-Copy (`*p2 = *p1;`, k_fork.c:64), setzt dann
alle Zeiger auf Teilstrukturen zurück (k_fork.c:66-72) und baut sie über
`FORK_SHARE*`-Flags entweder als *shared* (Refcount-Inkrement,
`share_mem`/`share_fd`/`share_cwd`/`share_sigacts`, k_fork.c:143-180) oder als
*copy* (`copy_mem` etc.) neu auf. Neuer eigener Kernel-Stack
(`p2->sysstack`, k_fork.c:81-84), neue PID (`newpid()`, k_fork.c:77), Reset
der Signal-/Ptrace-/Timer-Felder (k_fork.c:79-109). Anhängen an die globale
`proclist` unter `splhigh()` (k_fork.c:207-215).

### 2.2 `Pfork()`/`Pvfork()` — dieselbe Datei, k_fork.c:251-448

- `sys_pvfork()` (k_fork.c:251-311): Kind teilt sich **keinen** eigenen
  Adressraum — der Elternprozess schläft (`sleep(WAIT_Q, (long)p)`,
  k_fork.c:297), bis das Kind terminiert oder per `Pexec` seinen eigenen
  Speicher überschreibt (klassisches Unix-`vfork`-Semantik).
- `sys_pfork()` (k_fork.c:325-448): Elternprozess-Speicher wird über
  Shadow-/Save-Regionen dupliziert (`fork_region`, k_fork.c:380) — auf einem
  MMU-losen 68000 die einzige Möglichkeit, zwei Adressräume "gleichzeitig"
  zu halten: bei jedem Kontextwechsel werden Original- und Shadow-Region
  physisch vertauscht (`swap_in_curproc()`, `sys/proc.c:375-409`,
  `quickswap()`-Aufruf proc.c:400/404). Das Kind wird mit hoher Priorität
  sofort lauffähig gemacht (`run_next(p, 3)`, k_fork.c:443) und der
  Elternprozess ruft `yield()` (k_fork.c:445).

**Einordnung:** Dieses Shadow-Region-Swapping ist ein **MMU-loser
Notbehelf** für Adressraumtrennung auf 68000-Systemen ohne PMMU — auf
030/040/060 mit `!no_mem_prot` läuft stattdessen echte MMU-Trennung über je
eigene Seitentabellen (Abschnitt 5). Für NewTOS/AArch64 (immer MMU) ist
dieser Swap-Mechanismus **nicht** relevant — die MMU-Variante ist das
architektonische Analogon.

### 2.3 Kernel-Threads — `references/freemint/sys/k_kthread.c:47-155`

`kthread_create()`/`kthread_create_v()` (k_kthread.c:61-135) ist **kein**
separates Leichtgewichts-Thread-Primitive — es ruft intern `fork_proc1()`
mit `FORK_SHAREVM|FORK_SHARECWD|FORK_SHAREFILES|FORK_SHAREEXT`
(k_kthread.c:72) auf. Ein "Kernel-Thread" ist also ein **vollwertiger**
`struct proc`, der sich den Adressraum mit dem Elternprozess teilt, mit
`p_flag |= P_FLAG_SYS` markiert (k_kthread.c:82, unkillbar) und dessen
initialer Kontext im Supervisor-Modus startet (`sr = 0x2000`,
k_kthread.c:109/115). **Befund:** FreeMiNT kennt architektonisch nur eine
Nebenläufigkeits-Einheit — den vollen Prozess (mit eigenem Kernel-Stack,
eigenem `CONTEXT`, eigenem Scheduler-Slot) — "Threads" sind lediglich
Prozesse mit geteiltem Adressraum, kein separates Leichtgewichts-Objekt.

---

## 3. Signale = der Upcall-Mechanismus

Das ist der für NewTOS direkt relevante Teil: FreeMiNTs Signal-Zustellung
**ist** exakt der EL1→EL0-Upcall, den NewTOS noch nicht hat.

### 3.1 Markieren / Prüfen — `references/freemint/sys/signal.c`

- `post_sig(p, sig)` (signal.c:145-225): Setzt Bit `sig` in `p->sigpending`
  (signal.c:203-204). Ist das Signal maskiert (`p->p_sigmask`), passiert
  sonst nichts (signal.c:212-213). Andernfalls wird der Zielprozess — falls
  er gerade wartet — von seiner Queue auf `READY_Q` verschoben
  (signal.c:216-224, unter `splhigh()`), **ohne** sofortigen Kontextwechsel.
- `check_sigs()` (signal.c:268-355): Wird aus `sleep()`
  (`sys/proc.c:452-454`, nur wenn `cond != (long) sys_pwaitpid`) und vom
  Syscall-Austritt (`sys/arch/syscall.S:600-610`, wenn noch unmaskierte
  `sigpending`-Bits übrig sind) aufgerufen. Iteriert die 32 Signalnummern
  (signal.c:298), maskiert das gerade behandelte Signal
  (`p_sigmask |= SIGACTION(...).sa_mask`, signal.c:323, klassische
  POSIX-`sa_mask`-Semantik) und ruft `handle_sig(i)` (signal.c:335).
- `handle_sig(sig)` (signal.c:370-474): Bei `SIG_IGN` → return
  (signal.c:380-381). Bei `SIG_DFL` → eingebautes Default-Verhalten
  (`SIGFPE`/`SIGWINCH`/`SIGCHLD` ignorieren, `SIGSTOP`/`SIGTSTP`/…→`stop()`,
  fatale Signale → `bombs()` + `terminate()`/`kernel_pterm()`,
  signal.c:386-467). Bei installiertem Handler → `sendsig(sig)`
  (signal.c:471), das architekturabhängige Frame-Bauen (s. 3.2).

### 3.2 Der eigentliche Upcall — `references/freemint/sys/arch/sig_mach.c`

`sendsig(ushort sig)` (sig_mach.c:45-252) — Kernstück:

1. **System-Prozess-Sonderfall** (sig_mach.c:100-121): Ein `P_FLAG_SYS`-
   Prozess (Kernel-Thread) kann nicht "in den User-Modus zum Signal-Handling"
   wechseln — der Handler wird **direkt als C-Funktionszeiger aufgerufen**
   (`((void (*)(short)) sigact->sa_handler)(sig);`, sig_mach.c:110), kein
   Frame-Bau, kein Trampolin.
2. **Normalfall** (sig_mach.c:123 ff.): Der zuletzt gesicherte Syscall-
   Kontext (`curproc->ctxt[SYSCALL]`, sig_mach.c:123) wird als Ausgangspunkt
   genommen. Auf dem **User- oder Supervisor-Stack des unterbrochenen
   Kontexts** (`stack = call->sr & 0x2000 ? call->ssp : call->usp`,
   sig_mach.c:147) werden zwei Fake-Stack-Frames aufgebaut
   (Kommentar sig_mach.c:125-131):
   - Ein `struct sigcontext` (PC/USP/SR des unterbrochenen Kontexts,
     sig_mach.c:38-43, 166-171) — das "was war vorher"-Trio.
   - Darüber ein Frame für den Aufruf `sig_handler(sig)` mit Rücksprung-
     Adresse `ut->sig_return_p` (sig_mach.c:175) — ein **Trampolin im
     Adressraum des Prozesses** (`struct user_things`, s.u.), das nach
     Rückkehr des Handlers `Psigreturn()` aufruft.
3. `call->pc = sigact->sa_handler;` (sig_mach.c:181) — der (gesicherte)
   Kontext wird so umgebogen, dass er beim nächsten Wiederanlauf **direkt
   im User-Handler** landet, mit dem präparierten Stack.
4. `save_context(&newcurrent)` (sig_mach.c:193) + `restore_context(call)`
   (sig_mach.c:228) — der modifizierte Kontext wird sofort aktiviert; der
   Prozess läuft im User-Modus im Signal-Handler weiter, **auf einem neuen,
   separaten System-Stack-Bereich** (`newstack`, sig_mach.c:135,215-220),
   damit rekursive/verschachtelte Signale (z.B. während `Supexec()`) den
   ursprünglichen System-Stack nicht überschreiben.
5. Rückkehr des Handlers → Sprung zum Trampolin (`sig_return_p`) → Syscall
   `Psigreturn()` (`sys_psigreturn()`, sig_mach.c:265-316): liest das
   `FRAME_MAGIC`-markierte Stack-Frame (sig_mach.c:275-281), stellt
   `curproc->sysstack` zurück (sig_mach.c:290), entmaskiert das behandelte
   Signal (sig_mach.c:291) und ruft `restore_context(oldctxt)`
   (sig_mach.c:311) — der ursprüngliche (durch `sendsig` unterbrochene)
   Kontext wird reaktiviert, `sendsig()` kehrt (im Sinne von `save_context`s
   "zweitem Durchlauf") an Zeile sig_mach.c:193 zurück und macht mit dem
   normalen Syscall-Austritt weiter (sig_mach.c:231-251).

**Das "User-Trampolin"** (`struct user_things`, referenziert in
`arch/user_things.h`, Feld `sig_return_p`/`pc_valid_return_p`,
sig_mach.c:164/175/271/293) ist ein pro-Prozess in den Adressraum
gemapptes kleines Codestück (analog zu einem VDSO-Trampolin), das den
Rücksprung vom User-Handler zurück in den Kernel realisiert, **ohne** dass
der Handler selbst `Psigreturn()` explizit aufrufen muss.

**Einordnung (das ist der für NewTOS zentrale Punkt):** FreeMiNTs
Signal-Zustellung ist strukturell identisch zu einem klassischen Unix-
Signal-Upcall: (1) EL1 präpariert einen künstlichen User-Stack-Frame und
biegt `PC`/`SP` des zu resumierenden Kontexts auf den Handler um, (2) der
Handler läuft im User-Modus, (3) ein Trampolin-Stub im User-Adressraum
sorgt für den kontrollierten Rücksprung per Syscall. Für NewTOS bedeutet das:
ein äquivalenter Mechanismus bräuchte (a) einen analogen Per-Prozess-
Trampolin im EL0-Adressraum, (b) das Umbiegen von `ELR_EL1`/`SP_EL0` im
`ExceptionFrame` beim nächsten `eret`, und (c) einen Rücksprung-Syscall
analog zu `Psigreturn()`. Nichts davon existiert in NewTOS heute (s.
Abschnitt 7).

### 3.3 Synchrone Exceptions → Signale

`sys/arch/sig_mach.c:479-577`: CPU-Exceptions werden auf Signale gemappt
(`sigbus()`→`SIGBUS`, `sigaddr()`→`SIGSEGV`, `sigill()`→`SIGILL`,
`sigpriv()`→`SIGPRIV`, `sigfpe()`→`SIGFPE`, `sigtrap()`→`SIGTRAP`), die dann
denselben `raise()`→`post_sig()`+`check_sigs()`-Pfad durchlaufen
(signal.c:360-365) — synchrone Fehler (Bus Error, Adressfehler,
Illegal Instruction) und asynchrone Signale (Kill, Interrupt) laufen durch
**dieselbe** Zustellmaschinerie.

---

## 4. IPC

Alle drei IPC-Mechanismen sind auf der `sleep()`/`wake()`-Primitive
aufgebaut — es gibt keinen separaten Blockier-Mechanismus für IPC:

### 4.1 `Pmsg()` — Rendezvous-Mailboxen, `references/freemint/sys/rendez.c:125-263`

`sys_p_msg(mode, mbid, ptr)`: Fünf-Wort-Nachricht (zwei `long` + PID,
Kommentar rendez.c:26-30). Wenn Leser/Schreiber bereits auf denselben `mbid`
wartet, wird die Nachricht direkt kopiert und der Partner geweckt
(`got_rendezvous`, rendez.c:168-222); sonst blockiert der Aufrufer via
`sleep(WAIT_Q, WAIT_MB)` in einer Schleife, bis `mb_mode == -1`
(rendez.c:242-244, `WAIT_MB = 0x3a140001L`, `mint/proc.h:149`). Mode `2`
(„write dann read") realisiert einen Request/Reply-Rendezvous, bei dem der
Schreiber synchron zum Leser auf `PDxx` (`0xFFFF0000 | pid`) umschaltet
(rendez.c:212-217).

### 4.2 `Psemaphore()` — Zähl-losem (Mutex-artigem) Semaphor, rendez.c:330-479

Globale verkettete Liste `semalist` (rendez.c:328) von `struct sema { id,
owner }` (rendez.c:321-326). Mode `2` (get) blockiert via
`sleep(WAIT_Q, WAIT_SEMA)` (rendez.c:405, `WAIT_SEMA = 0x3a140003L`,
mint/proc.h:150) mit optionalem Timeout über `addtimeout()`/`unsemame()`
(rendez.c:301-319, 398-401). Mode `3`/`1` (release/destroy) weckt alle
Wartenden via `wake(WAIT_Q, WAIT_SEMA)` (rendez.c:465) — danach müssen alle
Geweckten erneut um das Semaphor konkurrieren (`goto loop`, rendez.c:413).
Beim Prozess-Ende werden alle vom Prozess gehaltenen Semaphore automatisch
freigegeben (`free_semaphores()`, rendez.c:486-508, aufgerufen aus
`terminate()`).

### 4.3 Pipes — `references/freemint/sys/pipefs.c` (Dateisystem-Ebene)

Kurz geprüft, nicht tiefenanalysiert (außerhalb des Scheduling-Kernscopes):
Pipes sind als eigenes Pseudo-Filesystem implementiert (`pipefs.h`
eingebunden in `sys/dosfile.c:32`) und blockieren ebenfalls über
`sleep(IO_Q, (long)p)` (`sys/pipefs.c:878, 1019`) — dieselbe Primitive wie
alles andere.

---

## 5. Speicher-/Privilegienmodell

### 5.1 `struct memspace` — `references/freemint/sys/mint/proc.h:67-81`

Pro Prozess: Array von `MEMREGION*` (`mem[]`/`addr[]`, proc.h:73-74),
`page_table`/`pt_mem` (proc.h:75-76, die MMU-Seitentabelle dieses Prozesses),
`base` (proc.h:80, die TOS-`BASEPAGE`).

### 5.2 Der `no_mem_prot`-Schalter

**Zentraler Befund:** MMU-basierter Speicherschutz ist bei FreeMiNT
**optional und Hardware-abhängig**, kein Fixpunkt der Architektur:

- `init_page_table_ptr()` (`sys/proc_help.c:51-96`) legt nur dann eine
  Seitentabelle an, wenn `WITH_MMU_SUPPORT` compiliert **und**
  `!no_mem_prot` zur Laufzeit (proc_help.c:54-95).
- `no_mem_prot` wird автоматisch gesetzt, wenn keine PMMU erkannt wird
  (`sys/arch/init_mach.c:240-243`, `if (!no_mem_prot && !pmmu) ... no_mem_prot
  = 1;`) — z.B. auf einem 68000-System ganz ohne MMU — oder per Bootmenü
  explizit abgeschaltet (`sys/bootmenu.c:211`, `no_mem_prot =
  (strncmp(arg,"YES",3)==0) ? 0 : 1;` — invertierte Logik laut Kommentar
  bootmenu.c:211).
- Bei `no_mem_prot == 1`: flacher, ungeschützter Adressraum — **strukturell
  identisch zum EmuTOS-Modell** (ein Supervisor-Adressraum, keine
  Prozess-Isolation).
- Bei `no_mem_prot == 0` (030/040/060 mit erkannter PMMU): jeder Prozess
  bekommt eine eigene Seitentabelle; `change_context()`
  (`sys/arch/context.S:474-489`) lädt beim Kontextwechsel `crp`/`tc` aus dem
  Zielkontext und flusht die ATC (`pflusha`) — der Adressraumwechsel ist
  Teil **jedes** Kontextwechsels, nicht eines separaten Mechanismus.

**Einordnung:** FreeMiNT selbst ist also kein reines "Protected-Mode"-System
— es ist ein Kernel, der *optional* MMU-Schutz nutzt, wenn die Hardware ihn
hat, und andernfalls auf ein EmuTOS-artiges Flat-Modell zurückfällt. Für den
NewTOS-Vergleich zählt der **MMU-aktive Zweig** (030/040/060 =
`!no_mem_prot`), weil AArch64/RPi4 immer eine MMU hat — dieser Zweig ist
architektonisch das, was NewTOS bereits per-Prozess-`TTBR0_EL1` umsetzt
(s. Abschnitt 7).

---

## 6. XaAES — die AES als präemptiver Kernel-Client (nicht: kooperativer In-Prozess-Server)

### 6.1 Design-Rationale (Primärquelle: `references/freemint/xaaes/xaaes-kernel.txt`)

Das Dokument (Frank Naumann, 2003-06-05) begründet explizit den Umzug der
AES vom User-Space-Server-Prozess (mit Pipes + `Pmsg()`-Synchronisation,
xaaes-kernel.txt Abschnitt 4, "very complex and due to this a little bit
buggy ... the FreeMiNT kernel is allowed to preempt at any time") in ein
**Kernel-Modul**: "the AES belongs to the kernel and should run in kernel
space" (xaaes-kernel.txt Abschnitt 1). Ziel: "we can directly use the kernel
sleep/wake mechanism and hence self define at which points we taskswitch and
at which points not" (Abschnitt 4).

### 6.2 Trap-Dispatch läuft *im Kontext des aufrufenden Prozesses*

`XA_handler(void *_pb)` — `references/freemint/xaaes/src.km/handler.c:479-739`
— ist die zentrale AES-Trap-2-Routine. Sie wird **synchron innerhalb des
Kernel-Traps des aufrufenden Prozesses** ausgeführt (kein Message-Send an
einen separaten "AES-Prozess"): `cmd_routine = aes_tab[cmd].f;` (handler.c:612)
gefolgt von direktem Aufruf `cmd_rtn = (*cmd_routine)(lock, client, pb);`
(handler.c:639) — der aufrufende Prozess (`get_curproc()`, handler.c:514)
*ist* der Prozess, der die AES-Funktion ausführt. Optionales Bildschirm-Lock
(`lock_screen`/`unlock_screen`, handler.c:628-642) schützt gemeinsame
Zeichenoperationen.

Jeder AES-Client ist also **ein eigener, voller FreeMiNT-Prozess** —
`struct xa_client` hat einen Rückzeiger `struct proc *p`
(`references/freemint/xaaes/src.km/xa_types.h:2133`, "context back ptr") und
zusätzlich `struct proc *tp` (xa_types.h:2135, Kommentar "Thread" — ein
per-Client Kernel-Thread für asynchrone Arbeit, s.u.).

### 6.3 Blockieren = echtes `sleep()`/`wake()` auf `IO_Q`

Kann eine AES-Funktion (z.B. `evnt_multi`) nicht sofort erfüllt werden, blockiert
der aufrufende Prozess **direkt über den Kernel-Scheduler**, nicht über
Polling: `do_block()` (`xaaes/src.km/xa_evnt.c:334-359`) —
`client->sleepqueue = IO_Q; client->sleeplock = ...; sleep(IO_Q,
client->sleeplock);` (xa_evnt.c:352-355). Aufgeweckt wird der Client z.B. aus
einem Timeout-Callback via `wake(IO_Q, client->sleeplock)`
(`xa_evnt.c:507-518`, `wakeme_timeout()`), oder — für den `SELECT_Q`-Zweig —
via `wakeselect()` (xa_evnt.c:514-515, Aufruf derselben Kernel-Primitive wie
`sys/proc.c:781-799`).

Das `client->block`-Funktionszeigerfeld (xa_types.h:2149) ist austauschbar:
Standard-Clients nutzen `cBlock` (`xa_appl.c:170`, dispatcht während der
Blockade anstehende interne Redraw-/C-Events, `k_main.c` ~1178 zeigt eine
weitere Variante `iBlock` für einen internen/Interrupt-artigen Blockmodus,
und der AES-eigene Root-Client `C.Aes` nutzt `aesthread_block`
(`k_main.c:1778`)).

### 6.4 Der AES-eigene "Thread" ist wieder nur ein Prozess

`k_main.c` zeigt an mehreren Stellen `client->tp` — dieser Zeiger wird über
denselben `kthread_create()`/`fork_proc1(FORK_SHAREVM|…)`-Pfad wie jeder
andere Kernel-Thread besetzt (Abschnitt 2.3) — es gibt **keinen** separaten
"leichten AES-Thread"-Typ; ein XaAES-internes "Thread" ist exakt derselbe
Mechanismus wie ein beliebiger FreeMiNT-Kernel-Thread.

### 6.5 Vergleich zum kooperativen EmuTOS/NewTOS-Modell

**Befund (FreeMiNT-seitig, s.o.):** N reale, vom präemptiven Scheduler
verwaltete Prozesse rufen dieselbe In-Kernel-AES-Logik über einen
gemeinsamen Trap-Handler auf; Blockieren = echter Kontextwechsel via
`sleep()`/`wake()`.

**Einordnung (EmuTOS/NewTOS-seitig — bewusst knapp, siehe Scope-Hinweis
oben):** Laut bereits etabliertem Projektwissen (`docs/MEMORY.md`,
Einträge "Altitude: syscall compat, not HW emulation" und
"Single-tasking: app owns full screen") ist EmuTOS/NewTOS aktuell
Single-Tasking mit **einem** aktiven GEM-Programm, das den Bildschirm exklusiv
besitzt; kooperative Umschaltung (Accessories, Desktop) läuft *innerhalb
eines* Kontrollflusses ohne eigenen Prozess-Scheduler. Eine belastbare
file:line-Extraktion von EmuTOS' `evnt_multi`-Äquivalent konnte ich in der
knappen Prüfung dieses Scopes nicht lokalisieren (kein Treffer für
`evnt_multi` als Funktionsdefinition unter `references/emutos/aes/*.c` —
nur Aufrufstellen in `gemsuper.c`, `gemfmalt.c`, `geminit.c`). Diese
Detailtiefe liegt außerhalb des Auftragsumfangs dieses Dokuments (paralleler
EmuTOS-Researcher) — hier nur als **Nicht-behauptete, unbelegte Lücke**
vermerkt, nicht als Fakt verwendet.

---

## 7. Bezug zu NewTOS — was bereits vorhanden ist, was fehlt

Geprüfter NewTOS-Stand (2026-07-24, `desk-rebuild`-Branch):

### 7.1 Bereits vorhandene Primitiven (architektonisch näher an FreeMiNTs
geschütztem Modell als an EmuTOS' flachem Modell)

- **EL0/EL1-Split + MMU + Per-Prozess-`TTBR0_EL1`**: `l0_pa: AtomicU64`
  (`tos-api/src/bdos/process.rs:210-213`, "Phys of the per-process L0
  page-table root that gets written to `TTBR0_EL1`") — direktes Analogon zu
  FreeMiNTs `crp`/`tc`-Feldern im `CONTEXT` (Abschnitt 1.6/5.2) und deren
  Restore in `change_context()`. Der tatsächliche Umschaltcode
  (`install_ttbr0`, `kernel/src/arch/aarch64/exceptions.rs:412-422,
  581-618`) ist bereits vorhanden und wird bei jedem `Pexec`/`Pterm`-Wechsel
  genutzt.
- **Trap-Frame = Kontext**: `ExceptionFrame`
  (`kernel/src/arch/aarch64/exceptions.rs:52-68`, GPRs, `lr`, `elr`, `spsr`,
  `esr`, `sp_el0`) ist das AArch64-Analogon zu FreeMiNTs `struct context`
  (Abschnitt 1.1) — beide sind "alles was zum Fortsetzen eines
  unterbrochenen Kontrollflusses nötig ist".
- **Ein `Switch`-Deskriptor** existiert bereits (`tos-api/src/bdos/
  process/sched.rs:37-47`: `slot`, `fresh`, `l0_pa`, `entry_va`) — die
  Grundform für "was muss der Handler tun, um auf Prozess X zu wechseln" ist
  strukturell vorhanden, auch wenn sie heute nur kooperativ genutzt wird.

### 7.2 Explizit fehlend (laut eigenem Doc-Kommentar bereits als Erweiterungsrichtung vorgesehen)

- **Kein präemptiver Scheduler / keine Zeitscheiben:** `sched.rs:1-22`
  bezeichnet sich selbst explizit als "Cooperative round-robin scheduler …
  A peer gives up the CPU with `Pwait` … or by blocking on input" — es gibt
  keinen Timer-getriebenen `preempt()`-Äquivalent (FreeMiNT Abschnitt 1.7),
  keine `proc_clock`/`time_slice`-Zählung, keine Prioritäten/Aging
  (FreeMiNT Abschnitt 1.4).
- **Keine Signal-Zustellung / kein Upcall-Mechanismus:** Es existiert kein
  NewTOS-Äquivalent zu `sendsig()`/`Psigreturn()`
  (FreeMiNT Abschnitt 3.2) — kein Bau eines User-Stack-Signal-Frames, kein
  Per-Prozess-Trampolin im EL0-Adressraum, kein Rücksprung-Syscall. Das ist
  laut Aufgabenstellung "deferred EL0-upcall infra" — FreeMiNTs `sendsig()`
  ist die vollständige Referenzimplementierung dieses genauen Mechanismus,
  nur auf m68k statt AArch64 (PC/SP-Frame-Manipulation → `ELR_EL1`/`SP_EL0`
  im `ExceptionFrame`).
- **Keine Ready-Queue / keine Prozesszustände jenseits "runnable"**: `sched.rs`
  kennt nur ein Bit `runnable` pro Slot (`registry`-artige Flat-Iteration in
  `pick_next()`, sched.rs:81-90) — keine Unterscheidung `WAIT_Q`/`IO_Q`/
  `STOP_Q`/`SELECT_Q`/`ZOMBIE_Q` wie FreeMiNTs acht Queues (Abschnitt 1.3).
- **Kein `sleep()`/`wake()`-Äquivalent**, also auch kein einheitliches
  Blockier-Primitive für zukünftiges IPC (Pmsg/Psemaphore-Analoga, Abschnitt
  4) oder für einen AES-Client-Blockiermechanismus analog zu XaAES'
  `do_block()` (Abschnitt 6.3) — ein zukünftiges NewTOS-AES müsste, wenn es
  FreeMiNT/XaAES-artig auf echten Prozessen statt Kooperation aufgebaut
  werden soll, zuerst dieses Fundament besitzen.
- **Kein `fork()`-Äquivalent mit Shared/Copy-Semantik**: NewTOS' `registry.rs`
  ist ein reiner ELF-Blob-Lookup für `Pexec`, kein Prozess-Duplizieren
  (FreeMiNT Abschnitt 2.1/2.2 `fork_proc1`/`Pfork`/`Pvfork`) — für NewTOS
  potenziell irrelevant, da GEMDOS-`Pexec`-Programme klassischerweise selten
  `Pfork()` nutzen (TOS-Domäne), aber relevant, falls FreeMiNT-Kompatibilität
  (`freemint-extension-direction`-Memory-Eintrag) `Pfork`/Kernel-Threads
  einschließen soll.

**Aufwandsrelevante Einordnung für den Lead:** Der teuerste Einzelbaustein ist
nicht der Scheduler-Algorithmus selbst (FreeMiNTs Round-Robin-mit-Skip ist
konzeptionell einfach und mit den bereits vorhandenen `Switch`/`TTBR0`-
Bausteinen relativ direkt nachbaubar), sondern **der Signal-Upcall**
(Abschnitt 3.2): Er verlangt einen Per-Prozess-User-Space-Trampolin,
korrektes Frame-Bauen im EL0-Stack, einen neuen "Rücksprung"-Syscall
(`Psigreturn`-Äquivalent) und sorgfältige Nested-Signal-Handhabung
(separater System-Stack-Bereich pro Signal-Verschachtelung,
sig_mach.c:133-144) — nichts davon hat heute in NewTOS auch nur ein
Teil-Fundament, während der TTBR0-Switch-Mechanismus (der andere große
FreeMiNT-Baustein) in NewTOS bereits produktiv ist.

---

## 8. Key-Primitiven-Tabelle

| Dimension | FreeMiNT | NewTOS heute |
|---|---|---|
| **Scheduler-Typ** | Präemptiv (User-Mode, 20-ms-Timer-ISR, `intr.S:245-467`) + synchrone Prüfung an jeder Syscall-Grenze (`syscall.S:583-598`); Kernel-Mode-Code selbst nie mitten in der Ausführung präemptiert | Rein kooperativ: Wechsel nur bei `Pwait` (GEMDOS 0x4D) oder explizitem Blockieren (`sched.rs:1-22`) |
| **Scheduling-Policy** | Round-Robin über FIFO-`READY_Q` mit Skip-Counter-Priorität (`SLICES(pri)`, `proc.h:20`) + 1×/s Aging (`reset_priorities()`, `proc.c:242-255`) | Round-Robin über alle `runnable`-Slots, keine Priorität (`sched.rs:81-90`) |
| **Kontextwechsel-Mechanismus** | `save_context`/`change_context` (Assembler, `context.S`); inkl. optionalem MMU-`crp`/`tc`-Reload + `pflusha` (`context.S:474-489`) | `install_ttbr0` + `ExceptionFrame`-Restore beim `eret` (`exceptions.rs:412-422`); TTBR0-Switch bereits vorhanden |
| **Prozesszustände** | 8 benannte Queues: `CURPROC/READY/WAIT/IO/ZOMBIE/TSR/STOP/SELECT` (`mint/proc.h:296-305`) | 1 Bit: `runnable` pro Slot (`sched.rs`, `registry`) — kein Zustandsautomat |
| **Signal-/Upcall-Mechanismus** | Voll ausgebaut: `post_sig`/`check_sigs`/`handle_sig` (`signal.c`) + architekturabhängiger `sendsig()`-Frame-Bau + User-Trampolin + `Psigreturn()` (`sig_mach.c:45-316`) | Nicht vorhanden ("deferred EL0-upcall infra") |
| **IPC** | `Pmsg` (Rendezvous-Mailbox, `rendez.c:125-263`), `Psemaphore` (`rendez.c:330-479`), Pipes (`pipefs.c`) — alle auf `sleep()`/`wake()` | Nicht vorhanden |
| **Speicher-/Privilegienmodell** | Optional: MMU-Schutz nur bei erkannter PMMU (`!no_mem_prot`, `init_mach.c:240-243`); sonst Flat-Modell wie EmuTOS. Bei MMU: Per-Prozess-Seitentabelle (`memspace.page_table`, `proc.h:75`) | Immer MMU (AArch64): Per-Prozess `TTBR0_EL1`/L0-Tabelle bereits vorhanden (`process.rs:210-213`) — näher am FreeMiNT-MMU-Zweig als am Flat-Zweig |
| **AES-Ausführungsmodell** | Ein Kernel-Modul-Trap-Handler (`XA_handler`, `handler.c:479-739`), der **im Kontext des aufrufenden realen Prozesses** läuft; Blockieren = `sleep(IO_Q, …)` (`xa_evnt.c:334-359`) | Kooperativ, Single-Tasking-App besitzt den Bildschirm exklusiv (Projektwissen, nicht in diesem Scope neu belegt) |

---

## Quellen (alle mit `datei:zeile`, gepinnter Clone `f906416a…`)

- `references/freemint/sys/mint/proc.h` — `struct proc`, Queues, Konstanten.
- `references/freemint/sys/mint/arch/context.h` — `struct context`, `PROC_CTXTS`.
- `references/freemint/sys/proc.c` — Scheduler-Kern (`sleep`/`wake`/`preempt`/`add_q`/`rm_q`/`reset_priorities`).
- `references/freemint/sys/proc.h` — `SLICES`-Makro, externe Deklarationen.
- `references/freemint/sys/arch/context.S` — `build_context`/`save_context`/`restore_context`/`change_context`.
- `references/freemint/sys/arch/intr.S` — Timer-ISR (`mint_5ms`/`mint_vbl`), asynchrone Präemption.
- `references/freemint/sys/arch/syscall.S` — Syscall-Eintritt/-Austritt, synchrone Präemptionsprüfung.
- `references/freemint/sys/k_fork.c` — `fork_proc1`, `Pfork`, `Pvfork`.
- `references/freemint/sys/k_kthread.c` — Kernel-Thread-Erzeugung.
- `references/freemint/sys/signal.c` — Signal-Markieren/-Prüfen/-Default-Handling.
- `references/freemint/sys/arch/sig_mach.c` — `sendsig`, `Psigreturn`, Exception→Signal-Mapping.
- `references/freemint/sys/rendez.c` — `Pmsg`, `Psemaphore`.
- `references/freemint/sys/pipefs.c`, `sys/dosfile.c` — Pipes (grob geprüft).
- `references/freemint/sys/proc_help.c`, `sys/arch/init_mach.c`, `sys/bootmenu.c` — `no_mem_prot`, Seitentabellen.
- `references/freemint/xaaes/xaaes-kernel.txt` — Design-Rationale Kernel-Modul-AES.
- `references/freemint/xaaes/src.km/handler.c` — `XA_handler`-Trap-Dispatch.
- `references/freemint/xaaes/src.km/xa_types.h` — `struct xa_client`.
- `references/freemint/xaaes/src.km/xa_evnt.c`, `k_main.c`, `xa_appl.c` — Client-Blockierung, Kernel-Threads.
- NewTOS: `tos-api/src/bdos/process.rs`, `tos-api/src/bdos/process/sched.rs`,
  `tos-api/src/bdos/process/registry.rs`, `kernel/src/arch/aarch64/exceptions.rs`,
  `kernel/src/arch/aarch64/el0.rs`, `kernel/src/arch/aarch64/user_entry.rs`.

## Offene Punkte / STOP-Blöcke

- **Pipes (`pipefs.c`) nur oberflächlich geprüft** — Byte-Puffer-Semantik,
  Kapazität, `O_NONBLOCK`-Verhalten nicht extrahiert. Für die
  Aufwandsschätzung des Schedulers/Signal-Upcalls nicht kritisch; bei Bedarf
  Folgeauftrag.
- **EmuTOS-seitiger Vergleichspunkt in Abschnitt 6.5 bewusst unbelegt
  gelassen** (keine `file:line` für EmuTOS' `evnt_multi`-Äquivalent
  gefunden) — liegt im Scope des parallelen EmuTOS-Researchers, nicht
  nachträglich hier spekulativ ergänzt.
- **`Ssystem()`/Prioritäts-Syscalls (`Pnice` o.ä.) nicht extrahiert** — für
  die Kern-Scheduler-Mechanik nicht notwendig, aber falls der Lead
  Priorität-API-Kompatibilität mitschätzen will, wäre ein Folgeauftrag
  nötig (`sys/dossig.c`, `sys/dos.c` prüfen).
