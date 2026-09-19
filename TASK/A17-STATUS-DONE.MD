# A17 — Quittung

**Status: ERLEDIGT** · 2026-09-19 · reine Spec-/Belegkorrektur, kein Verhalten

## Was getan wurde

1. `docs/api/aes/window-graphics.md` § `wind_open`: „Globals written" nennt
   jetzt den **Wert** des PREV-Rechtecks — das uebergebene Rechteck, weil
   `wm_open` nach `draw_change` (`:1426`) noch `w_setsize(WS_PREV, w_handle,
   pt)` ausfuehrt (`references/emutos/aes/gemwmlib.c:1427`, gepinnter Commit
   `1a7484ab`; `w_setsize` = `rc_copy`, `:265-268`) — und die beobachtbare
   Folge (`WF_PXYWH` == `WF_CXYWH` direkt nach `wind_open`, TT-Fall 253, A12).
2. Zeilenbelege `:1426` → `:1427`, am gepinnten Stand nachgelesen
   (`sed -n 1424,1429p`): `tos-api/src/aes/window.rs:2359`,
   `tos-api/tests/aes_reference.rs:152` und `:220` (das Ticket nannte zwei
   Stellen; es sind drei, die Zeilen waren seit dem 10.09. gewandert).

## Pruefung

`cargo test -p tos-api --test aes_reference` unveraendert gruen (Kommentare
nur). Kein Produktcode, keine Hardwareabnahme.
