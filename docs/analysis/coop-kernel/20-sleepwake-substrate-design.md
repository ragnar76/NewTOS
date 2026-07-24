# Sleep/Wake-Scheduler-Substrat — Design (kooperativer Kern, Schritt 3)

> Stand: 2026-07-24, Branch `desk-rebuild`. Design-Phase-Deliverable vor
> Implementierung. Richtungsentscheidung (Lead+User 2026-07-24):
> **EmuTOS-kooperative POLICY auf FreeMiNT-förmigen PRIMITIVEN**. Schritt 1
> (generischer EL0-Upcall, `ee82ead`) und Schritt 2 (`etv_term`-Dispatch via
> Upcall, `022c94c`) sind gelandet. Dieses Dokument ist **Schritt 3**: das
> Ready-Queue-/`sleep(cond)`/`wake(cond)`-Substrat, der `event.rs`-Busy-Poll-
> Refactor und die Deliver-on-Resume-Zustellung an blockierte AES-Clients.
>
> Vorgängerdokumente (in dieser Reihenfolge lesen): `00-effort-comparison.md`
> (Schritt-2-Zeile), `emutos-cooperative-model.md` (§2.2/§7.3), `freemint-
> multitasking-model.md` (§1.2/§1.3), `10-upcall-signal-design.md` (§8/§9).
>
> **Lesekonvention.** Jede *Mechanismus-/Struktur-Aussage* ist mit `datei:zeile`
> belegt (NewTOS `kernel/…`/`tos-api/…` oder Referenz `references/…`).
> Design-Entscheidungen sind **Empfehlung** (meine Wahl, begründet), echte
> Unentschiedenheiten **OFFEN** (Optionen + Empfehlung). Wo eine Quelle nicht
> belegbar war, steht **OFFEN**, keine erfundene Zahl.

---

## 0. Fakten- und Zitattabelle (Ausgangslage)

| Baustein | Zustand heute | Beleg |
|---|---|---|
| Kooperativer Round-Robin, `Switch{slot,fresh,l0_pa,entry_va}` | vorhanden (nur `Pwait`) | `tos-api/src/bdos/process/sched.rs:37-47` |
| Run-Set = ein Bit `runnable` pro Slot, Flat-Scan `pick_next` | vorhanden | `sched.rs:81-90`, `process.rs:275` |
| Frame-Save bei Yield (`saved_frame`) + Resume | vorhanden | `sched.rs:69-76`, `exceptions.rs:416-424,463-473` |
| TTBR0-Switch + EL0-Frame-Rewrite (`apply_switch`) | vorhanden, produktiv | `exceptions.rs:436-479` |
| Lifecycle-`State{Free,Loaded,Running,Terminated}` | vorhanden | `process.rs:84-96` |
| Prozesszustand jenseits `runnable` (Warte-Zustand/`wait_cond`) | **fehlt** | `sched.rs:1-22` (Selbstbeschreibung) |
| `sleep()`/`wake()`-Blockier-Primitive | **fehlt** — Busy-Poll im Trap-Handler | `event.rs:139-140,310-382,452` |
| Generischer EL0-Upcall + `Psigreturn` + Deliver-Check | **vorhanden** (Schritt 1/2) | `exceptions.rs:389-398,701-760,781-844` |
| Desktop-Ausführungsmodell | EL1-intern, ruft AES via **echtem `svc #2`** | `kernel/src/desk/mod.rs:101-114,512-533` |
| Launched GEM-App | echter EL0-Prozess (eigenes TTBR0, Scheduler-Peer) | `exceptions.rs:960-…` (`handle_pexec`), `sched.rs:62-67` |
| Input-Pfad (IRQ füllt Ring, kein Sched-State) | vorhanden | `kernel/src/drivers/gic.rs:166-182`, `kernel/src/host_env.rs:41-…` |
| EL0-IRQ-Route (für vex/Timer-Wake) | **fehlt** (`0x480 → __vec_unhandled`) | `kernel/src/arch/aarch64/exception_vectors.s:108` |

---

## 1. Ziel & die Single-Tasking-Realität

### 1.1 Was Schritt 3 liefern soll

Die sechs `evnt_*`-Funktionen (`evnt_button/mouse/multi/keybd/mesag/timer`)
warten heute jeweils per **Busy-Poll im Trap-Handler**: `while !cond() {
h.pump_input(); }` bzw. `loop { …; h.pump_input(); }`
(`event.rs:139-140` `evnt_button`, `:238-239` `evnt_mouse`, `:310-382`
`evnt_multi`, `:400-405` `evnt_keybd`, `:428-453` `evnt_mesag`, `:472-485`
`wait_ms`). Der Doc-Header von `event.rs` benennt das explizit als
Bring-up-Kompromiss: „NewTOS has one AES application for now, so each wait is a
cooperative poll-loop … Multitasking … lands later; the API surface is
unchanged" (`event.rs:7-12`). Ziel von Schritt 3: diese sechs Schleifen durch
**ein** Block-Primitiv `sleep(cond)` ersetzen und die Weck-Seite `wake(cond)`
an den Input-/Message-/Timer-Quellen anbringen — bei **unverändertem**
beobachtbaren `evnt_multi`-Kontrakt (`emutos-cooperative-model.md` §3/§6).

### 1.2 Die harte Wahrheit: heute existiert genau EIN Kontext zur Zeit

Zwei — und nur zwei — Kontroll-Flüsse können den Bildschirm besitzen, **nie
gleichzeitig** ([[singletasking-app-owns-full-screen]]):

1. **Der Desktop**, EL1-intern. `kernel_main` ruft `desk::run()`
   (`desk/mod.rs:534`), das über `event_loop` (`desk/main.rs:50`) in
   `evnt_multi` blockiert. Der Desktop ist **kein** Scheduler-Peer:
   `sched::current()` ist `NONE`, `RUN` ist null (`current_abi()`→`Lp64`,
   `process.rs:607-612`), und `handle_pwait` no-op't bei `cur >= MAX_PROCESSES`
   (`exceptions.rs:410-413`). Er triggert den AES-Trap `svc #2` (EL1→EL1) und
   busy-pollt im Handler.
