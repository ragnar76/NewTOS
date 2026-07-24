# Kooperativer Kern — Aufwandsschätzung & Richtungsentscheidung

> Stand: 2026-07-24, Branch `desk-rebuild`. Lead-Synthese aus den zwei
> quellen-gegroundeten Analysen `emutos-cooperative-model.md` (EmuTOS-AES-
> Kooperativ-Scheduler) und `freemint-multitasking-model.md` (FreeMiNT
> präemptiv + Signale). Beantwortet die User-Frage: „Aufwand abschätzen, was
> besser ist — direkt FreeMiNT-artig implementieren oder wie EmuTOS."

## Das entscheidende Bild in einem Satz

**NewTOS' Fundament ist heute schon FreeMiNT-förmig, nicht EmuTOS-förmig** — die
*harten* Primitiven (geschützte per-Prozess-Adressräume via `TTBR0_EL1`, ein
Kontext = `ExceptionFrame`, ein `Switch`-Deskriptor, sogar ein kooperativer
Round-Robin-Scheduler in `sched.rs`) sind bereits die geschützten Multitasking-
Bausteine, die FreeMiNT nutzt, **nicht** EmuTOS' flacher 68k-Supervisor.

## Warum „EmuTOS-Stil" ≠ „EmuTOS-treu"

EmuTOS' Scheduler-*Mechanik* (`switchto()` = reiner SP+Register-Tausch zwischen
privaten Supervisor-Stacks, **ohne** Adressraumwechsel, flach, keine MMU;
`emutos-cooperative-model.md` §2.2, Key-Table) hat auf NewTOS' EL0/EL1+MMU-Modell
**keinen sauberen Platz**: ein AES-Thread-Wechsel zwischen zwei *Anwendungen*
wäre bei uns kein SP-Tausch, sondern ein `TTBR0_EL1`-Wechsel — also genau das,
was `sched.rs` schon tut, und genau das, was FreeMiNTs `change_context()` tut
(`emutos-…` §7.3, `freemint-…` §7.1). NewTOS hat den flachen Supervisor-Modus mit
dem EL0/EL1-Pivot bereits verlassen; EmuTOS' Mechanik 1:1 nachzubauen hieße,
diese Schutz-Architektur zu *umgehen*.

**Aber:** die EmuTOS-*Policy* (Yield-nur-beim-Blockieren, keine Präemption, die
`forkq()`-Trennung „IRQ hinterlässt nur eine Notiz / Dispatcher arbeitet sie
synchron ab", das AES-Event/Message-Modell, Desktop + 6 Accessories + 1 serielle
Haupt-App) ist **architektur-neutral** (Scheduling-Policy, keine 68k-Spezifika,
`emutos-…` §7.3) und **ist** die treue Basis, die das Projekt will
([[base-emutos-clean-on-pi-before-extension]], [[singletasking-app-owns-full-screen]]).

→ Die Achse „EmuTOS vs. FreeMiNT" spaltet sich in **zwei Ebenen**, die getrennt
zu entscheiden sind:

| Ebene | Richtige Wahl | Begründung |
|---|---|---|
| **Mechanik / Primitiven** | **FreeMiNT-förmig** (ist schon da) | geschützte per-Prozess-Kontexte (TTBR0), Ready-Queue + `sleep()`/`wake()`, Signal-Upcall — NewTOS hat das Meiste; EmuTOS' Flat-`switchto` passt nicht |
| **Policy / Verhalten** | **EmuTOS-kooperativ** (zuerst) | Yield-on-Block, keine Präemption, AES-44-Fn, Desktop+Accessories, App-besitzt-Schirm = treue Basis + heutiges Single-Tasking |

Das ist **exakt** die ursprünglich vom User formulierte Absicht: „kooperativ
implementieren, aber so, dass man es ohne Wegwerf auf FreeMiNT-Niveau hochziehen
kann."

## Der teuerste Baustein ist für BEIDE Wege derselbe

Der **EL0-Upcall-/Signal-Mechanismus** ist unvermeidbar — egal welcher Stil:
- **EmuTOS-Policy:** um einem *blockierten* EL0-AES-Client ein Event zuzustellen
  bzw. ihn an seinem Blockierpunkt fortzusetzen, muss der EL1-Kern über die
  EL0/EL1-Grenze „hochrufen" — das ist der Upcall.
- **FreeMiNT-Stil:** die Signal-Zustellung `sendsig()` **ist** dieser Upcall.
- Und er ist **ohnehin** schon fällig für H1/M1 (etv_*), **vex** (Maus-ISR→EL0),
  **G_USERDEF** (ub_code→EL0), **P4-012** (eralert) — vier+ wartende Konsumenten.

FreeMiNTs `sendsig()`/`Psigreturn()` (`freemint-…` §3.2, `sig_mach.c:45-316`) ist
die **vollständige Referenzimplementierung genau dieses Mechanismus**, nur m68k
statt AArch64 (PC/SP-Frame-Bau → `ELR_EL1`/`SP_EL0` im `ExceptionFrame`). Beide
Researcher benennen ihn als den einzigen Baustein **ohne** heutiges Teil-Fundament
in NewTOS (`freemint-…` §7.2/Aufwandsnotiz) — während der andere große
FreeMiNT-Baustein (TTBR0-Kontextwechsel) bei uns bereits **produktiv** ist.

## Was NewTOS schon hat vs. was fehlt

