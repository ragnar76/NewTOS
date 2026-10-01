# measurement-2026-09-27-desk46

Belege zu **DESK46** — „Konsolenprogramme nach U3: Bildschirm blitzt weiss, und
aus EmuCON heraus friert es ein" (`TASK/DESK46.md`).

Baum: `desk-rebuild` @ `4d00e83` + die DESK46-Änderung (unversioniert im Baum,
`method/product-desk46.diff`). Alles headless in QEMU `raspi4b` (640×480,
Dev-Bau) bzw. `virt`. **Keine echte Pi 4, nichts geflasht, `/Volumes/BOOT`
nicht angefasst, nichts committet.** Kein `*.img` beigelegt (Auflage).

```
serial/       die Seriellprotokolle aller Läufe
screendumps/  die QMP-Screendumps dazu
analysis/     wo es stehenbleibt und warum (Befund 2)
method/       Messaufbau: Produktdiff, Scratch-Patches, Injektorskripte, INF
build/        Bauen, clippy, Hosttests, Driftprobe
```

## Die beiden Befunde, je in einem Satz

| | Urteil |
|---|---|
| **Befund 1** — `.NCP` vom Desktop blitzt weiss | **kein Defekt.** Der Textschirm wird betreten, das Programm zeichnet seine Ausgabe vollständig, und der Desktop kommt sofort darüber — die Folge der Entscheidung vom 2026-09-26 (`console_pause_for_key` entfällt). Gemessene Dauer des Blitzes: **5 Ticks à 5 ms = 25 ms**. |
| **Befund 2** — aus EmuCON heraus friert es ein | **Defekt, reproduziert und behoben.** Ein Daten-Abort auf Adresse 0 unmittelbar nach `Pexec`, danach `halt()`. Ursache: `finish_terminate` stellte die Register eines **EL1**-`Pexec`-Aufrufers nicht wieder her. **Kein U3-Rückschritt** (Beleg: `analysis/where-it-stops.txt` §5). |

## Befund 1 — was im Weiss steht