2. **Eine gestartete GEM-App**, EL0-Prozess mit eigenem TTBR0 und Slot. Der
   Desktop startet sie via GEMDOS-`Pexec` (`handle_pexec`); bei `Pterm` bringt
   `finish_terminate` den EL1-Desktop-Kontext zurück (SPSR.M-Diskriminierung
   `EL1t` vs `EL0t`, `exceptions.rs:611,632`). Also strikt seriell —
   Desktop → App → Desktop, ein Slot besetzt zur Zeit (EmuTOS §5.2-Modell).

**Der Input kommt asynchron, aber ohne zweiten Scheduler-Kontext:** Der
IRQ-Handler `rust_irq_handler` (`gic.rs:166-182`) füllt nur den Konsolen-Ring
(`pl011_rx_drain`, `:176`) — er fasst **keinen** Scheduler-/AES-Zustand an (das
ist bereits EmuTOS' „IRQ hinterlässt nur eine Notiz"-Trennung, `emutos-…`
§3.6). `pump_input` demaskiert kurz IRQs, damit der RX-IRQ feuern kann, und
pollt USB-HID (`host_env.rs:41-…`). Auf realem Pi-4 wird der RX-IRQ nicht
zugestellt ([[pl011-rx-irq-not-delivered-on-real-hw]]); dort ist der
USB-Poll in `pump_input` die einzige Quelle.

### 1.3 Was `sleep`/`wake` mit EINEM Kontext konkret ändert — ehrlich

Mit genau **einem** lauffähigen Kontext gibt es beim Blockieren *niemanden*, zu
dem `sleep()` umschalten könnte — exakt der Fall, den `on_yield` schon heute als
„nur lauffähiger Peer → weiterlaufen" behandelt (`sched.rs:104-107`,
`exceptions.rs:428`). Der Busy-Poll lässt sich deshalb **nicht** ersatzlos durch
„wegschalten und jemand anderen laufen lassen" ersetzen — es gibt keinen anderen.
Was Schritt 3 mit einem Kontext **real** bringt:

- **Ein einziger Idle-/Poll-Punkt statt sechs.** Der `pump_input`-Spin wird zur
  *Idle-Eigenschaft des Schedulers* („die Run-Menge ist sonst leer") — genau
  EmuTOS' `schedule()`-Leerlauf, der `chkkbd()` pollt und per `STOP` bis zum
  Interrupt anhält (`emutos-…` §2.4). Die Verhaltensgleichheit zu heute bleibt
  1:1 (kein Regressionsrisiko), aber der Poll ist an **einer** Stelle statt in
  sechs `evnt_*`-Funktionen dupliziert.
- **Der Warte-Zustand + `wait_cond` + die Weck-Aufrufstellen** (`post_msg`,
  Input-Pass, Timer-Deadline) werden **jetzt** verdrahtet, sodass Schritt 4
  (echte Threads) `event.rs` **nicht mehr anfassen** muss — die API-Fläche
  bleibt stabil (`event.rs:7-12`).
- **Die Deliver-on-Resume-Naht** (§6) wird jetzt eingezogen und mit einem
  2-Prozess-Selftest getestet — sie schließt den sechsten Upcall-Konsumenten
  („Upcall an blockierten AES-Client", `10-upcall-…` §1/§8), ohne dass der
  Screen-Manager existieren muss.

**Der echte Mehrwert (genuines Umschalten auf einen zweiten Thread) fällt erst
in Schritt 4 an** (Screen-Manager + Accessories als reale Threads,
`00-effort-comparison.md` Schritt 3/4). Schritt 3 ist das **tragfähige Fundament
dafür**, kein sofortiger Multitasking-Gewinn. Diese Ehrlichkeit ist Absicht:
`base-emutos-clean-on-pi-before-extension` verlangt, die treue Basis zuerst
sauber zu haben, statt Multitasking vorzuziehen.

---

## 2. AES-Thread-Granularität + Privilegmodell (die zentrale Entscheidung, §7.3)

`emutos-cooperative-model.md` §7.3 markiert dies als *die* echte
Design-Entscheidung: EmuTOS' AES-Threads sind alle gleich privilegiert (flacher
68k-Supervisor, `switchto` = reiner SP+Register-Tausch **ohne**
Adressraumwechsel, `emutos-…` §2.2). NewTOS hat EL0/EL1 + MMU. Zwei Optionen:

### Option A — Uniforme EL0-Prozesse

Screen-Manager, Accessories **und** App laufen als EL0-Prozesse; jeder
AES-Thread-Wechsel ist ein `TTBR0`-Switch (wie `sched.rs`/`apply_switch` heute,
wie FreeMiNTs `change_context`, `freemint-…` §1.6). Uniform, aber: Screen-Manager
und Accessories existieren **nicht** als EL0-Apps, und der Desktop zeichnet seine
Dialoge heute **in-kernel** (direkte `objc_draw`/`render_tree`, LP64-Trees,
[[desktop-bypasses-app-abi-path]]).

**Konsequenz A:** Der Screen-Manager + Accessories müssten als neue EL0-Apps
gebaut werden, und der Desktop-in-kernel-Zeichenpfad (LP64) müsste vollständig
auf den EL0-App-ABI-Pfad (ILP32-fähig) umgestellt werden — ein großer Umbau, der
gegen [[base-emutos-clean-on-pi-before-extension]] und
[[desktop-bypasses-app-abi-path]] arbeitet. **Verworfen für Schritt 3.**

### Option B — Hybrid (EL1-interne Dienste + EL0-App) — **Empfehlung**

Der launched App-Prozess ist EL0 (eigenes TTBR0); Screen-Manager + Accessories
sind **EL1-interne kooperative Threads** — genau die Form, in der der Desktop
heute schon existiert (`desk/` läuft EL1-intern, zeichnet in-kernel, ruft die AES
über echte `svc #2`-Traps, `desk/mod.rs:101-114`). Ein Wechsel **zwischen**
EL1-internen Threads ist **kein** Adressraumwechsel; ein Wechsel **zur** EL0-App
ist ein `TTBR0`-Switch.

**Warum B mit dem vorhandenen Substrat zusammenfällt (Empfehlung, belegt):**

- Der Desktop ist bereits ein EL1-interner „Thread" (ein EL1-Kontrollfluss, der
  in die AES traps). Der Kontext-Save-Apparat (`ParentFrame`/`saved_frame`)
  sichert **`spsr` mit** (`process.rs:427,482-490`), sodass ein EL1-interner
  Kontext (`SPSR.M = EL1t`) genauso darstellbar und resümierbar ist wie ein
  EL0-Kontext — `finish_terminate` diskriminiert die beiden Fälle bereits über
  `SPSR.M` (`exceptions.rs:611,632`). Das Substrat **kann** EL1-interne Threads
  also schon repräsentieren.
