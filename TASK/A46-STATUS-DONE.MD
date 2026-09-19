# A46 — Quittung

**Status: DONE (Hostbeleg + QEMU-Abnahme; echte Pi 4 offen)** · 2026-09-19/20 ·
uncommitted im Baum (Basis `f65893a`). `TASK/A46.md`s Kopfzeile bleibt
unveraendert.

## Soll (gepinnte Quelle `1a7484ab`)

`aes/gemoblib.c:833-844`, String-Block von `just_draw`, unter
`CONF_WITH_NICELINES` (Default **1**, `include/config.h:1312-1314`; der
`0`-Default in `:319-320` gilt nur fuer die Minimalprofile):

```c
if ((obtype == G_STRING) && (state & DISABLED) && (len > 3) && is_dashes((char *)spec, len))
    gsx_cline(t.g_x, t.g_y+t.g_h/2, t.g_x+t.g_w-1, t.g_y+t.g_h/2);
else
    gsx_tblt(IBM, tmpx, tmpy, len);
```

| Frage | Quelle | Befund |
|---|---|---|
| `is_dashes` | `gemoblib.c:455-464` | `while(len--) if (*s++ != '-') return FALSE; return TRUE;` — `len` = `strlen` (`expand_string`, `util/optimopt.S:41-47`) |
| Geometrie | `:838-841` | Zeile `t.g_y + t.g_h/2`, von `t.g_x` bis `t.g_x+t.g_w-1` einschliesslich — **Objektbreite**, nicht Textbreite |
| `gsx_cline` | `gemgraf.c:188-195` | `v_pline(2, pxy)` zwischen `gsx_moff`/`gsx_mon`; setzt **keine** Attribute |
| Linienstil | `gemgraf.c:420-422` | `vsl_type(7)`, `vsl_width(1)`, `vsl_udsty(0xffff)` → durchgezogen, 1 px (`gsx_xline` stellt `0xffff` wieder her, `:162`; `add_3d_effect` ebenso, `gemoblib.c:226-227`) |
| Schreibmodus | `gemoblib.c:816` | `gsx_attr(TRUE, MD_TRANS, tcol)` unmittelbar davor → `MD_TRANS` |
| Linienfarbe | `gemgraf.c:75, 201-240` | `gsx_attr(TRUE, …)` setzt nur `gl_tcolor`; `gl_lcolor` bleibt der letzte `gsx_attr(FALSE, …)`-Wert. In der Menuefolge ist das BLACK vom Rand der Aufklapp-`G_BOX` (`gsx_attr(FALSE, MD_REPLACE, bcol)`, `:663`, `bcol` = 1 aus dem Farbwort `0x..1100`) |
| Stipple-Frage | `:917-929` | Die Linie wird **vor** dem Zustandsblock gezeichnet; `DISABLED` legt danach `bb_fill(MD_TRANS, FIS_PATTERN, IP_4PATT, …)` in WHITE ueber das ganze Objekt — die Linie wird wie die Striche gestippelt → gepunktete Linie |