`screendumps/a3-3-console.png`: der Konsolenschirm im Augenblick, in dem
`C:\D46\SYSINFO.NCP` (Doppelklick vom Desktop) fertig ist und bevor `sh_chgrf`
zurück in den Grafikmodus schaltet. Es stehen **alle sieben Ausgabezeilen** des
Programms da, schwarz auf weiss. `screendumps/a3-4-desktop-back.png` zeigt den
Desktop danach, mit dem Fenster `C:\D46\` unverändert.

Damit der Screendump überhaupt möglich ist, hält ein **Scratch**-Eingriff den
Schirm 3 s fest (`method/desk46_patch.py`, `D46_HOLD`, Voreinstellung 600
Ticks) — im Produkt gibt es diese Pause nicht.

Die **Dauer** ist mit `D46_HOLD=0` gemessen (Lauf `A2`,
`serial/A2-serial.log`), die `sh_chgrf`-Marken tragen den 200-Hz-Tick:

```
[d46] sh_chgrf: want=0 gl_shgem=1 t=4333     <- Schirm wird alpha (geräumt)
[shell] sh_ldapp: starting C:\D46\SYSINFO.NCP, sh_nextapp=0, sh_isgem=0
SYSINFO: ... done.
[d46] sh_chgrf: want=1 gl_shgem=0 t=4338     <- Schirm wird wieder grafisch
```

**5 Ticks = 25 ms** in QEMU, Ausgabe inbegriffen. Auf die Rückfrage „stand da
Text?" lautet die belegte Antwort: ja, vollständig — nur 25 ms lang.

## Befund 2 — reproduziert, lokalisiert, behoben

**Reproduziert** (Lauf `B3`, Kernel ohne Fix, `shell.rs` unangetastet): Ctrl-Z
öffnet EmuCON, `pwd` antwortet, `run sysinfo.ncp` bringt die volle Ausgabe und
`[program exited]` — und dann bricht die Eingabeaufforderung nach `C:\` ab.
`screendumps/b5-frozen-console.png` ist genau dieses Bild, 14 s nach dem Start
des Kommandos aufgenommen.

**Die Frage des Tickets nach der Tastatur ist damit beantwortet: nein.** Der
Kernel steht in `unhandled` → `arch::aarch64::halt()`
(`kernel/src/arch/aarch64/exceptions.rs:1827-1874`); es läuft keine Schleife
mehr, die eine Taste lesen könnte. Sichtbar auch daran, dass der
Injektor-Herzschlag (`[d46] hb …s`, eine Zeile je Sekunde aus der
Tastaturwarteschleife) in `serial/B3-serial.log` mit dem `[EXC]` aufhört und
der danach eingeplante `ver`-Befehl nie ankommt.

**Lokalisiert:** `analysis/where-it-stops.txt` — Protokoll, `addr2line`,
Disassemblierung, und die zwei Zeilen, die es entscheiden:

```
[d46] finish_terminate: slot=2 from_el0=0 full=0 pf.spsr=0x80000004 rc=0
[d46]   child frame : x19=0x0 x29=0x0 lr=0x1010 elr=0x1018
[d46]   parent saved: x19=0xffff000000149fd5 x29=0xffff000000338b40 …
```

`from_el0=0` **und** `full=0` heisst: kein einziges GPR wurde
zurückgeschrieben. EmuCON läuft nach dem `Pexec` mit dem **Registersatz des
Kindes** weiter, und der erste Zugriff über ein callee-saved Register liest
aus Adresse 0.

**Behoben** (`method/product-desk46.diff`): ein neues, ausdrückliches
Kennzeichen `Process::parent_frame_pexec`, das `handle_pexec` für **jeden**
Start setzt, den es umleitet — gleich von welcher Ausnahmestufe der Aufrufer
kam. `finish_terminate` stellt daraufhin den vollen Registersatz her und legt
den Beendigungscode in `x0`. Die Unterscheidung hängt damit nicht mehr an
`SPSR.M`, das zwei der drei Fälle nicht trennen kann (der geparkte
Accessory-Rahmen und der Kernel-Start-Dreier sind beide `EL1t`).

**Nachgewiesen** (Lauf `B4`, `serial/B4-serial.log`, `0` Treffer auf
`EXC|PANIC|panic`):

```
[d46] finish_terminate: slot=2 pexec_caller=1 full=0 …
[program exited]
C:\D46> ver
NewTOS 0.1 - clean-room TOS for AArch64
C:\D46> exit
[shell] sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1
[desk] desktop up (1 drives, 1 window(s) from the context)
```

`screendumps/b4-7-after-ver.png` zeigt die lebende Konsole,
`screendumps/b4-9-end.png` den Desktop danach.

**Kein Rückschritt auf dem Desktop-Startweg** (Lauf `A2`): dort meldet
`finish_terminate` für `\AUTO\HELLO.NCP` **und** `C:\D46\SYSINFO.NCP`
`pexec_caller=0 full=0` — der Teilrestore von `enter_el0_with_resume_capture`
bleibt unangetastet, und der Desktop kommt mit seinem Fenster zurück.

## Die Läufe

| Lauf | Kernel | Skript | Zweck |
|---|---|---|---|
| `A` | Scratch, **ohne** Fix, `D46_HOLD=600` | `runA` | Befund 1: der Konsolenschirm beim Doppelklick |
| `A2` | Scratch, **mit** Fix, `D46_HOLD=0` | `runA` | Dauer des Blitzes (25 ms) + Regressionsprobe des Desktop-Startwegs |
| `A3` | Scratch, **mit** Fix, `D46_HOLD=600` | `runA` | Befund-1-Bild auf dem gefixten Kernel |
| `B` | Scratch, ohne Fix, Marker **mit** `{}` | `runB` | Befund 2 zum ersten Mal reproduziert |
| `B2` | Scratch, ohne Fix, Marker ohne `{}` + Registerabzug | `runB` | Registersatz + `finish_terminate`-Arm |
| `B3` | Scratch, ohne Fix, `shell.rs` **unangetastet** | `runB` | Befund 2 im **Produktpfad** |
| `B4` | Scratch, **mit** Fix, `shell.rs` unangetastet | `runB` | der Nachweis: Konsole lebt, `exit` bringt den Desktop |
| `B5` | Kernel von `B3` | `runB` | Screendump des eingefrorenen Bildes |
| `qemu-virt-boot` | Produktbaum, Board `qemu-virt` | — | zweites Board bootet bis `[desk] desktop up`, 0 `[EXC]` |

## Messaufbau

**Abbild.** `tools/make-sd-image.sh <img> 64` (einzige zulässige Quelle,
[[auto-folder-console-only]]), dann `method/sd-prep.sh`: `C:\NEWDESK.INF` ←
`method/newdesk-d46.inf` (öffnet ein Fenster auf `C:\D46\`) und
`C:\D46\SYSINFO.NCP` als Kopie von `C:\SYSINFO.NCP` — ein einziges Symbol im
Fenster, damit die Klickkoordinate (64,124) eindeutig ist.

**Scratch.** `method/make_scratch_d46.sh <ws> <scratch> <script.rs>` — `rsync`
der Kisten in den Job-Scratch (nie der gemeinsame Baum) +
`method/apply_injector.py` (der GUI04-IKBD-Injektor mit `Step::Key`) +
`method/desk_arm_patch.py` (Uhr startet, wenn unser Desktop steht) +
`method/desk46_patch.py` [`noshell`] + `method/desk46_probe2.py`.

Was `desk46_patch.py` hinzufügt, alles nur im Scratch:
* `h02_inject::step()` **auch** in `console_read_byte_blocking` und
  `console_read_key_blocking` — sonst erreicht keine Taste EmuCON, das
  ausserhalb der AES-Ereignisschleife blockiert;
* ein Herzschlag im Injektor, eine Zeile je Sekunde — der Unterschied zwischen
  „steht" und „wartet";
* Marken an jedem Zustandswechsel der Shell-Schleife, mit 200-Hz-Tick;
* das Festhalten des Konsolenschirms (`D46_HOLD`, für Befund 1);
* mit `noshell`: `kernel/src/shell.rs` bleibt unberührt, damit ein Lauf das
  Produkt zeigt und nicht unsere Marker.

`desk46_probe2.py` fügt zwei Beobachtungen ohne Verhaltensänderung hinzu: den
vollen Registerabzug in `unhandled`, und in `finish_terminate` den gewählten
Arm plus drei Zeugenregister (Kind-Rahmen gegen Eltern-Schnappschuss).

**Tastendruck statt Mausklick für *Execute EmuCON*.** Ctrl-Z ist die
Tastenabkürzung von `CLIITEM` (`#K`-Feld 12 = `5A` = 'Z',
`kernel/src/desk/menu.rs:113`, `deskmain.c:169-260`); der Weg über
`hndl_kbd` → `lookup_ascii_shortcut` → `hndl_menu` ist derselbe wie beim
Klick und braucht keine ausgemessene Menügeometrie.

