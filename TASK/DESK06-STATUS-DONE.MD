# DESK06 — STATUS: DONE (2026-09-23)

**Auftrag:** `File → Execute EmuCON` (`Item::ExecEmuCon`) einen Handler geben
und den Eintrag freischalten; den Mechanismus selbst aus der Quelle bestimmen
und berichten, welcher der beiden Wege gegangen wurde.

Basis: Branch `desk-rebuild`, HEAD `84587a5`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset` auf dem gemeinsamen
Baum.

Messung: [`measurement-2026-09-23-desk06/`](../measurement-2026-09-23-desk06/).
Spezifikation: `docs/api/desktop/deskpro.md` (§ NewTOS implementation notes)
und `docs/api/desktop/deskmain.md` (§ `do_filemenu`'s `CLIITEM`, DESK06).

---

## 1. Warum der Eintrag grau war, und warum er es nicht sein darf

`men_update` nennt `CLIITEM` in **keiner** `ILL_*`-Tabelle
(`deskmain.c:143-160`) und in **keinem** `menu_ienable`-Aufruf (`:464-609`).
`WITH_CLI` ist im Profil an. Der Punkt braucht weder Auswahl noch Fenster:
`do_filemenu`s Zweig ruft `pro_run` bedingungslos, und nur der optionale
`shel_wdef`-Schritt schaut ueberhaupt auf das oberste Fenster
(`deskmain.c:750-770`). In NewTOS war er grau, weil der Sammelzweig
`_ => false` ihn erfasste — mit einem Kommentar, der genau das sagte. Beides
ist weg; `item_enabled` hat jetzt `Item::ExecEmuCon => true` mit der
Fundstelle.

## 2. Der Weg: Modusumschaltung, kein Textfenster

Die Frage des Tickets beantwortet die Quelle: **EmuTOS kennt kein Textfenster
im Grafikschirm.** Die Kette ist

```
deskmain.c:753   pro_run(FALSE, DEF_CONSOLE, G.g_work, -1, -1)
deskpro.c:41       shel_write(SHW_EXEC, isgraf=0, isover=1, "EMUCON", tail)
gemshlib.c:172-173   sh_nextapp = CONSOLE_APP;  sh_isgem = 0
gemshlib.c:513-523   sh_chgrf: gl_shgem 1 -> 0  =>  sh_toalpha()
gemshlib.c:246-260   Pfeilzeiger, giveerr, ratexit, gsx_mfree, Workstation zu
gemshlib.c:614-622   sh_ldapp: coma_start;  set_default_desktop()
gemshlib.c:513-523   sh_chgrf: gl_shgem 0 -> 1  =>  sh_tographic()
```

Die Gedaechtnisregel „eine gestartete TOS-Anwendung besitzt den ganzen Schirm
ueber dem Desktop" gilt fuer die Konsole **staerker**, nicht schwaecher: sie
zeichnet nicht ueber den Desktop, der Desktop-*Prozess* endet
(`done = TRUE`, `deskmain.c:753`) und EmuCON hat die Maschine. Der
Screendump `cli-C-console.png` zeigt genau das — Vollbild-Textschirm, kein
Desktop, kein Mauszeiger.

Was die NewTOS-Konsole dafuer braucht, war vorher zu klaeren und hat
gehalten: Ausgabe geht ueber `Cconout` → `console_write_byte` → `fbcon`, also
in denselben Rahmenpuffer; Eingabe geht ueber `Cnecin` →
`console_read_key_blocking`, das **unabhaengig** von der AES-Ereignisschleife
pollt (USB-HID auf der Pi, PL011-RX auf qemu-virt) — die Konsole braucht die
AES also nicht. Beim Verlassen kehrt `run_console` auf `exit`/`quit` einfach
zurueck (`kernel/src/shell.rs:16-28`); es gibt nichts abzuraeumen.

## 3. Was gebaut wurde

| Datei | Inhalt |
|---|---|
| `kernel/src/desk/pro.rs` (**neu**) | EmuTOS `desk/deskpro.c`: `pro_run`, `pro_exec`; dazu `desk_busy_on`/`desk_busy_off` (`desksupp.c:1840-1852`), die bisher kein Zuhause hatten |
| `kernel/src/desk/mod.rs` | aesbind `shel_write` (121) + `shel_wdef` (127), Konstante `HGLASS` (`deskbind.h:33`), `pub mod pro`, und die Schleife in `run()` |
| `kernel/src/desk/main.rs` | `do_filemenu` liefert wieder ein `done`; neuer Zweig `Item::ExecEmuCon => done = do_execcli()`; `do_execcli` = `deskmain.c:750-770` |
| `kernel/src/desk/menu.rs` | `Item::ExecEmuCon => true` |
| `kernel/src/shell_loop.rs` | `sh_run_pending_console` — ein Durchgang von `sh_main`s Rumpf fuer `CONSOLE_APP` |

Beide AES-Aufrufe sind **Traps**, wie alles in `desk/`. Im AES-Protokoll des
Laufs:

```
[aes] pid=0 op  78 in=(2,0,0,0,0,0)   -> graf_mouse(HGLASS)   = desk_busy_on
[aes] pid=0 op 121 in=(1,0,1,0,0,0)   -> shel_write(SHW_EXEC, isgem=0, isover=1, "EMUCON", tail)
[aes] pid=0 op 127 in=(0,0,0,0,0,0)   -> shel_wdef("", "C:\")  (nur mit offenem Fenster)
…
[aes] pid=0 op  78 in=(0,0,0,0,0,0)   -> graf_mouse(ARROW)    = desk_busy_off
```

## 4. Die benannte Abweichung — und ihre Fundstelle

`do_execcli` liefert `done = TRUE`, die Ereignisschleife endet: bis dahin ist
es EmuTOS. **Danach** weicht NewTOS ab, und zwar an der Stelle, die schon
benannt war (Luecke G1): dort endet der Desktop-Prozess und `sh_main`
(`gemshlib.c:708-731`) startet das wartende Programm und anschliessend den
Desktop von vorn. NewTOS' Desktop ist ein Kernelaufruf, der sich nicht neu
starten laesst:

* `desk::global::init` ist idempotent — ein zweiter Aufruf tut nichts
  (`global.rs:705-715`), die `DeskState` bliebe die alte;
* es gibt kein `win_start()` beim Wiedereintritt, waehrend EmuTOS' `deskmain`
  es auf **jedem** Eintritt ruft (`deskmain.c:2024`) und dazu `G.g_cnxsave`
  frisch belegt und nullt (`:1966-1974`);
* `sh_main`s `wm_init()` (`:717`) haette die AES-Fenstertabelle geleert,
  bevor der Desktop wieder eintritt — er wuerde an Fenster glauben, die es
  nicht mehr gibt;
* und die Bruecke, ueber die EmuTOS die Sitzung hinuebertraegt —
  `cnx_put(); app_save(FALSE)` in den Shell-Puffer (`deskmain.c:2152-2153`),
  von `app_start()` per `shel_get` zurueckgelesen — hat NewTOS nicht.

Das ist derselbe Neustartpfad, den [DESK14](DESK14.md) §4 ausdruecklich als
**Lead-Entscheidung** zurueckhaelt. Er gehoert nicht in dieses Ticket.

Also faehrt `desk::run` jetzt eine Schleife und laesst den wartenden Lauf
**verschachtelt** durch den Schleifencode selbst laufen:

```rust
loop {
    main::event_loop();
    if !crate::shell_loop::sh_run_pending_console() { break; }
    menu::men_update();
    redraw::redraw_desktop();
    pro::desk_busy_off();
}
appl_exit();
```

`sh_run_pending_console` ist `sh_chdef` / `sh_chgrf` / `sh_ldapp`s
Konsolenzweig / `sh_chdef` / `sh_chgrf` — **ohne** `wm_init()` und
`sh_draw(…, TRUE)` (`:717-719`): beide bereiten Schirm und Fenstertabelle
fuer einen Desktop vor, der gleich **neu gebaut** wird, und wuerden genau den
Zustand zerstoeren, den der noch stehende Desktop gleich zeichnet (Luecke G2,
auf dem GEM-Startpfad schon so benannt, `main::desktop_resume`). Die drei
Zeilen nach dem Aufruf sind, was der neu gestartete `deskmain` fuer den
Schirm taete (`deskmain.c:2064-2071`).

Der Desktop gibt die AES dabei **nicht** ab (kein `appl_exit`/`menu_bar(NULL)`/
`WF_NEWDESK(NULL)`): dieser Dreisatz ist EmuTOS' Epilog eines *endenden*
Prozesses, und ein laufender EmuTOS-Desktop ist nie in dem Zustand. Der
vorhandene `.NCP`-Startpfad (`launch_native`, `Flavor::Console`) macht es
genauso — ein Konsolenprogramm ist kein AES-Klient.

## 5. Tatsaechlich ausgefuehrte Pruefung

Hosttest: `cargo run -p xtask -- test` → **932 passed / 0 failed / 1 ignored**
(Basis 932/0/1). **Kein neuer Hosttest**, und das ist kein Versaeumnis: der
ganze Pfad liegt in der `no_std`-aarch64-Kiste `kernel`, und `xtask test`
faehrt nur `tos-api`/`tos-types`/`xtask` (`tools/xtask/src/main.rs:582`).

Bau, `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat`
vor **jedem** Board:

```
cargo xtask build --board rpi4       tos-api 64 / kernel 94   (Basis 64/94)
cargo xtask build --board qemu-virt  tos-api 64 / kernel 94   (Basis 64/94)
cargo clippy --features board-rpi4       tos-api 80 / kernel 125 (Basis 80/125)
cargo clippy --features board-qemu-virt  tos-api 80 / kernel 125 (Basis 80/125)
grep -c "desk/" auf beiden Clippy-Laeufen:  0
```

Protokolle in `measurement-2026-09-23-desk06/build/`.

QEMU raspi4b, 640x480, `-display none`, QMP-Screendumps, Injektor in der
Kratzkopie. Vier Laeufe, **0** `[EXC]`/`[PANIC]` in allen vier Protokollen,
`fsck_msdos -n` auf allen drei Kartenabbildern sauber (Phase 1/2/3 ohne
Beanstandung):

1. **probe** — `Execute EmuCON  ^Z` ist auf dem nackten Desktop der
   **einzige** schwarze Eintrag des File-Klappmenues; der serielle Auszug der
   Objektzustaende zeigt `state=0x8` auf jedem anderen Eintrag und `0x0` auf
   Eintrag 13.
2. **cli** — Konsole im Vollbild; `ver` und `dir` hineingetippt und
   beantwortet (26 Eintraege in `C:\`); `exit` bringt den Desktop
   **pixelgleich** zurueck (0 Pixel Unterschied gegen `cli-A-idle.png`).
3. **wdef** — mit offenem `C:\`-Fenster: `op 121` dann `op 127`, `pwd`
   antwortet. Danach unterscheidet sich der Schirm in genau **zwei** Punkten.
4. **redraw** — Kontrolllauf **ohne** Konsole, ueber den vorhandenen
   `.NCP`-Startpfad: **dieselben zwei Punkte**.

### Die zwei Punkte, genau

(593,43) und (48,444) — der obere rechte und der untere linke Eckpunkt des
aeusseren Fensterrahmens (Fenster `48..593 x 43..444`). Vor der
Ganzschirm-Neuzeichnung desktopgruen `(0,255,0)`, danach weiss
`(255,255,255)`. Die beiden anderen Ecken, (48,43) schwarz und (593,444)
grau `(189,189,189)`, sind in beiden Bildern gleich — es sind die
3D-Fasenfarben.

Der Unterschied gehoert **nicht** diesem Ticket. Der Kontrolllauf faehrt den
vorhandenen `.NCP`-Startpfad (`main::launch_native`, `Flavor::Console` →
`redraw::redraw_desktop`), fasst `Execute EmuCON` nicht an und kippt dieselben
zwei Punkte. Es ist ein Unterschied zwischen `wind_open`s **inkrementeller**
Rahmenzeichnung und der **Ganzschirm**-Neuzeichnung: die inkrementelle laesst
zwei Eckpunkte ungeschrieben. Braucht ein eigenes Ticket (siehe unten).

(Ein erster Kontrollversuch ueber `Options → Read .INF file...` taugte nicht:
`load_and_apply()` lehnt die 21-Byte-`NEWDESK.INF` des Kartenabbilds ab, also
lief gar keine Neuzeichnung und „0 Pixel Unterschied" bewies nichts. Der
Vorgang steht im Kopf der Aktionsdatei, damit ihn niemand wiederholt.)

## 6. Was bewusst weggelassen wurde

* **Der Desktop-Neustartpfad** (`win_start()` beim Wiedereintritt,
  `app_save(FALSE)`/`shel_get`-Bruecke). DESK14 §4, Lead-Entscheidung.
* **`pro_run`s `(wh, curr)`-Paar** und das `do_wopen(FALSE, wh, curr,
  &G.g_desk)` dahinter (`deskpro.c:60-61`). Der einzige Aufrufer uebergibt
  `wh = -1` und ueberspringt den Schritt; `do_wopen` gibt es im Baum nicht.
  Zwei tote Parameter und ein unerreichbarer Zweig waeren schlechter als die
  benannte Luecke — sie kommen mit `do_wopen` zurueck, wenn der
  Doppelklickpfad durch `pro_run` gefuehrt wird.
* **Der Doppelklick-Startpfad** wurde **nicht** auf `pro_run` umgestellt.
  Das ist Luecke G1 und ein eigener Auftrag.

## 7. Offen

**Hardwareabnahme Pi 4**, HW-Checkliste Punkt **40**
(`docs/api/desktop/_hw-test-2026-09-17.md`): der `shel_wdef`-Schritt mit
einem echten Unterordner (in QEMU nur die Wurzel gemessen), zwei Laeufe
hintereinander (die Schleife darf sich nicht verbrauchen), `Ctrl-Z`, und die
1920x1080-Optik des Textschirms.

**Neues Ticket vorgeschlagen (1):** die zwei Rahmen-Eckpunkte, die `wind_open`
ungeschrieben laesst und die Ganzschirm-Neuzeichnung weiss macht.

**Neues Ticket vorgeschlagen (2):** im Kontrolllauf faellt auf, dass der
vorhandene `.NCP`-Startpfad den **Mauszeiger auf dem Textschirm stehen laesst**
(`redraw-C-program.png`, der schwarze Fleck bei (100,195)) — er ruft nur
`fbcon::clear_and_home`, nicht `sh_toalpha`. Der neue EmuCON-Weg hat das nicht
(`cli-C-console.png`, kein Zeiger), weil er ueber `sh_chgrf` geht. Der
`.NCP`-Pfad sollte denselben Weg nehmen; ausserhalb dieses Tickets.