- Die **einzige** echte Substrat-Ergänzung, die B erzwingt: `apply_switch`
  installiert heute **immer** `install_ttbr0(sw.l0_pa)` (`exceptions.rs:445-451`).
  Für einen EL1-internen Thread darf kein TTBR0-Switch passieren (oder es müssen
  die Boot-Tables aktiv sein). Der `Switch`-Deskriptor (oder der Slot) braucht
  daher eine **Privileg-Klasse** (EL0-Prozess → TTBR0-Switch; EL1-intern → kein
  Switch). Klein und lokal auf `apply_switch` + `sched::switch_to` beschränkt.
  Auf der Resume-Seite diskriminiert `SPSR.M` bereits (s.o.).

**Konsequenz für das Substrat (Schritt 3 vs. 4):** In **Schritt 3** existieren im
Run-Set nur EL0-Peers (die App) — `apply_switch` bleibt unverändert, die
Privileg-Klasse ist ein *vorbereitetes*, in Schritt 3 nicht ausgeübtes Feld. In
**Schritt 4** wird der Desktop/Screen-Manager ein EL1-interner Thread-Slot, und
`apply_switch` respektiert die Privileg-Klasse.

**Empfehlung: Option B.** Ein-Satz-Begründung: B ist die *heutige Realität*
(EL1-Desktop + EL0-App) ohne Umbau, das Kontext-Substrat trägt EL1-interne
Threads bereits (SPSR-Save + SPSR.M-Resume-Diskriminierung), und die einzige
Ergänzung ist ein Privileg-Klassen-Flag im `Switch`/Slot, das `apply_switch` den
TTBR0-Switch überspringen lässt.

**OFFEN (B, Detail):** Ob ein EL1-interner Thread einen *eigenen EL1-Stack*
braucht — hängt am Block-Modell in §4 (stack-preserving vs. restartable). Für den
kooperativen Desktop-in-kernel-Fall entscheidet sich das mit §4.

---

## 3. Prozesszustände + Ready-Queue

### 3.1 Zustandsautomat (minimal, kooperativ-first)

Heute existiert **ein** Bit `runnable` pro Slot (`process.rs:275`), über das
`pick_next` flach iteriert (`sched.rs:81-90`) — **kein** Warte-Zustand
(`sched.rs:1-22` beschreibt sich selbst so). Vorschlag: ein **Laufzeit**-Zustand
orthogonal zum Lifecycle-`State{Free,Loaded,Running,Terminated}`
(`process.rs:84-96`), mit **drei** Zuständen (deutlich weniger als FreeMiNTs acht
Queues `CURPROC/READY/WAIT/IO/ZOMBIE/TSR/STOP/SELECT`, `freemint-…` §1.3,
`references/freemint/sys/mint/proc.h:296-305`):

```
                 sleep(cond)  [Frame sichern, wegschalten oder idle-pollen]
   ┌──────────┐  ───────────────────────►  ┌──────────┐
   │ RUNNING  │                             │ WAITING  │  (wait_cond gesetzt,
   │(sched::   │  ◄───────────────────────  │          │   runnable=false)
   │ current) │  Scheduler wählt (Resume)   └────┬─────┘
   └────┬─────┘                                   │ wake(cond)  [runnable=true]
        │  Pwait / yield                          ▼
        ▼                                    ┌──────────┐
   ┌──────────┐  ◄──────────────────────────│  READY   │ (runnable=1, im
   │  READY   │   Scheduler wählt            └──────────┘  Round-Robin)
   └──────────┘
```

- **RUNNING** — auf der CPU: `sched::current() == slot` (`sched.rs:49-53`).
- **READY** — im Round-Robin: `runnable == true` (das heutige Bit,
  `process.rs:275`, `sched.rs:81-90`).
- **WAITING(cond)** — **neu**: `runnable == false` **und** `wait_cond` gesetzt.

Das ist die Skizze aus `10-upcall-…` §9 (dort schon vorgezeichnet), hier als
Schritt-3-Deliverable konkretisiert.

### 3.2 Datenstruktur — Feld statt Liste

Aus FreeMiNT übernommen wird das **`wait_cond`-Konzept** (`freemint-…` §1.1, ein
beliebiger `long`-Schlüssel, auf den gewartet wird, `references/freemint/sys/mint/
proc.h:146`) — **nicht** die verkettete Warteschlange. FreeMiNTs `q_prev/q_next`
(`proc.h:262-265`) + acht `sysq[]`-Listen (`freemint-…` §1.2,
`references/freemint/sys/proc.c:56,299-342`) sind bei `MAX_PROCESSES = 16`
(`process.rs:63`) Overkill. Kooperativ-first reicht:

```rust
// Neues Feld im Process-Slot (process.rs:266-…, im Stil der sched-Felder):
pub wait_cond: AtomicU64,   // 0 = kein Wait; sonst der Weck-Schlüssel (§4.1)
```

- **READY vs WAITING** wird weiter über das vorhandene `runnable`-Bit
  unterschieden; `wait_cond != 0` markiert „wartet gezielt auf diesen Schlüssel".
- `pick_next` (`sched.rs:81-90`) bleibt der Flat-Scan über `runnable` — ein
  WAITING-Slot hat `runnable == false` und wird übersprungen; `wake(cond)` setzt
  `runnable` wieder (§4.2).

**Empfehlung:** Flat-Feld + `runnable`-Bit-Wiederverwendung, keine
Listenverkettung. **OFFEN (Schritt 4):** falls Prioritäten/Aging (`freemint-…`
§1.4, `SLICES`) oder viele Waiter pro `cond` teuer werden, verkettete Per-`cond`-
Queues nachrüsten — additiv, außerhalb Schritt 3.

---