| Baustein | NewTOS heute | Quelle |
|---|---|---|
| Per-Prozess-Adressraum (TTBR0) + Umschaltcode | **vorhanden, produktiv** | `process.rs:210-213`, `exceptions.rs:412-422` |
| Kontext = Trap-Frame | **vorhanden** (`ExceptionFrame`) | `exceptions.rs:52-68` |
| `Switch`-Deskriptor + Frame-Save + kooperativer Round-Robin | **vorhanden** (GEMDOS-Ebene, `Pwait`) | `sched.rs:37-47`, `sched.rs:1-22` |
| Ready-Queue + Prozesszustände (mehr als 1 `runnable`-Bit) | **fehlt** (nur Flat-Iteration) | `sched.rs:81-90` |
| `sleep()`/`wake()`-Blockier-Primitive | **fehlt** (event.rs busy-pollt im Trap-Handler) | `event.rs:131-467` |
| **Signal-/Upcall-Mechanismus** (`sendsig`-Analog) | **fehlt — der teuerste Posten** | — |
| Präemption / Zeitscheiben / Prioritäten | **fehlt** (bewusst; kooperativ zuerst) | `sched.rs:1-22` |

## Aufwandsschätzung — der empfohlene Weg (Policy=EmuTOS, Primitiven=FreeMiNT-förmig)

Bottom-up, jeder Schritt Developer→Tester, keine Wegwerf-Arbeit:

1. **EL0-Upcall-/Signal-Mechanismus** (modelliert 1:1 auf FreeMiNTs `sendsig`/
   `Psigreturn`, adaptiert auf AArch64/`ExceptionFrame`): EL0-Signal-Frame bauen,
   `ELR_EL1`/`SP_EL0` auf den Handler biegen, per-Prozess-Trampolin im EL0-Raum,
   `Psigreturn`-Rücksprung-Syscall, verschachtelte-Signal-Behandlung. **GROSS,
   aber unvermeidbar & sowieso fällig** (H1/M1/vex/G_USERDEF/eralert). Referenz
   liegt vor. → *erster Schritt, weil alles andere darauf aufsetzt.*
2. **Prozess-/Scheduler-Substrat**: `sched.rs`' kooperativen Round-Robin (hat
   schon `Switch`+TTBR0+Frame-Save) zu echter Ready-Queue + `sleep()`/`wake()`-
   Blockier-Primitive + AES-Thread-Granularität ausbauen. → *MODERAT (Fundament
   da).*
3. **EmuTOS-kooperatives AES-Verhalten** (die 44 Fn): AESPD-Äquivalent,
   `dsptch()`/`switchto()` auf AES-Thread-Ebene (EL0-App-Thread = TTBR0-Wechsel,
   EL1-interner Dienst = kein Adressraumwechsel — die eine echte Design-
   Entscheidung, `emutos-…` §7.3), `forkq()`-IRQ→Dispatcher-Trennung, Event/
   Message-Routing, Screen-Manager + Accessories; `event.rs`' Busy-Poll durch
   echtes Block/Yield ersetzen. → *MODERAT-GROSS, aber gut spezifiziert
   (`emutos-cooperative-model.md` mappt es).*
4. **Präemption + Prioritäten + Mehr-App + SMP** = **DEFERRED, additiv.**
   FreeMiNTs `preempt()` ist ein *Aufsatz* auf `sleep()`/`wake()`, kein Rewrite
   (`freemint-…` §1.7 + Aufwandsnotiz: „der Scheduler-Algorithmus selbst ist
   nicht der teure Teil"). Genau hier greift „ohne Wegwerf hochziehbar".

## Warum NICHT „direkt voll FreeMiNT jetzt"

Das hieße: die AES *jetzt* XaAES-artig als echte Multi-Prozess-Client/Server-AES
umbauen (`freemint-…` §6) + Präemption sofort. Das ist **mehr** Aufwand, **jetzt**,
und verletzt [[base-emutos-clean-on-pi-before-extension]] (Over-Engineering, bevor
die treue Basis auf dem Pi sauber läuft). Der Mehr-App-Betrieb ist ein *Policy*-
Aufsatz, den die FreeMiNT-förmigen Primitiven später tragen — es gibt keinen
Grund, ihn vor der sauberen Basis zu bauen.

## Warum NICHT „reiner EmuTOS-Stil (Flat-switchto)"

Das hieße, EmuTOS' flachen Supervisor-`switchto` auf ein EL0/EL1+MMU-System zu
zwingen — entweder die Schutz-Architektur umgehen (alle AES-Threads flach/EL1,
Konflikt mit dem EL0-App-Modell [[el0-app-memory-must-be-mapped]]) oder die
EL0/EL1-Grenze doch überbrücken (= der Upcall, den man dann trotzdem baut). Kein
Gewinn, aber Reibung mit dem vorhandenen Fundament.

## Empfehlung

**Hybrid = EmuTOS-Policy auf FreeMiNT-förmigen Primitiven, Startpunkt der
EL0-Upcall-/Signal-Mechanismus (1:1 nach FreeMiNTs `sendsig`).** Least-Effort,
kein Wegwerf, unblockt H1/M1/vex/G_USERDEF/eralert als Nebeneffekt, und lässt
Präemption/Mehr-App/SMP später additiv zu. Das ist die vom User ursprünglich
formulierte Absicht, jetzt quellen-belegt.

**Vor der Implementierung: eine kurze Design-Phase** (Signal-Frame-Layout auf
AArch64, `Psigreturn`-Syscall-Nummer, Trampolin-Platzierung im EL0-TPA relativ zu
B1s Sysvar-Region, Nested-Signal-Stack, Ready-Queue-Zustandsautomat) — dann
Schritt 1.
