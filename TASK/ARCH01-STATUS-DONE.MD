# ARCH01 — STATUS: DONE (2026-09-24)

**Auftrag:** Erst pruefen, ob die Vermutung des Tickets stimmt (Wettlauf
zwischen `msr elr_el1` und `eret`), und zwar indem der Fall auf Bestellung
ausgeloest wird; danach — und nur dann — das Fenster schliessen.

Basis: Branch `desk-rebuild`, HEAD `7e9b4aa`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine reale Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset` auf dem geteilten
Baum (nur lesendes `git show HEAD:…` fuer die „vorher"-Kopien).

Messung: [`measurements/measurement-2026-09-24-arch01/`](../measurements/measurement-2026-09-24-arch01/).
Quelle: ARM ARM **DDI 0487M.c** (nicht in `references/`; ueber Arms
Dokumentendienst gelesen) — D1.4.2, D1.4.4, C6.2.157, C6.2.288, C5.2.3.

---

## 1. Die Hypothese: bestaetigt, durch Ausloesen

`method/force_race_patch.py` haelt in einer **Kopie** des Baums das Fenster
kuenstlich auf — 16.777.216 Schleifendurchlaeufe zwischen
`msr elr_el1, {entry}` und dem `eret`, also weit mehr als die 5 ms der
200-Hz-Uhr. Damit faellt der Fehler bei **jedem** Programmstart, nicht bei
jedem siebten (`serial/forced-before-serial.log`):

```
[auto] running C:\AUTO\HELLO.NCP
[launch] EL0: slot 0 entry 0x1000
[EXC] synchronous (non-SVC) exception (Instruction Abort (lower EL)):
      ESR=0x000000008200000e ELR=0xffff0000000c4e80 SPSR=0x0000000000000000
[EXC] FAR=0xffff0000000c4e80
```

Gleiche ESR, gleiches `SPSR = 0`, gleiches `ELR == FAR ==` Kernadresse wie in
der Meldung aus DESK19 (RUNINFO §7). Die Adresse ist diesmal zuordenbar
(`evidence/forced-before-disasm.txt`):

```
ffff0000000c4e78: msr ELR_EL1, x21
ffff0000000c4e7c: mov x11, #0x1000000
ffff0000000c4e80: subs x11, x11, #0x1      <-- ELR/FAR zeigen hierher
ffff0000000c4e84: b.ne  ...e80
ffff0000000c4e90: eret
```

`ELR_EL1` traegt den Kern-PC **aus der Warteschleife**: dorthin hat ihn der
Ausnahmeeintritt geschrieben, und der `eret` hat ihn nach EL0 mitgenommen.

Dieselbe Kopie, dieselbe Warteschleife, mit `msr daifset, #0xf` davor:
sauberer Start, `[auto] 1 AUTO program(s) run`, `[desk] desktop up`
(`serial/forced-after-serial.log`, `evidence/forced-after-disasm.txt`).

## 2. Was die Quelle sagt

| Stelle | Aussage | Wofuer |
|---|---|---|
| D1.4.2 R_DQXFW | „Fields in SPSR_ELx are written … The preferred exception return address is written to ELR_ELx." | der Eintritt ueberschreibt beide Register — der Defekt |
| D1.4.2 R_WTXBY | „All of PSTATE.{D, A, I, F, SP} are set to 1." | warum der Trap-Rueckweg von Haus aus sicher ist |
| D1.4.4 R_BWCFK | „The contents of PSTATE are set to the values held in SPSR_ELx." | warum die Maske den `eret` nicht ueberlebt |
| C6.2.157 `ERET` | „restores PSTATE from the SPSR, and branches to the address held in the ELR" | dito |
| C6.2.288 `MSR (immediate)` | `DAIFSet`: `PSTATE.D|=op[3] … PSTATE.F|=op[0]` | was `#0xf` tut |
| C5.2.3 `DAIF` | D[9] A[8] I[7] F[6] | Bitlagen |

## 3. Der Fix

`kernel/src/arch/aarch64/user_entry.rs`, `enter_el0_with_resume_capture`:

```
"msr   daifset, #0xf",      // neu
"msr   spsel, #1",
"msr   sp_el0, {sp}",
"msr   elr_el1, {entry}",
"mov   {tmp}, #0",
"msr   spsr_el1, {tmp}",
"eret",
```

Eine Zeile, plus Begruendung im Quelltext und im Modulkopf (Schritt 3 der
Aufzaehlung). Keine Umstellung der Reihenfolge, kein `isb`, kein zweiter
Puffer. Ausgeliefert nachgesehen: `evidence/fixed-shipping-disasm.txt`.

Die Maske deckt genau das Fenster ab, in dem `SP_EL0`/`ELR_EL1`/`SPSR_EL1`
den **Benutzer**zustand tragen; asynchrone Ausnahmen sind die einzigen, die
hineingelangen koennen (die Befehle darin ruehren keinen Speicher an). Nach
dem `eret` ist sie weg, weil das geschriebene `SPSR_EL1` `0` ist.

## 4. Zwei Korrekturen an der Aufzaehlung des Tickets

**(a) Betroffene: elf Aufrufstellen in zehn Dateien, nicht „acht
Selbsttests".**

