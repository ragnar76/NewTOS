# measurement-2026-09-26-desk43-u1

Belege zu **DESK43 / U1** (`docs/analysis/desktop-userspace-plan-v2.md` §5 U1,
Maßnahme 4 aus §6.4): das Instrument, mit dem jede weitere Einheit des
Desktop-Umzugs geprüft wird.

Baum: `desk-rebuild` @ `b3cec0c` + die drei U1-Änderungen.
EmuTOS-Referenz: `references/emutos` @ `1a7484ab`.
Alles headless in QEMU `raspi4b` (640×480, Dev-Bau). **Keine echte Pi 4.**

```
method/
  commands.md                    wie gefahren, inkl. der Fehlerquelle beim 1. Anlauf
  scratch-measurement.diff       der Scratch-Stand des Messlaufs (zurückgedreht)
  scratch-control-el0-gate.diff  der Scratch-Stand des Kontrolllaufs (zurückgedreht)
serial/
  boot-widened-gate.log          Messlauf: Tracer an, Tor erweitert
  boot-control-el0-gate.log      Kontrolllauf: Tracer an, altes EL0-Tor
  boot-final-tracer-off.log      Auslieferungsstand: Tracer aus
  boot-no-card-smoke.log         Rauchprobe ohne SD-Karte (0 Laufwerke)
trace/
  QEDTRACE-widened-gate.log                 die Rohdatei des Messlaufs (508 Z.)
  QEDTRACE-widened-gate-with-pro-hoist.log  derselbe Boot mit der QED-Geometrie-
                                            umleitung, zum Vergleich
  desktop-section-annotated.txt             der Desktop-Abschnitt, Opcodes
                                            benannt, Wiederholungen gerafft
analysis/
  deskmain-comparison.md         Zeile für Zeile gegen deskmain.c:1950-2075
  differences.md                 die zwölf Abweichungen, bewusst nicht behoben
  drift-probe.txt                tools/desk-drift-probe.sh -v
```

## Die drei Zahlen

| | |
|---|---|
| **Driftprobe** | **47** (A 21 · B 21 · C 5) — der Ausgangswert, den U2…U10 unterbieten müssen |
| **Kontrolle** | mit dem alten `dbg_from_el0`-Tor: **0 Byte** Trace bei einem Desktop-Boot |
| **Messung** | mit dem erweiterten Tor: **27 Zeilen** Desktop-Anlauf, `appl_init` … `menu_bar`, gegen `deskmain.c` diffbar |

## Der Kernbefund in einem Satz

Die Aufrufkette, die der Auftrag nennt — `appl_init → graf_handle →
wind_get(DESKWH, WF_WXYWH) → … → menu_bar(tree,1)` — ist ab jetzt messbar, und
sie stimmt an Anfang und Ende, hat aber **an der zweiten Stelle ein Loch**:
`graf_handle` wird nie gerufen (AES-Lücke G4, fällig in U7).