## 4. `sleep(cond)` / `wake(cond)` auf der vorhandenen `sched.rs`-Maschinerie

### 4.1 Was eine „Wait-Bedingung" ist

Ein `cond` ist ein **Weck-Klassen-Schlüssel**, nicht ein pro-Waiter-erfüllbares
Prädikat. FreeMiNT hält es genauso: `sleep(que, cond)` schläft auf einem
`long`-Schlüssel, `wake(que, cond)` weckt **alle** Waiter mit passendem
`wait_cond` (`freemint-…` §1.2, `references/freemint/sys/proc.c:490-716`, `do_wake`
`:665-701`); jeder Geweckte prüft danach sein **volles** Prädikat erneut und
schläft ggf. wieder (`references/freemint/sys/rendez.c:413` `goto loop`). Das ist
genau das Modell für `evnt_multi`, dessen „Prädikat" eine Kombination von `MU_*`-
Bits ist (`event.rs:22-28,310-376`).

**Empfehlung: `cond` = `u64`-Schlüssel, High-Bits = Klasse, Low-Bits = Argument:**

| Klasse | Wann erfüllt | Weck-Aufrufer (§5.2) |
|---|---|---|
| `WAIT_INPUT` | Taste/Button/Maus-Rechteck-Änderung verfügbar | Input-Pass (EmuTOS `forkq`/`chkkbd`, `emutos-…` §3.6) |
| `WAIT_MESG(pid)` | Nachricht in der pid-FIFO | `pd::post_msg` (`event.rs:91-92`) |
| `WAIT_TIMER` | Deadline überschritten | Idle-Pass prüft `monotonic_millis` (Timer-IRQ deferred, §5.2) |

Das eigentliche `evnt_multi`-Prädikat (welche `MU_*`-Bits gefeuert haben) wird
**beim Wiederanlauf** neu ausgewertet (die `evnt_multi`-Schleife läuft ihre
Checks erneut) — exakt FreeMiNTs „wake → re-check → ggf. re-sleep". **OFFEN:**
exakte Bit-Aufteilung des `u64` (Klasse in Bit 48+, pid/deadline darunter).

### 4.2 Die Primitive (Wiederverwendung von `handle_pwait`/`apply_switch`)

`sleep(cond)` ist die **Verallgemeinerung von `handle_pwait`** (`exceptions.rs:
405-430`): Frame in `saved_frame` sichern (`:416-424`), `wait_cond = cond`,
`runnable = false`, nächsten Peer via `on_yield` wählen (`sched.rs:104-111`); bei
Peer → `apply_switch` (`:436-479`), sonst Idle-Fallback (§4.4).

`wake(cond)` ist FreeMiNTs `do_wake` (`references/freemint/sys/proc.c:665-701`):
alle Slots mit `wait_cond == cond` → `runnable = true`, `wait_cond = 0`;
**weckt nicht selbst um** (kein sofortiger Kontextwechsel, wie FreeMiNT,
`freemint-…` §1.2). Der Geweckte läuft erst, wenn der aktuelle Kontext das
nächste Mal `sleep`t oder terminiert.

```rust
// sched.rs (neben on_yield/switch_to/on_exit)
pub fn wake(cond: u64) {
    for p in PROCESS_TABLE.iter() {
        if p.wait_cond.load(Acquire) == cond {
            p.wait_cond.store(0, Release);
            p.runnable.store(true, Release);     // WAITING → READY
        }
    }
}
```

### 4.3 Der strukturelle Kern-Konflikt: mid-Syscall-Kontext bewahren

**Das ist die zentrale strukturelle OFFEN-Frage des Substrats, gleichrangig mit
§2.** `handle_pwait` ist ein *Blatt*-Syscall: sein ganzer Job ist „yielden", die
Fortsetzung ist nur „nach dem `svc` zu EL0 zurückkehren" (`apply_switch`s
Resume-Zweig restauriert `saved_frame` verbatim und setzt `gpr[0] = 0`,
`exceptions.rs:463-473`). `evnt_multi` ist **kein** Blatt-Syscall: es muss
schleifen, **bis** ein Event feuert, **dann** Ergebnisse schreiben und
zurückkehren (`event.rs:310-388`). Der Busy-Poll hält den Rust-Aufrufrahmen die
ganze Wartezeit auf dem EL1-Exception-Stack am Leben.

Um `evnt_multi` blockieren zu lassen, gibt es zwei Modelle:

- **S1 — stack-preserving (EmuTOS/FreeMiNT-treu).** Jeder Thread bekommt einen
  eigenen EL1-Kernel-Stack; `sleep()` tauscht den EL1-SP (Save/Restore der
  callee-saved + SP), sodass der `evnt_multi`-Rust-Rahmen den Block **überlebt**
  und `sleep()` beim Resume *in die Schleife zurückkehrt*. Das ist EmuTOS'
  `switchto` (`references/emutos/aes/gemasm.S:169-194`, privater Supervisor-Stack
  je Thread) bzw. FreeMiNTs `change_context` (`freemint-…` §1.6,
  `references/freemint/sys/arch/context.S:445-592`). **Das tut `apply_switch`
  heute NICHT** — es schreibt nur den EL0-Frame um und verlässt sich auf `eret`
  (`exceptions.rs:436-479`).

- **S2 — restartable syscall (ELR-Rewind).** `sleep()` sichert `saved_frame` mit
  `elr` auf die **`svc`-Instruktion** (`elr − 4`) statt dahinter; beim Resume
  re-executet `eret` den `svc`, `evnt_multi` wird **von vorn** neu betreten,
  prüft neu, und feuert diesmal. Nutzt die vorhandene EL0-Frame-Rewrite-Maschine
  (`apply_switch`) mit **zwei** kleinen Deltas (**Korrektur, bei der 3b-Umsetzung
  belegt**): (1) ELR-Rewind beim Save; (2) ein `restartable`-Flag am `Switch`,
  damit `apply_switch`s Resume-Zweig `gpr[0]` NICHT bedingungslos auf 0 zwingt
  (`exceptions.rs:472` — das ist der korrekte Pwait-Rückgabewert, aber für S2
  falsch: der re-executete GEM-`svc #2` braucht den AES-Selektor 200 in `x0`
  intakt). Beide Deltas klein + Pwait-byte-erhaltend; die ursprüngliche „nur ein
  Delta"-Behauptung war falsch (S2 bleibt dennoch billiger als S1 — kein
  Per-Thread-EL1-Stack). **Voraussetzung:** die
  Checks vor dem Block sind read-only/idempotent, und Zustands-Mutation passiert
  **nur** auf der feuernden (zurückkehrenden) Runde.

