# EL0→EL1-Upcall-/Signal-Zustellung — Design (kooperativer Kern, Baustein 1)

> Stand: 2026-07-24, Branch `desk-rebuild`. Design-Phase-Deliverable vor
> Implementierung. Richtungsentscheidung (Lead+User 2026-07-24):
> **EmuTOS-kooperative POLICY auf FreeMiNT-förmigen PRIMITIVEN**; dieser
> Upcall-/Signal-Mechanismus ist der erste Baustein (siehe
> `00-effort-comparison.md` §"Der teuerste Baustein", Schritt 1).
> Referenzmodell: FreeMiNTs `sendsig()`/`Psigreturn()`, adaptiert m68k→AArch64.
>
> **Lesekonvention.** Jede *Mechanismus-Aussage* ist mit `datei:zeile`
> belegt (FreeMiNT `references/freemint/…` oder NewTOS `kernel/…`/`tos-api/…`).
> Design-Entscheidungen sind explizit als **Empfehlung** markiert (meine Wahl,
> begründet), echte Unentschiedenheiten als **OFFEN** mit Optionen. Wo eine
> Quelle nicht belegbar war, steht **OFFEN** statt einer erfundenen Zahl.

---

## 1. Ziel & Konsumenten

Ein **Upcall** stellt einem EL0-Prozess einen *Rückruf* zu, den der EL1-Kern
ausgelöst hat: er biegt den nächsten Wiedereintritt des Prozesses so um, dass
zuerst eine **Handler-Funktion im EL0-Adressraum** läuft (mit Argumenten in
Registern), und danach der Prozess an der unterbrochenen Stelle exakt
weitermacht. Das ist strukturell ein klassischer Unix-Signal-Upcall
(`freemint-multitasking-model.md` §3.2, "Einordnung").

Zugestellt wird also: **eine Handler-Adresse (EL0-VA) + eine kleine Argumentliste
+ eine Typ-/Maskennummer**, an **genau einen** EL0-Prozess (den Prozess-Slot in
`PROCESS_TABLE`, `tos-api/src/bdos/process.rs:406-407`).

