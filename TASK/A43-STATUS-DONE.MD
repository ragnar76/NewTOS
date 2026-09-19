# A43 — Quittung

**Status: ERLEDIGT** · 2026-09-19 · Verhaltensfix `menu_settings` (39), hostverifiziert

## Was getan wurde

1. `tos-api/src/aes/menu.rs` `menu_settings`: `if flag == 0 … else …` →
   `match flag { 0 => lesen, 1 => schreiben, _ => {} }`. Der `_`-Arm liest
   und schreibt nichts, wie `mn_settings` ohne `default`
   (`references/emutos/aes/gemmnext.c:596-616`, gepinnter Commit `1a7484ab`;
   `switch(flag)` `:598-615`). Die Negativ-Ignorierregel (`>= 0` je Feld) und
   `clamp_height` waren im `flag == 1`-Arm bereits vorhanden und sind
   unveraendert.
2. Derselbe `settings`-Block, Defaults angeglichen (A20-Nebenbefund, belegt):
   `display 0 / drag 0 / delay 250 / speed 10 / height 19` →
   `200 / 10000 / 250 / 0 / 16` = `mnext_init` (`gemmnext.c:103-110`) mit
   `INIT_*_VALUE` (`:71-75`). `mnext_init` klemmt `height` per
   `clamp_height`; der obere Rand (`gl_rfull.g_h / gl_hchar`) greift erst
   unter ~275 Bildzeilen, kein NewTOS-Profil — statisch 16 ist auf allen
   Profilen der Wert, den `clamp_height(16)` liefert. Der Block hat im Kernel
   genau einen Leser (`menu_settings` selbst; `/usr/bin/grep -rn SETTINGS
   tos-api/src`), TeraDesk ruft `menu_settings` nicht auf (`sdk/apps/teradesk`
   ohne Treffer) — keine Zeichen-/Zeitwirkung.
3. Spec `docs/api/aes/menu.md` § `menu_settings`: No-op-Satz fuer
   `flag ∉ {0, 1}`, oberer Klemmrand und die Anfangswerte nachgetragen.
4. Neuer Hosttest `tos-api/tests/aes_menu_settings.rs` (Dispatcher-Harness
   `aes_reference/harness.rs`, LP64): Defaults; `flag = 1` schreibt;
   `flag ∈ {2, -1, 3, MAX, MIN}` mit einem Block, der alle Felder aendern
   wuerde → `flag = 0` liest die unveraenderten Werte und der uebergebene
   Block bleibt unberuehrt; `flag = 1` mit negativen Feldern aendert nichts
   (ganz und gemischt); `height` 0/1/4 klemmt auf 5; Rueckgabe stets 1.

## Pruefung

* Test vor Fix rot: zuerst an den Defaults (`display 0, drag 0, delay 250,
  speed 10, height 19` statt `200, 10000, 250, 0, 16`); nach dem
  Defaults-Schritt allein rot am Befund A-2: `menu_settings(2)` mit
  `1002/2000/3000/4000/6` → `menu_settings(0)` las `1002/2000/3000/4000/6`
  statt `111/222/333/444/10`. Nach dem Selektor-Fix gruen.
* `cargo run -p xtask -- test`: **633** gruen / 0 rot / 1 ignoriert
  (Grundlinie 631 + die zwei neuen Tests A43/A44).
* `cargo xtask build --board rpi4 --release` und `--board qemu-virt --release`
  (je frisch, `Compiling tos-api` + `Compiling kernel`): Exit 0, `tos-api` 64 /
  `kernel` 94 Warnungen, Delta 0. Clippy `-p kernel --lib` je Board:
  `tos-api` 81 / `kernel` 125, Delta 0 (der Treffer `menu.rs:1155` ist die
  Grundlinien-Warnung „`Aespb` more private than `menu_settings`", vorher
  `:1110`).
* Kein QEMU-Lauf: kein Zeichen- und kein Ladepfad betroffen (s. o., ein
  Leser). Keine Hardwareabnahme noetig; der Tester kann `aes_menu_settings`
  als Regressionsfall fuehren.

## Nebenbefunde (unberuehrt)

* `clamp_height` oberer Rand: NewTOS `backend.height() / CELL_H` (volle
  Bildhoehe, 16 fest) gegen EmuTOS `gl_rfull.g_h / gl_hchar` (Bild minus
  Menueleiste, Fonthoehe der Workstation) — 30 statt 28 bei 640×480; greift
  nur bei `height > 28`.
* `sdk/gemlib/overlay/mt_gem.h.in:1257-1264` `MN_SET` hat auf LP64
  `long`-Felder (8 Byte); der Kernel liest `MnSet` mit `i32`. Ein natives
  LP64-Programm und der Kernel legen den Block verschieden aus.
* `menu_settings` mit `addr_in[0] == 0` antwortet 0; `crysbind` liesse
  `ret = TRUE` (EmuTOS prueft nicht und wuerde bei `flag = 0` nach 0
  schreiben). Randfall ohne App-Wirkung.