**Empfehlung: S2 (restartable) für Schritt 3** — Begründung:

- Die Aufgabe verlangt `sleep`/`wake` „on the EXISTING `sched.rs` machinery
  (`Switch` + full EL0-frame save + `TTBR0`)" — genau S2. S1 wäre ein neuer
  Mechanismus (EL1-Stack-Switch), den `apply_switch` heute nicht hat.
- Die `evnt_*`-Waits sitzen **am Kopf einer reproduzierbaren Schleife** — es gibt
  keinen mittendrin gehaltenen, nicht-reproduzierbaren Rechenzustand.
- **Idempotenz ist bereits gegeben:** `evnt_multi`s einzige mutierende Aktion vor
  der Rückkehr ist `src.queue.pop()` (`event.rs:359`), das **nur** passiert, wenn
  eine Nachricht existiert — und dieser Fall **feuert und kehrt zurück**
  (`fired |= MU_MESAG`, `:369-379`), blockiert also nie. Der Block-Pfad
  (`fired == 0`, `:378-382`) ist read-only. Analog `menu_intercept`: es postet
  eine Nachricht und feuert über denselben MESAG-Dequeue in **derselben** Runde
  (`:323,353-372`). Der Pop wird also nie doppelt ausgeführt.

**OFFEN (S2, zu verifizieren):** der `menu_intercept`-Cancel-Pfad
(`event.rs:96-97`: Cursor über Bar, Dropdown abgebrochen, keine Nachricht →
Block) würde bei Restart das Dropdown erneut öffnen. In **Schritt 3** irrelevant
(nur Idle-Fallback, kein Restart, §4.4); in **Schritt 4** vor dem ersten echten
Switch prüfen. **OFFEN (Ausblick):** S1 ist das eigentliche allgemeine Modell
(nötig, sobald ein blockierender Syscall nicht-reproduzierbaren mid-Rechenzustand
hat, oder für echt nebenläufige EL1-interne Threads mit eigenem Rust-Stack in
Schritt 4). S2 ist die kooperativ-first-Wiederverwendung; S1 der spätere
additive Upgrade-Pfad.

### 4.4 Idle-Fallback (der Ein-Kontext-Fall) — was `sleep` OHNE Peer tut

`sleep(cond)` hat zwei Ausgänge:

```rust
fn sleep(cond: u64) {           // in sched.rs, aufgerufen aus event.rs
    let cur = sched::current();
    // WAITING markieren:
    set_wait_cond(cur, cond); set_runnable(cur, false);
    loop {
        match on_yield(cur) {                       // sched.rs:104-111
            Some(sw) => {                            // (b) echter Peer (Schritt 4):
                // saved_frame mit ELR-Rewind sichern (S2), dann Switch.
                // KEHRT NICHT in diesen Rust-Rahmen zurück — der Prozess wird
                // später via Re-Issue seines svc wieder aufgenommen.
                save_frame_restartable(cur);         // elr -= 4
                mark_yielded(cur); apply_switch(frame, &sw); return;
            }
            None => {                                // (a) kein Peer (Schritt 3):
                // EmuTOS schedule()-Idle: pump + re-check (emutos-… §2.4)
                host().unwrap().pump_input();
                if woken(cur) {                       // wake(cond) hat runnable gesetzt
                    set_runnable(cur, true); return;  // Caller re-checkt & committed
                }
            }
        }
    }
}
```

- **Ausgang (a), Idle-Fallback (Schritt 3, ein Kontext):** kein Peer → `pump_input`
  (`host_env.rs:41-…`) + Re-Check. Verhaltensgleich zum heutigen Busy-Poll — nur
  an **einer** Stelle statt in sechs `evnt_*`. `wake(cond)` (§5.2) ist hier
  quasi-implizit: der Idle-Pass selbst prüft, ob `cond` erfüllt ist. **Ehrlich:**
  `wake(cond)` wird erst mit Peers (Schritt 4) *load-bearing*; es wird jetzt
  trotzdem verdrahtet, damit Schritt 4 `event.rs` nicht anfasst.
- **Ausgang (b), echter Switch (Schritt 4):** Peer vorhanden → S2-Restart-Save +
  `apply_switch`. Wird in Schritt 3 nur vom 2-Prozess-Selftest (§7.3) ausgeübt.

**Empfehlung:** Diese Doppel-Ausgang-Form. Sie landet `sleep`/`wake` + `wait_cond`
+ WAITING **jetzt**, ist mit einem Kontext regressionsfrei (Ausgang a ≡ heute),
und bereitet Ausgang (b) + Deliver-on-Resume vor, testbar via 2-Prozess-Selftest,
ohne dass der Screen-Manager existiert.

---

## 5. `event.rs`-Refactor

### 5.1 Vom Busy-Poll zu `sleep(cond)`

Jede der sechs Schleifen wird zu: Prädikat read-only prüfen → bei Nicht-Erfüllung
`sleep(cond)` → nach Rückkehr (Ausgang a) oder Re-Issue (Ausgang b) neu prüfen.
Konkret pro Funktion:

| Funktion | heutiger Poll | `cond` | mutierender Commit nur bei Feuern |
|---|---|---|---|
| `evnt_keybd` (`:398-408`) | `loop { console_try_read_key; pump_input }` (`:400-405`) | `WAIT_INPUT` | Key lesen = feuert |
| `evnt_button` (`:131-149`) | `while !matches { pump_input }` (`:139-140`) | `WAIT_INPUT` | `count_clicks` = feuert |
| `evnt_mouse` (`:229-244`) | `while !rect_event { pump_input }` (`:238-239`) | `WAIT_INPUT` | `fill_rets` = feuert |
| `evnt_mesag` (`:421-454`) | `loop { queue.pop; pump_input }` (`:428-453`) | `WAIT_MESG(pid)` | `queue.pop` (`:438`) nur bei Present |
| `evnt_timer` (`:460-467`) | `while now<deadline { spin }` (`:481-483`) | `WAIT_TIMER` | Deadline = feuert |
| `evnt_multi` (`:278-389`) | `loop { …checks…; pump_input }` (`:310-382`) | `WAIT_INPUT`∪`WAIT_MESG`∪`WAIT_TIMER` | `queue.pop` (`:359`) nur bei Present |

