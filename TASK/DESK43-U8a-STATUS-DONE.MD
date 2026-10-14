# DESK43 U8a — Entkopplung im Kernel: `RefCell`, D4, Selbsttests — Quittung

> Stand 2026-10-14 · Basis `main` @ `6aedd159` (U7 = `2c7895db`) · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2 (Zeile U8a); Entscheidung D4 in `TASK/DESK43.md`
> („D4 entschieden"); `TASK/DESK45.md`
> Messung: [`measurements/measurement-2026-10-14-desk43-u8a/`](../measurements/measurement-2026-10-14-desk43-u8a/RUNINFO.TXT)
> **Kein Bildschirmunterschied (0 Pixel).** Nichts committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`.
> `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: `kernel/src/desk/` nennt den Kernel nicht mehr — der
Zustand liegt in `RefCell` statt `crate::sync::SpinMutex`, der Desktop gibt
nichts mehr aus (D4; die Zeile `[desk] desktop up` kommt jetzt vom Kernel),
und die vier Selbsttests drucken ueber die Konsole statt ueber `kprintln`.
Driftprobe **11 → 4**.

---

## 1. Was sich geaendert hat

| Datei | Aenderung |
|---|---|
| `desk/global.rs` | `DeskStatic<T>`: **ein** `unsafe impl Sync` mit dokumentierter Einfaden-Invariante fuer alle Desktop-Statics; `DESK` = `DeskStatic<RefCell<Option<DeskState>>>`; `init`/`reset`/`with`/`screen_addr` → `borrow`/`borrow_mut`; `MENU_H` entfernt |
| `desk/inf.rs` | `SESSION` = `DeskStatic<RefCell<…>>`; 20 `lock()` → `borrow()`/`borrow_mut()`; Selbsttests `kprintln!` → `tprintln!` |
| `desk/menu.rs` | `unsafe impl Sync for MenuTree` → `static MENU: DeskStatic<MenuTree>` (unsafe-Zahl bleibt 24); `MENU_H` → `title_h()` = `gl_hchar() + 3` (RSC `769`, `desk_rsc.c:255-283`, `rsrc_obfix` `gemrslib.c:76-82`; = `gl_hbox`, `gemgraf.c:412`) |
| `desk/mod.rs` | `use crate::kprintln` und beide Produktzeilen weg (`appl_init failed`: EmuTOS testet `gl_apid` nicht, `deskmain.c:1953`; `desktop up`: D4); `#[cfg(feature = "selftest-desk")] selftest::run_once()` nach dem Prolog; Modulkopf (STOP-Notiz U4) durch D4-Notiz ersetzt |
| `desk/main.rs` | `cnx_get` wieder `void` wie EmuTOS (die Fensterzahl speiste nur die alte Zeile) |
| `desk/selftest.rs` (neu) | Konsolen-Schreiber (`Cconout` je Byte), `tprintln!`, `run_once` (einmal je Boot: ins → dir → inf → inf_install, Abschluss `[desk-selftest] done`) |
| `desk/dosbind.rs` | `dos_conout` (`Cconout`, GEMDOS 0x02) — nur fuer die Selbsttests; Zeichen im Register, daher aus EL1 nutzbar (anders als `Cconws`/`ldtrb`) |
| `desk/{dir,ins}.rs`, `desk/redraw.rs` | `tprintln!` in den Selbsttests; Kommentare „lock" → „borrow" |
| `shell_loop.rs` | `observe_desktop_gem_call`: Kernel-Zeile `[desk] desktop up (N drives)`; `DESKTOP_ACTIVE` um `crate::desk::run()` |
| `arch/aarch64/exceptions.rs` | Aufruf des Beobachters fuer jede GEM-Trap, vor dem Tracer |
| `kernel/Cargo.toml` | Feature `selftest-desk` |
| `docs/sdk/06-image-and-testing.md` | Tabellenzeile `[desk] desktop up` → Kernel-Quelle; Selbsttest-Zeilen |

Die ~173 `global::with`-Aufrufstellen sind **unveraendert** — `with` behaelt
seine Signatur, nur die Zelle darunter wechselt.

## 2. `RefCell` und `Sync` — Begruendung

`RefCell` ist `!Sync`, ein `static` braucht `Sync`. Ohne neues `unsafe` geht
das nicht (kein `thread_local` im Kernel, kein `spin` im kuenftigen
Desktop-Crate). Darum **ein** generischer Wrapper `DeskStatic<T>` mit genau
einem `unsafe impl<T> Sync`, und dafuer faellt das bisherige
`unsafe impl Sync for MenuTree` weg — **unsafe bleibt 24**.

Invariante (Doku an `DeskStatic`): jeder Zugriff kommt aus dem einen
Kontrollfluss des Desktops — heute vom Boot-CPU-Kontext in `sh_ldapp`s
DESKTOP-Arm; kein Interrupt, kein AES-Rueckruf, kein anderer Prozess nennt ein
Desktop-Item (Driftprobe-Gruppe C: der Kernel erreicht den Desktop nur ueber
`run`/`reset`, nie waehrend `run` laeuft); waehrend ein Trap des Desktops
geparkt ist (ACC03) laeuft kein Desktop-Code; als EL0-Programm (U9) ein
einfaediger Prozess. Ein verschachteltes `with` war unter dem Spinlock ein
Haenger und ist jetzt eine Panik.

Geprueft: statischer Scan auf verschachtelte Borrows
(`method/nested-borrow-scan.py`): 3 Treffer, alle Namenskollisionen
(Methode vs. freie Funktion gleichen Namens), kein echter. Dynamisch: Serie,
8 Ansichten, 54 Selbsttestpruefungen ohne Panik.

## 3. Die neue Zeile `desktop up` und wer sie liest

```
[desk] desktop up (N drives)
```

* **Wer druckt:** der Kernel, `kernel/src/shell_loop.rs`
  `observe_desktop_gem_call`, aufgerufen aus dem `svc`-Handler fuer jede
  GEM-Trap vor der Dispatch. Schluessel „Desktop-Aufrufer": GEM-Trap aus
  `Space::Kernel`, Selektor 200, waehrend `DESKTOP_ACTIVE` (nur im
  DESKTOP-Arm gesetzt). `appl_init` (Opcode 10, `aesdefs.h:17`) schaerft,
  das naechste `evnt_multi` (25, `aesdefs.h:32`) druckt einmal — gleiches
  Kriterium wie vorher (einmal je `deskmain`-Lauf, unmittelbar vor der
  Ereignisschleife), von aussen beobachtet. Liest nur `control[0]` ueber
  `Space::get_u64/get_u16` (kein neues `unsafe`).
* **N** = `drvbits().count_ones()` = was `Dsetdrv` dem Desktop lieferte.
  **M** („window(s) from the context") hat ausserhalb des Desktops keine
  Quelle und **entfaellt**.
* **Gleiche Position im Log:** Serien-Logs u8a vs U7 unterscheiden sich je
  Modus in genau zwei Zeilen (Kernelgroesse im `[mm]`-Block; Zeile 54 = die
  desktop-up-Zeile).
* **Leser:** `tools/` — **keiner**. Kein Skript parst N oder M als Wert.
  99 (unveraenderliche) Messskripte greifen auf das Praefix (`drive*.py`
  `--end`, `summarize.sh`) → funktionieren weiter. 109 (unveraenderliche)
  Messskripte patchen den Quelltext am alten `kprintln` (`desk_arm_patch.py`,
  `desk-arm-m3.py`, `a79_patch*.py`, `inj-desk-*.py` …) → scheitern bei
  Wiederanwendung am `assert`; die Kopien dieses Laufs schaerfen an der
  Kernel-Zeile. Volle Liste: `checks/u8a-desktop-up-consumers.txt`.
* **Achtung bei `selftest-desk`-Builds:** dort kommt die Zeile **waehrend**
  des dir-Tests (dessen `dir_op` setzt das erste `evnt_multi` ab). Harness
  fuer Selbsttest-Builds: auf `[desk-selftest] done` warten.

## 4. Selbsttests (DESK45) — Platzierung

Gewaehlt: **im Desktop belassen, Ausgabe ueber die Konsole, Lauf per
Feature** (`--features selftest-desk`). Begruendung: die zwei im Auftrag
genannten Formen gehen in U8a nicht ohne Nebenwirkung —
(a) ein eigenes EL0-Testprogramm braucht den Desktop als Crate (U8b);
(b) Kernel-Selbsttestcode ausserhalb `desk/`, der `crate::desk::…` ruft,
erhoeht Driftprobe-Gruppe C (11 → 5 statt 4) und bindet den Kernel an
Desktop-Interna. Die gewaehlte Form laesst `desk/` ohne `crate::`/`kprintln`,
die Tests bleiben immer uebersetzt (Signaturbrueche fallen auf), und
`selftest.rs` zieht in U8b unveraendert ins Crate — von dort ist es ein
kleiner Schritt zum eigenen Testprogramm (DESK45 B). Ausgabe wie ein
Testprogramm: GEMDOS-Konsole (`Cconout`, serielle Leitung + Bildschirm).

Lauf (ein Kommando: `method/selftest-run.sh`), Ergebnis:

| Test | Ergebnis |
|---|---|
| `ins::selftest` | 13 PASS, **1 FAIL** |
| `dir::selftest` | 12/0 |
| `inf::selftest` | 13/0 |
| `inf::selftest_install` | 15/0 |

**Der eine FAIL ist vorbestehend und kein Desktopfehler:** „ins_devices on a
complete desktop reports no change" — die Karte (`NEWDESK.INF`) hat
Laufwerksicons C, D, E, F, die Laufwerkstabelle nur C:; `ins_devices`
entfernt D/E/F und meldet zu Recht eine Aenderung. Der Test setzt eine Karte
voraus, deren Icons zur Laufwerkstabelle passen. Referenzlauf auf HEAD
(`method/head-selftest-ref.py`): identischer FAIL, identische PASS-Zeilen.

**Zweiter Altbefund:** `dir::selftest` bleibt am Bestaetigungsdialog
„DELETE FILE(S)" stehen (Karte: *confirm deletes* an; der Test ist aelter als
das Warten). Der Lauf beantwortet ihn mit Return per Injektor
(`method/inj-selftest-return.py`, nur im Scratch-Baum); auf HEAD steht er dort
ebenso. Testcode **nicht** geaendert (kein Abschwaechen).

Gegenprobe (DESK45-Abnahme): Produktkern 40 s auf Kartenkopie — Karte
sha256-gleich, 0 Selbsttestzeilen.

## 5. Pruefungen (Einzelheiten RUNINFO)

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1347/0/1 (+ neue) | **1347/0/1** (keine neuen: geaenderter Code ist Kernel-Crate, nicht hostbaubar) |
| Release-Warnungen rpi4 / qemu-virt | 64/94 | **64/94** beide, 0 Fehler (auch mit `selftest-desk`) |
| Clippy rpi4 / qemu-virt | 76/125 | **76/125** beide; Verteilung je Datei identisch zu U7; mit `selftest-desk` ebenfalls 76/125 |
| Driftprobe / unsafe | 4 / ≤ 24 | **4** (A 0 · B 0 · C 4 · D 0) / **24** |
| `cargo xtask emutos-consts` | gruen | total=448 matched=281 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=165 (= U7) |
| SDK-Hosttests | 20/0/0 | **20/0/0** |
| `programs/build.sh` | baut | baut; HELLOGEM.NAP byte-gleich zu U7/U5b (`3110e2e3…`) |
| QEMU raspi4b on/off/strict (12 Laeufe) | = U7 | Screendumps **15/15 byte-gleich** zu U7; APITEST **106/0/5** alle Modi, 0 Zeilen Diff; 0 `[EXC]`/`PANIC` |
| Injektor-Ansichten (8) | = U7 | **8/8 byte-gleich** zu U7 `views-u7-*` |
| Oracle-Sequenz | kein Diff erwartet | nur −2 Aufrufe (`Dgetdrv`/`Dsetdrv` der geloeschten Debugzeile) + Keytbl-Zeigerwert (.bss) + Build-Stempel; s. u. |
| qemu-virt Boot | Desktop | `[desk] desktop up (0 drives)`, End-Screendump byte-gleich zu U7 |
| Selbsttests | laufen, gruen | laufen in einem Kommando; 53 PASS, 1 FAIL (vorbestehend, §4) |

**Oracle-Diff** (`checks/u8a-oracle-diff.txt`): gegen U7s `SVCSEQ-u7.LOG`
(688 vs 690 Zeilen, 554 vs 556 Aufrufe). Erwartet war „kein Diff" — es gibt
genau **einen** Aufrufunterschied, und er ist der Zweck von D4: die zwei
GEMDOS-Aufrufe `Dgetdrv`/`Dsetdrv` unmittelbar vor dem ersten `evnt_multi`
gehoerten zur geloeschten Desktopzeile (`dos_sdrv(dos_gdrv())` fuer N). Sonst:
Keytbl-Rueckgabezeiger (`.bss`, kleinere `.text`), Boot-Info „Built:"-Stempel
(Git-Rev „unknown" im U7-`git archive`-Baum vs „6aedd159" im rsync-Baum). Die
Seq-Screendumps unterscheiden sich in 97 px der Info-Zeile des C:-Fensters
(„3423301" vs „3423225 bytes used", 76 B = Groessendifferenz der Spurdatei
`C:\SVCSEQ.LOG` selbst); ohne Tracer ist dasselbe Fenster byte-gleich (§5
Ansichten).

## 6. Guards (`docs/api/desktop/_rebuild-plan.md` §12)

```
Guards (docs/api/desktop/_rebuild-plan.md §12):
  DRIFT-PROBE TOTAL: 4 (before: 11)   DRIFT-PROBE UNSAFE: 24 (baseline 24)
  EMUTOS-CONSTS: total=448 matched=281 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=165
                 (EmuTOS snapshot unchanged)
  DESK-CITE: fns=548 cited=359 newtos=5 uncited=184            (whole desktop; before 541/357/0/184)
  DESK-CITE --since 6aedd159: 45 fns touched, 24 cited, 5 NEWTOS, 16 uncited:
    NEWTOS (added): global.rs DeskStatic::new, DeskStatic::deref (Rust plumbing, a C global needs no Sync wrapper);
                    selftest.rs ConOut::write_str, line, run_once (self-test output/runner; EmuTOS's desktop has none)
    uncited, touched only mechanically:
      lock() -> borrow()/borrow_mut():  global.rs with, screen_addr; inf.rs reset, apply_inf_bytes,
                                        installed_doctype, lookup_association
      kprintln! -> tprintln!:           dir.rs selftest; inf.rs selftest, selftest_install; ins.rs selftest
      MENU_H -> title_h():              menu.rs build_menu (title_h itself cites desk_rsc.c/gemrslib.c/gemgraf.c)
      comment edit / block inserted below, attributed to the fn above: global.rs patcol_all; inf.rs default;
                                        ins.rs mk; redraw.rs win_shwall_nocursor, snapshot_window_handles
  Oracle: run (unit changes what the desktop calls): diff = -Dgetdrv/-Dsetdrv of the removed debug line, Keytbl
          pointer value, build stamp; measurements/measurement-2026-10-14-desk43-u8a/checks/u8a-oracle-diff.txt
```

## 7. Offen fuer U6b / U8b (und spaeter)

1. **Gruppe C = 4** (`mod desk`, `crate::desk::reset` ×2, `crate::desk::run`)
   bleibt bis U9/U10; U8b stellt die Probe auf das Crate um.
2. **`DeskStatic`** zieht mit nach U8b; im EL0-Programm (U9) bleibt die
   Invariante wahr (ein Faden). Wer je einen zweiten Faden/Interrupt-Zugriff
   einfuehrt, muss den Wrapper ersetzen.
3. **Desktop-up-Schluessel fuer U9:** `observe_desktop_gem_call` erkennt den
   Desktop heute an „EL1 + DESKTOP-Arm aktiv". Als EL0-Prozess muss der
   Schluessel „der vom DESKTOP-Arm gestartete Prozess" werden (Slot/Basepage);
   die ABI-breite Lesung von `control` ist schon da, ein `Space::User`-Lesefehler
   wuerde als Fault vermerkt — dann `get_*` ohne Fault-Vermerk oder Lesen nach
   dem AES-Dispatch pruefen.
4. **`dos_conout`** liegt in `dosbind.rs`; mit U6b wandern die Bindungen nach
   `newtos-gem` — `Cconout` dort als normale Bindung anbieten.
5. **Selbsttests als eigenes Programm (DESK45 B)** erst nach U8b moeglich;
   `selftest.rs` ist dafuer schon ohne Kernelpfad.
6. **Testpflege (nicht U8a):** `ins::selftest` setzt eine zur
   Laufwerkstabelle passende Karte voraus; `dir::selftest` sollte den
   Bestaetigungsdialog selbst behandeln (oder vorher *confirm deletes*
   abschalten und wiederherstellen). Entscheidung DESK45.
7. **Nicht gefahren:** *Read .INF file*-Neustart (zweites `appl_init` →
   zweite Zeile), Ziehen/Ablegen, Kopier-/Umbenennen-/Install-Dialoge — nur
   der statische Borrow-Scan deckt sie ab.
8. Kommentar `desk/mod.rs` bei `appl_init` („pid (=1 in the kernel boot
   context)") ist seit S7 falsch (Review §2, Doc-Liste) — unberuehrt.

## 8. Selbstpruefung

Aufgabe in einem Satz: `desk/` vom Kernel loesen (`SpinMutex` → `RefCell`,
keine `kprintln` mehr, D4-Zeile in den Kernel, Selbsttests ohne Kernelpfad
lauffaehig), ohne Bildschirmaenderung. Geaendert: 9 Dateien `kernel/src/desk/`
+ neu `desk/selftest.rs`, `shell_loop.rs`, `exceptions.rs`, `kernel/Cargo.toml`,
`docs/sdk/06-image-and-testing.md`, diese Quittung, das Messverzeichnis.
Nicht angefasst: `tos-api`, `tos-types`, `programs/`, `tools/`,
`docs/STATUS.md`, `TASK/DESK43.md`, `UNSAFE_BASELINE`. Mitgenommen ueber den
Kern hinaus: `MENU_H` → `title_h()` (U7-Punkt 1, trivial),
`cnx_get` → `void` (Rueckgabe hatte nur die geloeschte Zeile).
Beide Boards bauen (64/94, 76/125).