```
kernel/src/launch.rs:122                       run_program_with_tail_rc
kernel/src/selftest_d3.rs:374                  kernel/src/selftest_d6.rs:254
kernel/src/selftest_d7.rs:151                  kernel/src/selftest_ec4.rs:254
kernel/src/selftest_etv_term.rs:251            kernel/src/selftest_pexec.rs:286
kernel/src/selftest_pexec_d4d5.rs:177          kernel/src/selftest_upcall.rs:228
kernel/src/selftest_sleepwake_switch.rs:363 und :422
```

Neun Selbsttestdateien, eine davon mit zwei Aufrufen. Keiner haengt heute in
einem Bootpfad — `kernel/src/lib.rs` ruft nur `selftest_emmc::run()`
(Erinnerung `unwired-selftests-rot-silently`).

**(b) `Pexec` ist nicht betroffen.** Der GEMDOS-Trap aus EL0 benutzt den
Helfer nicht: `exceptions.rs:1509-1520` schreibt den Ausnahmerahmen um
(`frame.elr = child.entry_va`, `frame.spsr = 0`) und kehrt ueber `EXC_BODY`
zurueck — dort ist `DAIF` seit dem Ausnahmeeintritt maskiert. Im Betrieb ist
`launch.rs` damit der einzige Verbraucher, ueber drei Wege: Doppelklick /
Ablegen auf ein Programm (`desk/main.rs:655,695`), `\AUTO\` beim Booten
(`lib.rs:396`), Shell-Schleife / `#Z`-Autostart (`shell_loop.rs:334`).

## 5. Ein zweites Fenster derselben Form? Nein

Drei `eret` im ganzen Kernel:

* `exception_vectors.s:73` (EXC_BODY) — maskiert seit dem Ausnahmeeintritt;
  kein Behandler gibt vorher frei. `gic::enable_irqs` (`msr daifclr, #0x2`)
  hat genau einen Aufrufer: `lib.rs:299`, einmal beim Booten.
* `boot.rs:291` (EL2→EL1) — `SPSR_EL2 = 0x3c5` (DAIF maskiert), und zu diesem
  Zeitpunkt gibt es weder Vektortabelle noch GIC.
* `user_entry.rs` — dieser, jetzt maskiert.

Ausserhalb dieser Stellen schreibt nichts `elr_el1`/`spsr_el1` als
Systemregister; die Behandler fassen nur den Rahmen im Speicher an.

## 6. Abnahme

```
zwanzig Startlaeufe (DESK19 d19-full, je frische Kartenkopie, ein Kernel)
  [EXC]                     0
  [PANIC]                   0
  EL0-Starts                40   (je \AUTO\HELLO.NCP + Ziehstart)
  CMDLINE: len 13 arg …     20 von 20
  Zeilen je Protokoll       354  (in allen zwanzig gleich)
serial/run01 gegen DESK19 serial/d19-full-serial.log: eine Zeile Unterschied
  (der Uhrzeitstempel) — das Startverhalten ist unveraendert.
qemu-virt: [desk] desktop up, 0 [EXC], 0 [PANIC]
```

## 7. Bau + Regression (geteilter Baum)

```
cargo run -p xtask -- test                938 passed / 0 failed / 1 ignored
                                          (Basis 938 / 0 / 1)

cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
cargo run -p xtask -- build --board rpi4       tos-api 64 / kernel 94 (Basis 64/94)
cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
cargo run -p xtask -- build --board qemu-virt  tos-api 64 / kernel 94 (Basis 64/94)

cargo clippy --features board-rpi4        tos-api 80 / kernel 125 (Basis 80/125)
cargo clippy --features board-qemu-virt   tos-api 80 / kernel 125 (Basis 80/125)
```

Delta ueberall 0. Der geteilte Zielordner enthaelt zuletzt den **rpi4**-Bau
(Erinnerung `cargo-xtask-board-target-footgun`).

## 8. Geaendert

| Datei | Was |
|---|---|
| `kernel/src/arch/aarch64/user_entry.rs` | `msr daifset, #0xf` + Begruendung; Modulkopf Schritt 3 |
| `docs/MEMORY.md` | **QUIRK-003** (Symptom, Ursache, Fix, warum schwer wiederzufinden, Quelle) |
| `docs/ARCHITECTURE.md` | Prozessmodell: „EL0 entry is uninterruptible" mit Verweis auf QUIRK-003 |
| `docs/STATUS.md` | Recent Decisions 2026-09-24 |
| `TASK/ARCH01.md` | Status + `## Abschluss` |
| `measurements/measurement-2026-09-24-arch01/` | RUNINFO, method/, serial/ (23), evidence/ (4), screendumps/ (6), manifest.sha256 |

## 9. Offen / fuer eigene Tickets

* **Reale Pi steht aus.** Der Fehler ist architektonisch und nicht
  QEMU-spezifisch, auf echter Hardware aber seltener (kuerzeres Fenster
  relativ zur Uhr). Ein Hardwarelauf ist Sache des Leads/Users.
* **Die neun Selbsttests haengen in keinem Bootpfad** — sie sind
  Aufrufstellen, aber keine Pruefung. Eigenes Ticket wert
  (`unwired-selftests-rot-silently`).