`evnt_multi` wartet auf **mehrere** Klassen zugleich (`MU_*`-Maske,
`event.rs:279`). In S2 genügt: die Schleife (`:310-382`) bleibt read-only bis zum
`fired != 0`-Break (`:378-379`); an Stelle des finalen `h.pump_input()` (`:381`)
steht `sleep(cond_union)`, wobei `cond_union` die aktivierte(n) Klasse(n)
kodiert. Der beobachtbare Kontrakt (`RET_CODE` = gefeuerte Bits, `EV_MX/MY/MB/KS`,
`EV_KRET`, `EV_BRET`, `event.rs:385-388`) bleibt **unverändert** — nur der Warte-
*Mechanismus* wechselt (`emutos-…` §6, „API-Fläche unverändert").

### 5.2 Wer `wake(cond)` ruft

Die Weck-Seite spiegelt EmuTOS' Wake-Quellen (`emutos-…` §3.5/§3.6):

- **`WAIT_MESG(pid)` — bei `post_msg`.** `pd::post_msg` (aufgerufen aus
  `menu_intercept`, `event.rs:91-92`, und aus dem AES-`ap_sendmsg`-Pfad) reiht in
  die pid-FIFO ein → direkt danach `sched::wake(WAIT_MESG(pid))`. Das ist EmuTOS'
  `aqueue`→`azombie`→`signal` (`emutos-…` §3.3/§4.3). **Empfehlung:** Wake-Aufruf
  in `pd::post_msg` selbst (eine Stelle, alle Poster profitieren).
- **`WAIT_INPUT` — im kooperativen Input-Pass, NICHT im ISR.** Der IRQ-Handler
  füllt nur den Ring (`gic.rs:176`), fasst keinen Sched-Zustand an — EmuTOS'
  „IRQ hinterlässt nur eine Notiz, der Dispatcher arbeitet sie synchron ab"
  (`forkq`/`forker`, `emutos-…` §3.6). **Empfehlung:** `wake(WAIT_INPUT)` aus dem
  Idle-/Pump-Pass (§4.4), wenn er neuen Ring-Inhalt beobachtet (EmuTOS `chkkbd`
  aus `schedule()`, `emutos-…` §2.4). Damit bleibt die IRQ→Dispatcher-Trennung,
  die auch `10-upcall-…` fordert.
- **`WAIT_TIMER` — Deadline-Prüfung im Idle-Pass.** Heute spinnt `wait_ms` auf
  `monotonic_millis` (`event.rs:481-483`). Der Idle-Pass prüft Deadlines und ruft
  `wake(WAIT_TIMER)`. Ein **Timer-IRQ**-getriebenes Wecken (EmuTOS `tchange`/`dlr`
  Compare-Timer, `emutos-…` §3.5) ist **deferred** — es braucht die
  Timer-IRQ-Route, die parallel zur EL0-IRQ-Route (`10-upcall-…` §7.3) fehlt.

**OFFEN:** ob zusätzlich ein leichtes `iwake` direkt aus dem ISR (FreeMiNT
`iwake`, IRQ-safe, `references/freemint/sys/proc.c:754-774`) für Latenz gewünscht
ist. Empfehlung: **nein** in Schritt 3 (forkq-Trennung wahren); bei Bedarf
additiv.

---

## 6. Deliver-on-Resume (Upcall an einen blockierten AES-Client)

Das ist der sechste Upcall-Konsument aus `10-upcall-…` §1/§8: einem EL0-AES-
Client, der in `evnt_multi` blockiert, ein Event/Upcall zustellen. Schritt 1/2
haben die Mechanik (`deliver_upcall`/`handle_psigreturn`, `exceptions.rs:701-844`)
und den Deliver-Check am Syscall-Rücksprung (`:389-398`) gelandet — aber **nur**
für den nicht-blockierten Fall (Zustellung am `eret` des laufenden Syscalls). Für
einen *blockierten* Client reitet die Zustellung auf dem **Resume** (`10-upcall-…`
§8):

1. **Wecken:** `post_upcall(slot, desc)` (`process.rs:661-676`) + `wake(cond)` (der
   Client wird READY). FreeMiNT: `post_sig` verschiebt `nrl → READY_Q`,
   `references/freemint/sys/signal.c:216-224`.
2. **Scheduler wählt den Slot:** `on_yield`/`switch_to` (`sched.rs:104-111,92-100`),
   `apply_switch` installiert dessen TTBR0 (`install_ttbr0`, `exceptions.rs:445-451`)
   und lädt im **Resume-Zweig** (`:463-473`) `saved_frame` in `frame`.
3. **Redirect hier:** Ist für `sw.slot` ein Upcall pending, biegt der Resume-Zweig
   statt auf `saved_frame.elr` auf den **Handler** um. Der bisherige
   `saved_frame`-Inhalt (jetzt in `frame`) wandert per `deliver_upcall` auf den
   kernel-seitigen Upcall-Stack (`upcall_ctx[]`, `process.rs:290`,
   `deliver_upcall` `exceptions.rs:730-746`). Der Client läuft im Handler **in
   seinem eigenen TTBR0** (gerade installiert) — genau §1's sechster Fall. Bei
   Handler-Ende via Trampolin/`Psigreturn` (`:781-844`) wird der Block-Kontext
   resümiert; für `evnt_multi` heißt das: der (restartete, §4.3) Wait liefert
   danach das Event, das ihn geweckt hat.

**Konkrete Einbaustelle (Empfehlung, minimale Wiederverwendung):** Am Ende von
`apply_switch`s Resume-Zweig — **nach** `set_current(bp)`/`set_running`
(`exceptions.rs:475-478`, damit `slot_for(current())` in `deliver_upcall` den
richtigen Slot findet) — ein Nachsatz:

```rust
// nach exceptions.rs:478, nur im !fresh-Zweig:
if let Some(u) = tos_api::bdos::process::take_pending_upcall(sw.slot) {
    deliver_upcall(frame, u);   // exceptions.rs:701 — snapshottet frame
}                               // (jetzt = resümierter Block-Ktx, inkl. gpr[0])
                                // in upcall_ctx, biegt auf Handler um
```

`deliver_upcall` snapshottet `frame` **nach** dem Ergebnis-Write (§6.2 in
`10-upcall-…`) — hier ist `frame` bereits der geladene `saved_frame`, also der
Block-Kontext samt seines `gpr[0]`. Reine Wiederverwendung von `deliver_upcall`,
**keine** Änderung an der `Switch`-Struktur oder am Trap-Pfad — passt zur
`sched.rs`-Doktrin „Scheduling-Änderungen bleiben im Modul" (`sched.rs:10-18`).

**OFFEN (klein):** `deliver_upcall` nutzt heute `slot_for(current())`
(`exceptions.rs:712`). Da wir `sw.slot` schon kennen, sauberer: eine
`deliver_upcall_for(slot, …)`-Variante ohne `current()`-Abhängigkeit. Empfehlung:
explizit `sw.slot` übergeben.

**Ehrliche Einordnung (ein Kontext):** In Schritt 3 wird dieser Resume-Zweig
(`!fresh`) mit einem Kontext **nicht** vom Desktop/App erreicht (kein Peer, nur
Idle-Fallback §4.4). Er wird ausschließlich vom **2-Prozess-Selftest** (§7.3)
ausgeübt — der aber schließt den „Upcall an blockierten Client"-Fall genau ab.

---

## 7. Scope-Split Schritt 3 vs. Schritt 4 + Testbarkeit mit einem Kontext

### 7.1 Was in Schritt 3 landet (substrat + `event.rs`, ein Kontext testbar)

1. **RunState-Erweiterung**: `wait_cond: AtomicU64` (§3.2) + `RUNNING/READY/
   WAITING`-Semantik über `runnable` (`process.rs:275`) + `wait_cond`.
2. **`sleep(cond)`/`wake(cond)`** in `sched.rs` (§4): `sleep` = verallgemeinertes
   `handle_pwait` mit Idle-Fallback (Ausgang a) + Restartable-Switch (Ausgang b);
   `wake` = `do_wake`-Analog. Privileg-Klasse im `Switch`/Slot **vorbereitet**
   (§2), in Schritt 3 nicht ausgeübt (`apply_switch` unverändert).
3. **`event.rs`-Refactor** (§5): sechs Busy-Polls → `sleep(cond)`; Weck-Aufrufe in
   `pd::post_msg` + Idle-Pass. Beobachtbarer `evnt_*`-Kontrakt unverändert.
4. **Restartable-Block (S2, §4.3)**: ELR-Rewind-Save + read-only-vor-Block.
5. **Deliver-on-Resume** (§6): Redirect im `apply_switch`-Resume-Zweig.

### 7.2 Was in Schritt 4 landet (echte Threads)

- Screen-Manager + Accessories als **EL1-interne kooperative Threads** (Option B,
  §2) mit eigenem Slot; `apply_switch` respektiert die Privileg-Klasse
  (TTBR0-Switch überspringen für EL1-intern).
- Ggf. **S1 (stack-preserving, §4.3)** / eigene EL1-Stacks je Thread, falls ein
  blockierender Kontext nicht-reproduzierbaren Rust-Zustand hält.
- **Timer-IRQ-Wake** (EmuTOS `tchange`/`dlr`, `emutos-…` §3.5) + **EL0-IRQ-Route**
  für den vex-Konsumenten (`10-upcall-…` §7.3, `exception_vectors.s:108`).
- Präemption/Prioritäten/Aging = **deferred, additiv** (`00-effort-comparison.md`
  Schritt 4; FreeMiNTs `preempt` ist ein Aufsatz auf `sleep`/`wake`, `freemint-…`
  §1.2, `references/freemint/sys/proc.c:351-368`).

### 7.3 Testen mit einem Kontext

- **Regressionstest (Idle-Fallback, Ausgang a):** Der Live-Desktop + eine
  gestartete App laufen unverändert (`event.rs`-Refactor darf `evnt_multi`s
  beobachtbares Verhalten nicht ändern). Interaktiv nur auf realem Pi-4
  verifizierbar ([[desktop-interactive-verification-real-hw]]); statisch via
  QMP-Screendump.
- **Wake-on-Input-Pfad (ein Kontext):** Instrumentierter Selftest: ein
  WAITING(WAIT_INPUT)-Slot; ein Key wird in den Ring gespeist (IRQ/USB-Pfad,
  `gic.rs:176`); der Idle-Pass beobachtet den Ring, ruft `wake(WAIT_INPUT)`, die
  WAITING→READY-Transition feuert, `evnt_keybd`/`evnt_multi` liefert den Key.
- **2-Prozess-Selftest (Ausgang b + Deliver-on-Resume, machbar):** Zwei EL0-
  Selftest-Slots via `admit` (`sched.rs:62-67`) — das Muster existiert bereits
  (`kernel/src/selftest_upcall.rs` treibt EL0-Handler; `kernel/src/selftest_pexec.rs`
  erzeugt EL0-Prozesse). Prozess A `sleep(cond)` (echter Switch, kein Peer-No-op,
  weil B lauffähig ist), Prozess B `wake(cond)` + `post_upcall(A)` + `sleep`; der
  Scheduler wählt A, `apply_switch`-Resume-Redirect stellt A den Upcall zu, A
  läuft im Handler, `Psigreturn` resümiert den Block, `evnt_*` liefert das Event.
  Prüft: Switch, Restart, Deliver-on-Resume, `Psigreturn`-Resume, `gpr[0]`-Erhalt.
  Gated, nicht im Boot (wie `selftest_upcall.rs`).

---

## 8. Offene Fragen + vorgeschlagene Implementierungsreihenfolge

### 8.1 OFFEN (Optionen + Empfehlung markiert)

1. **AES-Thread-Granularität (§2).** *Empfehlung: Option B (Hybrid)* — EL1-interne
   Dienste + EL0-App, matcht die heutige Realität, Substrat trägt es bereits
   (SPSR-Save/Resume-Diskriminierung), einzige Ergänzung = Privileg-Klasse im
   `Switch`. OFFEN: eigener EL1-Stack je EL1-internem Thread (hängt an §4.3).
2. **Block-Modell (§4.3): S2-restartable vs. S1-stack-preserving.** *Empfehlung:
   S2 für Schritt 3* (Wiederverwendung der EL0-Frame-Rewrite-Maschine, `evnt_*`
   sitzt am reproduzierbaren Schleifenkopf, Mutation nur auf feuernder Runde).
   **OFFEN:** S1 als allgemeiner Mechanismus/Schritt-4-Upgrade, sobald
   nicht-reproduzierbarer mid-Rechenzustand oder echt nebenläufige EL1-Threads
   auftreten.
3. **`wait_cond`-Kodierung (§4.1).** *Empfehlung: `u64`, Klasse in High-Bits,
   pid/deadline darunter.* **OFFEN:** exakte Bit-Aufteilung.
4. **`wake(WAIT_INPUT)`-Aufrufstelle (§5.2).** *Empfehlung: kooperativer Idle-/
   Pump-Pass (EmuTOS `forkq`/`chkkbd`), nicht der ISR.* **OFFEN:** leichtes `iwake`
   aus dem ISR für Latenz (Empfehlung: nein in Schritt 3).
5. **Timer-Wake (§5.2).** *Empfehlung: Deadline-Prüfung im Idle-Pass jetzt;
   Timer-IRQ deferred zu Schritt 4.*
6. **Deliver-on-Resume-Aufruf (§6).** *Empfehlung: `deliver_upcall_for(sw.slot,…)`
   ohne `current()`-Abhängigkeit.*
7. **`menu_intercept`-Cancel-Idempotenz unter Restart (§4.3).** **OFFEN** — in
   Schritt 3 irrelevant (kein Restart, nur Idle), vor dem ersten echten Switch in
   Schritt 4 verifizieren.
8. **Bekommt der Desktop (heute kein Sched-Peer) in Schritt 3 einen Slot?**
   *Empfehlung: nein — erst in Schritt 4 als EL1-interner Thread.* In Schritt 3
   nutzt seine `evnt_multi` den Idle-Fallback (Verhalten unverändert).

### 8.2 Vorgeschlagene Implementierungsreihenfolge (Developer-Tasks, bottom-up)

Jeder Schritt Developer→Tester, keine Wegwerf-Arbeit
([[limit-concurrent-workers-token-budget]]: seriell):

1. **Substrat-Felder + Primitive (ohne Restart/Switch).** `wait_cond`-Feld
   (`process.rs`), `sleep(cond)` mit **nur** Idle-Fallback (Ausgang a) + `wake(cond)`
   in `sched.rs`. *Test:* Unit-artig, plus Ring-Wake-Selftest (§7.3).
2. **`event.rs`-Refactor.** Sechs Busy-Polls → `sleep(cond)`; `wake` in
   `pd::post_msg` + Idle-Pass. *Test:* keine Regression am Live-Desktop/App
   (Screendump + real-Pi interaktiv).
3. **Restartable-Block (S2) + echter Switch (Ausgang b).** ELR-Rewind-Save,
   read-only-vor-Block; `apply_switch` unverändert (nur EL0-Peers). *Test:*
   2-Prozess-Selftest, Switch + Restart.
4. **Deliver-on-Resume-Redirect** im `apply_switch`-Resume-Zweig (§6). *Test:*
   2-Prozess-Selftest: blockierter Client erhält Upcall, resümiert korrekt,
   `gpr[0]` intakt.
5. **(Schritt 4, außerhalb dieses Deliverables)** EL1-interne Thread-Slots +
   Privileg-Klassen-TTBR0-Skip in `apply_switch` + Screen-Manager/Accessories;
   danach Timer-IRQ-Wake + EL0-IRQ-Route (vex).

---

### Quellen (alle `datei:zeile` oben zitiert)

- **NewTOS:** `tos-api/src/bdos/process/sched.rs` (`Switch`/`on_yield`/`switch_to`/
  `pick_next`/`admit`/`mark_yielded`), `tos-api/src/bdos/process.rs` (`Process`-
  Slot, `runnable`/`has_ctx`/`saved_frame`/`State`, `post_upcall`/
  `take_pending_upcall`, `current_abi`), `kernel/src/arch/aarch64/exceptions.rs`
  (`ExceptionFrame`, `rust_sync_handler`-Deliver-Check, `handle_pwait`/
  `apply_switch`/`finish_terminate`/`deliver_upcall`/`handle_psigreturn`),
  `kernel/src/arch/aarch64/exception_vectors.s` (IRQ-Vektoren), `tos-api/src/aes/
  event.rs` (sechs Busy-Polls), `kernel/src/desk/mod.rs`+`main.rs` (EL1-Desktop,
  `svc #2`, `event_loop`), `kernel/src/drivers/gic.rs` (`rust_irq_handler`),
  `kernel/src/host_env.rs` (`pump_input`), `kernel/src/selftest_upcall.rs`+
  `selftest_pexec.rs` (EL0-Selftest-Muster).
- **EmuTOS:** `references/emutos/aes/gemasm.S:169-194` (`switchto`), `aes/gemdisp.c`
  (`schedule`-Idle, `forkq`/`forker`), via `emutos-cooperative-model.md`
  §2.2/§2.4/§3.5/§3.6/§7.3.
- **FreeMiNT:** `references/freemint/sys/proc.c:490-716` (`sleep`/`wake`/`do_wake`),
  `:351-368` (`preempt`), `:754-774` (`iwake`), `sys/mint/proc.h:146,296-305`
  (`wait_cond`, Queues), `sys/arch/context.S:445-592` (`change_context`),
  `sys/signal.c:216-224` (`post_sig`→READY_Q), via `freemint-multitasking-model.md`
  §1.1-1.6/§3.1.
- **Design:** `00-effort-comparison.md` (Schritt 2/3/4), `10-upcall-signal-design.md`
  §8/§9 (Deliver-on-Resume, Ready-Queue-Skizze), `emutos-cooperative-model.md` §7.3
  (Granularität-Entscheidung).