Der Mechanismus ist die **gemeinsame** Grundlage für fünf wartende Konsumenten
(`docs/analysis/open-tasks-post-2a.md:113-117` benennt sie als "vier+ Konsumenten
derselben Infra"):

| # | Konsument | Was upgerufen wird | heutiger Stand (belegt) |
|---|---|---|---|
| **H1** | `etv_*`-Hooks | installierter `etv_term`/`etv_timer`-Handler bei Term/Tick | Shadow gesetzt, **nie dispatcht**: `vector()` existiert, Kommentar "once the EL0-callback mechanism … lands" (`tos-api/src/bios/vectors.rs:167-176`); Slots `ETV_TIMER/CRITIC/TERM` (`vectors.rs:28-32`) |
| **M1** | `etv_critic` | Rwabs-Disk-Error → installierter Critical-Error-Handler + Retry/Cancel | `ETV_CRITIC` definiert, rwabs-Pfad ruft ihn nie (`open-tasks-post-2a.md:44-49`) |
| **P4-012** | `eralert` | `fm_show`/`eralert` Laufwerks-Critical-Alerts | braucht erst M1-Verdrahtung (`open-tasks-post-2a.md:44-49`) |
| **vex** | Maus-/Timer-ISR | `vex_butv/motv/curv/timv`-Vektor aus dem ISR in die EL0-App | Vektor wird nur **gespeichert**, nie aufgerufen: `swap_vector` (`tos-api/src/vdi/mouse.rs:655-690`) |
| **G_USERDEF** | AES `ub_code` | applikationsgezeichnetes Objekt: `ub_code`-Callback | EL1/same-privilege-Fall umgesetzt (`kernel_built`-Diskriminator, `tos-api/src/aes/object.rs:85-95`), **EL0-Ziel deferred** (`open-tasks-post-2a.md:50-52`) |

Plus der sechste, für den kooperativen Kern zentrale Fall: **Event-Zustellung an
einen blockierten EL0-AES-Client** (heute Busy-Poll im Trap-Handler,
`tos-api/src/aes/event.rs:308-382`; §8).

**Entwurfskonsequenz (Empfehlung).** Alle fünf/sechs Konsumenten brauchen
*dieselbe* Mechanik (Frame bauen, PC/SP umbiegen, Trampolin, Rücksprung-Syscall),
aber sie liefern eine **Handler-Adresse direkt** (der installierte Vektor, das
`ub_code`, der `etv_*`-Slot) — **nicht** eine POSIX-Signalnummer, die erst in
einer Dispositions-Tabelle nachgeschlagen wird. Der Mechanismus ist deshalb
**disposition-agnostisch** zu bauen: ein generisches
`deliver_upcall(frame, handler_va, &args, mask_bit)` als Kern, und — nur falls
später `Pkill`/`Psignal`-Kompatibilität gewünscht ist — eine dünne
Signalnummer→Handler-Tabelle (FreeMiNTs `p_sigacts`, `signal.h:107-120`) *darüber*.
FreeMiNTs `sendsig()` selbst ist bereits so geschichtet: `handle_sig` schlägt
`sa_handler` nach (`signal.c:471`), aber `sendsig` ab da ist reiner
Frame-Bau (`sig_mach.c:123-252`) ohne Signal-Semantik. Diese Schichtung 1:1
übernehmen.

---

## 2. FreeMiNT `sendsig`, seziert (die Referenz)

Alle Zeilen aus `references/freemint/sys/arch/sig_mach.c` (m68k).

### 2.1 Einstieg & Sonderfälle

- `sigact = &SIGACTION(curproc, sig)` (`sig_mach.c:48`) = `curproc->p_sigacts->sigact[sig]`
  (Makro `signal.h:120`) — Handler-Disposition.
- **System-Prozess** (`P_FLAG_SYS`, Kernel-Thread): kein User-Mode-Wechsel möglich,
  der Handler wird **direkt als C-Funktion** aufgerufen
  (`((void(*)(short))sigact->sa_handler)(sig)`, `sig_mach.c:100-121`, bes. :110).
  → *Analogon in NewTOS:* der EL1/same-privilege-Fall, den G_USERDEF bereits
  umsetzt (`object.rs:85-95` `kernel_built`); kein Upcall nötig.

### 2.2 Der Frame-Bau (Normalfall, `sig_mach.c:123-192`)

1. `call = &curproc->ctxt[SYSCALL]` (`:123`) — der beim Trap-Eintritt gebaute
   Kontext (`build_context`, `context.S:74-217`) ist Ausgangspunkt. `PROC_CTXTS=2`,
   Slots `SYSCALL`/`CURRENT` (`context.h:84-86`).
2. **Separater System-Stack pro Verschachtelung** (`:133-144`, der in der Aufgabe
   genannte Kernpunkt):
   `newstack = ((ulong)&newcurrent) - 0x40 - 12 - 0x100`, geprüft gegen
   Unterlauf (`< curproc->stack + ISTKSIZE + 256` → "stack overflow", `:136-140`)
   und Überlauf (`> curproc->stack + STKSIZE` → FATAL, `:141-144`). Damit
   überschreiben rekursive/verschachtelte Signale (z.B. während `Supexec`) den
   ursprünglichen System-Stack nicht.
3. `oldsysctxt = *call` (`:146`) — der unterbrochene Kontext wird gesichert.
4. `stack = call->sr & 0x2000 ? call->ssp : call->usp` (`:147`) — auf **dem Stack
   des unterbrochenen Kontexts** (Supervisor- oder User-Stack) werden zwei
   Fake-Frames gebaut:
   - `sigctxt = {sc_pc, sc_usp, sc_sr}` = PC/USP/SR des unterbrochenen Kontexts
     (`struct sigcontext`, `:38-43`; gefüllt `:169-171`) — das "was war vorher".
   - Darüber gepusht (`:172-175`): Zeiger auf `sigctxt`, `call->sfmt & 0xfff`
     (Vektor-/Format-Offset zur Handler-Diskriminierung), `sig` (das
     Handler-Argument) und **`ut->sig_return_p`** (`:175`) — die Rücksprungadresse
     = das Trampolin (`ut = curproc->p_mem->tp_ptr`, `:164`).
   - `call->usp`/`call->ssp` auf den neuen Stack-Top gesetzt (`:176-179`).
5. `call->pc = sigact->sa_handler` (`:181`) — der Kontext wird auf den User-Handler
   umgebogen.

### 2.3 Aktivierung & Rückkehr (`sig_mach.c:193-251`)

6. `save_context(&newcurrent)` (`:193`, setjmp-Muster): beim **ersten** Durchlauf
   (Rückgabe 0) wird der separate System-Stack mit `FRAME_MAGIC`, `oldstack`, `sig`
   markiert (`:220-224`), `leave_kernel()` (`:226`), dann `restore_context(call)`
   (`:228`) — der modifizierte Kontext wird sofort aktiviert; der Prozess läuft im
   User-Modus **im Handler** weiter.
7. Kehrt der Handler zurück → springt via Rücksprungadresse ins **Trampolin**
   (`sig_return`) → Syscall `Psigreturn()`. Der reaktiviert `newcurrent`
   (`restore_context(oldctxt)`, `:311`), wodurch `save_context` in `sendsig`
   "das zweite Mal" zurückkehrt (`:193`) und die Zeilen `:240-245`
   `ctxt[SYSCALL]` aus `oldsysctxt` (gepatcht mit den `sigctxt`-Werten)
   wiederherstellen — normaler Syscall-Austritt.

### 2.4 Das Trampolin & `Psigreturn`

**Trampolin** (`references/freemint/sys/arch/user_things.S`, Symbol `sig_return`) —
ein pro-Prozess in den Adressraum kopiertes Codestück (`struct user_things`,
`user_things.h:44-64`; `sig_return_p`/`pc_valid_return_p` `:53-54`):

```
sig_return:
    addq.l  #8,sp            // Signalnummer + sfmt vom Stack nehmen
    move.w  #0x011a,-(sp)    // Psigreturn(), GEMDOS-Funktion 0x11A
    trap    #1
pc_valid_return:             // "es sind wir" (Kernel prüft darauf)
    move.w  #-998,-(sp)
    bsr.s   terminateme      // Sicherheitsnetz: kehrt der Handler doch
                             // fehlerhaft zurück → Pterm
```

**`Psigreturn`** (`sys_psigreturn`, `sig_mach.c:265-316`):
`frame = curproc->sysstack` (`:275-276`), `sig = frame[2]` (`:277`),
`FRAME_MAGIC`+Bereichsprüfung (`:278-281`), `curproc->sysstack = frame[1]`
(alten System-Stack wiederherstellen, `:290`),
`p_sigmask &= ~(1L<<sig)` (Signal entmaskieren, `:291`); im Normalfall
`oldctxt = (CONTEXT*)((&frame[2]) + 0x40 + 0x100)`, `CTXT_MAGIC`-Prüfung,
`restore_context(oldctxt)` (`:304-311`). Der `longjmp`-aus-dem-Handler-Fall
(`ctxt[SYSCALL].pc != pc_valid_return_p`) wickelt *alle* Signal-Frames ab
(`:293-300`).

### 2.5 Markieren/Prüfen (Ober­schicht, `references/freemint/sys/signal.c`)

- `post_sig(p, sig)` (`signal.c:145-225`): setzt `p->sigpending |= (1<<sig)`
  (`:203-204`); ist es maskiert (`p->p_sigmask`, `:212-213`), passiert nichts
  weiter; sonst wird ein *wartender* Zielprozess `nrl → READY_Q` verschoben
  (`:216-224`) — **kein** sofortiger Kontextwechsel.
- `check_sigs()` (`signal.c:268-355`): aufgerufen aus `sleep()` und am
  Syscall-Austritt; maskiert das behandelte Signal (`p_sigmask |= sa_mask`, `:323`)
  und ruft `handle_sig(i)` (`:335`) → `sendsig(sig)` (`signal.c:471`).
- Signal-Felder im Prozess: `sigpending`, `p_sigmask`, `nsigs`
  (`references/freemint/sys/mint/proc.h`, Block "GEMDOS extension" der
  `struct proc`).

---

## 3. AArch64-Abbildung

Grundlegender struktureller Unterschied, der die Abbildung vereinfacht:

> **FreeMiNT** hält pro Prozess einen *persistenten* Trap-Kontext
> (`ctxt[SYSCALL]`, bei jedem Trap via `build_context` gebaut), und `sendsig`
> tanzt zwischen zwei Kontexten (`call` und `newcurrent`), weil der Handler
> *innerhalb desselben Kernel-Aufrufs* aktiviert wird.
>
> **NewTOS** baut den Kontext (`ExceptionFrame`, `kernel/src/arch/aarch64/exceptions.rs:52-68`)
> bei jedem Trap frisch auf dem EL1-Exception-Stack (`EXC_BODY`,
> `exception_vectors.s:23-46`), der Rust-Handler *mutiert* ihn, und das
> abschließende `eret` (`exception_vectors.s:51-73`) vollzieht den Transfer.
> Es gibt **keinen** persistenten Syscall-Kontext über den Handler hinaus — die
> Rolle von `ctxt[]` übernimmt der per-Prozess-`ParentFrame`-Schnappschuss
> (`process.rs:294-351`), den `Pexec`/`Pterm`/`Pwait` bereits benutzen.

Damit kollabiert FreeMiNTs Zwei-Kontext-Tanz zu **einem** Schritt: der Kern muss
nur den *unterbrochenen* EL0-Kontext sichern (damit `Psigreturn` ihn
wiederherstellt) und den aktuellen `ExceptionFrame` auf den Handler umbiegen —
das `eret` erledigt den Rest.

| FreeMiNT-Schritt (m68k) | NewTOS-Abbildung (AArch64) | Beleg |
|---|---|---|
| `call->pc = sa_handler` | `frame.elr = handler_va` | `exceptions.rs:441` (Muster) |
| `call->usp = neuer Frame-Top` | `frame.sp_el0 = handler_sp` (16-align'd, s.u.) | `exceptions.rs:443,946` |
| `sig` auf Stack (Handler-Argument) | `frame.gpr[0] = arg0` (`sig`), weitere Args → `gpr[1..]` (AAPCS64) | `exceptions.rs:430,939` |
| `ut->sig_return_p` auf Stack (Rücksprung) | `frame.lr = trampoline_va` (AAPCS64: Rücksprung in **LR**, nicht auf Stack) | `exceptions.rs:432` (Muster) |
| `sigctxt{pc,usp,sr}` auf User-Stack | **kernel-seitiger** Save des unterbrochenen `ExceptionFrame` (Empfehlung, §6) | `process.rs:294-351` |
| `save_context`/`restore_context(call)`, dann `eret` implizit | direktes `eret` am Handler-Ende (EXC_BODY) | `exception_vectors.s:73` |
| `restore_context(oldctxt)` in `Psigreturn` | `frame.*` aus dem gesicherten Kontext zurückkopieren (wie `finish_terminate`) | `exceptions.rs:554-566` |

### 3.1 Wiederverwenden oder erweitern? — `enter_el0` vs. Handler-Redirect

Es gibt in NewTOS **zwei** Wege nach EL0:

1. **`enter_el0_with_resume_capture`** (`user_entry.rs:114-239`) — der
   *Kernel-initiierte* Absprung (Boot/Selftest startet ein EL0-Programm); setzt
   `SP_EL0`/`ELR_EL1`/`SPSR_EL1` und macht selbst `eret` (`:207-216`).
2. **Handler-Redirect** — der Rust-Trap-Handler mutiert `frame.elr/sp_el0/spsr/gpr`
   und lässt das EXC_BODY-`eret` (`exception_vectors.s:73`) den Absprung tun; so
   arbeiten `Pexec` (`exceptions.rs:936-946`), `Pwait`/`apply_switch` (`:426-445`)
   und `Pterm`/`finish_terminate` (`:554-566`).

**Empfehlung:** Der Upcall nutzt **Weg 2** (Handler-Redirect), weil er *aus einem
Trap-/IRQ-Kontext heraus* zustellt (Syscall-Grenze oder ISR) und der Frame ohnehin
schon auf dem Exception-Stack liegt. Es wird **keine** neue EL0-Eintritts-Asm
gebraucht — `deliver_upcall` ist reine `frame`-Mutation, identisch im Stil zu
`apply_switch`'s Fresh-Zweig (`exceptions.rs:426-434`). Weg 1 bleibt für die
Kernel-initiierten Fälle.

---

## 4. Das EL0-Trampolin

Ein kleines EL0-Codestück, das der Handler bei `ret` anspringt und das dann
`Psigreturn` aufruft — 1:1 FreeMiNTs `sig_return` (`user_things.S`), aber
AArch64.

### 4.1 Wohin im EL0-VA? — Entscheidung

Die per-Prozess-TPA-Layout (B1/Option 1, `tos-api/src/bdos/process/loader.rs`):

```
0x000  BasePage                    [0x000, 0x100)   (loader.rs:161,177)
0x400  klassische Sysvars          $400-Region      (populate_sysvars, :779-839)
0x5C0  OSHEADER                     [0x5C0, 0x5F0)   (OSHEADER_VA, :666)
0x5F0  Cookie-Jar                   [0x5F0, 0x670)   (COOKIE_JAR_VA, :669; SLOTS :673)
0x670  ── SYSVAR_AREA_END ──                         (:676)
       ▓▓ freie Lücke [0x670, 0x1000) ≈ 2448 Bytes ▓▓  (innerhalb Page 0)
0x1000 RESERVED_LOW_END = App-Image-Basis            (:186; SDK newtos.ld)
 …     Text/Data/BSS/Heap
p_hitpa (=USER_TPA_TOP) EL0-Stack ↓                  (:168)
```

**Empfehlung: kernel-geschriebener Trampolin-Stub in der reservierten Lücke
`[0x670, 0x1000)`**, z.B. bei **`TRAMPOLINE_VA = 0x700`** (16-Byte-align, weit vor
`RESERVED_LOW_END`). Begründung, alles belegt:

- Die Lücke liegt **innerhalb Page 0**, die — wie jede TPA-Page — als
  `UserRwExec` gemappt wird (`loader.rs:505-517`, `map_page(…, PageFlags::UserRwExec)`
  für `off in 0..TPA_SIZE`), also EL0-**ausführbar**. Kein Extra-Mapping nötig.
- Kollidiert **mit nichts**: über SYSVAR_AREA_END (`0x670`), unter
  RESERVED_LOW_END (`0x1000`); Segmente < `0x1000` werden ohnehin abgewiesen
  (`validate_segment` → `SegmentVaOutOfRange`, `loader.rs:1002-1010`), Heap/Stack
  liegen weit darüber.
- Wird beim Laden geschrieben — genau da, wo `populate_sysvars` schon per
  Kernel-High-Half-Alias in Page 0 schreibt (`loader.rs:736-865`) — und liegt
  **innerhalb** des Cache-Maintenance-Fensters `[base_kva, base_kva+image_top)`
  (`loader.rs:496-498`), weil `image_top ≥ 0x1000 > 0x700`; die frisch
  geschriebenen Trampolin-Bytes sind also für den EL0-Instruction-Fetch sichtbar
  (Real-HW; QEMU cache-los). Das ist **exakt** FreeMiNTs Modell "Kernel kopiert
  `user_things` in den Prozess-Adressraum" (`user_things.S`), nur an fester Adresse
  statt via `tp_ptr`.

**Alternative (verworfen für jetzt): crt0-/runtime-gelieferter `sa_restorer`**
(Linux-Stil). Trade-off: bräuchte SDK-crt0-Änderungen und dass **jede** App einen
Restorer registriert; der kernel-geschriebene Stub braucht **null** App-Kooperation
(auch für 1:1-portierte, unveränderte Apps — vgl.
[[port-target-app-unmodified]]). Kein Vorteil, mehr Kopplung → verworfen.

**OFFEN:** exakter Offset (0x700 ist ein Vorschlag) und ob der Stub ein eigenes
Guard-Wort bekommt. Unkritisch, in der Implementierung fixierbar.

### 4.2 Trampolin-Body (AArch64, Vorschlag)

Bei Handler-Rückkehr (`ret` → `PC = LR = TRAMPOLINE_VA`):

```asm
trampoline:                 ; x0 = Handler-Rückgabewert (von Psigreturn ignoriert)
    mov  x8, #0x11A         ; GEMDOS-Funktion Psigreturn (bit-exakt FreeMiNT 0x11A)
    svc  #1                 ; Trap #1 = GEMDOS  → kehrt NICHT zurück
    ; Sicherheitsnetz (FreeMiNT pc_valid_return), falls Psigreturn doch zurückkäme:
    mov  x0, #0xFC1A        ; -998 als WORD (UWORD-Exitcode)
    mov  x8, #0x4C          ; Pterm
    svc  #1
```

Kein EL0-Stack-Push nötig: Signalargument und Rücksprungadresse reisen in
Registern (`x0`, `LR`) statt auf dem Stack (AAPCS64) — schlanker als FreeMiNTs
`addq.l #8,sp`-Aufräumen (`user_things.S`, `sig_return`).

---

## 5. `Psigreturn`-Syscall

Der Rücksprung-Syscall, den das Trampolin macht. Er **muss den `ExceptionFrame`
umschreiben** (Wiederherstellen von `elr`/`sp_el0`/`spsr`/GPRs des unterbrochenen
Kontexts) — er kann daher **nicht** die Blatt-Dispatch-Tabelle nutzen (die liefert
nur einen Wert in `x0`, `gemdos/mod.rs:130-133`).

### 5.1 Nummer & Einordnung — Entscheidung

**Empfehlung: GEMDOS-Funktion `0x11A` (282) über Trap #1**, bit-exakt mit
FreeMiNT (`user_things.S`, `move.w #0x011a`), frame-mutierend im Kernel-Handler
behandelt. Konkret:

- Einbau als weiterer frame-mutierender Sonderfall im GEMDOS-Zweig von
  `rust_sync_handler`, direkt neben Pterm/Pexec/Pwait
  (`exceptions.rs:283-297`):

  ```rust
  if matches!(class, TrapClass::Gemdos) {
      let func = frame.gpr[8] as u16;
      // … 0x4C/0x00/0x31 → handle_pterm; 0x4B → handle_pexec; 0x4D → handle_pwait …
      if func == 0x11A { handle_psigreturn(frame); return; }   // NEU
  }
  ```

- Erreicht **nie** `gemdos::dispatch`: `0x11A > MAX_FN (0x57)` (`gemdos/mod.rs:31`)
  würde dort ohnehin `EINVFN` liefern (`:127-129`); der frame-mutierende Sonderfall
  fängt es vorher ab — dieselbe Logik, mit der `Pexec`/`Pterm`/`Pwait` schon vor der
  Tabelle abzweigen. (`Ssystem` @ `0x154` zeigt den Präzedenzfall eines
  `>MAX_FN`-GEMDOS-Codes, `gemdos/mod.rs:124-126`.)

**Alternative (verworfen): dediziertes SVC-Immediate** (z.B. `svc #0x51A`) als
eigene `TrapClass`. Trade-off: saubere Trennung von GEMDOS, aber erfindet eine
NewTOS-eigene Nummer ohne TOS-Präzedenz und bricht die Konvention "Trap-Nummern
spiegeln m68k" (`trap.rs:22-49`), ohne Kompatibilitätsgewinn. → GEMDOS `0x11A`
gewinnt.

### 5.2 Was `handle_psigreturn` tut

1. Aktuellen Prozess-Slot bestimmen (`slot_for(current())`, `process.rs:553-565`).
2. Obersten gesicherten Kontext vom **kernel-seitigen Upcall-Stack** dieses Slots
   *poppen* (§6) — den `ExceptionFrame`-Schnappschuss, den `deliver_upcall`
   gesichert hat.
3. Diesen Schnappschuss zurück über `frame` kopieren (GPRs, `lr`, `elr`, `spsr`,
   `sp_el0`) — **exakt** das Muster von `finish_terminate` (`exceptions.rs:554-566`)
   bzw. `apply_switch`'s Resume-Zweig (`:435-445`).
4. Die Upcall-Maske für den behandelten Typ löschen (FreeMiNT `p_sigmask &=~…`,
   `sig_mach.c:291`).
5. `return` → EXC_BODY-`eret` (`exception_vectors.s:73`) resümiert den
   unterbrochenen EL0-Kontext, inklusive des **ursprünglichen Syscall-Rückgabewerts
   in `x0`** (der beim Sichern in Schritt §6.2 mitgesichert wurde).

Fehlerfall (leerer Upcall-Stack / Konsistenzbruch): `frame` unangetastet lassen und
`EINVFN` in `x0` — analog `finish_terminate`s Misuse-Zweig (`exceptions.rs:521-527`).

---

## 6. Gesicherter Kontext & Verschachtelung

### 6.1 Wo der Vor-Upcall-Kontext liegt — Entscheidung

**Empfehlung: kernel-seitiger, per-Prozess Kontext-Stack** — eine kleine
Fixed-Depth-Anordnung von `ParentFrame`-Schnappschüssen im `Process`-Slot, **nicht**
auf dem EL0-Stack (FreeMiNT sichert `sigctxt` auf dem User-Stack, `sig_mach.c:166-171`).

Neue Felder im `Process`-Slot (`process.rs:188-269`), im Stil der vorhandenen
`saved_frame`/`parent_frame`:

```rust
pub upcall_ctx: [ParentFrame; MAX_UPCALL_NEST],  // je 34×AtomicU64 (process.rs:294-300)
pub upcall_depth: AtomicU8,                        // aktuelle Verschachtelungstiefe
pub upcall_mask: AtomicU32,                         // p_sigmask-Analogon (Typ-Bits)
```

Begründung (Empfehlung, mit Trade-off):

- **Robuster:** Der Resume-Kontext liegt kernel-seitig; eine EL0-App kann ihn nicht
  korrumpieren. FreeMiNTs on-stack-`sigcontext` ist prinzipiell app-manipulierbar
  (bewusst, wegen `longjmp`, `sig_mach.c:293-300`).
- **Einfacher auf AArch64:** Die `ParentFrame`-Schnappschuss-Maschinerie existiert
  bereits (`process.rs:302-351`) und wird von `Pexec`/`Pterm`/`Pwait` verwendet;
  `deliver_upcall`/`handle_psigreturn` sind damit reine Kopi
  (`gpr(i).store/load`) — kein neuer Mechanismus.
- **Trade-off / OFFEN:** Der FreeMiNT-`longjmp`-aus-dem-Handler-Fall (Handler
  kehrt nie via Trampolin zurück, sondern springt woanders hin und ruft später
  `Psigreturn` zum Abwickeln *aller* Frames, `sig_mach.c:293-300`) wird mit
  kernel-seitigem Stack **nicht** automatisch unterstützt — er bräuchte eine
  explizite "unwind bis Tiefe N"-Variante von `Psigreturn`. Die fünf/sechs
  NewTOS-Konsumenten (§1) sind **Call-and-Return-Callbacks** (etv_*/vex/G_USERDEF/
  eralert), kein `setjmp`/`longjmp`-Signal-Idiom → für sie irrelevant. **OFFEN**,
  falls je echte MiNT-`longjmp`-Signalsemantik gebraucht wird; dann on-EL0-stack
  wie FreeMiNT nachrüsten.

### 6.2 Was genau gesichert wird

Der **volle restaurierbare `ExceptionFrame`** (30 GPRs + `lr` + `elr` + `spsr` +
`sp_el0`) — exakt der Umfang eines `ParentFrame` (`process.rs:274-300`). Wichtig:
gesichert wird **nach** dem Schreiben des Syscall-Ergebnisses in `frame.gpr[0]`
(`exceptions.rs:370`), damit der ursprüngliche Rückgabewert des unterbrochenen
Syscalls im Schnappschuss steht und nach dem Handler korrekt an die App geht
(§7.2). Anders als in FreeMiNT (wo die callee-saved-Konvention den unterbrochenen
Code teilweise selbst rettet) sichern wir **alle** GPRs, weil der unterbrochene
EL0-Code an beliebiger Stelle mittendrin ist und *alle* Register live sein können.

### 6.3 Verschachtelung

`deliver_upcall` *pusht* (`upcall_depth += 1`, Schnappschuss in
`upcall_ctx[depth]`), `handle_psigreturn` *poppt*. Auf der EL0-Seite laufen
verschachtelte Handler natürlich auf dem **weiter nach unten wachsenden
EL0-Stack** (jeder Handler-Frame unter dem vorigen), was FreeMiNTs "lässt den
User-Stack wachsen" (`sig_mach.c:149-155`) entspricht; der kernel-seitige Stack
spiegelt diese Tiefe.

**Max-Verschachtelung** = `MAX_UPCALL_NEST`. **OFFEN (Vorschlag: 8).** FreeMiNT
bindet die Tiefe an den 16-KiB-System-Stack, der pro Nest ~12 Bytes wächst
(`sig_mach.c:141-144,155`); NewTOS bindet sie an dieses Array (Speicher:
`ParentFrame`=272 B × `MAX_UPCALL_NEST` × `MAX_PROCESSES`=16 → bei 8 ≈ 34 KiB BSS,
vertretbar). Überschreitung → Delivery unterdrücken (Upcall bleibt pending) oder
`Pterm(-998)`, wie FreeMiNTs "stack overflow"-ALERT (`sig_mach.c:138`). **OFFEN**,
welches Verhalten.

### 6.4 Handler-`SP_EL0` (Alignment)

Der Handler läuft auf dem **aktuellen `SP_EL0`** der App (kein separater
Signal-Stack), er pusht seinen eigenen Frame darunter. AAPCS64 verlangt
16-Byte-Alignment beim Aufruf: an einer **Syscall-Grenze** ist `SP_EL0` bereits
16-align'd (die App machte `svc` mit align'tem Stack); an einer **IRQ-Grenze**
evtl. nicht → `handler_sp = frame.sp_el0 & !0xF`. Optional (FreeMiNT-artig) ein
kleiner Guard-Abstand (`- 0x20`) unter `SP_EL0` als Red-Zone-Schutz. **OFFEN**
(Guard ja/nein).

---

## 7. Zustellpunkte

FreeMiNT prüft Signale (a) in `sleep()` (`proc.c:452-454`) und (b) am
Syscall-Austritt (`syscall.S:600-610`) — nie mitten in der Ausführung, immer an
einer Grenze (`freemint-multitasking-model.md` §3.1/§1.7). NewTOS spiegelt das:

### 7.1 Primär: am Syscall-Rücksprung nach EL0

Die exakte Einbaustelle ist das **Ende von `rust_sync_handler`**, unmittelbar
neben dem bereits existierenden "deferred action"-Hook `take_pending_terminate`
(`exceptions.rs:363-366`) — dem strukturellen Zwilling des Upcall-Checks (eine am
Trap-Ende konsumierte, aufgeschobene Aktion, die den Frame umbiegt). Reihenfolge:

1. Syscall-Ergebnis berechnen (`exceptions.rs:326-350`).
2. `take_pending_terminate()` (`:363-366`) — hat Vorrang (verwirft das Ergebnis,
   terminiert).
3. Ergebnis nach `frame.gpr[0]` schreiben (`:370`).
4. **NEU:** `if let Some(u) = take_pending_upcall(current_slot) { deliver_upcall(frame, u); return; }`
   — sichert den Frame *inklusive* des in Schritt 3 geschriebenen Ergebnisses (§6.2)
   und biegt auf den Handler um.

`take_pending_upcall`/`post_upcall` sind das `sigpending`/`post_sig`-Analogon
(`signal.c:203-204`), lock-frei über Atomics im Slot (wie
`request_terminate`/`take_pending_terminate`, `process.rs:473-486`); Maskenprüfung
= FreeMiNTs `p_sigmask`-Check (`signal.c:212-213`).

### 7.2 Rückgabewert-Erhalt

Weil der Frame *nach* dem Ergebnis-Write gesichert wird (§6.2), liefert
`Psigreturn` den ursprünglichen `x0`-Wert des unterbrochenen Syscalls korrekt
zurück — der Handler ist transparent für den laufenden Aufruf.

### 7.3 IRQ-Zustellung (vex/Timer) — Vorbedingung/OFFEN

Der **vex**-Konsument (Maus-/Timer-ISR → EL0) stellt aus einem **IRQ** zu: der ISR
macht `post_upcall`, und beim `eret` des IRQ *zurück nach EL0* (SPSR.M == EL0t)
läuft derselbe Delivery-Check. **Aber:** NewTOS hat heute **keine** EL0-IRQ-Route
— der Lower-EL-IRQ-Vektor (`0x480`) zeigt auf `__vec_unhandled` (halt),
`exception_vectors.s:108`. Der vex-Pfad braucht also **zuerst** die
Lower-EL-IRQ-Vektor-Verdrahtung + einen `rust_irq_handler`, der bei EL0-Herkunft
den Delivery-Check ausführt. **OFFEN/Vorbedingung** — die vier
Syscall-Grenze-Konsumenten (etv_term, etv_critic/eralert, G_USERDEF) brauchen das
**nicht** und gehen zuerst.

---

## 8. Interaktion mit `TTBR0`/Blockieren

Der **kooperative Kernfall**: Zustellung an einen EL0-AES-Client, der gerade in
einem Event-Wait blockiert. Heute *busy-pollt* `evnt_multi` im Trap-Handler
(`event.rs:308-382`, `while … { h.pump_input() }`) — er kehrt während des Wartens
nie nach EL0 zurück, es gibt keinen zweiten Kontext (`event.rs:7-12`;
`emutos-cooperative-model.md` §7.1). Solange das so ist, *kann* ein Upcall an einen
"blockierten" Client nur **innerhalb** dieses einen Trap-Aufrufs zugestellt werden
— und der kann nicht `eret`en. Deshalb ist Schritt 2 des Fahrplans (Busy-Poll →
`sleep()`/`wake()`, `00-effort-comparison.md` Schritt 2) die Voraussetzung.

**Sobald** `evnt_multi` wie `Pwait` blockiert (Frame sichern + wegschalten,
`handle_pwait`, `exceptions.rs:377-402`), ist die Zustellung sauber und **reitet
auf dem normalen Resume**:

1. Client wecken: `post_upcall(slot)` + Slot runnable/ready machen (FreeMiNTs
   `post_sig` verschiebt `nrl → READY_Q`, `signal.c:216-224`).
2. Wählt der Scheduler den Slot später (`on_yield`/`switch_to`, `sched.rs:104-111`,
   `92-100`), installiert `apply_switch` dessen `TTBR0` (`install_ttbr0(sw.l0_pa)`,
   `exceptions.rs:421-423`) und würde im **Resume-Zweig** (`:435-445`) den
   gesicherten `saved_frame` zurückspielen.
3. **Redirect hier:** Ist für den resümierten Slot ein Upcall pending, biegt der
   Resume-Zweig statt auf `saved_frame.elr` auf den **Handler** um (der bisherige
   `saved_frame` wandert auf den kernel-seitigen Upcall-Stack, §6). Der Client läuft
   also im Handler **in seinem eigenen `TTBR0`** — genau das, was §1's sechster
   Fall verlangt. Bei Handler-Ende via Trampolin/`Psigreturn` wird der ursprüngliche
   Block-Kontext resümiert; für den kooperativen AES heißt das: der `evnt_multi`
   liefert danach das Event, das ihn geweckt hat.

Anbindung: `sched::Switch` (`sched.rs:37-47`) trägt schon
`slot`/`fresh`/`l0_pa`/`entry_va`; für den Upcall-Redirect genügt ein Blick auf
`take_pending_upcall(sw.slot)` im Resume-Zweig von `apply_switch` — **keine**
Änderung an der `Switch`-Struktur oder am Trap-Pfad, nur an dieser Verzweigung
(passt zur `sched.rs`-Doktrin "Scheduling-Änderungen bleiben im Modul",
`sched.rs:10-18`).

---

## 9. Ready-Queue-/Prozesszustand-Skizze (sekundär)

Nur so weit, wie es Upcall + Schritt-2-Substrat rahmt — **nicht** der volle
Scheduler. Kooperativ-first, deutlich weniger Zustände als FreeMiNTs acht Queues
(`mint/proc.h:296-305`).

Heute: ein Bit `runnable` pro Slot (`process.rs:264`), Flat-Round-Robin
(`pick_next`, `sched.rs:81-90`), kein Warte-Zustand — Blockieren = Busy-Poll
(`event.rs`). Vorschlag für Schritt 2 (drei Laufzeit-Zustände über der
Lifecycle-`State` `Free/Loaded/Running/Terminated`, `process.rs:71-99`):

```
                 sleep(cond)  [Frame sichern, wegschalten]
   ┌──────────┐  ───────────────────────►  ┌──────────┐
   │ RUNNING  │                             │ WAITING  │  (wait_cond)
   │ (rlr=1)  │  ◄───────────────────────   │          │
   └────┬─────┘   Scheduler wählt (resume)  └────┬─────┘
        │  Pwait / yield                          │ wake(cond) / post_upcall
        ▼                                         ▼
   ┌──────────┐  ◄──────────────────────────────────┐
   │  READY   │  (runnable=1, im Round-Robin)         │
   └──────────┘  ─────────────────────────────────────┘
```

Neue Primitive (Namen FreeMiNT-nah, `proc.c:490-716`):

- `sleep(cond)` — verallgemeinert `handle_pwait` (`exceptions.rs:377-402`): Frame in
  `saved_frame` sichern, `wait_cond = cond` setzen, `runnable=false`, nächsten Peer
  wählen (`on_yield`, `sched.rs:104-111`). Ersetzt die `pump_input`-Busy-Loops.
- `wake(cond)` — alle Slots mit passendem `wait_cond` wieder `runnable=true`
  (FreeMiNT `do_wake`, `proc.c:665-701`); weckt nicht selbst um (wie FreeMiNT).
- `wait_cond: AtomicU64` — neues `Process`-Feld (FreeMiNTs `wait_cond`,
  `proc.h:146`); ein Schlüssel (Event-Typ/Adresse), auf den gewartet wird.

Der Upcall hängt an genau dieser Naht: `post_upcall` macht `wake`-artig ready, und
der **Resume** (nicht der `sleep`-Aufruf) fährt den Delivery-Check (§8). Präemption/
Prioritäten/Aging (FreeMiNT §1.4-1.7) sind **DEFERRED**, additiv obendrauf
(`00-effort-comparison.md` Schritt 4) — außerhalb dieses Deliverables.

---

## 10. Offene Fragen & Implementierungsreihenfolge

### 10.1 OFFEN (Optionen + Empfehlung markiert)

1. **Upcall-Payload-Modell.** *Empfehlung:* generischer Descriptor
   `{handler_va, args:[u64;N], mask_bit}` — die Konsumenten liefern die
   Handler-Adresse direkt (§1). Eine POSIX-`sigacts`-Nummer→Handler-Tabelle
   (`signal.h:107-120`) nur als optionale Oberschicht für spätere
   `Pkill`/`Psignal`-Kompatibilität. **OFFEN:** `N` (wie viele Arg-Register;
   Vorschlag 4 — deckt vex-Eventdaten, `etv_critic`-Fehlercode,
   G_USERDEF-`PARMBLK`-Ptr ab).
2. **Kernel-seitiger Save vs. on-EL0-Stack** (§6.1). *Empfehlung:* kernel-seitig.
   **OFFEN** nur, falls echte MiNT-`longjmp`-Signalsemantik verlangt wird.
3. **Trampolin-Offset** (§4.1). *Empfehlung:* `0x700` in `[0x670,0x1000)`.
   **OFFEN:** exakter Wert/Guard.
4. **`Psigreturn`-Nummer** (§5.1). *Empfehlung/entschieden:* GEMDOS `0x11A`,
   frame-mutierend. (Kein echtes OFFEN — bit-exakt FreeMiNT, kollisionsfrei
   `>MAX_FN`.)
5. **`MAX_UPCALL_NEST`** + Overflow-Verhalten (§6.3). **OFFEN** (Vorschlag 8;
   Overflow → Delivery unterdrücken oder `Pterm(-998)`).
6. **IRQ-Zustellpfad** (§7.3). **OFFEN/Vorbedingung** — Lower-EL-IRQ-Vektor +
   `rust_irq_handler` fehlen (`exception_vectors.s:108`); blockiert nur **vex**,
   nicht die Syscall-Grenze-Konsumenten.
7. **Guard-Abstand unter Handler-`SP_EL0`** (§6.4). **OFFEN** (Vorschlag: `- 0x20`).

### 10.2 Vorgeschlagene Implementierungsreihenfolge

Bottom-up, jeder Schritt Developer→Tester (keine Wegwerf-Arbeit):

1. **Generische Upcall-Primitive** (erster Developer-Task):
   - `Process`-Felder `upcall_ctx[]`/`upcall_depth`/`upcall_mask` +
     `post_upcall`/`take_pending_upcall` (Analogon `request_terminate`,
     `process.rs:456-486`).
   - Trampolin-Stub-Schreiber im Loader (in `populate_sysvars`-Nachbarschaft,
     `loader.rs:736-865`, innerhalb des Cache-Fensters `:496-498`).
   - `deliver_upcall(frame, handler_va, &args)` (Frame sichern → Redirect, Stil
     `apply_switch` `exceptions.rs:426-445`).
   - `handle_psigreturn` (GEMDOS `0x11A`) im Gemdos-Sonderfallblock
     (`exceptions.rs:283-297`).
   - Delivery-Check am `rust_sync_handler`-Ende (nach `:370`, neben
     `take_pending_terminate` `:363`).
   - *Test:* Selftest installiert einen Handler, `post_upcall` aus einem Syscall,
     Handler läuft + kehrt via Trampolin/`Psigreturn` zurück, Original-`x0` intakt.
2. **Erster echter Konsument** an der Syscall-Grenze (kein IRQ nötig): **G_USERDEF
   `ub_code`→EL0** (WP3, `object.rs:85-95`) *oder* **etv_term/etv_critic** (H1/M1,
   `vectors.rs:167-176`) — je nach einfacherer End-to-End-Testbarkeit. Danach
   **P4-012 eralert** (baut auf M1 auf).
3. **Sleep/Wake-Substrat** (§9, `00-effort-comparison.md` Schritt 2) +
   Deliver-on-Resume für blockierte AES-Clients (§8): `evnt_multi`-Busy-Poll
   (`event.rs:308-382`) durch `sleep(cond)`/`wake(cond)` ersetzen; Upcall-Redirect
   im Resume-Zweig von `apply_switch`.
4. **vex/IRQ-Pfad** (nach Lower-EL-IRQ-Verdrahtung, §7.3) — der einzige Konsument,
   der die IRQ-Route braucht.

---

### Quellen (alle `datei:zeile` oben zitiert)

- **FreeMiNT:** `sys/arch/sig_mach.c` (`sendsig` :45-252, `sys_psigreturn` :265-316,
  separater System-Stack :133-144), `sys/signal.c` (`post_sig`/`check_sigs`/
  `handle_sig`), `sys/arch/user_things.S`+`.h` (Trampolin, `struct user_things`),
  `sys/mint/signal.h` (`sigacts`/`SIGACTION`), `sys/mint/proc.h` (Signal-Felder,
  Queues), `sys/mint/arch/context.h` (`PROC_CTXTS`).
- **NewTOS:** `kernel/src/arch/aarch64/exceptions.rs` (`ExceptionFrame`,
  `rust_sync_handler`, `handle_pwait`/`apply_switch`/`finish_terminate`,
  Gemdos-Sonderfallblock), `kernel/src/arch/aarch64/exception_vectors.s` (EXC_BODY),
  `kernel/src/arch/aarch64/user_entry.rs` (`enter_el0_with_resume_capture`,
  `restore_boot_ttbr0`), `kernel/src/arch/aarch64/el0.rs` (VA-Fenster),
  `tos-api/src/bdos/process.rs` (`Process`/`ParentFrame`/`RUN`/deferred terminate),
  `tos-api/src/bdos/process/sched.rs` (`Switch`/`on_yield`/`switch_to`),
  `tos-api/src/bdos/process/loader.rs` (TPA-/B1-Layout, `populate_sysvars`),
  `tos-api/src/gemdos/mod.rs` (Dispatch/`MAX_FN`), `tos-api/src/trap.rs`
  (`TrapClass`), `tos-api/src/bios/vectors.rs` (etv_*), `tos-api/src/vdi/mouse.rs`
  (vex), `tos-api/src/aes/object.rs` (G_USERDEF), `tos-api/src/aes/event.rs`
  (Busy-Poll), `docs/analysis/open-tasks-post-2a.md` (Konsumentenliste).
