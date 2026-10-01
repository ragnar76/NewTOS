# DESK43 / U4 — Quittung: Der Kernel braucht `desk::` nicht mehr

Stand: **fertig, mit zwei benannten Stehenbleibern** · 2026-09-26
Baum: `desk-rebuild` @ `c2076c0` + unversionierte Änderung
Plan: `docs/analysis/desktop-userspace-plan-v2.md` §5 U4
Belege: `measurements/measurement-2026-09-26-desk43-u4/`

---

## 0. In einem Satz

Der Endungstest für `\AUTO\` und `C:\ACC\` liegt jetzt im Suchmuster des
Kernels statt im Klassifizierer des Desktops, `check_alt_letter_key` liest
`Keytbl(-1,-1,-1)->shift[]` über `svc #14` statt `crate::keymap`, und drei der
vier About-Box-Strings sind Desktop-Konstanten geworden — Driftprobe **38 → 32**,
Gruppe C ist exakt die Liste, die U4 als Abnahme nennt.

---

## 1. Was gemacht wurde

### 1.1 Der Endungstest wandert in das Suchmuster (Gruppe C, −2)

EmuTOS klassifiziert an dieser Stelle nichts nach; die Endung **ist** das
Suchmuster:

```c
/* bios/bios.c:713-714 */   err = Fsfirst("\\AUTO\\*.PRG", 7);
/* aes/geminit.c:272   */   strcpy(D.g_work, "\\*.ACC");
```

| Datei · Funktion | vorher | nachher |
|---|---|---|
| `kernel/src/launch.rs` · `run_auto_folder` | `list_dir_ex("C:\AUTO\*.*")` + `crate::desk::ftype::classify` | `list_dir_ex("C:\AUTO\*.NCP")`, kein `use crate::desk::…` mehr |
| `kernel/src/accs.rs` · `SEARCH` / `count_accs` | `("C:\ACC\","C:\ACC\*.*")`, `("C:\","C:\*.*")` + `classify` | `("C:\ACC\","C:\ACC\*.NAC")`, `("C:\","C:\*.NAC")`, kein `use crate::desk::…` mehr |

