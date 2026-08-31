# Desktop als User-Space-Anwendung — Design und Stufenplan

> Stand: 2026-09-01 · Branch `desk-rebuild` · EmuTOS-Pin `1a7484ab`
> Auftrag: Lead. **Nur Analyse + Dokument** — `kernel/`, `tos-api/`, `tos-types/`
> sind in diesem Job unberührt geblieben.
> Nachfolger von ADR-022 („Endgame: der 1:1-Desktop wird eine ersetzbare
> EL0-AES-App"). Entwurf für **ADR-023** in §11.

Die Entscheidung ist getroffen und steht hier nicht zur Debatte: **der Desktop
wird zum dritten Mal neu gebaut und läuft danach im User-Space wie jede andere
Anwendung.** Dieses Dokument beantwortet ausschließlich das *Wie*, gestuft und
überprüfbar.

**Dass es der dritte Versuch ist, ist die wichtigste Randbedingung.** §7 ist
deshalb kein Anhang, sondern der Kern: dort steht, was den dritten Anlauf
mechanisch — nicht disziplinarisch — daran hindert, wie die ersten beiden
abzudriften.

---

## Legende: gemessen vs. angenommen

| Marke | Bedeutung |
|---|---|
| **[V]** | Verifiziert an Quelle/Baum mit `file:line` in diesem Job. |
| **[A]** | Annahme / Schätzung. Als solche gekennzeichnet, nicht als Befund. |
| **[STOP]** | Entscheidung des Leads nötig, bevor die betroffene Stufe startet. |

---

## 1. Nachgemessene Ausgangslage — inklusive Korrekturen am Briefing

Ich habe die Zahlen des Briefings nachgemessen statt sie zu übernehmen.

### 1.1 Bestätigt

- **[V]** `kernel/src/desk/` = **9234 Zeilen** über 15 Dateien. Größte:
  `main.rs` 1851, `inf.rs` 1547, `dir.rs` 1189, `win.rs` 887, `menu.rs` 726,
  `mod.rs` 628, `global.rs` 551, `model.rs` 452, `insapp.rs` 265, `icons.rs` 261,
  `act.rs` 260, `redraw.rs` 231, `app.rs` 188, `dlg.rs` 127, `ftype.rs` 71.
- **[V]** Start aus dem Kernel-Boot-Pfad: `desk::run()` in
  `kernel/src/lib.rs:329`.
- **[V]** Der Desktop ruft die AES **schon heute über den echten GEM-Trap**
  (`svc #2`, Selektor 200, `kernel/src/desk/mod.rs:104-113`) mit einem
  6-Zeiger-AESPB. Das ist die gute Nachricht: 22 AES-Opcodes gehen bereits
  durch die Trap-Grenze und ändern sich beim Umzug **nicht**.

### 1.2 Korrekturen

1. **[V] „`desk/mod.rs:159` nennt den Kontext ‚the desktop's kernel-boot
   context'"** → die Stelle ist **`kernel/src/desk/mod.rs:169`**.

2. **[V] „sechs direkte `crate::`-Abhängigkeiten"** — davon sind **zwei nur
   Doc-Kommentare**, kein Code:
   - `crate::launch::run_program` — nur in Doc-Kommentaren
     (`main.rs:443`); die echten Aufrufe sind `run_program_with_tail`
     (`main.rs:484`, `main.rs:522`).
   - `crate::shell::run_console` — **nur** Doc-Kommentar (`main.rs:452`).
     Es gibt **keinen** Aufruf. Der Menüpunkt „Execute EmuCON"
     (`menu.rs:209`) landet in `do_deskmenu(_item)` (`main.rs:768`), das
     **jedes** Desk-Menü-Item ignoriert und die About-Box zeigt.
   Dafür fehlen im Briefing zwei echte:
   - `crate::sync::SpinMutex` — 2 Statics (`global.rs:514`, `inf.rs:253`).
   - `crate::kprintln` — 4 `use`, ~17 Aufrufe (davon 13 in totem Selftest-Code).

3. **[V] „etwa 40 direkte `tos_api::`-Aufrufe"** — ich zähle **56 Call-Sites**
   über 21 Symbole (Einzelaufstellung §5). Die Differenz ist Zählweise, nicht
   Widerspruch: das Briefing zählt vermutlich distinkte Import-/Pfadnennungen.

4. **[V] Die Prämisse zu `ap_private` ist falsch — und das ist wichtig.**
   Das Briefing sagt: „EmuTOS's desktop has its own resource; `ap_private`
   depends on it." Tatsächlich:
   ```c
   /* aes/gemsuper.c:95-96 */
   tree = rs_trees[DESKTOP];
   pglobal->ap_private = tree[ROOT].ob_spec;
   ```
   `rs_trees[]` ist die **AES-eigene** eingebaute Resource `gem.rsc`
   (`aes/gem_rsc.c:574-578`), `DESKTOP` ist Index 2 (`aes/gem_rsc.h:48`), und
   ihr ROOT hat `ob_spec = 4419L` (`aes/gem_rsc.c:551-556`) — exakt der auf dem
   TT gemessene Wert. **Nicht** die Desktop-Resource `desk/desktop.rsc`.
   Der Baum, den der Desktop per `wind_set(WF_NEWDESK)` registriert, landet in
   `gl_newdesk` (`aes/gemwmlib.c:1685-1689`) und wird **ausschließlich** von
   `w_drawdesk` gelesen (`:346-350`, `:1911-1914`) — er berührt `ap_private`
   nie.

   **Folge:** **D-R1-6 ist nicht durch diese Arbeit blockiert.** Es ist ein
   AES-Server-Defekt, der sofort und unabhängig behoben werden kann (Stufe S1b).
   Unser `desktop_bg_obspec()` (`tos-api/src/aes/window.rs:577-600`) liest den
   falschen Baum; die Doc-Begründung dort ist damit gegenstandslos.

   **D-R1-5 (`ap_id 0`) ist dagegen echt blockiert** — es setzt voraus, dass der
   Desktop ein Prozess ist, der *terminiert*, bevor die Anwendung startet (§2).

---

## 2. Das EmuTOS-Prozessmodell, aus der Quelle belegt

Das ist die Grundlage für alles Weitere. Alle Zeilenangaben verifiziert.

### 2.1 Es gibt genau **einen** AESPD für Desktop *und* Anwendung

```c
/* aes/geminit.c:875-876 */   curpid = 0;  rlr->p_pid = curpid++;   /* → pid 0 */
/* aes/geminit.c:888 */       gl_mowner = ctl_pd = iprocess("SCRENMGR", ctlmgr); /* pid 1 */
/* aes/struct.h:36 */         #define NUM_PDS (NUM_ACCS + 2)  /* acc's + ctrlpd + dos appl. */
```

AESPD 0 ist die „dos appl". `sh_ldapp` **benennt sie um**, statt eine neue
anzulegen — für den Desktop *und* für die Anwendung:

```c
/* aes/gemshlib.c:600-612  (DESKTOP_APP) */
sh_show("");  p_nameit(rlr, fname);  p_setappdir(rlr, D.s_cmd);
if (aes_run_rom_program(deskstart)) { wm_new(); }

/* aes/gemshlib.c:632-645  (NORMAL_APP) */
sh_show(fname);  p_nameit(rlr, fname);  p_setappdir(rlr, D.s_cmd);
rlr->p_flags = 0;
ret = dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn);
```

`appl_init` liefert `rlr->p_pid` (`aes/gemsuper.c:94`) → **immer 0**. Das ist
D-R1-5, und es ist keine Konstante, die man irgendwohin schreibt: es ist ein
Prozessmodell.

**Kein Konflikt, weil sequenziell:** Desktop und Anwendung teilen AESPD 0
*nacheinander*, nie gleichzeitig. Der Desktop terminiert, bevor die Anwendung
startet (§2.2). Damit löst sich auch **AUD-P1-004(b)** auf
(`docs/analysis/aes-audit/p1-appl-pd-dispatch.md:85-88`, „Desktop und erste
EL0-App mappen beide auf pid 1 → geteilter Deskriptor"): das ist ein Artefakt
unseres **verschachtelten** Modells, in dem der Desktop während der App
weiterlebt. Der dort vorgeschlagene 1:1-Fix („getrennte Räume für Desktop und
EL0-App") ist für den *gestarteten* Fall **nicht** EmuTOS-konform — EmuTOS teilt
den Deskriptor bewusst. **Diese Audit-Empfehlung ist zu korrigieren.**

### 2.2 Der Desktop ist ein gewöhnliches TOS-Programm, das *stirbt*

`desk/deskstart.S` ist ein normaler TOS-Programmkopf: `Mshrink`, eigener Stack,
`jsr _deskmain`, `Pterm`. Gestartet wird er als echter Basepage-Prozess:

```c
/* aes/gemshlib.c:570-580 */
LONG aes_run_rom_program(PRG_ENTRY *entry) {
    PD *pd = (PD *) Pexec(PE_BASEPAGEFLAGS, (char*)PF_STANDARD, "", NULL);
    pd->p_tbase = (UBYTE *) entry;
    return dos_exec(PE_GOTHENFREE, NULL, (const char *)pd, NULL);
}
```

Also: **im ROM-Image, aber über den regulären Prozess-Erzeugungspfad.**

Der Epilog von `deskmain()` ist genau die Sequenz, die unser
`desktop_relinquish()` von Hand nachbaut:

```c
/* desk/deskmain.c:2154-2166 */
cnx_put();  app_save(FALSE);              /* Zustand in den Shell-Puffer */
menu_bar(NULL, 0);                        /* :2160 */
wind_set(DESKWH, WF_NEWDESK, NULL, 0, 0); /* :2163 */
appl_exit();                              /* :2166 */
```
→ `deskstart.S` → `Pterm`.

### 2.3 Der Shell-Loop gehört der **AES**, nicht dem Desktop

```c
/* aes/gemshlib.c:692-731 */
void sh_main(BOOL isauto, BOOL isgem) {
    ...
    do {
        sh_chdef(psh);  sh_chgrf(psh);
        if (gl_shgem) { wm_init(); ratinit(); sh_draw(D.s_cmd, TRUE); } /* :717-719 */
        if (rc && !IS_BIOS_ERROR(rc)) fm_error(-rc-31);
        rc = sh_ldapp(psh);                                             /* :725 */
    } while ((psh->sh_doexec != SHW_SHUTDOWN) && !gl_changerez);
}
```

`sh_main`/`sh_ldapp` liegen in `aes/gemshlib.c` — **AES-Code**. Ein
kernel-residenter Shell-Loop bei uns ist damit **1:1, keine Abkürzung**.

### 2.4 Der Desktop-Zustand überlebt im **AES-Shell-Puffer**

Das ist der Mechanismus, der den „Desktop stirbt bei jedem Start" überhaupt
tragbar macht — und er beantwortet nebenbei, warum `save_windows` eine Erfindung
ist:

- `app_save(FALSE)` serialisiert den **kompletten** Desktop-Zustand
  (View/Sort/Prefs/`#W`-Fensterrechtecke/ANODEs) im EMUDESK.INF-Textformat und
  legt ihn per `shel_put(outbuf, len+1)` in den AES-Puffer
  (`desk/deskapp.c:1106-1297`, `shel_put` bei `:1293`).
- `app_start()` liest ihn beim nächsten Desktop-Start per
  `shel_get(buf, SIZE_SHELBUF)` zurück (`desk/deskapp.c:832`); nur wenn die
  Signatur nicht `#` ist, wird von Platte gelesen (`:833-836`).
- Größe: `SIZE_SHELBUF = 4192` (`include/sysconf.h:74`), die ersten
  `CPDATA_LEN = 128` Bytes sind reserviert (`desk/deskapp.c:136,1123-1124`).
- Fensterpositionen kommen über `cnx_put()` (`desk/deskmain.c:1371-1427`) in
  `G.g_cnxsave` und von dort in die `#W`-Zeilen.

Unser `SHELLBUF: usize = 4192` (`tos-api/src/aes/shell.rs:29`) stimmt bereits.

### 2.5 Nach einer Anwendung räumt die AES auf: `wm_new()`

```c
/* aes/gemwmlib.c:1875-1893 */
void wm_new(void) {
    while (ml_ocnt > 0) wm_update(END_MCTRL);
    while (wind_spb.sy_tas > 0) wm_update(END_UPDATE);
    for (wh = 1; wh < NUM_WIN; wh++) {
        if (D.w_win[wh].w_flags & VF_ISOPEN) wm_close(wh);
        if (D.w_win[wh].w_flags & VF_INUSE)  wm_delete(wh);
    }
}
```
Aufgerufen in `sh_ldapp` bei `:609` und `:677`. **Alle** Fenster weg; der
Desktop baut seine beim Neustart aus `cnx_get()` wieder auf
(`desk/deskmain.c:2055`).

Unser `save_windows`/`restore_windows` (`tos-api/src/aes/window.rs:631-666`)
ist die Snapshot-Variante davon — sie existiert **nur**, weil unser Desktop
während der App weiterlebt. Im Zielmodell ist sie die Abweichung, nicht das
Design. `wind_new` (Opcode 109) gibt es bereits
(`tos-api/src/aes/window.rs:2163-2176`) und macht exakt `wm_new`.

### 2.6 `ap_private`-Kette, vollständig

```
C:\EMUDESK.INF  #Q-Zeile
  → process_inf1()             aes/geminit.c:383-388   (bg[3], bgfound)
  → set_aes_background(bg[n])  aes/geminit.c:724-727
  → rs_trees[DESKTOP][ROOT].ob_spec = (0x1143 & 0xffffff00) | patcol
                               aes/geminit.c:635-641
  → appl_init → ap_private     aes/gemsuper.c:95-96
```
Der Desktop liest `#Q` **separat noch einmal** für sein eigenes `G.g_patcol`.
Zwei unabhängige Leser derselben Zeile — das ist das 1:1-Design.

### 2.7 `#Z` (Autorun) wird von der **AES** ausgeführt, nicht vom Desktop

```c
/* aes/geminit.c:438-460 (process_inf2) */
else if (tmp == 'Z') {                 /* "#Z 01 C:\THING.APP@" */
    scan_2(pcurr, &isgem);
    ... sh_wdef(tmpptr2, tmpptr1); *isauto = TRUE;
}
/* aes/geminit.c:734,739 */
isgem = process_inf2(&isauto);
sh_main(isauto, isgem);
```
Bei uns: `#Z` wird in `kernel/src/desk/inf.rs:366-369` geparst, in `:488-491`
zurückgeschrieben — und **nie ausgeführt** [V]. Es steht auch an der falschen
Stelle (Desktop statt AES).

### 2.8 `shel_write` ist die einzige Startsequenz des Desktops

```c
/* desk/deskpro.c:36-63 */
static WORD pro_exec(...) { desk_busy_on(); ret = shel_write(SHW_EXEC, isgraf, isover, pcmd, ptail); ... }
WORD pro_run(WORD isgraf, char *cmd, char *tail, WORD wh, WORD curr) { ... }
```
`pro_run` liefert `done` → `deskmain` verlässt seine Schleife → Epilog → `Pterm`
→ `sh_ldapp` startet das gemerkte Programm. **Der Desktop ruft nie `Pexec`.**

Bei uns hat `pending_launch()` (`tos-api/src/aes/shell.rs:581`) **null
Konsumenten** [V] — das ist P6-005, und es ist genau das fehlende Glied.

### 2.9 „Execute EmuCON" ist derselbe Mechanismus

```c
/* desk/deskmain.c:751-753 (do_filemenu, CLIITEM) */
G.g_work[1] = '\0';
done = pro_run(FALSE, DEF_CONSOLE, G.g_work, -1, -1);
```
`DEF_CONSOLE = "EMUCON"` (`include/aesdefs.h:627`), `DEF_DESKTOP = "EMUDESK"`
(`aes/geminit.h:13`) — **magische Namen**, die `sh_write` klassifiziert
(`gemshlib.c:172`) und `sh_ldapp` dispatcht (`:600`, `:615-622`).

---

## 3. Die empfohlene Form

### 3.1 Kurzfassung

> **Der Desktop wird ein eigenständiges, natives LP64-`no_std`-Rust-Programm
> (`.NAP`), das als ELF-Blob im Kernel-Image liegt und über die
> Programm-Registry als regulärer EL0-Prozess gestartet wird — exakt EmuTOS'
> „im ROM, aber ein echter Basepage-Prozess". Der Shell-Loop (`sh_main`/
> `sh_ldapp`) lebt im Kernel/AES, nicht im Desktop. Der Desktop startet nichts
> selbst: er ruft `shel_write` und terminiert.**

### 3.2 Q1 — Wie wird ein EL0-Programm zur Shell?

**Was schon da ist [V]:**
- `tos-api/src/bdos/process/pexec.rs:189` löst `path` **zuerst** gegen die
  RAM-Registry auf (`registry::lookup`), erst dann gegen FAT16. Die Registry
  (`registry.rs`, `MAX_PROGRAMS = 8`) hält `&'static [u8]`-ELF-Blobs und wird
  von zwei Selftests benutzt (`selftest_pexec.rs:238`,
  `selftest_pexec_d4d5.rs:140`). **Das ist unser `aes_run_rom_program`.**
- `Pexec(PE_LOADGO)` aus EL0 funktioniert vollständig: Parent-Frame-Snapshot,
  `parent_l0_pa`, `parent_slot`, TTBR0-Wechsel, Rückkehr über `Pterm`
  (`kernel/src/arch/aarch64/exceptions.rs:1324-1400`).
- Kernel→EL0-Start mit Rückkehr: `enter_el0_with_resume_capture`
  (`kernel/src/launch.rs:110-120`) — das ist `dos_exec(PE_GOTHENFREE)`-Semantik.
- Der Loader mappt jeder EL0-App eine 4-MiB-TPA, füllt Basepage, Sysvars,
  Cookie-Jar und zwei Trampoline in `[0x400, 0x1000)`
  (`loader.rs:488-500`).
- `Malloc`/`Mxalloc` liefern EL0 eine gemappte User-VA
  (`tos-api/src/gemdos/memory.rs:56-83,126-176`, ADR-021).

**Was zwischen „wir können eine App starten" und „der Desktop ist `rlr`" fehlt:**

| # | Fehlt | EmuTOS-Referenz | Wo es hingehört |
|---|---|---|---|
| 1 | Shell-Loop `sh_main` mit `sh_nextapp`-Zustandsmaschine | `gemshlib.c:692-731`, `:76-83` | Kernel (`sh_ldapp` braucht `enter_el0_*`), Zustand in `tos-api/src/aes/shell.rs` |
| 2 | `sh_ldapp`-Dispatch (DESKTOP/CONSOLE/AUTORUN/NORMAL) | `gemshlib.c:592-681` | ebd. |
| 3 | `sh_chdef` (Laufwerk/Verzeichnis setzen) | `gemshlib.c:535-566` | ebd. |
| 4 | `sh_chgrf`/`sh_toalpha`/`sh_tographic` (Alpha↔Grafik) | `gemshlib.c:223-260,513-523` | ersetzt `fbcon::activate()` im Desktop |
| 5 | `sh_draw(cmd, TRUE)` = CLEAR_SCREEN zwischen Programmen | `gemshlib.c:269-289` | braucht S1b (AES-DESKTOP-Baum) |
| 6 | Konsument für `pending_launch()` (**P6-005**) | `gemshlib.c:645` | `sh_ldapp` |
| 7 | `process_inf1` (`#Q`, `#R`/Rez) im AES-Boot | `geminit.c:343-391` | `tos-api/src/aes/` |
| 8 | `process_inf2` (`#E` Doppelklick, **`#Z` Autorun**) | `geminit.c:407-464` | ebd. |
| 9 | `wm_new()` nach App-Ende | `gemshlib.c:609,677` | `sh_ldapp`; Opcode 109 existiert |
| 10 | AESPD-Nummerierung + `p_nameit`/`p_setappdir` auf `rlr` | `gempd.c:77-86,88-102`; `gemshlib.c:604-605,634-635` | `tos-api/src/aes/{appl,pd}.rs` |

**[V]** Für Punkt 1–3 gibt es bereits eine Spec-Grundlage:
`docs/api/aes/scrap-shell.md:440-500` und `docs/api/emutos-boot-flow.md:90-118`;
die Lückenliste dort (`:147-158`) benennt „no shell loop / `shel_write`
dispatch" schon als 🔴.

**[STOP-1] Schichtfrage.** `sh_ldapp` braucht `enter_el0_with_resume_capture`
(Kernel/arch) und `pexec::load` (tos-api). Meine Empfehlung: **Zustand + Policy
in `tos-api/src/aes/shell.rs`** (`SHELL`-Struct, `sh_nextapp`, `sh_chdef`,
`sh_write`, `sh_read`), **Schleife + Start in `kernel/src/shell_loop.rs`**
(neu, neben `kernel/src/shell.rs`). Begründung: EmuTOS legt beides in
`gemshlib.c`, aber dort ist die AES *ist* der Kernel; bei uns ist die
EL0-Umschaltung arch-spezifisch. Wenn der Lead die Schleife komplett in
`tos-api` will, braucht `tos-api` einen Callback-Trait für den EL0-Eintritt —
auch vertretbar, aber eine Layering-Entscheidung, die ich nicht rate.

### 3.3 Q2 — Wo lebt der Desktop?

**Empfehlung: eigenes Crate, Artefakt als ELF-Blob ins Kernel-Image
eingebettet, Start über die Registry.**

```
programs/                       (eigener Cargo-Workspace, in Cargo.toml
                                 bereits als `exclude` vorgesehen — aber
                                 [V] das Verzeichnis existiert heute NICHT)
├── newtos-gem/                 SDK-Crate  = EmuTOS desk/gembind.c + aesbind.h
│   ├── aes.rs                  AESPB + gem_trap + alle aesbind-Wrapper
│   ├── gemdos.rs bios.rs xbios.rs vdi.rs
│   ├── heap.rs                 GlobalAlloc über den TPA-Heap
│   └── start.rs                _start / panic_handler  (= deskstart.S)
└── desktop/                    das Programm selbst
    └── src/{main,app,act,dir,dlg,ftype,global,icons,inf,insapp,menu,
              model,redraw,win}.rs
```

Warum eingebettet statt Datei auf der SD-Karte:

- **[V] EmuTOS macht es so.** `aes_run_rom_program(deskstart)` startet
  ROM-residenten Code als echten Prozess (`gemshlib.c:570-580,606`). Eine
  Datei auf Platte wäre die Abweichung.
- **Kein Boot-Risiko.** Fehlt die Karte oder die Datei, kommt das System immer
  noch hoch. Ein Desktop, der von `C:` abhängt, ist ein zweites
  Ausfallszenario, das wir uns nicht kaufen müssen.
- **Der magische Name bleibt.** `DEF_DESKTOP = "NEWDESK"` wird der
  Registry-Schlüssel — strukturgleich zu `"EMUDESK"`.
- **Ersetzbarkeit bleibt erhalten.** ADR-022s Ziel („ersetzbarer Desktop") läuft
  über `#Z`/`shel_wdef` auf einen Platten-`.NAP`, exakt wie bei EmuTOS. Der
  eingebettete bleibt der Default.

Build-Kette:
```
cargo xtask build-programs   # baut programs/* → target/newtos-programs/*.NAP
cargo xtask build --board …  # kernel/build.rs: include_bytes! des Blobs
```
`kernel/build.rs` **muss hart fehlschlagen**, wenn `NEWDESK.NAP` fehlt — nicht
auf einen leeren Blob zurückfallen. Ein leerer Blob wäre ein Stub.

**[A]** Das Programm-Image passt in die 4-MiB-TPA. Rechnung: Objekt-Arena
`INITIAL_ITEM_SLOTS = 8 × 350 = 2800` × 32 B (LP64-`Object`) ≈ 90 KB
(`kernel/src/desk/global.rs:75-78`), plus `ScreenInfo`-Parallelarray,
`Vec<FNode>` und ~200–400 KB Code. **Ungemessen** — Stufe S5 muss es messen.
`TPA_SIZE` steht in `tos-api/src/bdos/process/arena.rs:52`.

### 3.4 Q3 — Die Resource

Zwei getrennte Dinge, die das Briefing zusammenzieht:

**(a) Die AES-eigene `gem.rsc`-DESKTOP-Struktur — gehört in diese Arbeit.**
Wir haben `FSELECTR` als konvertierten Baum (`tos-api/src/aes/gem_fsel.nrsc`,
`fsel.rs:82`), aber **keinen** DESKTOP-Baum (Index 2). Gebraucht für:
- `ap_private` (D-R1-6) — `gemsuper.c:95-96`
- `sh_draw(cmd, TRUE)` CLEAR_SCREEN zwischen Programmen — `gemshlib.c:276-282`
- `set_aes_background` aus `#Q` — `geminit.c:635-641`
Umfang: 3 Objekte (`gem_rsc.c:551-570`), ROOT `ob_spec = 4419`, ein `G_TEXT`
`DTNAME` für den Programmnamen in der Leiste. Klein. → **Stufe S1b.**

**(b) Die Desktop-eigene `desktop.rsc` (`desk_rs_init`/`desk_xlate_fix`,
`deskmain.c:1979-1990`) — gehört NICHT in diese Arbeit.**
Wir bauen Menü und Dialoge heute von Hand (`menu.rs` 726 Z., `dlg.rs`,
`insapp.rs`). Ob die zu einer `.NRSC` werden, ist orthogonal zur EL0-Frage: der
Code baut die Bäume in *seinem* Speicher, egal auf welcher Ebene er läuft. →
**Stufe S9, nach dem Umzug.** Vorher zu machen hieße, zwei große Umbauten zu
überlagern.

**[V] Wichtig für S5:** `men_update` schreibt `ob_state` **direkt** in den
eigenen Baum — EmuTOS auch (`deskmain.c:475-486`). Das ist 1:1 und bleibt so,
auch aus EL0 (die AES liest den Baum beim Zeichnen; QED/TeraDesk beweisen den
Pfad).

### 3.5 Q5 — Was die AES braucht, um den Desktop als Client zu bedienen

**[V] Keine neuen AES-Opcodes.** Deckt sich mit ADR-022s Validierung („Die
Validierung fand **keine** AES-Server-seitigen Lücken"). Der Desktop benutzt
heute 22 Opcodes; zusätzlich gebraucht werden `graf_handle` (77),
`wind_update` (107), `menu_icheck` (31), `shel_write/get/put/read` (121/122/
123/120), `form_dial/error/center` (51/53/54) — **alle vorhanden**
(`tos-api/src/aes/mod.rs:282-361`).

Gebraucht werden **fünf Server-Korrekturen** (Details §6):

| | Was | EmuTOS | Unser Stand |
|---|---|---|---|
| S1a | `menu_bar(NULL, 0)` muss `MNTREE` löschen | `gemmnlib.c:724-728` | `tos-api/src/aes/menu.rs:140-143` steigt bei `tree == NULL` mit `ret 0` **vorzeitig aus** → im EL0-Modell bleibt ein toter User-VA-Zeiger stehen |
| S1b | AES-DESKTOP-Baum + `#Q` + `ap_private` | `gem_rsc.c:551-556`, `geminit.c:635-641,724-727`, `gemsuper.c:95-96` | fehlt; `window.rs:577-600` liest den falschen Baum |
| S1c | `Fsetdta`/`Fgetdta` per-Prozess `p_xdta` | `bdos/fsmain.c:396,403-405` | `tos-api/src/bdos/fs/find.rs:49-65` benutzt ein **globales** `CUR_DTA`; die Basepage hat `p_xdta` bei 0x20 und der Loader füllt es (`loader.rs:644`) |
| S1d | XBIOS `Keytbl` (0x10) mit User-VA-Tabellen | `desk/deskmain.c:1144-1145` | `tos-api/src/xbios/mod.rs:121-128`: nicht implementiert, liefert die Funktionsnummer |
| S7 | `ap_id` = AESPD-Index (0 für die dos-appl) | `gemsuper.c:94`, `geminit.c:875-876` | `tos-api/src/aes/appl.rs:191-201`: `slot_for(bp)+1`, im Kernel-Kontext 1 |

Dazu die **Löschung** von `save_windows`/`restore_windows`
(`window.rs:631-666`) zugunsten von `wind_new` (Opcode 109, existiert).

### 3.6 Q6 — Was aus den 9234 Zeilen wird

Ehrliche Aufteilung, Datei für Datei:

| Datei | Z. | Schicksal |
|---|---|---|
| `main.rs` | 1851 | **Umzug**, minus ~150 Z. Startblock (`launch_native`/`run_gem_app`/`desktop_relinquish`/`desktop_resume`/`console_pause_for_key`, `main.rs:465-580`) → ersetzt durch `pro_run`/`pro_exec` (~30 Z.). `check_alt_letter_key` → `Keytbl`. |
| `inf.rs` | 1547 | **Umzug**, +`shel_get`/`shel_put` in `app_start`/`app_save` (§2.4), −`#Q`/`#E`/`#Z`-Teile, die in die AES wandern (dort **zusätzlich**, nicht statt — 1:1 hat zwei Leser). `SpinMutex` → `RefCell`. |
| `dir.rs` | 1189 | **Umzug**; 15 GEMDOS-Aufrufe → echte `svc #1`. Selftest (`:1086-1176`) → EL0-Testprogramm. |
| `win.rs` | 887 | **Umzug**, unverändert außer `CELL_W/H` → `gl_wchar/gl_hchar`. |
| `menu.rs` | 726 | **Umzug**; `host()` → `wind_get`. |
| `mod.rs` | 628 | **Aufspaltung**: aesbind (~500 Z., `:40-540`) → `newtos-gem`; `run()` (`:544-628`) → `deskmain()` + `deskstart`. |
| `global.rs` | 551 | **Umzug**; `SpinMutex` → `RefCell`/`static mut` (Single-Thread-EL0). |
| `model.rs` | 452 | **Umzug**; `Fsetdta/Fsfirst/Fsnext` → Traps. |
| `insapp.rs`/`icons.rs`/`act.rs`/`redraw.rs`/`app.rs`/`dlg.rs`/`ftype.rs` | 1403 | **Umzug**, praktisch unverändert. `ftype.rs` wird zusätzlich im Kernel gebraucht (s. u.). |

**Ehrliches Verhältnis [A, gerechnet, nicht gemessen]:**
- ~**8600 Z. (93 %)** ziehen im Wesentlichen unverändert um (Modulpfade und
  `use`-Zeilen ändern sich in jeder Datei, die Logik nicht).
- ~**450 Z. (5 %)** werden mechanisch umgeschrieben: 56 Bypass-Call-Sites, 2
  `SpinMutex`, ~17 `kprintln`.
- ~**180 Z. (2 %)** werden gelöscht: der Startblock in `main.rs` und die
  `save/restore_windows`-Klammer.
- ~**500 Z.** wandern aus `mod.rs` ins SDK-Crate.

**Kernel-Restposten [V]:** `kernel/src/launch.rs:137` benutzt
`crate::desk::ftype::{classify, FileType, Flavor}` für den AUTO-Ordner. Nach dem
Umzug braucht der Kernel eine eigene, winzige Endungsprüfung. Das ist **1:1**,
keine Duplikation aus Bequemlichkeit: EmuTOS' `autoexec` hat seinen eigenen
Filter `Fsfirst("\\AUTO\\*.PRG", 7)` (`bios/bios.c:713-714`), völlig unabhängig
von `desk/`.

**Toter Code, der mitgenommen werden muss [V]:** `desk::dir::selftest`
(`dir.rs:1086`), `desk::inf::selftest` (`inf.rs:1388`),
`desk::inf::selftest_install` (`inf.rs:1477`) haben **keine Aufrufer**. Sie sind
das, was der Tester als „desk-Selftests" fahren würde. Sie müssen ein
EL0-Testprogramm werden (Vorbild `sdk/apps/apitest`), sonst gehen sie beim
Umzug still verloren.

### 3.7 Was der Desktop **nicht** mitnehmen kann — der ehrliche Befund

Das Briefing verlangt: „If some capability genuinely cannot cross to EL0, name
it." Es sind drei Dinge, und alle drei sind lösbar — aber keins davon
stillschweigend.

**(1) [V] EmuTOS' Desktop ist gar kein reiner AES-Client.** Er ruft
AES-*interne* VDI-Helfer, weil er mit der AES in *einem* Binary liegt:
`gsx_pline`/`gsx_attr`/`gsx_sclip` (`desk/deskact.c:204,219-220`),
`gsx_trans` (`desk/deskapp.c:445,527-529`), `gsx_tblt`/`gsx_gclip`
(`desk/deskinf.c:321-340`), `gsx_sclip` (`desk/deskwin.c:616`). Ebenso liest er
AES-Globals direkt: `gl_wchar`, `gl_hchar`, `gl_wschar`, `gl_hschar`,
`gl_nplanes`.
→ **Bereits entschieden**: ADR-022 legt fest, dass `gsx_trans` durch den
öffentlichen VDI-Trap `vr_trnfm` und `menu_text` durch den echten Opcode ersetzt
werden. Unser Desktop macht das gar nicht erst (er zeichnet über `objc_draw` und
`graf_*`), ist an dieser Stelle also *reiner* als EmuTOS. Kein neues Problem.
→ **Offen bleibt `gl_wschar`/`gl_hschar`** (Kleinschrift-Metrik für
Icon-Beschriftung, `deskapp.c:519-520,810-811`). Dafür gibt es **keinen
AES-Aufruf**. Heute irrelevant, weil wir überall `CELL_W/H` (8/16) benutzen und
der 6×6-Font ohnehin zurückgestellt ist (Tracker C1/E3). **[STOP-2]** Wenn C1
kommt: entweder eine AES-Erweiterung (Abweichung) oder `vqt_fontinfo` auf einer
eigenen `v_opnvwk`-Workstation (was jede GEM-App tut). Nicht in dieser Arbeit
entscheidbar — Researcher-Frage.

**(2) [V] `Keytbl` liefert einen Zeiger, den EL0 nicht dereferenzieren kann.**
`check_alt_letter_key` macht `keytab = (KEYTAB *)Keytbl(-1,-1,-1);
drive = keytab->shift[HIBYTE(thechar)]` (`deskmain.c:1144-1145`). Auf EmuTOS
zeigt das in ROM/RAM ohne Schutz. Bei uns wäre es eine Kernel-VA → Fault.
→ **Lösung mit dem etablierten Mechanismus**: `KEYTAB` (3 Zeiger) + die drei
128-Byte-Tabellen in den reservierten Low-Bereich der TPA schreiben, wie
`populate_sysvars`/`write_upcall_trampoline` es schon tun
(`loader.rs:488-500`). **[V] Platz ist da**: belegt bis `USERDEF_RET_SLOT_VA`
0x760 (`loader.rs:728`), frei bis `RESERVED_LOW_END` 0x1000 (`:186`) — 2200 B
für 24 + 3×128 = 408 B. `Keytbl` liefert dann bei `current() != null` die
User-VA (analog zu `Malloc`, ADR-021), sonst die Kernel-VA.
→ Nutzt außerdem jeder portierten App, die `Keytbl` ruft (TeraDesk tut es).
**[V]** Die Tabellen existieren bereits im passenden Format:
`kernel/src/keymap.rs:26-35` (`norm`/`shft`/`caps` je `[u8; 128]`).

**(3) [V] Das globale `CUR_DTA` wird zur Zeigerleiche.** `Fsetdta` speichert
heute in ein globales Static (`find.rs:49-56`) statt in `run->p_xdta`
(`bdos/fsmain.c:403-405`). Solange nur der EL1-Desktop scannt, fällt das nicht
auf. Sobald der Desktop ein EL0-Prozess ist, der stirbt, zeigt `CUR_DTA` in
einen toten Adressraum — und heute schon überschreibt QEDs `Fsetdta` das des
Desktops. Zusatzbefund: `Fgetdta` trunkiert auf `Long` (`find.rs:60-65`) und
kann eine High-Half-Kernel-DTA gar nicht zurückgeben; eine EL0-User-VA schon.
Der Umzug **repariert** das, statt es zu verursachen.

**Kein Privileg bleibt.** Nach S8 hat der Desktop exakt die Rechte von QED:
eine 4-MiB-TPA, den GEMDOS/BIOS/XBIOS/VDI/AES-Trap, sonst nichts.

---

## 4. Q7 — Was unterwegs bricht, und was der Rückweg ist

**An jeder Stufengrenze bootet das System.** Das ist konstruktiv, nicht
hoffnungsvoll:

- **S1–S4** fassen den Desktop-Boot-Pfad **nicht** an. `desk::run()` bleibt
  `lib.rs:329`. Rückweg: die einzelnen Fixes sind unabhängig revertierbar.
- **S5** legt ein neues Crate an und **verdrahtet es nicht**. Der Kernel baut
  unverändert; `cargo xtask build-programs` ist ein zusätzlicher Schritt.
  Rückweg: Verzeichnis liegen lassen.
- **S6** ist der Umschalter: `sh_ldapp`s `DESKTOP_APP`-Arm ruft entweder
  `Pexec(PE_LOADGO, "NEWDESK")` **oder** — hinter dem Cargo-Feature
  `desk-in-kernel` — weiter `desk::run()`. Rückweg = ein Feature-Flag.
  Der Default kippt erst, wenn der Tester grün meldet.
- **S7** ändert die AESPD-Nummerierung. Rückweg: eine Konstante.
- **S8** löscht. Findet nur nach grünem S7 statt.

**Was in S6 mit hoher Wahrscheinlichkeit bricht [A, benannte Vorhersagen]:**
1. Der Bildschirmzustand *zwischen* Programmen. `sh_draw(CLEAR_SCREEN)` +
   `wm_new` + `wind_new` in falscher Reihenfolge → Desktop-Fensterreste hinter
   der neuen App. Genau das Symptom, gegen das `desktop_relinquish` erfunden
   wurde (Findings-Log F5).
2. Zeigerleichen in der AES nach dem Desktop-`Pterm`: `MNTREE` (→ S1a),
   `NEWDESK_TREE` (`window.rs:520-524`, wird von `wind_set(…, NULL)` gelöscht —
   [V] vorhanden), `w_name`/`w_info` pro Fenster (→ von `wind_new` gelöscht,
   [V] `window.rs:2166`), `CUR_DTA` (→ S1c).
3. Der Rust-Global-Allocator gegen `MAX_USER_BLOCKS = 32`
   (`tos-api/src/bdos/process/usermem.rs:80`), falls je Allokation ein `Malloc`
   abgesetzt wird. **Deshalb** empfehle ich den TPA-Heap (`__bss_end` …
   `p_hitpa`) als Allocator-Arena — das ist auch, was ein C-TOS-Programm nach
   `Mshrink` tut.

---

## 5. Q4 — Das Bypass-Register, Einzelposten

**56 Call-Sites, 21 `tos_api`-Symbole, 8 `crate`-Symbole.** Jede Zeile wird ein
Trap oder verschwindet. Spalte „Trap-Äquivalent" ist der Befund, nach dem
gefragt wurde.

### 5.1 GEMDOS — 13 Symbole, 22 Call-Sites · **alle haben ein Trap-Äquivalent**

Diese Aufrufe bauen heute schon ein `TrapArgs` und rufen die Funktion direkt
(`gargs()`/`gemdos_args()`); der Umbau auf `svc #1` ist mechanisch.

| # | Symbol | Call-Sites | Trap |
|---|---|---|---|
| 1 | `bdos::fs::file::fopen` | `dir.rs:186`, `inf.rs:75` | Fopen 0x3D ✓ |
| 2 | `…::fcreate` | `dir.rs:192`, `inf.rs:101` | Fcreate 0x3C ✓ |
| 3 | `…::fclose` | `dir.rs:207`, `inf.rs:93`, `inf.rs:116` | Fclose 0x3E ✓ |
| 4 | `…::fread` | `dir.rs:197`, `inf.rs:83` | Fread 0x3F ✓ |
| 5 | `…::fwrite` | `dir.rs:202`, `inf.rs:109` | Fwrite 0x40 ✓ |
| 6 | `…::fdelete` | `dir.rs:160` | Fdelete 0x41 ✓ |
| 7 | `…::frename` | `dir.rs:180` | Frename 0x56 ✓ |
| 8 | `…::fdatime` | `dir.rs:215` | Fdatime 0x57 ✓ |
| 9 | `bdos::fs::dirops::dcreate` | `dir.rs:172` | Dcreate 0x39 ✓ |
| 10 | `…::ddelete` | `dir.rs:166` | Ddelete 0x3A ✓ |
| 11 | `bdos::fs::find::fsetdta` | `dir.rs:270`, `model.rs:281` | Fsetdta 0x1A ✓ — **aber S1c nötig** (per-Prozess `p_xdta`) |
| 12 | `…::fsfirst` | `dir.rs:274`, `model.rs:293` | Fsfirst 0x4E ✓ |
| 13 | `…::fsnext` | `dir.rs:288`, `model.rs:306` | Fsnext 0x4F ✓ |

Zusätzlich in EL0 nötig, heute nirgends gerufen: `Dgetdrv` 0x19
(`deskmain.c:1951 G.g_stdrv = dos_gdrv()`), `Dsetdrv` 0x0E / `Dsetpath` 0x3B
(für `sh_chdef`, `gemshlib.c:526-533`). Alle im Dispatcher vorhanden **[A]** —
nicht einzeln nachgeprüft.

### 5.2 BIOS — 1 Symbol, 3 Call-Sites · **Trap-Äquivalent vorhanden**

| # | Symbol | Call-Sites | Trap |
|---|---|---|---|
| 14 | `bios::drive::drvbits` | `app.rs:119`, `main.rs:729`, `mod.rs:619` | Drvmap 0x0A ✓ (`tos-api/src/bios/mod.rs:49,70,209`) |

### 5.3 VDI — 3 Symbole, 21 Call-Sites · **1 hat KEIN Äquivalent**

| # | Symbol | Call-Sites | Trap |
|---|---|---|---|
| 15 | `vdi::font::CELL_W` | 12 (`act.rs`, `app.rs`, `icons.rs`, `menu.rs`, `win.rs`, `inf.rs`) | **`graf_handle` (77)** → `gl_wchar`; ✓ vorhanden, liefert 8 (`tos-api/src/aes/graf.rs:454-457`) — verhaltensgleich |
| 16 | `vdi::font::CELL_H` | 8 | **`graf_handle` (77)** → `gl_hchar` = 16 ✓ |
| 17 | `vdi::mouse::display_at` | `mod.rs:615` | **✗ KEIN Äquivalent.** EmuTOS' Desktop positioniert den Zeiger **nie**; das macht die AES in `ratinit()` (`gemshlib.c:237`, in `sh_tographic`). → **Der Aufruf entfällt ersatzlos; die Positionierung wandert in den AES-Shell-Loop** (S3). Der Doc-Kommentar `mod.rs:604-613` beschreibt ihn selbst als Reaktion auf eine „tos-api cursor-init note" — das ist die richtige Stelle. |

### 5.4 Host / Framebuffer — 1 Symbol, 6 Call-Sites · **1 hat KEIN Äquivalent**

| # | Symbol | Call-Sites | Trap |
|---|---|---|---|
| 18 | `host::host().raster().width()/height()` | `insapp.rs:107`, `menu.rs:504`, `dlg.rs:123`, `inf.rs:1099`, `mod.rs:545` | **`wind_get(DESKWH, WF_WXYWH)`** ✓ — genau was EmuTOS tut (`deskmain.c:1959`). Fünf identische 4-Zeiler → eine Funktion. |
| 19 | `host::host().console_read_key_blocking()` | `main.rs:578` | **✗ KEIN Äquivalent — und es soll keins geben.** Teil von `console_pause_for_key` („Press any key to return to the desktop"). EmuTOS hat das nicht: `sh_chgrf` schaltet direkt via `sh_tographic` zurück (`gemshlib.c:513-523`). **[STOP-3]** Der Doc-Kommentar (`main.rs:562-565`) begründet es mit „classic TOS", nicht mit EmuTOS. Unser Profil ist EmuTOS Falcon/TOS4. **Empfehlung: ersatzlos streichen.** Wenn der Lead es behalten will, ist es eine benannte, in `docs/api/` einzutragende Abweichung. |

### 5.5 AES ohne Opcode — 3 Symbole, 4 Call-Sites · **alle drei ohne Äquivalent**

| # | Symbol | Call-Sites | Befund |
|---|---|---|---|
| 20 | `aes::window::redraw()` | `redraw.rs:209`, `main.rs:543` | **✗ Kein AES-Opcode.** Ganzschirm-Repaint. EmuTOS' Desktop fordert das nie an — er bekommt `WM_REDRAW`. `redraw.rs:209` steckt in `redraw_desktop()`, dessen eigener Doc-Kommentar (`redraw.rs:195-205`) schon sagt, dass die Aufrufer auf `form_dial(FMD_START/FINISH)` + `WM_REDRAW` umgestellt gehören. `main.rs:543` (in `desktop_relinquish`) wird durch `sh_draw(CLEAR_SCREEN)` im Shell-Loop ersetzt. |
| 21 | `aes::window::save_windows()` | `main.rs:537` | **✗ NewTOS-Erfindung.** Ersatz: `wind_new` (109) in `sh_ldapp` (`gemshlib.c:609,677`). → **löschen** (`window.rs:631-648`). |
| 22 | `aes::window::restore_windows()` | `main.rs:556` | **✗ NewTOS-Erfindung.** Kein EmuTOS-Gegenstück: der Desktop baut aus `cnx_get()` neu auf. → **löschen** (`window.rs:656-666`). |

`aes::window::compute_rlist` ist **[V] kein Call-Site** — nur zwei
Doc-Erwähnungen (`redraw.rs:40`, `mod.rs:257`). Der echte Weg ist bereits
`wind_get(WF_FIRSTXYWH/WF_NEXTXYWH)`.

### 5.6 Typen über die Crate-Grenze — 3 Symbole

| # | Symbol | Nutzungen | Lösung |
|---|---|---|---|
| 23 | `aes::object::Object` | 34 | → **`tos-types`** verschieben, `tos-api` re-exportiert |
| 24 | `aes::object::IconBlk` | 13 | dito |
| 25 | `aes::object::TedInfo` | 6 | dito |

Nicht verschoben wird `ObjTree` (ABI-abhängiger Accessor) — der bleibt
serverseitig. Es geht nur um das POD-Layout plus die `G_*`/`OB_*`-Konstanten.
Der Desktop baut LP64-Bäume; **[V]** `current_abi()` liefert für einen
LP64-EL0-Prozess `Abi::Lp64` (`tos-api/src/bdos/process.rs:668-674`) — also
dieselbe Baumbreite wie heute. **Kein ABI-Risiko.**

### 5.7 Kernel-Symbole — 8 Symbole

| # | Symbol | Call-Sites | Ersatz |
|---|---|---|---|
| 26 | `launch::run_program_with_tail` | `main.rs:484`, `main.rs:522` | `shel_write(SHW_EXEC, …)` + `done = TRUE` (`deskpro.c:36-63`) |
| 27 | `launch::run_program` | 0 (nur Doc) | entfällt |
| 28 | `shell::run_console` | 0 (nur Doc) | `pro_run(FALSE, DEF_CONSOLE, …)` (`deskmain.c:751-753`) |
| 29 | `keymap::to_ascii` | `main.rs:726` | **XBIOS `Keytbl` (0x10)** — **fehlt** (S1d) |
| 30 | `fbcon::activate` | `main.rs:483` | `sh_chgrf`/`sh_tographic` im Shell-Loop (`gemshlib.c:513-523`) |
| 31 | `fbcon::write_byte` | `main.rs:575` | entfällt mit `console_pause_for_key` (STOP-3), sonst `Cconws` 0x09 |
| 32 | `sync::SpinMutex` | `global.rs:514`, `inf.rs:253` | `RefCell`/`static mut` — Single-Thread-EL0 |
| 33 | `kprintln` | ~17, davon 13 in totem Selftest-Code | `Cconws`; Selftests → EL0-Testprogramm |

### 5.8 Zusammenfassung der versteckten Arbeitspakete

**Ohne Trap-Äquivalent, jedes ein eigener Posten:**

1. **XBIOS `Keytbl` (0x10)** + per-Prozess-`KEYTAB` in der TPA-Low-Region.
   *(Blockiert `check_alt_letter_key`. Nutzt auch TeraDesk.)*
2. **`Fsetdta`/`Fgetdta` auf `p_xdta`** statt globalem `CUR_DTA`.
   *(Latenter Bug schon heute: QED überschreibt die DTA des Desktops.)*
3. **AES-interner `DESKTOP`-Baum** (`gem.rsc` TREE 2) inkl. `set_aes_background`
   und `#Q`-Lesen im AES-Boot. *(Schließt D-R1-6; liefert `sh_draw`.)*
4. **`menu_bar(NULL, 0)` muss wirken** statt vorzeitig auszusteigen.
   *(Sonst Zeigerleiche nach Desktop-`Pterm`.)*
5. **`save_windows`/`restore_windows` löschen**, `wind_new` an die richtige
   Stelle im `sh_ldapp`.
6. **`aes::window::redraw()`-Aufrufer auflösen** (2 Stellen) — der eine wird
   `sh_draw`, der andere gehört auf `WM_REDRAW`.
7. **`vdi::mouse::display_at` aus dem Desktop entfernen**, `ratinit()`-Äquivalent
   in den Shell-Loop.
8. **`ap_id` = AESPD-Index**, primäre Anwendung = 0; `p_nameit`/`p_setappdir`
   auf `rlr` beim Start *(schließt zusätzlich AUD-P1-001: `appl_find` liefert
   heute nie einen Namen)*.
9. **Shell-Loop** `sh_main`/`sh_ldapp`/`sh_chdef`/`sh_chgrf`/`sh_draw` inkl.
   Konsument für `pending_launch()` **(P6-005)**.
10. **`process_inf1`/`process_inf2` im AES-Boot** inkl. **`#Z`-Ausführung**
    (heute geparst, nie ausgeführt).
11. **`Object`/`IconBlk`/`TedInfo` nach `tos-types`** verschieben.
12. **Rust-EL0-Programmsubstrat**: `programs/`-Workspace, `newtos-gem`-SDK,
    Global-Allocator auf dem TPA-Heap, `_start`, `panic_handler`,
    `cargo xtask build-programs`, `kernel/build.rs`-Einbettung.
13. **AUTO-Ordner-Endungsprüfung im Kernel** (Ersatz für
    `desk::ftype::classify`, 1:1 zu `bios/bios.c:713-714`).
14. **Desk-Selftests** (`dir.rs:1086`, `inf.rs:1388`, `inf.rs:1477`) als
    EL0-Testprogramm re-homen — sonst gehen sie still verloren.
15. **`do_deskmenu` ignoriert sein `item`** (`main.rs:768`) und „Execute
    EmuCON" steht im falschen Menü (EmuTOS: `CLIITEM` in *File*,
    `deskmain.c:751`). Wird durch den Shell-Loop erst *ausführbar* — ich führe
    es als Posten, damit es nicht als „schon immer so" durchrutscht.

---

## 6. Warum genau diese Form — und nicht die naheliegenden Alternativen

Wo eine echte Gabelung besteht, empfehle ich eine Seite und sage, was mich
umstimmen würde.

| Gabelung | Empfehlung | Was mich umstimmen würde |
|---|---|---|
| Desktop **eingebettet** (Registry) vs. **Datei auf `C:`** | **Eingebettet.** EmuTOS' `aes_run_rom_program` (`gemshlib.c:570-580`); kein Boot-Risiko; magischer Name = Registry-Schlüssel. | Wenn der Lead will, dass der Desktop **ohne Kernel-Rebuild** austauschbar ist. Dann: Datei zuerst, Registry als Fallback — aber dann ist ein fehlender Desktop ein Boot-Ausfall, den EmuTOS nicht hat. |
| Shell-Loop im **Kernel** vs. in **`tos-api`** | **Kernel** (Schleife), **`tos-api`** (Zustand/Policy). Der EL0-Eintritt ist arch-spezifisch. | Ein Callback-Trait in `tos-api` für den EL0-Eintritt. Sauberer geschichtet, aber ein Indirektionsschritt mehr auf dem heikelsten Pfad. **[STOP-1]** |
| Rust-Allocator: **TPA-Heap** vs. **`Malloc` pro Allokation** | **TPA-Heap** (`__bss_end` … `p_hitpa`). Was ein C-TOS-Programm nach `Mshrink` tut; umgeht `MAX_USER_BLOCKS = 32`. | Wenn der Desktop mehr Heap braucht, als in der TPA übrig ist — dann eine `Malloc`-Arena obendrauf (nicht pro Allokation). |
| **Ein** Crate für Desktop+SDK vs. **zwei** | **Zwei** (`newtos-gem` + `desktop`). `newtos-gem` = EmuTOS `desk/gembind.c`+`aesbind.h`; ein zweiter EL0-GEM-Client (Testprogramm) braucht es sofort. | Nichts Naheliegendes. Ein Crate spart eine `Cargo.toml` und kostet die Wiederverwendbarkeit. |
| Desktop-Resource (`desk_rs_init`) **jetzt** vs. **danach** | **Danach** (S9). Orthogonal zur EL0-Frage; zwei große Umbauten zu überlagern ist genau die Fehlerklasse, die Versuch 1 und 2 gekostet hat. | Wenn sich in S5 herausstellt, dass der handgebaute Menü-/Dialogbau ohne Resource gar nicht sauber aus dem Kernel lösbar ist. Danach sieht es nicht aus (`menu.rs` baut in eigenen Statics). |

---

## 7. Was den dritten Anlauf am Abdriften hindert

Die ersten beiden Rebuilds sind abgedriftet, weil **Abdriften möglich war und
nichts außer Disziplin dagegenstand**: jede Abkürzung war ein `crate::` weit
entfernt, und ADR-016s Formulierung („behaviour, not line-by-line") gab ihr
sogar Deckung. ADR-022 hat die Formulierung repariert; das hier repariert die
*Möglichkeit*.

**1. Der Bypass wird ein Compilerfehler, kein Review-Befund.**
Nach S8 hat `programs/desktop` weder `kernel` noch `tos-api` im
Abhängigkeitsgraphen. Die 56 Call-Sites aus §5 sind nicht „verboten" — die
Symbole **existieren nicht**. Man kann `fbcon::write_byte` nicht mehr aus
Versehen aufrufen, so wenig wie QED es kann. Das ist der eigentliche Grund,
warum User-Space die Anti-Drift-Maßnahme *ist* und nicht bloß Aufräumen.

**2. Eine Ein-Zeilen-Driftprobe, pro Stufe laufbar.**
```sh
# muss 0 ergeben
grep -rc 'tos_api::\|crate::fbcon\|crate::keymap\|crate::launch\|crate::shell\|kprintln' \
     programs/desktop/src/ | awk -F: '{s+=$2} END {print s}'
# muss genau newtos-gem + tos-types listen
grep -A20 '^\[dependencies\]' programs/desktop/Cargo.toml
```
Zwischen S5 und S8 sinkt diese Zahl von 56 auf 0. Sie ist ein **Messwert pro
Stufe**, keine Meinung.

**3. Funktionsnamen-Korrespondenz als Zahl.**
ADR-022 fordert gleichnamige Funktionen. Ein Skript, das die Funktionsnamen aus
`references/emutos/desk/*.c` gegen die `fn`-Namen in `programs/desktop/src/`
diffed, macht daraus einen Report mit drei Spalten: *vorhanden*, *fehlt*,
*erfunden*. „Erfunden" ist die Driftspalte. Sie darf zwischen zwei Stufen nicht
wachsen, ohne dass die Zeile im Stufenbericht steht.

**4. Der Desktop wird von demselben Instrument beobachtbar wie QED.**
Sobald er ein EL0-Prozess ist, laufen seine AES-Aufrufe durch denselben
`svc`-Pfad, den `kernel/src/dbg_qedtrace.rs` und der Reference-Dumper
(`docs/TOS-REFERENCE-DUMPER.md`, `docs/analysis/aes-abi-trace/`) mitschneiden.
Die Boot-Sequenz des Desktops ist damit gegen `deskmain.c:1953-2073` **diffbar**
— dieselbe Methode, die D-R1-5/D-R1-6 überhaupt erst gefunden hat. Heute ist
der Desktop für dieses Instrument unsichtbar; genau in diesem toten Winkel ist
der Drift beide Male gewachsen.

**5. Die Akzeptanzkriterien sind Messwerte von einem echten TT, keine Meinungen.**
`ap_id == 0` und `ap_private == 0x1143` stehen in
`docs/analysis/aes-r1-findings.md:369-398,555-566`. Man kann sie nicht
weginterpretieren. Sie sind das Abnahmekriterium von S7 bzw. S1b.

**6. Die alte Implementierung wird gelöscht — als eigene Stufe mit eigenem Tor.**
Dass zwei Desktops im Baum lagen, ist der Grund, warum Versuch 2 der
Ausgangspunkt von Versuch 3 wurde. **S8 ist Pflicht, nicht Aufräumen.** Der Plan
gilt nicht als abgeschlossen, solange `kernel/src/desk/` existiert. Das Fenster,
in dem beide existieren, ist S5–S8 und damit benannt und befristet.

**7. Jede Stufe nennt ihre Evidenz und ihre Vorhersage, bevor sie läuft.**
Eine falsch getroffene Vorhersage ist ein Befund und geht in den Stufenbericht.
Eine Stufe, deren Evidenz sich nicht herstellen lässt, ist keine Stufe.

**Ehrliche Einschränkung:** Punkt 1 wird erst mit S8 wahr. Von S5 bis S8 gibt es
zwei Desktops und der alte ist weiter erreichbar. Das ist das Risikofenster
dieses Plans, und die Gegenmaßnahme ist, dass S8 ein reguläres, benanntes
Stufentor mit Abnahmebedingung ist und nicht „machen wir später".

---

## 8. Der Stufenplan

Abhängigkeitsgraph:

```
S0 ─┬─► S1 ─┬────────────────► S3 ─┐
    │       │                      ├─► S6 ─► S7 ─► S8 ─► S9
    ├─► S2 ─┴─► S5 ────────────────┘
    └─► S4 ─────┘
```
Parallelisierbar: **S1 ‖ S2 ‖ S4**; danach **S3 ‖ S5**.
Serialisiert (Token-Budget, Memory `limit-concurrent-workers-token-budget`):
höchstens zwei Worker gleichzeitig, Reihenfolge Developer → Tester → Commit.

---

### S0 — Spec-Lücke schließen (Researcher, kein Code)

**Ziel.** Die Prozessidentitäts-Semantik von `sh_ldapp` steht **nirgends** in
`docs/api/`. `docs/api/aes/scrap-shell.md:48-53,494-503` erklärt den Shell-Loop
ausdrücklich zur „Referenz, frei reimplementierbar" — das ist mit dem stehenden
„1:1, keine Abweichungen" nicht vereinbar und wäre eine Einladung zum Drift.

**Deliverable.** `docs/api/aes/shell-loop.md`, das mit `file:line` beantwortet:
1. `gem_main`-Sequenz (`geminit.c:753-800`).
2. `process_inf1` — `#Q`, `#E`/Rez (`:343-391`).
3. `process_inf2` — `#E` Doppelklick, `#Z` → `sh_wdef` + `isauto` (`:407-464`).
4. AESPD-Nummerierung: 0 = dos-appl, 1 = SCRENMGR, 2+ = ACC
   (`:875-876,888`; `struct.h:36`).
5. `p_nameit`/`p_setappdir` **auf `rlr`** für Desktop *und* App
   (`gemshlib.c:604-605,634-635`; `gempd.c:77-102`).
6. `aes_run_rom_program` (`:570-580`).
7. `sh_main`-Schleife inkl. `wm_init`/`ratinit`/`sh_draw(TRUE)` (`:692-731`).
8. `sh_ldapp`-Dispatch, alle vier Arme (`:592-681`), inkl. `wm_new` (`:609,677`)
   und dem `AP_OPEN`-Aufräumen (`:646-656`).
9. `sh_chdef`/`sh_chgrf`/`sh_toalpha`/`sh_tographic` (`:223-260,513-566`).
10. Der Zustands-Übergabekanal `app_save→shel_put` / `shel_get→app_start`
    inkl. `CPDATA_LEN = 128`-Präfix (`deskapp.c:136,832,1123-1124,1293`).
11. Die `ap_private`-Kette (§2.6) und der `rs_trees[DESKTOP]`-Inhalt
    (`gem_rsc.c:551-570`).
12. `DEF_DESKTOP`/`DEF_CONSOLE` als magische Namen (`geminit.h:13`,
    `aesdefs.h:627`; `gemshlib.c:172`).

Außerdem: **Korrektur an `docs/api/aes/scrap-shell.md`** (der Loop ist nicht
„frei"), und **Korrektur an `docs/analysis/aes-audit/p1-appl-pd-dispatch.md:89-90`**
(der dort empfohlene 1:1-Fix „getrennte Räume für Desktop und EL0-App" ist für
den gestarteten Fall EmuTOS-widrig, §2.1).

**Überprüfbares Ende.** Auditor zeichnet Punkt 1–12 gegen `file:line` ab; die
zwei Korrekturen sind eingetragen.

**Vorhersage.** Die zwei Stellen, an denen unser `inf.rs` heute nicht passt,
werden (a) das 128-Byte-`CPDATA`-Präfix im Shell-Puffer und (b) `#Z` mit
`isgem`-Flag + `sh_wdef`-Aufspaltung in Programm *und* Verzeichnis
(`geminit.c:449-456`) sein.

---

### S1 — Die vier AES-/BIOS-Server-Korrekturen (Desktop unangetastet)

**Abhängigkeit.** S0. **Parallel zu** S2, S4.

**S1a — `menu_bar(NULL, 0)`.** `tos-api/src/aes/menu.rs:140-143` steigt bei
`tree == NULL` mit `ret 0` aus. EmuTOS' `mn_bar` dereferenziert `tree` auf dem
Hide-Pfad **nicht** und setzt `gl_mntree = NULL` (`gemmnlib.c:724-728`).
→ Hide-Pfad vor die Null-Prüfung ziehen. *Kein Regressionsrisiko:* der heutige
Desktop übergibt bewusst einen Nicht-Null-Baum (`main.rs:529-531`) und läuft
unverändert weiter.

**S1b — AES-DESKTOP-Baum + `#Q` + `ap_private`.**
- Eingebauter `DESKTOP`-Baum (3 Objekte, ROOT `ob_spec = 4419`,
  `gem_rsc.c:551-570`), `ob_height = Bildschirmhöhe` bei WS-Open
  (`geminit.c:715-716`).
- `set_aes_background(patcol)` (`geminit.c:635-641`).
- `process_inf1`-`#Q`-Lesen im AES-Boot (`geminit.c:383-388,724-727`).
- `appl_init.ap_private = rs_trees[DESKTOP][ROOT].ob_spec`
  (`gemsuper.c:95-96`) statt `desktop_bg_obspec()`
  (`tos-api/src/aes/window.rs:577-600` → entfällt).

**S1c — `Fsetdta`/`Fgetdta` auf `p_xdta`.** `bdos/fsmain.c:396,403-405`.
`tos-api/src/bdos/fs/find.rs:49-65`; die Basepage hat das Feld (`p_xdta` @0x20,
`tos-types/src/basepage.rs:113-115`) und der Loader füllt es (`loader.rs:644`).

**S1d — XBIOS `Keytbl` (0x10).** `KEYTAB{unshift,shift,capslock}` + 3×128 B in
die reservierte TPA-Low-Region (frei ab 0x768, `loader.rs:728`, bis 0x1000,
`:186`). Rückgabe: User-VA bei laufendem EL0-Prozess, Kernel-VA sonst (analog
`Malloc`, ADR-021). Tabellen: `kernel/src/keymap.rs:26-35`.

**Überprüfbares Ende.**
1. Neues EL0-Testprogramm (`sdk/apps/apitest` erweitern) meldet über `Cconws`:
   `ap_private == 0x1143` ohne `#Q`-Zeile; `== 0x1141` (o. ä.) mit
   `#Q 41 …` in `C:\NEWDESK.INF`.
2. Dasselbe Programm: `Fsetdta(buf); Pexec(child); Fgetdta() == buf` —
   das Kind darf die DTA des Elters nicht verändern.
3. Dasselbe Programm: `k = Keytbl(-1,-1,-1); k->shift[0x1E] == 'A'` — aus EL0
   dereferenziert, ohne Fault.
4. `menu_bar(NULL, 0)` aus dem Testprogramm: kein Fault, `menu_bar(tree,1)`
   danach zeichnet wieder.
5. Regression: Boot-Screendump beider Boards identisch zum Vorzustand.
6. `cargo build` sauber für `board-qemu-virt` **und** `board-rpi4`.

**Vorhersage.** S1b schließt **D-R1-6 sofort**, ohne den Desktop anzufassen —
QED/TeraDesk sehen `0x1143` statt `0x0070`. S1c fördert einen zweiten latenten
Fehler zutage: der heutige EL1-Desktop übergibt eine **Kernel**-DTA, die
`Fgetdta` gar nicht zurückgeben könnte (`find.rs:60-65`); der Umbau macht das
sichtbar, weil `p_xdta` bei `current() == null` keinen Platz hat. Rechne damit,
dass für den EL1-Aufrufer ein Sonderweg (globales Fallback) nötig bleibt,
solange der Desktop im Kernel läuft — und dass der mit S8 verschwindet.

**Rückweg.** Vier unabhängige Commits, einzeln revertierbar.

---

### S2 — Das EL0-Programmsubstrat (Rust `no_std` `.NAP`)

**Abhängigkeit.** S0 (nur für Namenskonventionen). **Parallel zu** S1, S4.

**Was gebaut wird.**
- `programs/` als eigener Cargo-Workspace. **[V] Achtung:** `Cargo.toml:11-13`
  schließt `programs` bereits aus — das Verzeichnis existiert aber **nicht**.
  Der Ausschluss ist heute schal; er wird jetzt eingelöst.
- `programs/newtos-gem`: `gem_trap`/`Aespb`, alle aesbind-Wrapper (Umzug aus
  `kernel/src/desk/mod.rs:40-540`), GEMDOS/BIOS/XBIOS/VDI-Wrapper,
  `GlobalAlloc` über den TPA-Heap, `_start` (= `deskstart.S`,
  `Mshrink`-Äquivalent + `main` + `Pterm`), `panic_handler`.
- `programs/hello-gem`: minimaler `.NAP` — `appl_init` → `graf_handle` →
  `wind_get(DESKWH, WF_WXYWH)` → `wind_create/open` → `objc_draw` →
  `evnt_multi` → `wind_close/delete` → `appl_exit` → `Pterm`.
- `cargo xtask build-programs` (neuer Subcommand neben `build`/`run`/`test`,
  `tools/xtask/src/main.rs:28-40`).
- `kernel/build.rs`: `include_bytes!` des Blobs, **harter Fehler** wenn er fehlt.
- Registrierung im Boot: `registry::register(b"HELLOGEM", &BLOB)`.

**Überprüfbares Ende.**
1. `cargo xtask build-programs` erzeugt `target/newtos-programs/HELLOGEM.NAP`;
   `readelf -h` zeigt `ET_EXEC`, `EM_AARCH64`, `e_entry >= 0x1000`, genau ein
   `PT_LOAD`, kein `PT_DYNAMIC` (die Loader-Bedingungen,
   `tos-api/src/bdos/process/elf.rs:27,53,56,170-176`).
2. Zeitweiliger Selftest-Hook in `lib.rs` startet
   `Pexec(PE_LOADGO, "HELLOGEM")`; QMP-Screendump zeigt das Fenster; das
   Programm terminiert und der Kernel läuft weiter.
3. Gemessen und im Bericht: **Image-Größe + Heap-Bedarf** gegen
   `TPA_SIZE = 4 MiB` (`arena.rs:52`).
4. `cargo build` sauber für beide Boards; keine neuen Clippy-Warnungen.

**Vorhersage.**
- Der erste Fehlschlag wird **nicht** das ELF-Laden sein (zwei Selftests
  beweisen den Pfad), sondern die Heap-Grenze: `__bss_end` … `p_hitpa` korrekt
  aus der Basepage ableiten, ohne den Stack zu treffen.
- Zweitwahrscheinlichster: Rust erzeugt ohne
  `-C relocation-model=static`/`-C code-model=small` GOT-relative Zugriffe und
  damit ein `PT_DYNAMIC`, das der Parser (`elf.rs:27`) ablehnt.
- Ich erwarte **nicht**, dass die AES stolpert: `current_abi()` liefert für ein
  LP64-EL0-Programm dasselbe `Abi::Lp64` wie heute für den EL1-Desktop
  (`process.rs:668-674`).

**Rückweg.** Nichts im Boot-Pfad geändert außer einem auskommentierbaren Hook.

---

### S3 — Der Shell-Loop (Desktop bleibt vorerst in-kernel)

**Abhängigkeit.** S1b (für `sh_draw`), S2 (Startziel). **Parallel zu** S5.

**Was gebaut wird.**
- `tos-api/src/aes/shell.rs`: `SHELL`-Zustand (`sh_doexec`, `sh_nextapp`,
  `sh_isgem`, `sh_desk`, `sh_cdir`, `sh_apdir`) nach `gemshlib.c:76-83`;
  `sh_write` klassifiziert `DEF_CONSOLE`/`DEF_DESKTOP` (`:172`).
- `kernel/src/shell_loop.rs`: `sh_main` (`:692-731`), `sh_ldapp` (`:592-681`),
  `sh_chdef` (`:535-566`), `sh_chgrf`/`sh_toalpha`/`sh_tographic`
  (`:223-260,513-523` → heute `fbcon::activate`), `sh_draw(cmd, TRUE)`
  (`:269-289`, braucht S1b), `wm_new()` nach App-Ende (`:609,677`).
  Konsument für `pending_launch()` — **P6-005 geschlossen**.
- `tos-api/src/aes/`: `process_inf1` (`#Q`, `#R`) und `process_inf2`
  (`#E`-Doppelklick, **`#Z`**) beim AES-Boot; `#Z` → `sh_wdef` + `isauto`
  (`geminit.c:449-456`).
- `lib.rs:329`: `desk::run()` → `shell_loop::sh_main(isauto, isgem)`.
  Der `DESKTOP_APP`-Arm ruft **vorerst weiter `desk::run()`** — ein
  **benannter, befristeter Shim**, der in S6 verschwindet.
- Mauszeiger-Init (`ratinit()`, `gemshlib.c:237`) in `sh_tographic` — der
  Desktop-Aufruf `vdi::mouse::display_at` (`mod.rs:615`) entfällt.

**Überprüfbares Ende.**
1. `#Z 01 C:\HELLOGEM.NAP@` in `C:\NEWDESK.INF` (per `tools/make-sd-image.sh`):
   `HELLOGEM` läuft **vor** dem Desktop, terminiert, danach steht der Desktop.
   Zwei Screendumps (Autorun-Fenster / Desktop danach).
2. Ein `.NAP`, das `shel_write(SHW_EXEC, 1, 1, "C:\\HELLOGEM.NAP", "")` und dann
   `Pterm` macht, startet das nächste Programm — statt still ignoriert zu werden.
3. „Execute EmuCON" (nachdem der Item-Dispatch repariert ist, §5.8/15) landet
   im `CONSOLE_APP`-Arm; Screendump der Konsole und des Desktops danach.
4. Doppelklick auf `HELLO2.NCP` läuft über `sh_ldapp` statt über
   `launch::run_program_with_tail`.
5. `fsck_msdos` auf dem SD-Image sauber; beide Boards bauen; Boot-Screendumps
   ohne Regression.

**Vorhersage.** Der erste Bruch ist der **Bildschirmzustand zwischen den
Programmen**: `sh_draw(CLEAR_SCREEN)` vor `sh_ldapp` und `wm_new` danach müssen
in exakt EmuTOS' Reihenfolge stehen, sonst bleiben Desktop-Fenster hinter der
neuen App stehen — das Symptom, gegen das `desktop_relinquish` gebaut wurde
(F5). Zweite Vorhersage: der `AUTORUN_APP`→`DESKTOP_APP`-Übergang
(`set_default_desktop`, `gemshlib.c:583-589`) wird beim ersten Anlauf fehlen und
sich als „nach dem Autorun kommt kein Desktop" zeigen.

**Rückweg.** `lib.rs:329` auf `desk::run()` zurück (eine Zeile).

---

### S4 — Geteilte Typen nach `tos-types`

**Abhängigkeit.** keine. **Parallel zu** S1, S2.

`Object` (`tos-api/src/aes/object.rs:31`), `TedInfo` (`:449`), `IconBlk`
(`:478`) plus die `G_*`/`OB_*`/`WF_*`/`MU_*`-Konstanten nach `tos-types`;
`tos-api` re-exportiert, damit **kein einziger** heutiger Aufrufer sich ändert.
`ObjTree` bleibt serverseitig.

**Überprüfbares Ende.** `cargo build` beide Boards; `cargo xtask test`
(`tos-types` hat Host-Tests); Layout-Assertions (`size_of::<Object>() == 32`
LP64) als `const _: () = assert!(…)` in `tos-types`; Boot-Screendump identisch.

**Vorhersage.** Reine Verschiebung, kein Verhaltensdelta. Wenn doch eines
auftaucht, liegt es an einem `#[repr]`-Attribut, das beim Verschieben verloren
geht — die Layout-Assertions fangen genau das.

---

### S5 — Das Desktop-Crate: Umzug der 9234 Zeilen

**Abhängigkeit.** S2 (Substrat), S4 (Typen), S1c+S1d (Trap-Oberfläche).
**Parallel zu** S3.

**Was passiert.** §3.6 Datei für Datei. Die 56 Call-Sites aus §5 werden
Trap-Aufrufe oder verschwinden. `desk/mod.rs`s aesbind (~500 Z.) ist bereits in
S2 nach `newtos-gem` gezogen. `run()` wird `deskmain()` in der EmuTOS-Reihenfolge
`deskmain.c:1950-2073`:

```
dos_gdrv → appl_init → graf_handle → wind_get(DESKWH,WF_WXYWH)
→ wind_update(BEG) → desk_busy_on → wind_update(END)
→ app_start  (mit shel_get!)  → win_start → desk_verify
→ menu_icheck ×2 → app_blddesk → wind_set(WF_NEWDESK)
→ wind_update(BEG) → cnx_get → wind_update(END)
→ men_update → wind_update(BEG) → menu_bar(tree,1) → desk_busy_off
   → wind_update(END)
→ Schleife: evnt_multi | wind_update(BEG) | hndl_kbd/hndl_button/hndl_msg
   | wind_update(END)
→ Epilog: cnx_put, app_save(FALSE) [shel_put!], menu_bar(NULL,0),
   wind_set(WF_NEWDESK,NULL,0,0), appl_exit, Pterm
```
Neu gegenüber heute: `wind_update`-Klammern (heute gar nicht gerufen [V]),
`graf_handle` statt `CELL_W/H`, `shel_get`/`shel_put` in `app_start`/`app_save`,
der Epilog. Weg: `launch_native`/`run_gem_app`/`desktop_relinquish`/
`desktop_resume`/`console_pause_for_key` → `pro_run`/`pro_exec`.

Zusätzlich: `desk::ftype::classify`-Ersatz im Kernel für den AUTO-Ordner;
`dir.rs`/`inf.rs`-Selftests als EL0-Testprogramm.

**Überprüfbares Ende.**
1. `cargo xtask build-programs` erzeugt `NEWDESK.NAP`; die ELF-Bedingungen wie
   in S2; **Image- und Heap-Größe gemessen und berichtet** gegen 4 MiB.
2. Die Driftprobe (§7.2) meldet **0**.
3. Der Korrespondenz-Report (§7.3) listet die `deskmain.c`/`deskapp.c`/… -
   Funktionsnamen; Spalte „erfunden" ist im Bericht aufgeführt.
4. `NEWDESK.NAP` wird **noch nicht** gestartet; Kernel + Boot unverändert,
   Screendumps identisch, beide Boards bauen.

**Vorhersage.** Der teuerste Einzelposten ist nicht die Menge, sondern
`app_start`/`app_save`: heute liest `inf.rs` direkt die Datei
(`inf.rs:71-120`), künftig zuerst `shel_get` und nur bei ungültiger Signatur die
Datei (`deskapp.c:832-836`) — mitsamt dem 128-Byte-`CPDATA`-Präfix. Zweite
Vorhersage: `global.rs`s `SpinMutex`→`RefCell`-Umbau erzeugt Borrow-Konflikte
an den Stellen, wo heute `global::with(|g| …)` verschachtelt aufgerufen wird.

**Rückweg.** Neues Verzeichnis, nirgends verdrahtet.

---

### S6 — Umschalten

**Abhängigkeit.** S3 **und** S5.

`sh_ldapp`s `DESKTOP_APP`-Arm ruft `Pexec(PE_LOADGO, "NEWDESK")` (Registry).
Der alte Pfad bleibt hinter dem Cargo-Feature `desk-in-kernel` erreichbar;
Default **zunächst alt**, Umschaltung erst nach grünem Tester-Durchlauf.

**Überprüfbares Ende.**
1. Boot bis Desktop, QMP-Screendump beider Boards — visuell identisch zum
   in-kernel-Desktop (Menüleiste, Laufwerksicons, `C:`-Fenster).
2. Fenster öffnen/verschieben/schließen, Menü, Doppelklick auf `HELLO2.NCP` und
   auf `QED.NAP` — **auf realer Pi4-HW**, weil interaktiv + `C:`-Storage nötig
   (Memory `desktop-interactive-verification-real-hw`).
3. Nach App-Ende steht der Desktop mit **denselben** Fenstern wie vorher — jetzt
   aus `shel_get`/`cnx_get` rekonstruiert, nicht aus `restore_windows`.
4. `NEWDESK.INF` wird nach „Save Desktop" korrekt geschrieben; `fsck_msdos`
   sauber.
5. Beide Boards bauen; Feature-Flag zurück → alter Zustand.

**Vorhersage.** §4. Zusätzlich: nach dem ersten App-Start sind die
Fensterpositionen **falsch oder weg**, weil `cnx_put`/`app_save` die `#W`-Zeilen
noch nicht vollständig durch den Shell-Puffer trägt. Das ist der Beweis, dass
der Kanal aus §2.4 wirklich der Kanal ist — und der erste Ort zum Nachsehen.
Zweite Vorhersage: der Mauszeiger ist nach dem Desktop-Neustart an einer
anderen Stelle als vorher, weil `ratinit()` ihn setzt und wir das bisher im
Desktop taten.

**Rückweg.** Ein Feature-Flag.

---

### S7 — Prozessidentität: `ap_id 0`

**Abhängigkeit.** S6 grün. Vorher **nicht** machbar: solange der in-kernel-
Desktop während der App weiterlebt, wäre ein geteilter AESPD echt kollidierend
(AUD-P1-004b). Erst wenn der Desktop *terminiert*, ist EmuTOS' Modell korrekt.

**Was gebaut wird.**
- `current_pid()` (`tos-api/src/aes/appl.rs:191-201`) liefert den **AESPD-Index**:
  0 = dos-appl (Desktop **oder** gestartete App), 1 reserviert für SCRENMGR,
  2+ ACC (`geminit.c:875-876,888`; `struct.h:36`).
- `p_nameit`/`p_setappdir` auf dem laufenden AESPD beim Start
  (`gemshlib.c:604-605,634-635`; `gempd.c:77-102`) — schließt zusätzlich
  **AUD-P1-001** (`appl_find` liefert heute nie einen Namen).
- `save_windows`/`restore_windows` **löschen** (`window.rs:631-666`),
  `wind_new` an den EmuTOS-Stellen.
- `aes::window::redraw()`-Aufrufer auflösen.

**Überprüfbares Ende.**
1. AES-ABI-Trace eines desktop-gestarteten QED: `global[2] == 0` — **D-R1-5
   geschlossen** (`docs/analysis/aes-r1-findings.md:558`).
2. Zusammen mit S1b: `global[3..4]` = `0x1143` — **D-R1-6 bestätigt**
   (`:559`), diesmal am *gestarteten* Programm, dem Fall, der zählt.
3. `appl_find("QED")` liefert die id statt −1.
4. `wind_get(handle, WF_OWNER)` bzw. Message-Routing behandelt 0 als gültige
   id — Regressionstest über Fenster-Move/Top/Close auf realer HW.
5. Beide Boards bauen; Boot-Screendumps ohne Regression.

**Vorhersage.** Genau **eine** Stelle wird `0` als „kein Besitzer" auslegen.
Kandidaten: `window.rs:1619`, `:1905` (`w.owner = current_pid()`), `:936`,
`:2653`, `:2771` (`msg[1] = current_pid()`). Zweite Vorhersage: `pd::open(0)`
kollidiert mit `Aespd::EMPTY { pid: 0, in_use: false }`
(`tos-api/src/aes/pd.rs:253-260`) — die Suche `s.in_use && s.pid == pid` ist
zwar korrekt geschrieben, aber jede Stelle, die `pid == 0` als „ungesetzt"
prüft, bricht.

**Rückweg.** Die pid-Ableitung ist eine Funktion; zurückdrehen ist ein Commit.

---

### S8 — Löschen (Pflichtstufe)

**Abhängigkeit.** S7 grün, vom Tester bestätigt.

- `kernel/src/desk/` (9234 Z.) löschen, `pub mod desk;` (`lib.rs:61`) raus.
- Feature `desk-in-kernel` raus.
- `kernel/src/launch.rs`: `run_program_with_tail`s Desktop-Hälfte raus (die
  AUTO-Ordner-Funktion bleibt).
- Legacy `crate::desktop` / `crate::desktop_icons` (laut `mod.rs:5-7` „im Baum,
  aus dem Build") mit prüfen und, falls tot, mit löschen.

**Überprüfbares Ende.**
1. `grep -rc 'crate::desk' kernel/src/ == 0`; `kernel/src/desk/` existiert nicht.
2. Delta der Kernel-Binärgröße im Bericht (beide Boards).
3. Voller Regressionslauf: Boot + `svc`-Selftests + Screendumps + `fsck_msdos`,
   beide Boards; interaktiv auf realer Pi4.
4. Die Driftprobe aus §7.2 ist ab hier strukturell erzwungen, nicht mehr nur
   gemessen.

**Vorhersage.** Es tauchen 2–4 Kernel-Stellen auf, die noch auf `desk::` zeigen
und beim Umzug übersehen wurden — `launch.rs:137` (`ftype`) ist die einzige, die
ich heute sicher benennen kann; `initinfo`/`fbcon` sind Kandidaten.

---

### S9 — Nachzügler (nach dem Umzug, unabhängig)

- Desktop-eigene Resource (`desk_rs_init`/`desk_xlate_fix`,
  `deskmain.c:1979-1990`) als `.NRSC` (ADR-013).
- `EMUCON` als eigenes Registry-Programm statt Kernel-Funktion
  (`gemshlib.c:615-622` = `aes_run_rom_program(coma_start)`).
- `do_deskmenu`-Item-Dispatch + „Execute EmuCON" ins **File**-Menü
  (`deskmain.c:751-753`).
- `gl_wschar`/`gl_hschar` zusammen mit dem 6×6-Font (Tracker C1/E3) — **[STOP-2]**.
- ACC-Laden (`\*.ACC`, AESPD 2+) — erst hier sinnvoll, weil dann echte
  Nebenläufigkeit im AESPD-Modell nötig wird.

---

## 9. Was ich nicht ermitteln konnte

Offen und ausdrücklich **nicht** geraten:

1. **[A] Ob das Desktop-Programm in die 4-MiB-TPA passt.** Rechnung in §3.3
   (≈ 90 KB Arena + Vec-Daten + geschätzt 200–400 KB Code). **Nicht gemessen** —
   es existiert kein Rust-EL0-Programm, an dem man Codegröße kalibrieren könnte.
   S2 misst es an `hello-gem`, S5 an `NEWDESK.NAP`. Falls es nicht passt, ist
   `TPA_SIZE` (`arena.rs:52`) eine Konstante — aber die Wechselwirkung mit dem
   Pool-Allokator habe ich nicht analysiert.
2. **[A] Ob `MAX_USER_BLOCKS = 32` (`usermem.rs:80`) reicht.** Meine
   Allocator-Empfehlung (TPA-Heap) umgeht die Grenze; falls der Desktop doch
   `Malloc`-Blöcke im EmuTOS-Stil braucht (`G.g_cnxsave`, `g_wlist`, `g_screen`,
   `g_iblist`, `g_alist`, `g_atext`, `desk_rs_obj`, `desk_rs_strings` ≈ 8–10),
   passt es — aber ich habe die Zahl nicht gegen einen realen Lauf geprüft.
3. **Ob der blockierende `evnt_multi`-Pfad sich für einen EL0-Desktop genauso
   verhält wie heute für den EL1-Aufrufer.** Der S2-Rückgabepfad
   (`sched::SleepOutcome::Yield`, `tos-api/src/aes/event.rs:299-435`) ist für
   EL0-Clients (QED) im Einsatz, aber der Desktop ist der erste EL0-Client, der
   *dauerhaft* darin sitzt. Erster Messpunkt: `hello-gem` in S2.
4. **`gl_wschar`/`gl_hschar` ohne AES-Aufruf** — **[STOP-2]**, Researcher-Frage,
   an C1/E3 gekoppelt.
5. **Die Schichtfrage für `sh_ldapp`** — **[STOP-1]**, Lead-Entscheidung.
6. **`console_pause_for_key`** — **[STOP-3]**: streichen (meine Empfehlung, weil
   ohne EmuTOS-Gegenstück) oder als benannte Abweichung in `docs/api/` eintragen.
7. **Ich habe in diesem Job keinen Build gefahren.** Der `target/`-Baum wird mit
   parallelen Aufgaben geteilt und der Board-Footgun (Memory
   `cargo-xtask-board-target-footgun`) macht einen Zwischenbuild zu einem
   Risiko für andere. „Der Baum baut heute sauber auf beiden Boards" ist damit
   **nicht** von mir gemessen, sondern aus dem Baumzustand angenommen.
8. **Ob `Dgetdrv`/`Dsetdrv`/`Dsetpath` im GEMDOS-Dispatcher vollständig sind**
   (für `sh_chdef`). **[A]** vorhanden; nicht einzeln nachgeprüft.

---

## 10. Antwortübersicht zu den sieben Fragen

| # | Frage | Antwort | §|
|---|---|---|---|
| 1 | Wie wird ein EL0-Programm zur Shell? | Registry-Blob + `Pexec(PE_LOADGO)`; es fehlen 10 Bausteine, allen voran der Shell-Loop, `#Z` und der `pending_launch`-Konsument (P6-005) | 3.2 |
| 2 | Wo lebt der Desktop? | Eigenes Crate in `programs/`, ELF-Blob im Kernel-Image, Start über die Registry — EmuTOS' `aes_run_rom_program` | 3.3 |
| 3 | Die Resource? | Zwei Dinge: der **AES**-`DESKTOP`-Baum gehört hierher (S1b, schließt D-R1-6), die **Desktop**-Resource nicht (S9) | 3.4 |
| 4 | Die Bypasses? | 56 Call-Sites, 21+8 Symbole, einzeln in §5; 15 versteckte Arbeitspakete in §5.8 | 5 |
| 5 | Was die AES braucht? | Keine neuen Opcodes; fünf Server-Korrekturen + Löschung von `save/restore_windows` | 3.5 |
| 6 | Was aus 9234 Z. wird? | ≈93 % Umzug, ≈5 % mechanisch, ≈2 % Löschung, ~500 Z. ins SDK-Crate | 3.6 |
| 7 | Was bricht, was ist der Rückweg? | An jeder Grenze bootbar; S6 ist ein Feature-Flag; drei benannte Bruchkandidaten | 4 |

---

## 11. Entwurf ADR-023

> **ADR-023: Der Desktop ist ein EL0-Programm; der Shell-Loop gehört der AES**
> **Status:** Entwurf (Lead-Entscheidung ausstehend). Löst ADR-022s „Endgame" ein.
>
> **Entscheidung.** `kernel/src/desk/` wird durch ein eigenständiges natives
> LP64-`no_std`-Rust-Programm (`programs/desktop` → `NEWDESK.NAP`) ersetzt, das
> als ELF-Blob im Kernel-Image liegt und über die Programm-Registry als
> regulärer EL0-Prozess gestartet wird (EmuTOS `aes_run_rom_program`,
> `aes/gemshlib.c:570-580,606`). Der Desktop hat danach **keine** Privilegien,
> die QED nicht auch hat.
>
> Der Shell-Loop (`sh_main`/`sh_ldapp`/`sh_chdef`/`sh_chgrf`/`sh_draw`) gehört
> der AES/dem Kernel, nicht dem Desktop — wie in EmuTOS, wo er in
> `aes/gemshlib.c` liegt. Der Desktop startet nichts selbst: er ruft
> `shel_write` und terminiert (`desk/deskpro.c:36-63`,
> `desk/deskmain.c:2154-2166`).
>
> Desktop und gestartete Anwendung teilen **einen** AESPD (Index 0,
> `aes/geminit.c:875-876`, `aes/struct.h:36`), sequenziell, weil der Desktop vor
> dem Start der Anwendung terminiert. `appl_init` liefert beiden `ap_id 0`
> (`aes/gemsuper.c:94`). `ap_private` kommt aus dem **AES-eigenen**
> `rs_trees[DESKTOP][ROOT].ob_spec` (`aes/gemsuper.c:95-96`,
> `aes/gem_rsc.c:551-556`), nicht aus dem per `WF_NEWDESK` registrierten Baum.
> Der Desktop-Zustand überlebt seinen eigenen Tod im AES-Shell-Puffer
> (`app_save→shel_put` / `shel_get→app_start`, `desk/deskapp.c:832,1293`);
> `save_windows`/`restore_windows` entfallen zugunsten von `wm_new`
> (`aes/gemwmlib.c:1875-1893`).
>
> **Begründung.** (a) Zwei auf einem TT gemessene Defekte (D-R1-5, D-R1-6) sind
> ohne dieses Modell nicht ehrlich behebbar. (b) Es ist der dritte Rebuild-
> Anlauf; die ersten beiden sind abgedriftet, weil jede Abkürzung ein `crate::`
> weit weg war. Als eigenes Crate ohne `kernel`/`tos-api`-Abhängigkeit wird der
> Bypass zum Compilerfehler statt zum Review-Befund. (c) Der Desktop wird von
> demselben Trace-Instrument beobachtbar wie QED und damit gegen EmuTOS
> diffbar.
>
> **Konsequenzen.** `kernel/src/desk/` **wird gelöscht** (Stufe S8; der Plan
> gilt bis dahin als offen). `programs/` wird ein zweiter Cargo-Workspace mit
> einem SDK-Crate `newtos-gem`. `Object`/`IconBlk`/`TedInfo` ziehen nach
> `tos-types`. Fünf AES-/BIOS-Server-Korrekturen (S1) sind Voraussetzung.
> XBIOS `Keytbl` und per-Prozess-`p_xdta` werden nachgeholt. Der `EMUCON`-Pfad
> und `#Z` werden erstmals ausführbar.
>
> **Verweise.** ADR-022 (1:1-Rebuild durch den AES-Trap — dies löst dessen
> Endgame ein), ADR-021 (EL0-Speicherfenster), ADR-018 (`.NAP`),
> ADR-013 (`.NRSC`, für S9), `docs/analysis/aes-r1-findings.md`
> (D-R1-5/D-R1-6), `docs/analysis/aes-audit/p1-appl-pd-dispatch.md`
> (AUD-P1-001/004 — dessen 1:1-Fix-Empfehlung zu (b) hier korrigiert wird).
