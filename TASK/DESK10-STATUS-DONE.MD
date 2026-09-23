# DESK10 — STATUS: DONE (2026-09-23)

**Auftrag:** `desk/` ruft GEMDOS/BIOS als **Trap** statt als Rust-Funktion
(G6 der Lueckenliste), Verhalten unveraendert.

Basis: Branch `desk-rebuild`, HEAD `8e855d2`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/stash/reset` im
gemeinsamen Baum. Der Vergleichskernel („vorher") stammt aus einem
**losgeloesten Worktree** auf `8e855d2` im Job-Scratch (`git worktree add
--detach`), nach den Laeufen wieder entfernt — der gemeinsame Baum ist nie
ausgecheckt worden.

## Was EmuTOS an dieser Stelle tut (gepinnt: `references/emutos`)

EmuTOS' Desktop hat **zwei** echte Trapgrenzen, nicht eine:

* **AES** — `desk/gembind.c:53-97`: `GEMBLK` aus sechs Feldzeigern, `trap #2`
  mit `d0 = 200`. Das ist mechanisch genau das, was eine gewoehnliche
  GEM-Anwendung tut.
* **GEMDOS/BIOS** — `include/gemdos.h`'s `dos_*`-Inlines (`dos_sfirst` →
  `Fsfirst`, `:60-63`; `dos_open` → `Fopen`, `:70-73`; `dos_read`, `:80-83`;
  `dos_create`, `:110-113`; `dos_mkdir`, `:115-118`; `dos_delete`, `:125-128`;
  `dos_rename`, `:130-133`; `dos_rmdir`, `:135-138`; `dos_setdt` in
  `util/gemdos.c:49-56`) ueber `include/bdosbind.h:22-54`'s `trap1()`
  (`util/miscasm.S`) — ein **echter** `trap #1`, derselbe Einsprung, den jedes
  TOS-Programm benutzt. Der BIOS-Aufruf laeuft ueber
  `include/biosbind.h:29,79` (`Drvmap()` = `bios_l_v(0xa)`).

`bdosbind.h` gibt auch die Argumentreihenfolge vor, inklusive der zwei
Aufrufe mit totem Fuehrungswort: `Frename(0, old, new)` (`:53`) und
`Fdatime(timeptr, handle, wflag)` (`:54`).

Die Sonderstellung des `gsx_trans`-Direktaufrufs (`docs/api/desktop/
gembind.md`) betrifft die **AES**-Seite und ist hier nicht beruehrt worden.

## Was NewTOS hatte

`kernel/src/desk/mod.rs:123-135` stellt den GEM-Trap seit jeher selbst
(`svc #2`, Selektor 200, AESPB in `x1`) — das Muster war da. Die
GEMDOS-Seite aber nicht: `dir.rs`, `inf.rs` und `model.rs` bauten sich ein
`TrapArgs` von Hand und riefen damit die **Blattfunktionen** in
`tos_api::bdos::fs` auf; `app.rs`, `main.rs` und `mod.rs` griffen
`tos_api::bios::drive::drvbits()` direkt ab. Mechanisch ist das ein
gewoehnlicher Rust-Aufruf mit trap-foermigen Argumenten: der Desktop lief an
der ABI vorbei, die jede Anwendung benutzt, und ein Fehler im Dispatcher
blieb von ihm aus unsichtbar (Memory `desktop-bypasses-app-abi-path`).

## Was getan wurde

**Neu: [`kernel/src/desk/dosbind.rs`](../kernel/src/desk/dosbind.rs)** — das
GEMDOS/BIOS-Gegenstueck zum aesbind-Block in `desk/mod.rs`, als **eigenes
Modul**, weil EmuTOS es genauso trennt: die AES-Bindung ist desk-lokal
(`desk/gembind.c`), die GEMDOS-Bindung ist der gemeinsame Kopf
`include/gemdos.h`. Es enthaelt:

* `dos_trap(func, a0, a1, a2)` — `svc #1`, Funktionsnummer in `x8`,
  Argumente in `x0..x2`, Rueckgabe der vorzeichenerweiterte `Long` in `x0`
  (`docs/ARCHITECTURE.md`, „Native AArch64 callers"). `x3..x6` werden
  **genullt**: der Tracer des Dispatchers protokolliert den Rahmen
  (`tos-api/src/gemdos/trace.rs:743` gibt `x0..x6` aus), und das alte
  handgebaute `TrapArgs` setzte `regs[3..8] = 0` — der Rahmen soll nicht
  anfangen, Reste mitzuschleppen. `options(nostack)`, weil eine Ausnahme auf
  EL1 mit `SPSel = 0` auf `SP_EL1` laeuft; Speicher ist bewusst **nicht**
  als unberuehrt erklaert, denn `Fread`/`Fsfirst` schreiben durch die
  uebergebenen Zeiger.
* `bios_trap0(func)` — `svc #13` fuer die argumentlose Funktion
  (EmuTOS `bios_l_v`, `biosbind.h:79`).
* 14 Bindungen mit EmuTOS' Namen: `dos_sdta`, `dos_sfirst`, `dos_snext`,
  `dos_open`, `dos_create`, `dos_close`, `dos_read`, `dos_write`,
  `dos_delete`, `dos_rename`, `dos_mkdir`, `dos_rmdir`, `dos_setdt` und
  `drvmap` (EmuTOS `Drvmap`, klein geschrieben nach `CLAUDE.md`,
  „Naming Conventions" — dieselbe Regel, die aus `Fopen` ein `fopen` macht).
  Jede traegt das EmuTOS-Zitat ihres Aufrufers.

Der Desktop ruft **keine** XBIOS-Funktion, deshalb gibt es keine
`svc #14`-Bindung; eine ungenutzte waere toter Code.

**Umgestellt, ohne eine einzige Aufrufstelle zu aendern:**

* `dir.rs` — der lokale Block `gargs`/`cpath`/zehn `dos_*`-Huellen ist
  geloescht; die Huellen kommen jetzt aus `dosbind` und haben **dieselben
  Signaturen**, also blieben alle Aufrufstellen woertlich stehen.
  `scan_folder` geht ueber `dos_sdta`/`dos_sfirst`/`dos_snext`.
* `inf.rs` — `gargs`/`cpath` geloescht; `read_inf_file`/`write_inf_file`
  ueber `dos_open`/`dos_read`/`dos_close`/`dos_create`/`dos_write`.
* `model.rs` — `gemdos_args` und die drei Opcode-Konstanten geloescht;
  `scan_dir` ueber `dos_sdta`/`dos_sfirst`/`dos_snext` (`dos_sfirst` baut den
  NUL-terminierten Pfad, also faellt auch das lokale `cpath`-Feld weg).
* `app.rs`, `main.rs`, `mod.rs` — `drvbits()` → `dosbind::drvmap()`.

**Suche vorher/nachher** (`/usr/bin/grep -rn "tos_api::bdos\|tos_api::gemdos\
|tos_api::bios\|tos_api::xbios" kernel/src/desk/`): 11 Zeilen mit 8 echten
Aufrufen → **4 Zeilen, alles Kommentare**. Die vollstaendige Gegenueber-
stellung steht in `measurement-2026-09-23-desk10/RUNINFO.TXT`.

## Die zwei Stellen, an denen der Unterschied echt ist

Beide waren **vor** der Umstellung zu klaeren, und beide haben nichts
verschoben:

**(a) Die DTA.** `Fsetdta`/`Fsfirst`/`Fsnext` legen die aktuelle DTA ins
Basepage des **laufenden Prozesses** (`p_xdta`, `bdos/fsmain.c:396-405`).
Waehrend `desk::run` ist `tos_api::bdos::process::current()` **null** — der
Desktop laeuft im Kernel-Startkontext, dieselbe Tatsache, die `desk/mod.rs`
fuer `appl_init` bereits festhaelt. `tos-api/src/bdos/fs/find.rs:52-105`
leitet genau diesen Aufrufer auf seine `KERNEL_DTA`-Zelle. Der direkte
Aufruf betrat `find::fsetdta` an derselben Stelle, also benutzt der Trap
**dieselbe Zelle und denselben 44-Byte-Puffer** auf dem Stapel des
Desktops. Haette der Desktop ein Basepage, wuerde `set_cur_dta` die
Kernel-VA auf 32 Bit abschneiden (`find.rs:97-105` sagt das ausdruecklich) —
er hat keins.

**(b) Zeigerargumente.** Ein Trap aus EL0 traegt eine Benutzer-VA, der
Desktop eine hohe Kernel-VA. Die GEMDOS-Dateischicht liest ihre Zeiger
**roh** und uebersetzt nichts: `tos-api/src/bdos/fs/mod.rs:23-33` (`cstr`),
`bdos/fs/file.rs:297,373` (`ubuf as *const u8`). A83s
`usermem::resolve_kva` wird nur aus `aes/window.rs:393,474-486`,
`aes/object.rs:480` und `xbios/screen.rs:175-185` erreicht — nichts davon
liegt auf diesem Pfad. Eine Kernel-VA kommt unveraendert an; kein Puffer
musste umziehen, kein Pfad brauchte eine Sonderbehandlung.

Zwei weitere Eigenschaften wurden geprueft, weil ein Trap nicht nur
„dieselbe Funktion mit mehr Schritten" ist: eine Ausnahme maskiert
`PSTATE.DAIF`, aber der EMMC-Treiber **pollt**
(`kernel/src/drivers/bcm2711/emmc.rs:210`, `wr(IRPT_EN, 0); // poll, don't
signal`) und jede EL0-Anwendung macht ihre Datei-E/A schon so; und auf dem
16 KiB grossen Ausnahmestapel (`link-rpi4.ld:100-102`) liegt immer nur
**ein** GEMDOS-Rahmen, die Rekursion des Desktops (`dir_op`) bleibt auf dem
64-KiB-Kernelstapel.

## Eine benannte Abweichung — nicht hier eingefuehrt, nicht hier beseitigt

EmuTOS' Desktop liest die Laufwerksliste ueberhaupt nicht mit BIOS `Drvmap`,
sondern mit `dos_sdrv(dos_gdrv())` — `Dsetdrv(Dgetdrv())`, dessen Rueckgabe
die Bitmaske **ist** (`deskapp.c:714`). NewTOS liest `Drvmap` (BIOS 0x0A),
was dieselbe Bitmaske aus derselben Tabelle liefert
(`tos-api/src/bios/mod.rs:223-224` → `bios/drive.rs:145`). Der Auftrag lautete
„Verhalten darf sich nicht aendern", also wurde der Aufruf auf den Trap
umgestellt, **ohne** zu aendern, **welcher** Aufruf es ist. Die Angleichung
an `deskapp.c:714` ist eine eigene, einzeilige Aenderung und braucht ein
eigenes Ticket.

## Pruefung

**Hosttest: keiner moeglich, und das ist ein Befund, keine Auslassung.** Die
Aenderung liegt vollstaendig in `kernel/`, das
`forced-target = "aarch64-unknown-none-softfloat"` traegt;
`cargo run -p xtask -- test` faehrt nur `tos-api`, `tos-types` und `xtask`
(`tools/xtask/src/main.rs:582`, `HOST_TEST_PACKAGES`). Derselbe Befund steht
in DESK09s Quittung.

**QEMU raspi4b**, vier Laufpaare (vorher/nachher), alle ohne `[EXC]`/`[PANIC]`:

1. **Start** (sd-base): Bild **dateigleich** (md5
   `a53b0a3f…`), Startprotokoll `diff` **leer**, inklusive
   `[desk] desktop up (1 drives, 0 window(s) from the context)` — diese Zeile
   **ist** der `Drvmap`-Trap. Deckt `read_inf_file` (`Fopen`/`Fread`/
   `Fclose`) und `drvmap` ab.
2. **Vorgegebene `NEWDESK.INF`** (zwei `#W`-Zeilen): Bild **dateigleich**
   (md5 `c9ee3ca5…`), Protokoll gleich. Das Bild ist nicht leer — `C:\` zeigt
   „418936 bytes used in 26 items." mit Namen, Groessen und Datum, `C:\SUB\`
   „20 bytes used in 1 items." Der `Fsfirst`/`Fsnext`-Lauf ist also wirklich
   gelaufen.
3. **`Options → Save desktop`** ueber die echte Oberflaeche (Scratch-Kernel
   mit dem DESK08/09-IKBD-Injektor): alle drei Bilder dateigleich; das Abbild
   danach eingehaengt und die Datei ausgelesen — **bytegleich**, SHA-256
   `3671808d…` auf beiden Seiten.
4. **Dateioperationen**: `desk::dir::selftest()` (liegt seit Task #9 in
   `dir.rs:1010` und haengt in **keinem** Startpfad, Memory
   `unwired-selftests-rot-silently`) einmalig in der Scratch-Kopie
   aufgerufen. Er treibt `Dcreate` (New folder), `Fcreate`+`Fwrite`, eine
   Kopie (die das Ziel mit `Fdatime` stempelt), `Frename` (Rename), ein
   rekursives Loeschen (`Ddelete` + `Fdelete`) und den Zaehllauf, jedes mit
   frischem Rescan: **12 von 12 bestanden auf beiden Seiten, Ausgabe
   identisch** — und die beiden geschriebenen 64-MiB-Kartenabbilder sind
   **byteweise gleich**. `fsck_msdos -n` auf den Abbildern der Laeufe 3 und 4:
   Phase 1/2/3 sauber.

**Die Vergleichsbilder mussten neu aufgenommen werden, und das ist selbst ein
Befund.** Das Ticket verwies auf `measurement-2026-09-23-desk08-desk09/`.
Jene Bilder liegen **vor** `8e855d2` („no window at boot unless the context
saved one") und tragen noch das Startfenster bei (48,43,544,400); der
Vergleich ergibt 118314 Pixel in bbox (48,43,594,445) — genau dieses Fenster,
mit DESK10 nichts zu tun. Die Paare oben sind daher beide auf `8e855d2`
aufgenommen, die „vorher"-Seite aus dem losgeloesten Worktree.

**Bau:** `cargo clean -p kernel -p tos-api --target
aarch64-unknown-none-softfloat` vor **jedem** Board. `cargo xtask build
--board qemu-virt` und `--board rpi4`: **tos-api 64 / kernel 94 auf beiden**,
rc 0 — Basis 64/94, **Delta 0**. Clippy: kernel **125** auf beiden Boards
(Basis 125), tos-api **80** (Basis 80); **null** Treffer in
`kernel/src/desk/` auf beiden Boards. `cargo run -p xtask -- test`:
**929 bestanden / 0 fehlgeschlagen / 1 ausgelassen** (Basis 929/0/1).

## Artefakte

* `kernel/src/desk/dosbind.rs` (neu), `kernel/src/desk/{dir,inf,model,app,
  main,mod}.rs` — uncommitted im Baum;
  `measurement-2026-09-23-desk10/method/product-desk10.diff` +
  `dosbind.rs.new`.
* `docs/api/desktop/gembind.md` — neuer Abschnitt „NewTOS implementation
  notes — the desktop reaches the system only through traps (DESK10)": die
  Zwei-Schichten-Tafel, die vollstaendige Bindungstafel (14 Zeilen mit
  Trap, Nummer, TOS-Argumentreihenfolge, EmuTOS-Zitat und Aufrufstellen),
  die DTA- und Zeigerbefunde und die benannte Abweichung.
* `docs/api/desktop/_hw-test-2026-09-17.md` — neuer Punkt **36** (sieben
  Schritte).
* `docs/STATUS.md` — Phase-14-Zeile + eine Zeile in *Recent Decisions*
  (2026-09-23).
* `measurement-2026-09-23-desk10/` — `RUNINFO.TXT`, `method/`,
  `screendumps/`, `serial/`, `inf/`, `build/`, `manifest.sha256`.
  Kein `*.img`.

## Offen

**Hardwareabnahme:** Pi 4, HW-Checkliste Punkt **36**
(`docs/api/desktop/_hw-test-2026-09-17.md`). QEMU zeigt **ein** Laufwerk, die
Karte hat **vier** (`C:`–`F:`) — der Laufwerksbuchstaben-Schritt ist genau
der, den QEMU nicht abdeckt.

Nicht in einem Lauf enthalten: `File → Set file mask...` (kein neuer
GEMDOS-Aufruf, nur ein erneuter `scan_dir`, den Lauf 2 deckt — der
Dialogpfad mit Texteingabe wurde nicht gefahren) sowie die **Dialoge** von
`File → New folder...` und `File → Show info... → Rename`; deren Motor
(`dir::mkdir` / `dir::rename`, je eine Zeile ueber `dos_mkdir` /
`d_dofileren`) ist in Lauf 4 gefahren.