Der Unterverzeichnisfilter (`attrib & 0x10`) bleibt eine eigene Zeile und ist
jetzt als das kommentiert, was er ist: der Stellvertreter für EmuTOS'
Suchattribut `7`, das Verzeichnisse ausschließt (`bdos/fsdir.c:1840-1844`),
während `list_dir_ex` sie mitmeldet. Vorher hatte der Klassifizierer das
nebenbei miterledigt — die alte Doc-Begründung in `accs.rs` („die
Endungsprüfung würde ein Verzeichnis `FOO.NAC` ohnehin abweisen") ist damit
falsch geworden und ist korrigiert, nicht umformuliert.

**Gemessen** (`analysis/auto-and-acc-pattern.txt`, `serial/boot-rpi4-u4.log`):
`\AUTO\HELLO.NCP` läuft, `\AUTO\READ.ME` fällt weiterhin durch (`1 AUTO
program(s) run`), `C:\ACC\SYSACC.NAC` **und** `C:\SYSACC2.NAC` werden gefunden
(`2 file(s) found, 2 process(es) started`).

### 1.2 `check_alt_letter_key` liest die Tabelle über den Trap (Gruppe B, −1)

```c
/* desk/deskmain.c:1136-1159 */
keytab = (KEYTAB *)Keytbl(-1, -1, -1);
drive  = keytab->shift[HIBYTE(thechar)];
```

* **Neu: `kernel/src/desk/xbiosbind.rs`** — EmuTOS' `include/xbiosbind.h` als
  vierte Bindung neben aesbind (`svc #2`/200), `dosbind` (`svc #1`) und
  `vdibind` (`svc #2`/115). Inhalt: `xbios_trap` (`svc #14`) und
  `keytbl_query()` = `Keytbl(-1,-1,-1)`. Mehr nimmt der Desktop aus
  `xbiosbind.h` nicht: `Supexec` geht bei uns über `dos_getcookie` (DESK13),
  `deskrez.c`s Auflösungswechsel gibt es nicht.
* `kernel/src/desk/main.rs` — lokale `struct Keytab { normal, shift, caps }`,
  genau wie EmuTOS' Desktop sie sich selbst deklariert (`deskmain.c:64-69`),
  und `*tab.shift.add(sc)` statt `crate::keymap::to_ascii(sc, true, false,
  false)`.

**Wertgleich, gemessen** (`serial/keytbl-probe-fixed-PASS.log`):
`shift[0x1E]=0x41 shift[0x2E]=0x43` neben `keymap=0x41/0x43`. Alt-C öffnet
weiterhin C:. Dass das dieselben Bytes sein *müssen*, steht auch in der Quelle:
`Keytbl` bekommt seine Tabellen aus `keymap::snapshot()`
(`kernel/src/host_env.rs:344`), also aus genau dem Array, das `to_ascii`
indiziert.

### 1.3 Drei der vier `crate::initinfo`-Strings werden Desktop-Konstanten (Gruppe B, −3)

`VERSION_STRING`, `INDEPENDENCE_NOTICE`, `AUTHORSHIP_LINE` liegen jetzt in
`kernel/src/desk/inf.rs` neben den `CREDIT_*`-Zeilen der About-Box.

Die Begründung lag, wie der Auftrag sagte, **zweimal** — beide Seiten sind
geändert:

* `kernel/src/initinfo.rs` (der alte Satz: *„EmuTOS keeps the same single
  source"*) → ein Blockkommentar, der sagt, warum das Argument bei EmuTOS gilt
  und bei uns aufhört zu gelten: `desk_xlate_fix` zeigt `DEVERSN->ob_spec` auf
  das BIOS-Symbol `version[]` (`deskmain.c:1880`), weil EmuTOS' Desktop im
  selben ROM liegt wie sein BIOS. `programs/desktop` wird `kernel` nicht im
  Abhängigkeitsgraphen haben (Plan §6.4, Maßnahme 1).
* `kernel/src/desk/inf.rs` — derselbe Sachverhalt am Ort der neuen Konstanten,
  plus der Grund, warum es keine dritte Quelle gibt: es existiert **kein**
  Syscall, der den OS-*Namen* meldet (`NTOS` ist eine Zahl,
  `tos-api/src/cookie.rs:142`; `Sversion` ist die GEMDOS-Version).

---

## 2. Die zwei Stehenbleiber — beide gemessen, keiner „weil es sonst nicht läuft"

Der Plan verlangt 26; erreicht sind 32. Die Differenz sind genau diese zwei,
und in beiden Fällen ist die im Plan genannte Ersatzlösung **nachweislich
nicht verfügbar**. Nach der Ordnungsregel (§6.3: *„Kann eine Einheit die
Umgehung nicht kostenlos entfernen, steht sie an der falschen Stelle: erst den
Ersatz bauen, dann entfernen"*) sind sie hier benannt statt umgangen.

### 2.1 `crate::kprintln` (5 Zeilen) — `Cconws` ist nicht der Ersatz

Zwei voneinander unabhängige Gründe, beide an der Quelle:

1. **`Cconws` faultet auf einem Kernel-Zeiger.** Es liest seinen String mit dem
   unprivilegierten `ldtrb` (`tos-api/src/gemdos/console.rs:527`), also mit
   EL0-Rechten. Jedes Stringliteral in `desk/` liegt heute an einer Kernel-VA.
   Dieselbe Eigenschaft schließt schon `Ssystem`s Zeigerform für den Desktop
   aus — steht so in `docs/api/desktop/gembind.md`.
2. **Es wäre auch aus EL0 das falsche Gerät.** Die Konsole schreibt in
   Framebuffer **und** serielle Leitung (`kernel/src/host_env.rs:58-61` →
   `fbcon::write_byte` → `vdi::alpha_conout`), eine Diagnosezeile malte also
   Zeichen auf den laufenden Desktop. EmuTOS' `kprintf` geht auf das
   **Debug**-Gerät — `bconout1(1, c)` (`bios/kprint.c:91-99`), nicht
   `bconout2(2, …)`, das ist `cprintf`. Und `Bconout(1)` ist bei uns ein
   abwesendes Gerät, das das Byte verwirft (`tos-api/src/bios/chardev.rs:176-186`).

Zusatzbefund, der die Sache einordnet: EmuTOS' Desktop ruft `KDEBUG` zwar, aber
**jede** `desk/*.c` lässt es auskompiliert (`/* #define ENABLE_KDEBUG */`,
`deskact.c:19`, `deskapp.c:20`). Im Referenzbau druckt der Desktop also gar
nichts. Unser `kprintln` ist die immer-an-Fassung davon.

Die Begründung steht jetzt im Kopf von `kernel/src/desk/mod.rs`, damit die
nächste Einheit sie nicht neu herleiten muss.

### 2.2 `crate::initinfo::machine_name()` (1 Zeile) — der `_MCH`-Weg trägt nicht

Die Vorhersage des Auftrags war: *„`machine_name()` kommt aus dem
`_MCH`-Cookie."* Das geht nicht: `_MCH` ist bei uns ein **fester**
NewTOS-Marker, `0x0005_0000`, auf beiden Boards derselbe
(`tos-api/src/cookie.rs:143`, `docs/api/bios/_cookie-jar.md:136` — ausdrücklich
*„a NewTOS-specific, non-Atari marker"* statt der Atari-Maschinen-ID). Er kann
„Raspberry Pi 4 (BCM2711)" nicht von „QEMU aarch64-virt" unterscheiden.

Die *kostenlose* Entfernung gäbe es: **EmuTOS' `ADDINFO` hat überhaupt keine
Machine-Zeile** (`desk_rsc.c:772-873` — Titel, Version, Copyright, URL, GPL,
OK). Die Zeile zu streichen würde die Abhängigkeit beseitigen **und** die Box
EmuTOS-näher machen. Aber sie ist eine bewusste, dokumentierte Eigenzutat (sie
ist auch der Grund, warum die Box sich an ihren Text anpasst statt an
`ADDINFO`s feste 40 Zeichen), und sichtbare Eigenzutaten zu streichen ist eine
Anwenderentscheidung, keine Entwicklerentscheidung.

**Vorlage an den Anwender — eine von zwei:**
* **(a) Zeile streichen.** EmuTOS-konform, Driftprobe fällt auf 31, Aufwand ~10
  Zeilen. Die Maschine steht weiterhin auf dem Bootschirm.
* **(b) Zeile behalten.** Dann braucht sie eine Quelle, die der Umzug
  überlebt — also einen neuen NewTOS-Cookie (Boardname/-ID) oder eine
  `Ssystem`-Unterfunktion. Das ist neue API und gehört vor U8 entschieden,
  nicht in U8 hineingeschmuggelt.

Die Stelle ist im Code mit `[STOP — DESK43 U4]` markiert, an beiden Enden
(`desk/inf.rs` am Aufruf, `initinfo.rs` an der Funktion).

### 2.3 Nicht mein Posten: `crate::sync::SpinMutex` (2 Zeilen)

Der Plan weist sie U8 zu (§5, U8: *„`SpinMutex` → `RefCell`/`Cell`
(`global.rs:18`, `inf.rs:34`)"*). Unberührt.

---

## 3. Der Befund, der die Einheit fast still kaputtgemacht hätte

`tos_types::Long` ist `i32` (`tos-types/src/lib.rs:31`). Die erste Fassung von
`xbios_trap` hat ihn von `dosbind::dos_trap` übernommen. `Keytbl` liefert aber
eine **Adresse**, und im Kernel-Kontext ist das eine high-half-VA:

```
Lauf 1:  [EXC] Data Abort (same EL) ESR=0x96000004 ELR=0xffff0000000a71f8
         [EXC] FAR=0x00000000002fcaa8          <- die gekürzte Adresse
Lauf 2:  [u4probe] Keytbl=0xffff0000002fcaa0   <- dieselbe Adresse, ganz
```

Fix: `xbios_trap`/`keytbl_query` geben `u64` zurück. Die Begründung steht im
Modulkopf von `xbiosbind.rs`, damit der nächste Zusatz sie nicht wiederholt.

**Folgerung, die diese Einheit nicht anfasst:** `dosbind::dos_trap` hat
dieselbe Verengung. Heute ruft der Desktop keine zeigerliefernde
GEMDOS-Funktion (kein `Malloc`, kein `Pexec`), also ist es latent — aber es ist
dieselbe Falle, und die erste solche Ergänzung fällt hinein.
`analysis/keytbl-truncation.txt`.

Die Messsonde lag vorübergehend in `desk::run()` und ist **von Hand** wieder
entfernt (kein `git`-Eingriff); `grep -rn 'u4probe' kernel/src tos-api/src`
ist leer.

---

## 4. Die vier unverdrahteten Selbsttests — **Ticket, nicht Startstelle**

`TASK/DESK45.md` ist angelegt. Warum gefilt statt verdrahtet:

1. **Drei von vier schreiben auf die SD-Karte** (`dir.rs:2557`, `inf.rs:4381`,
   `inf.rs:4491` — die Doc-Kommentare sagen es selbst: *„mutates the SD
   image"*). Ein Aufruf im normalen Bootpfad fasst bei jedem Start
   Benutzerdaten an. Das Repo hat dafür eine Konvention und hält sie ein:
   *„Development self-tests are no longer invoked during the normal boot"*
   (`kernel/src/lib.rs:373-382`), und für den Ausnahmefall ein eigenes
   Cargo-Feature (`#[cfg(feature = "selftest-emmc")]`, `lib.rs:323`).
2. **Ihre Ausgabe hängt an §2.1.** Sie drucken über `crate::kprintln`. Wer sie
   verdrahtet, entscheidet zugleich, wohin ihre Zeilen gehen — und diese
   Entscheidung ist heute nicht zu treffen.

Das Ticket nennt beide Formen (Feature-Schalter jetzt, EL0-Testprogramm mit U8)
und sagt, dass Form B das Ticket an U8 hängt. Die vier sind **übersetzt** (kein
`#[cfg(test)]`), Signaturbrüche fängt also der Compiler; was fehlt, ist ein
Lauf.

---

## 5. Die Zahlen

| | |
|---|---|
| **Driftprobe** | **32** (A 20 · B 8 · C 4) — vorher **38** (A 20 · B 12 · C 6) |
| Bauen `--board rpi4`, frisch | tos-api **64** / kernel **94** (Basis 64/94, **Delta 0**), rc 0 |
| Bauen `--board qemu-virt`, frisch | tos-api **64** / kernel **94** (Basis 64/94, **Delta 0**), rc 0 |
| clippy, beide Feature-Sätze | tos-api **80** / kernel **125** (Basis 80/125, **Delta 0**) |
| clippy-Treffer in `desk/`, `launch.rs`, `accs.rs`, `initinfo.rs` | **0** |
| `cargo run -p xtask -- test` | **956 / 0 / 1** (Basis 956/0/1), dreimal wiederholt |
| `EXC`/`PANIC`/`panic` in den Produktläufen | **0** |

### Die Driftzahl, verrechnet

| Gruppe | 38 | 32 | Differenz |
|---|---:|---:|---|
| A `tos_api::` in `desk/` | 20 | 20 | unverändert — U5 (`aes::object`) und U7 (`vdi::font`) |
| B `crate::` in `desk/` | 12 | 8 | −1 `crate::keymap`, −3 `crate::initinfo` (von 4) |
| C Kernel → `desk` | 6 | 4 | −1 `launch.rs`, −1 `accs.rs` |

### Das Abnahmekriterium, wörtlich geprüft

`/usr/bin/grep -rn 'crate::desk' kernel/src --exclude-dir=desk`
(`analysis/proof-grep-crate-desk.txt`) liefert sieben Zeilen, davon
**zwei Code** — `shell_loop.rs:291-292`, der DESKTOP-Arm. Die übrigen fünf
(`shell_loop.rs:422`, `exceptions.rs:344-345`, `dbg_qedtrace.rs:25-26`) sind
Fließtext in Doc-Kommentaren; die Driftprobe streicht Kommentare und zählt sie
deshalb nicht. Dazu `lib.rs:62` (`pub mod desk;`) und `lib.rs:475`
(`desk::reset()`), die der grep nicht sieht, weil sie `desk::` ohne `crate::`
schreiben. **Das ist genau die Liste, die U4 als Überlebende nennt.**

---

## 6. Was geprüft ist und was der Tester noch fahren muss

**Selbst gefahren (Rauchprobe, headless):**

| Lauf | Ergebnis |
|---|---|
| rpi4, make-sd-image-Standardkarte | `\AUTO\HELLO.NCP` läuft, `READ.ME` nicht; beide `.NAC` geladen; `[desk] desktop up`; Screendump `screendumps/boot-u4-rpi4.png` zeigt Menüleiste, `DISK C`, Trash, Printer — keine Fremdzeichen |
| qemu-virt, ohne Karte | bis `[desk] desktop up`, 0 `EXC`/`PANIC` |
| Keytbl aus EL1 | Kernel-VA, `shift[0x1E]='A'`, wertgleich zum alten Pfad |

**Dem Tester übergeben — das ist der maßgebliche Teil:**

1. **Bootscreendump-A/B, beide Boards.** Meine Aufnahme ist nur *eine* Seite;
   den Vergleich gegen `c2076c0` habe ich **nicht** gefahren (dafür müsste der
   HEAD-Kernel gebaut werden, und das ist der Auftrag des Testers). Erwartung:
   **0 abweichende Pixel** — diese Einheit ändert keine Zeichenoperation.
   Methode: `method/run-boot.sh` + `measurements/measurement-2026-09-26-gui04/method/pixdiff.py`.
2. **`\AUTO\`-Lauf und `.NAC`-Erkennung im Seriellprotokoll**, beide Boards,
   gegen `c2076c0` verglichen. Die Zeilen müssen **identisch** sein. Das ist
   der einzige Verhaltenspfad, den U4 wirklich anfasst.
3. **Alt+Buchstabe → Laufwerksfenster.** Headless nur mit dem
   Scratch-IKBD-Injektor zu fahren (`measurements/measurement-2026-09-17-h03/method`), weil
   `raspi4b` in diesem Bau keinen USB-Stack hat (`[pcie] RC not present; USB
   disabled`). Erwartung: Alt-C öffnet das C:-Fenster wie vorher. Dass die
   *Werte* gleich sind, ist oben gemessen; was noch fehlt, ist der Klickweg
   davor.
4. **`.NTP` über den Konsolenarm** (DESK44, eigene Quittung) — Doppelklick auf
   ein `.NTP`; erwartet `sh_isgem=0` im Seriellprotokoll und ein Textbildschirm
   statt des Desktops.

**Nur auf echter Pi 4:** Punkt 3 und 4 mit echter Tastatur/Maus. Sonst nichts —
U4 fasst keinen Treiber und keine Hardware an. Die zwei offenen HW-Punkte aus
U3 (Doppelklick auf QED, *Execute EmuCON*) bleiben unverändert offen.

---

## 7. Berührte Dateien

| Datei | was |
|---|---|
| `kernel/src/launch.rs` · `run_auto_folder` | Suchmuster `*.NCP`, `classify`-Import weg, Doc auf `bios.c:713-714` |
| `kernel/src/accs.rs` · `SEARCH`, `count_accs` | Suchmuster `*.NAC` (beide Fundorte), `classify`-Import weg, Doc auf `geminit.c:272` |
| **neu** `kernel/src/desk/xbiosbind.rs` | `xbios_trap` (`svc #14`) + `keytbl_query()` |
| `kernel/src/desk/mod.rs` | `pub mod xbiosbind;`, `[STOP]`-Block zu `kprintln` |
| `kernel/src/desk/main.rs` · `Keytab`, `check_alt_letter_key` | Tabellenlesung über den Trap |
| `kernel/src/desk/main.rs` · `isgraf` | **DESK44**, eigene Quittung |
| `kernel/src/desk/ftype.rs` | **DESK44**, eigene Quittung |
| `kernel/src/desk/inf.rs` · `do_aboutdesk` + Konstantenblock | drei Strings lokal, `[STOP]` an `machine_name` |
| `kernel/src/initinfo.rs` | die zweite Hälfte derselben Begründung + `[STOP]`-Hinweis an `machine_name` |
| `kernel/src/desk/dosbind.rs` | Modulkopf behauptete *„no BIOS and no XBIOS function"* — durch `xbiosbind` falsch geworden, korrigiert |
| `docs/api/desktop/gembind.md` | dieselbe Behauptung, zweite Kopie; neuer Abschnitt „Die `svc #14`-Bindung" mit der Dreiertabelle |
| `docs/sdk/01-overview.md` | **DESK44**, eigene Quittung |
| **neu** `TASK/DESK45.md` | die vier unverdrahteten Selbsttests |

Nicht angefasst: `docs/STATUS.md`, `docs/MEMORY.md`, `TASK/DESK43.md`,
`TASK/DESK44.md`, der Plan.

---

## 8. Nebenbefund, nicht behoben

Ein Lauf von `cargo run -p xtask -- test` ist in
`tos-api/tests/sched_pexec_parent_suspend.rs:146` gescheitert
(*„the round-robin sees the parent again, left: 1, right: 2"*,
`build/xtask-test-flake.log`). Derselbe Test danach **5/5** einzeln grün, die
ganze Suite **3/3** grün. `tos-api` ist von dieser Einheit nicht angefasst
(`method/product-u4.diff.stat`) — ein Flake in einem Scheduler-Test mit
globalem Zustand, gemeldet, nicht angefasst.
