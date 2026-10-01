# DESK11 — STATUS: DONE (2026-09-24)

**Auftrag:** die Laufwerksmaske ueber GEMDOS statt ueber BIOS `Drvmap` lesen —
EmuTOS' `dos_sdrv(dos_gdrv())` (`references/emutos/desk/deskapp.c:714`).

Basis: Branch `desk-rebuild`, HEAD `8d271cb`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/stash/reset` im gemeinsamen
Baum. Der Vergleichskernel („vorher") stammt aus einem **losgeloesten
Worktree** auf `8d271cb` im Job-Scratch (`git worktree add --detach`), nach den
Laeufen wieder entfernt.

## Was EmuTOS an dieser Stelle tut

`deskapp.c:714` — `drivemask = dos_sdrv(dos_gdrv());`. `dos_gdrv()` ist
`Dgetdrv()` (`include/gemdos.h:45-48` → `bdosbind.h:30`, GEMDOS 0x19),
`dos_sdrv(d)` ist `Dsetdrv(d)` (`gemdos.h:105-108` → `bdosbind.h:26`, GEMDOS
0x0E). Dieselbe Redewendung steht an drei weiteren Stellen: `deskins.c:297`,
`desksupp.c:1575` und `desksupp.c:1819` (`valid_drive`, der Gegenpart unseres
`check_alt_letter_key`).

Der Kunstgriff ist, dass `Dsetdrv` **die Bitmaske zurueckgibt**, unabhaengig
davon, ob das Laufwerk tatsaechlich gewechselt hat
(`docs/api/gemdos/drive/0e-dsetdrv.md`, `bdos/fsmain.c:420-425`). Mit dem
eigenen aktuellen Laufwerk als Argument ist der Aufruf als *Setter* ein
bewusster Leerlauf — `Dgetdrv` liefert immer einen Wert im Bereich, also
besteht die Bereichspruefung und `xsetdrv` speichert das Laufwerk, das es
ohnehin war. Die Rueckgabe ist der ganze Zweck.

## Was NewTOS hatte

BIOS `Drvmap` (0x0A) ueber `dosbind::drvmap()` → `bios_trap0(0x0A)` → `svc #13`.
Dieselbe Bitmaske aus derselben Tabelle (`tos-api/src/bios/mod.rs:223-224` →
`bios/drive.rs:145`), aber eine Ebene tiefer: der Desktop griff **an der
GEMDOS-Laufwerksschicht vorbei**, die den Zustand „aktuelles Laufwerk"
besitzt. DESK10 hatte den Aufruf auf den Trap umgestellt, ohne zu aendern,
*welcher* Aufruf es ist („Verhalten unveraendert"), und die Abweichung
ausdruecklich als eigenes Ticket benannt.

## Was getan wurde

**`kernel/src/desk/dosbind.rs`:**

* neu `dos_gdrv() -> i16` — `Dgetdrv()`, GEMDOS 0x19, keine Argumente;
* neu `dos_sdrv(newdrv: i16) -> Long` — `Dsetdrv(newdrv)`, GEMDOS 0x0E, das
  Laufwerk in `x0`, die Bitmaske in `x0` zurueck;
* geloescht `drvmap()`, der Helfer `bios_trap0()` und die Konstante `DRVMAP`
  — eine ungenutzte Bindung waere toter Code;
* Modulkopf nachgezogen: der Desktop macht jetzt **keinen BIOS- und keinen
  XBIOS-Aufruf** mehr, es gibt also weder eine `svc #13`- noch eine
  `svc #14`-Bindung. (EmuTOS' Desktop bindet `biosbind.h` zwar ein, nimmt
  daraus aber nur `Drvmap()` — das er selbst nie benutzt — und `Dosound()`
  fuer das About-Osterei, das NewTOS nicht hat.)

**Drei Aufrufstellen**, alle auf `dos_sdrv(dos_gdrv())`:

| Datei | Stelle | EmuTOS |
|---|---|---|
| `app.rs` | `app_blddesk`s `drivemask` | `deskapp.c:714` |
| `main.rs` | `check_alt_letter_key` | `desksupp.c:1819` (`valid_drive`) |
| `mod.rs` | die `[desk] desktop up`-Startzeile | (NewTOS-eigen) |

**Suche nachher** (`/usr/bin/grep -rn "drvmap\|bios_trap0\|DRVMAP"
kernel/src/desk/`): kein Treffer.

## Pruefung

**Hosttest: keiner moeglich, und das ist ein Befund, keine Auslassung.** Die
Aenderung liegt vollstaendig in `kernel/`, das
`forced-target = "aarch64-unknown-none-softfloat"` traegt; `cargo run -p xtask
-- test` faehrt nur `tos-api`, `tos-types` und `xtask`
(`tools/xtask/src/main.rs`, `HOST_TEST_PACKAGES`). Derselbe Befund steht in
DESK09s und DESK10s Quittung.

**Bau:** `cargo clean -p kernel -p tos-api --target
aarch64-unknown-none-softfloat` vor **jedem** Board. `cargo xtask build
--board rpi4` und `--board qemu-virt`: **tos-api 64 / kernel 94 auf beiden**,
rc 0 — Basis 64/94, **Delta 0**. Clippy: tos-api **80**, kernel **125** auf
beiden Boards (Basis 80/125), **null** Treffer in `kernel/src/desk/`.
`cargo run -p xtask -- test`: **932 bestanden / 0 fehlgeschlagen /
1 ausgelassen** (Basis 929/0/1 + 3 neue aus DESK02).

**QEMU raspi4b**, ein Laufpaar (vorher/nachher) auf demselben, frisch
erzeugten Kartenabbild:

* Startprotokoll `diff`: **eine** Zeile, die Kernelgroesse
  (`2727936` → `2736128` Bytes). Alles andere Wort fuer Wort gleich.
* Laufwerkszeile identisch:
  `[desk] desktop up (1 drives, 0 window(s) from the context)` — **diese
  Zeile ist der Aufruf**.
* Startbild **bytegleich**: md5 `a53b0a3f076d7040d0a0f92f9152e611` auf beiden
  Seiten, pixdiff **0 differing pixels**.
* Die beiden Kartenabbilder nach den Laeufen: `cmp` gleich.
* Null `[EXC]`/`[PANIC]`.

## Artefakte

* `kernel/src/desk/dosbind.rs`, `kernel/src/desk/{app,main,mod}.rs` —
  uncommitted im Baum.
* `docs/api/desktop/gembind.md` — die Bindungstafel und der Abschnitt „The
  named deviation, closed by DESK11 (2026-09-23)" ersetzen den alten „One
  named deviation, unchanged by DESK10".
* `docs/api/desktop/_hw-test-2026-09-17.md` — neuer Punkt **37**, Schritt 1.
* `docs/STATUS.md` — Phase-14-Zeile + eine Zeile in *Recent Decisions*
  (2026-09-24).
* `measurements/measurement-2026-09-24-desk01-desk02/` — `RUNINFO.TXT`, `method/`,
  `screendumps/`, `serial/`, `inf/`, `build/`, `manifest.sha256`.
  Kein `*.img`.

## Offen

**Hardwareabnahme:** Pi 4, HW-Checkliste Punkt **37**, Schritt 1
(`docs/api/desktop/_hw-test-2026-09-17.md`). QEMU zeigt **ein** Laufwerk, die
Karte hat **vier** (`C:`–`F:`) — genau der Schritt, den QEMU nicht abdeckt.

Der zweite Aufrufer, `check_alt_letter_key`, ist auf der aktuellen
Tastaturschicht **wirkungslos** (die IKBD/Keymap faltet Alt in AltGr und
liefert nie ASCII `0` fuer eine Buchstabentaste). Das ist ein seit Task #8
vermerkter Befund der Tastaturschicht, kein DESK11-Befund; die Abfrage selbst
ist nachgezogen.

> **Berichtigung 2026-09-23 (DESK13).** Der Absatz darueber ist **falsch**,
> und er ist es, weil er einen alten Vermerk im Quelltext abgeschrieben hat,
> ohne ihn nachzufahren. `kernel/src/keymap.rs` `to_ascii` nullt ein
> alphabetisches Ergebnis, **sobald** das Alt-Bit gesetzt ist; `ikbd::to_ascii`
> macht daraus `None` und `ikbd::decode_key` legt das Wort als
> `scancode << 8 | 0` in die Schlange — genau die Form, die
> `check_alt_letter_key` braucht. Beide Tastaturquellen setzen das Bit (IKBD
> `SC_ALT = 0x38`; der USB-HID-Bericht bildet LAlt **und** RAlt darauf ab), und
> der AES reicht das Wort unveraendert als `EV_KRET` durch. `Alt`+`C` oeffnet
> `C:\` — in QEMU raspi4b gefahren,
> `measurements/measurement-2026-09-23-desk13/screendumps/b-altc.png`. Der Vermerk im
> Quelltext und der in `docs/api/desktop/_hw-test-2026-09-17.md` sind
> ebenfalls richtiggestellt.