**Versionsgatter:** `len > 3` kam mit `4b0d9518` (2025-08-20, „Refine
'nicelines' check", Uwe Seimets Einzelstrich-Dialogfall). `git merge-base
--is-ancestor 4b0d9518 VERSION_1_4` → **nein**. EmuTOS 1.4 (`etos512de.img`)
zeichnet also auch 1–3 Striche als Linie; NewTOS folgt der gepinnten Quelle
(`"---"` bleibt Text). In der Spec als Versionsdelta vermerkt (wie A05).

## Was gebaut wurde

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Fix | `tos-api/src/aes/object.rs` `just_draw`, `G_STRING`-Arm: `if ob_state & DISABLED != 0 && s.len() > 3 && is_dashes(s)` → `backend.draw_line(x, y+h/2, x+w-1, y+h/2, ink, Transparent, clip, 0xFFFF)` **statt** `gsx_tblt`; sonst unveraenderter Textpfad. Neu `fn is_dashes(&[u8]) -> bool`. Der bestehende DISABLED-Dither (`STIPPLE_50`, WHITE, transparent) liegt darueber wie bei EmuTOS. Linienfarbe = Tinte des Strings (BLACK; WHITE auf dem invertierten SELECTED-Grund) — NewTOS fuehrt kein klebriges `gl_lcolor`; in jeder erreichbaren Menuefolge ist das derselbe Wert | fertig |
| Test | `tos-api/tests/aes_object_nicelines.rs` (neu, `render_tree_lp64` = `objc_draw`-Rumpf, stiftindizierter Puffer): (a) DISABLED `"--------"` → Zeile `y+h/2` ueber die volle Objektbreite `x..=x+w-1` unter dem 50-%-Dither, keine Glyphenpixel, kein Pixel ausserhalb; (b) DISABLED `"---"` → Text (Gatter der gepinnten Quelle); (c) nicht-DISABLED `"--------"` → Text, kein Dither; (d) DISABLED `"-- -"` → Text | 4/4 |
| Spec | `docs/api/aes/object-form.md`: Callout nach Punkt 4 des `just_draw`-Durchgangs (Zeilen, Geometrie, Farbe/Modus, Dither, Versionsdelta) + Konfig-Hinweis `CONF_WITH_NICELINES` praezisiert (`len > 3`, Dither) | nachgezogen |

Menues gehen bei uns durch `draw_obj`/`draw_obj_clip` → `draw_tree` →
`just_draw` (TeraDesk-Tafeln `menu.rs:512/688/1051`, unser Desktop
`kernel/src/desk/menu.rs` mit `Sep(…)` → `ob_state |= DISABLED`, `:454`) —
der Fix greift ohne weitere Aenderung, siehe Screendumps.

## Geprueft (Belege in `measurement-2026-09-19-a46/`, Manifest `manifest.sha256`)

* **Rot vor / gruen nach** (`host-tests/red-before.txt`, `green-after.txt`):
  vor dem Hunk faellt genau (a) — Linienzeile 58 `._._._…` statt `.#.#.#…`,
  (b)(c)(d) gruen; danach 4/4.
* `cargo run -p xtask -- test`: **637 / 0 / 1** (Baseline 633 + 4);
  `aes_reference` 204/0, `aes_object_3d_box_border` 4/0,
  `aes_object_cursor_bracket` 3/0, `aes_object_font` 6/0 (`host-tests/xtask-test.log`).
* **Boards** (`build/`): `cargo xtask build --board rpi4` / `--board qemu-virt`
  rustc tos-api 64 / kernel 94, exit 0; Clippy tos-api 81 / kernel 125 (beide
  Boards). Baseline 64/94, 81/125 — **Delta 0**. Testdatei clippy-sauber.
* **QEMU raspi4b, Produktkernel** (`build/kernel8-a46-product-release-rpi4.img`,
  keine Instrumentierung): `screendumps/product1-tera-idle.png` **0 Pixel**
  Differenz zu `measurement-2026-09-19-a41/part2/screendumps/product1-tera-colour-icons-idle.png`;
  `screendumps/smoke-desk-up.png` **0 Pixel** zu `…/a41/part2/screendumps/smoke-desk-up.png`.
* **TeraDesk per `#Z` + Injektor** (Scratch-Kernel, `method/menu1.actions`:
  Zeiger auf *File* (88,8)): `tera-after-C-file-menu.png` — die drei Trenner
  der File-Tafel sind gepunktete Linien ueber die volle Tafelbreite (Zeile 139,
  88 Pixel x 64..239 im Wechsel; Glyphenzeilen 137/138 leer).
  `tera-before-C-file-menu.png` (derselbe Scratch-Kernel mit `object.rs` =
  HEAD): Bindestrich-Glyphen (137/138, je 68 Pixel). before/after: idle
  0 Pixel; Tafel 660 Pixel, bbox (64,137)–(239,220) = genau die drei
  Trennerzeilen. Stand vorher mit Fenster: `…/a41/part2/screendumps/inj1-C-file-menu.png`.
* **Unser Desktop** (sd-base, gleiche Sequenz; Injektor in der Scratch-Kopie
  zusaetzlich bei `[desk] desktop up` scharf): `desk-after-C-file-menu.png`
  Trenner als Linien (Zeilen 85/218/256, je 79 Pixel) statt Striche
  (`desk-before-C-file-menu.png`, 83+84/216+217/254+255); idle 0 Pixel,
  Tafel 576 Pixel, bbox (53,83)–(209,257).
* Alle sechs Serial-Logs: `grep -c "EXC\|panic"` = 0.

## Offen

* Echte Pi 4 (Tester/Lead): Tafel oeffnen, Trenner sichten — 1920×1080-Profil,
  gleiche Objekthoehe 16 → Linie bei `y+8`.
* `docs/STATUS.md` nicht angefasst (Scope-Vorgabe) — beim Commit nachziehen.
* Ausserhalb des Auftrags, nicht angefasst: SELECTED+DISABLED-Kombination
  (EmuTOS XORt nach dem Dither, NewTOS invertiert vorab) — betrifft Text wie
  Linie gleichermassen, in Menues unerreichbar (`menu_select` → `do_chg` mit
  `chkdisabled`, `gemmnlib.c:159-160, 196`, waehlt kein DISABLED-Objekt); das
  klebrige `gl_lcolor` wird nicht nachgebildet.