**Fahren.** `method/run.sh` (QEMU `raspi4b`, `-display none`, QMP-Screendump am
Seriellmarker) + `method/qmp_multi.py`.

## Zahlen

| | |
|---|---|
| Bauen `--board rpi4`, frisch | tos-api **64** / kernel **94** (Basis 64/94, Delta 0) |
| Bauen `--board qemu-virt`, frisch | tos-api **64** / kernel **94** (Basis 64/94, Delta 0) |
| clippy, beide Feature-Sätze | tos-api **80** / kernel **125** (Basis 80/125, Delta 0) |
| clippy-Treffer in `exceptions.rs` / `process.rs` | **0** |
| `cargo run -p xtask -- test` | **956 / 0 / 1** (2 von 3 Läufen); ein Lauf zeigte den T11-Flake |
| `sh tools/desk-drift-probe.sh` | **32** (Basis 32) |
| `EXC`/`PANIC`/`panic` in `A2`, `A3`, `B4`, `qemu-virt-boot` | **0** |

`build/xtask-test-T11-flake.log` ist der eine Lauf, in dem
`sched_pexec_parent_suspend::blocking_child_never_yields_to_its_own_parent`
fehlschlug — das ist `TASK/T11` und nicht diese Änderung (die beiden folgenden
Läufe sind sauber, und der Test läuft auf einer Fixture, die der geänderte
Code nicht berührt).
