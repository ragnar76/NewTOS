# A05 — Quittung

**Status: ERLEDIGT — Versionsunterschied belegt, kein Reparaturauftrag** · 2026-09-19

## Was getan wurde

Nur Feld 20 / Fall 167 (`wind_get(0, WF_OWNER)`, `int_out[0]`: TT 0, Pi 1).
EmuTOS 1.4 und die gepinnte Quelle getrennt gelesen — nicht aus dem
Gedaechtnis, sondern per `git` im Clone `references/emutos`:

| | EmuTOS 1.4 (`VERSION_1_4` = `978e3756`, 2025-06-07) | gepinnt (`1a7484ab`) |
|---|---|---|
| `include/aesdefs.h` | endet bei `WF_DCOLOR 19` (`:551`), kein `WF_OWNER` | `WF_OWNER 20` (`:552`) |
| `aes/gemwmlib.c` `wm_get` | kein Arm, Feld 20 → `default:` → `FALSE` | `case WF_OWNER:` (`:1579`), vier Worte + `TRUE` |
| Einfuehrung | — | `acb7f3b4` 2025-08-23 „Add partial implementation of WF_OWNER to wind_get" |
| `git merge-base --is-ancestor acb7f3b4 VERSION_1_4` | **nein** — nach dem 1.4-Tag | |

Der TT (EmuTOS 1.4) antwortet also 0, weil das Feld dort nicht existiert; die
Pi antwortet 1, weil NewTOS die gepinnte Quelle implementiert. Beide
Maschinen melden `ap_version 0x0340`.

## DELTAS-Regel

`tos-api/tests/aes_reference/harness.rs`, Block `WF_OWNER` (ab `:1262`):
sagt genau das, mit Commit und Zeilen, und erwartet fuer Fall 167 den
Wert der gepinnten Quelle (1). **Bleibt wirksam**, nicht durch `ret=0`
ersetzt. Ein Detail korrigiert: die Regel behauptete, 1.4 ende bei
`WF_COLOR 18`; es ist `WF_DCOLOR 19`. Keine Verhaltensaenderung.

## Pruefung

`cargo test -p tos-api --test aes_reference` (Fall 167 weiter gruen gegen
die DELTAS-Erwartung). Kein Produktcode angefasst. Keine Hardwareabnahme
noetig — Versionsfrage, keine Geraetefrage.

## Offen

Nichts. Die alternative Erwartung (0, falls jemand 1.4-Verhalten will) steht
in der Regel dokumentiert; NewTOS folgt bewusst der gepinnten Quelle.
