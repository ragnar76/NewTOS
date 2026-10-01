# DESK19 — STATUS: DONE (2026-09-23)

**Auftrag:** `fun_win2win`s Zweig „Ablegen auf ein Programm" nachbauen,
`build_selected_path` mit, dann die Radiogruppe *Parameter* in `inf_conf`
nachziehen.

Basis: Branch `desk-rebuild`, HEAD `9a85fd8`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurements/measurement-2026-09-23-desk19-desk20/`](../measurements/measurement-2026-09-23-desk19-desk20/).
Spezifikation: `docs/api/desktop/deskfun.md` (§ *Ablegen auf ein Programm*),
`docs/api/desktop/deskinf.md` (§ *Parameter* ist zurueck),
`docs/api/desktop/deskmain.md` (§ `CSAVE`).

---

## 1. Die Praemisse, gegen die Quelle geprueft

`references/emutos/desk/deskfun.c:942-1001`:

```c
pda = i_find(dst_wh, dst_ob, &pdf, NULL);
if (pda) {
    if (pda->a_aicon >= 0) {          /* dropping file on to an application */
        if (build_selected_path(&psw->w_pnode, destpath)) {
            exit_desktop = do_aopen(pda, TRUE, dst_ob,
                                    pdw->w_pnode.p_spec, pdf->f_name, destpath);
            return;                   /* <- nichts wird kopiert */
        }
    }
    datype = pda->a_type;
}
```

und `:903-928`:

```c
for (fn = pn->p_flist; fn; fn = fn->f_next) if (fnode_is_selected(fn)) break;
if (!fn) return FALSE;
#if CONF_WITH_DESKTOP_CONFIG
if (G.g_fullpath) { strcpy(pathname,pn->p_spec); add_fname(pathname,fn->f_name); }
else
#endif
{ strcpy(pathname,fn->f_name); }
```

Beide `CONF_WITH_*` stehen im 512K-Profil auf 1
(`include/config.h:1774-1784`): `CONF_WITH_DESKTOP_CONFIG` fuer die
`G.g_fullpath`-Abfrage, `CONF_WITH_DESKTOP_SHORTCUTS` fuer den
`if (tail)`-Zweig, den `do_aopen` braucht, damit der gebaute Pfad ueberhaupt
ankommt (`desksupp.c:968-977`). Die Praemisse wird **nicht** zurueckgezogen.

Ein Detail, das EmuTOS' eigener Test ist und uebernommen wurde: entschieden
wird ueber die **Symbolnummer** `a_aicon`, nicht ueber die Dateiendung. Das
ist dieselbe Zahl, die `win_icalc` dem Fenster gibt, also stimmt der gewaehlte
Zweig immer mit dem Bild auf dem Schirm ueberein.

## 2. Was gebaut wurde

| Datei | Funktion | EmuTOS |
|---|---|---|
| `kernel/src/desk/dir.rs` | `build_selected_path` | `deskfun.c:903-928` |
| `kernel/src/desk/dir.rs` | `i_find_win` | `desksupp.c:1753-1770` (Fensterarm) |
| `kernel/src/desk/dir.rs` | `fun_win2win` (Zweig + Rueckgabe) | `deskfun.c:958-970` |
| `kernel/src/desk/dir.rs` | `fun_drag` reicht `exit_desktop` durch | `deskfun.c:1302` |
| `kernel/src/desk/main.rs` | `do_aopen_drop` | `desksupp.c:884-986`, `isapp`+`tail` |
| `kernel/src/desk/main.rs` | `cnx_put`/`cnx_get` `cs_fullpath` | `deskmain.c:1392`, `:1454` |
| `kernel/src/desk/inf.rs` | `Afind.pappl`/`.appdir`/`is_installed` | `deskins.c:71-80` |
| `kernel/src/desk/inf.rs` | `INF_E5_ISFULL` in `env_to_state`/`state_to_env` | `deskapp.c:943`, `:1165` |
| `kernel/src/desk/inf.rs` | *Parameter* in `inf_conf` | `desk_rsc.c:1641-1663`, `deskinf.c:1113-1116`, `:1227` |
| `kernel/src/desk/global.rs` | `DeskState::g_fullpath`, `CSave::cs_fullpath` | `deskbind.h:181`, `deskapp.h:154` |
| `sdk/examples/cmdline.c` (neu) | druckt `p_cmdlin` + `Dgetpath` | — (Abnahmeprogramm) |
| `tools/make-sd-image.sh` | installiert `C:\CMDLINE.NCP` | — |

## 3. Die Befehlszeile kommt an

Dieselbe Karte, dieselbe Geste (`C:\README.TXT` auf `C:\CMDLINE.NCP` im
`C:\`-Fenster), ein Radioknopf Unterschied:

```
Full path   [d19] build_selected_path(src="C:\", first="README.TXT", fullpath=true)  -> "C:\README.TXT"
            CMDLINE: len 13 arg "C:\README.TXT"
File name   [d19] build_selected_path(src="C:\", first="README.TXT", fullpath=false) -> "README.TXT"
            CMDLINE: len 10 arg "README.TXT"
```

Bilder: `screendumps/d19-full-c-cmdline.png`,
`screendumps/d19-file-e-cmdline.png`. Vorher (HEAD, gleiche Geste) kommt die
Kopierbestaetigung: `screendumps/d19-before-c-dropped.png`.

## 4. *Default dir* ist beobachtbar geworden

DESK18 hat gemessen, dass beide Stellungen dasselbe Verzeichnis waehlen, weil
EmuTOS sie nur ueber Desktop-Verknuepfungen (DESK30) und genau diesen Zweig
unterscheidet. Mit dem Zweig:

```
Application   CMDLINE: cwd C:\           (das ZIELfenster, `pathname`)
Window        CMDLINE: cwd C:\DESKT19    (das oberste Fenster, `win_ontop()`)
```

Zwei Fenster nebeneinander (`screendumps/probe3-c-moved.png`), Quelle oben.
`desksupp.c:896-905`. Bilder: `d19-defdir-app-b-cmdline.png`,
`d19-defdir-win-c-cmdline.png`.

## 5. Benannte Abweichungen

* **Geschachtelter Start statt Desktop-Austritt.** EmuTOS setzt
  `exit_desktop` und verlaesst den Prozess; NewTOS startet geschachtelt
  (AES-Luecke G1, `main::launch_native`), also gibt `do_aopen_drop` `false`
  zurueck. Dieselbe Abweichung wie beim Doppelklick und beim Dokumentstart.
* **Zwei Antworten mehr als `do_aopen`.** NewTOS' Lader nimmt nur native
  AArch64-ELF (ADR-018): ein `.PRG`-Ziel bekommt die vorhandene
  m68k-Emulator-Meldung, ein Ziel ohne native Endung „This file is not a
  native NewTOS program". Der zweite Fall ist genauso erreichbar wie EmuTOS'
  eigene Eigenheit: eine installierte Anwendung traegt `a_aicon = IG_APPL`
  **und** ihre Dokumentmaske, und `app_afind_by_name` prueft `a_pdata` zuerst
  (`deskapp.c:1450-1454`) — EmuTOS wuerde das Dokument selbst starten.
  Interaktiv nicht gefahren.
* **Nur die erste Auswahl** erreicht die Befehlszeile, egal wie viele Dateien
  gezogen wurden. Das ist EmuTOS' eigene Schleife (ein `char destpath[]`).

## 6. Bau + Regression (geteilter Baum)

```
cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
cargo xtask build --board rpi4        tos-api 64 / kernel 94   (Basis 64/94)
cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
cargo xtask build --board qemu-virt   tos-api 64 / kernel 94   (Basis 64/94)

cargo clippy --features board-rpi4       tos-api 80 / kernel 125 (Basis 80/125)
cargo clippy --features board-qemu-virt  tos-api 80 / kernel 125 (Basis 80/125)
Treffer in kernel/src/desk/: 0

cargo run -p xtask -- test   938 passed / 0 failed / 1 ignored
```

`fsck_msdos -n`: Phase 1/2/3 sauber auf allen vierzehn Abbildern
(`serial/fsck.txt`). Keiner dieser Laeufe schreibt auf die Karte.

## 7. Offen, aus diesem Lauf gemeldet

* **Ein `[EXC]` in 7 Laeufen desselben Starts** —
  `enter_el0_with_resume_capture + 0x6c`, IRQ im Fenster zwischen
  `msr elr_el1` und `eret` (`kernel/src/arch/aarch64/user_entry.rs:209-216`).
  Betrifft **jeden** Aufrufer des Helfers und ist aelter als DESK19; siehe
  `RUNINFO.TXT` §7. Nicht gefixt (Architekturdatei, ausserhalb des Auftrags).
* `desk::inf::selftest` und `selftest_install` haengen weiterhin in **keinem**
  Bootpfad (Gedaechtnis `unwired-selftests-rot-silently`). Sie wurden
  mitgezogen und decken jetzt `INF_E5_ISFULL` und `funkey` ab, sind aber kein
  Beleg, weil sie nicht laufen.
