# Funktionsabdeckung der aktuellen Runde

Gezählt werden nur aufgezeichnete Fälle der aktuellen Tabellen, keine Preludes, Resets oder impliziten Initialisierungen. Ein gemessener Opcode bedeutet keine vollständige Funktion-/Pixelabdeckung. Ältere TT/Pi-Fensterläufe ersetzen die fehlende aktuelle Runde nicht.


## AES

| Opcode | Funktion | Aktuell gemessener Record | Task bei fehlendem Record |
|---|---|---|---|
| 10 | `appl_init` | Nein | [A06](A06.md) / [A07](A07.md) / [A08](A08.md) / [A09](A09.md) |
| 11 | `appl_read` | Nein | [G-AES-011](G-AES-011.md) |
| 12 | `appl_write` | Nein | [G-AES-012](G-AES-012.md) |
| 13 | `appl_find` | Nein | [G-AES-013](G-AES-013.md) |
| 14 | `appl_tplay` | Nein | [G-AES-014](G-AES-014.md) |
| 15 | `appl_trecord` | Nein | [G-AES-015](G-AES-015.md) |
| 17 | `appl_yield` | Nein | [G-AES-017](G-AES-017.md) |
| 19 | `appl_exit` | Nein | [G-AES-019](G-AES-019.md) |
| 20 | `evnt_keybd` | Nein | [G-AES-020](G-AES-020.md) |
| 21 | `evnt_button` | Nein | [G-AES-021](G-AES-021.md) |
| 22 | `evnt_mouse` | Nein | [G-AES-022](G-AES-022.md) |
| 23 | `evnt_mesag` | Nein | [G-AES-023](G-AES-023.md) |
| 24 | `evnt_timer` | Ja — Teilabdeckung | — |
| 25 | `evnt_multi` | Nein | [Q04](Q04.md) |
| 26 | `evnt_dclick` | Nein | [G-AES-026](G-AES-026.md) |
| 30 | `menu_bar` | Nein | [G-AES-030](G-AES-030.md) |
| 31 | `menu_icheck` | Nein | [G-AES-031](G-AES-031.md) |
| 32 | `menu_ienable` | Nein | [G-AES-032](G-AES-032.md) |
| 33 | `menu_tnormal` | Nein | [G-AES-033](G-AES-033.md) |
| 34 | `menu_text` | Nein | [G-AES-034](G-AES-034.md) |
| 35 | `menu_register` | Nein | [G-AES-035](G-AES-035.md) |
| 36 | `menu_popup` | Nein | [G-AES-036](G-AES-036.md) |
| 37 | `menu_attach` | Nein | [G-AES-037](G-AES-037.md) |
| 38 | `menu_istart` | Nein | [G-AES-038](G-AES-038.md) |
| 39 | `menu_settings` | Nein | [G-AES-039](G-AES-039.md) |
| 40 | `objc_add` | Nein | [G-AES-040](G-AES-040.md) |
| 41 | `objc_delete` | Nein | [G-AES-041](G-AES-041.md) |
| 42 | `objc_draw` | Nein | [G-AES-042](G-AES-042.md) |
| 43 | `objc_find` | Nein | [G-AES-043](G-AES-043.md) |
| 44 | `objc_offset` | Nein | [G-AES-044](G-AES-044.md) |
| 45 | `objc_order` | Nein | [G-AES-045](G-AES-045.md) |
| 46 | `objc_edit` | Nein | [G-AES-046](G-AES-046.md) |
| 47 | `objc_change` | Nein | [G-AES-047](G-AES-047.md) |
| 48 | `objc_sysvar` | Nein | [G-AES-048](G-AES-048.md) |
| 50 | `form_do` | Nein | [G-AES-050](G-AES-050.md) |
| 51 | `form_dial` | Nein | [G-AES-051](G-AES-051.md) |
| 52 | `form_alert` | Nein | [G-AES-052](G-AES-052.md) |
| 53 | `form_error` | Nein | [G-AES-053](G-AES-053.md) |
| 54 | `form_center` | Nein | [G-AES-054](G-AES-054.md) |
| 55 | `form_keybd` | Nein | [G-AES-055](G-AES-055.md) |
| 56 | `form_button` | Nein | [G-AES-056](G-AES-056.md) |
| 70 | `graf_rubbox` | Nein | [G-AES-070](G-AES-070.md) |
| 71 | `graf_dragbox` | Nein | [G-AES-071](G-AES-071.md) |
| 72 | `graf_mbox` | Nein | [G-AES-072](G-AES-072.md) |
| 73 | `graf_growbox` | Nein | [G-AES-073](G-AES-073.md) |
| 74 | `graf_shrinkbox` | Nein | [G-AES-074](G-AES-074.md) |
| 75 | `graf_watchbox` | Nein | [G-AES-075](G-AES-075.md) |
| 76 | `graf_slidebox` | Nein | [G-AES-076](G-AES-076.md) |
| 77 | `graf_handle` | Ja — Teilabdeckung | — |
| 78 | `graf_mouse` | Nein | [Q05](Q05.md) |
| 79 | `graf_mkstate` | Ja — Teilabdeckung | — |
| 80 | `scrp_read` | Nein | [G-AES-080](G-AES-080.md) |
| 81 | `scrp_write` | Nein | [G-AES-081](G-AES-081.md) |
| 82 | `scrp_clear` | Nein | [G-AES-082](G-AES-082.md) |
| 90 | `fsel_input` | Nein | [G-AES-090](G-AES-090.md) |
| 91 | `fsel_exinput` | Nein | [G-AES-091](G-AES-091.md) |
| 100 | `wind_create` | Nein | [M01](M01.md) / [A10](A10.md) |
| 101 | `wind_open` | Nein | [M01](M01.md) |
| 102 | `wind_close` | Nein | [M01](M01.md) |
| 103 | `wind_delete` | Nein | [M01](M01.md) |
| 104 | `wind_get` | Ja — Teilabdeckung | — |
| 105 | `wind_set` | Nein | [M01](M01.md) |
| 106 | `wind_find` | Ja — Teilabdeckung | — |
| 107 | `wind_update` | Nein | [G-AES-107](G-AES-107.md) |
| 108 | `wind_calc` | Ja — Teilabdeckung | — |
| 109 | `wind_new` | Nein | [M01](M01.md) |
| 110 | `rsrc_load` | Nein | [G-AES-110](G-AES-110.md) |
| 111 | `rsrc_free` | Nein | [G-AES-111](G-AES-111.md) |
| 112 | `rsrc_gaddr` | Nein | [G-AES-112](G-AES-112.md) |
| 113 | `rsrc_saddr` | Nein | [G-AES-113](G-AES-113.md) |
| 114 | `rsrc_obfix` | Nein | [G-AES-114](G-AES-114.md) |
| 120 | `shel_read` | Nein | [G-AES-120](G-AES-120.md) |
| 121 | `shel_write` | Nein | [G-AES-121](G-AES-121.md) |
| 122 | `shel_get` | Nein | [G-AES-122](G-AES-122.md) |
| 123 | `shel_put` | Nein | [G-AES-123](G-AES-123.md) |
| 124 | `shel_find` | Nein | [G-AES-124](G-AES-124.md) |
| 125 | `shel_envrn` | Nein | [G-AES-125](G-AES-125.md) |
| 126 | `shel_rdef` | Nein | [G-AES-126](G-AES-126.md) |
| 127 | `shel_wdef` | Nein | [G-AES-127](G-AES-127.md) |

## VDI

| Opcode | Funktion | Aktuell gemessener Record | Task bei fehlendem Record |
|---|---|---|---|
| 1 | `v_opnwk` | Nein | [G-VDI-001](G-VDI-001.md) |
| 2 | `v_clswk` | Nein | [G-VDI-002](G-VDI-002.md) |
| 3 | `v_clrwk` | Ja — Teilabdeckung | — |
| 4 | `v_updwk` | Ja — Teilabdeckung | — |
| 5 | `v_escape` | Ja — Teilabdeckung | — |
| 6 | `v_pline` | Ja — Teilabdeckung | — |
| 7 | `v_pmarker` | Ja — Teilabdeckung | — |
| 8 | `v_gtext` | Ja — Teilabdeckung | — |
| 9 | `v_fillarea` | Ja — Teilabdeckung | — |
| 10 | `v_cellarray` | Ja — Teilabdeckung | — |
| 11 | `v_gdp` | Ja — Teilabdeckung | — |
| 12 | `vst_height` | Ja — Teilabdeckung | — |
| 13 | `vst_rotation` | Ja — Teilabdeckung | — |
| 14 | `vs_color` | Nein | [G-VDI-014](G-VDI-014.md) |
| 15 | `vsl_type` | Ja — Teilabdeckung | — |
| 16 | `vsl_width` | Ja — Teilabdeckung | — |
| 17 | `vsl_color` | Ja — Teilabdeckung | — |
| 18 | `vsm_type` | Ja — Teilabdeckung | — |
| 19 | `vsm_height` | Ja — Teilabdeckung | — |
| 20 | `vsm_color` | Ja — Teilabdeckung | — |
| 21 | `vst_font` | Ja — Teilabdeckung | — |
| 22 | `vst_color` | Ja — Teilabdeckung | — |
| 23 | `vsf_interior` | Ja — Teilabdeckung | — |
| 24 | `vsf_style` | Ja — Teilabdeckung | — |
| 25 | `vsf_color` | Ja — Teilabdeckung | — |
| 26 | `vq_color` | Ja — Teilabdeckung | — |
| 27 | `vq_cellarray` | Ja — Teilabdeckung | — |
| 28 | `v_locator` | Nein | [G-VDI-028](G-VDI-028.md) |
| 29 | `v_valuator` | Ja — Teilabdeckung | — |
| 30 | `v_choice` | Ja — Teilabdeckung | — |
| 31 | `v_string` | Nein | [G-VDI-031](G-VDI-031.md) |
| 32 | `vswr_mode` | Ja — Teilabdeckung | — |
| 33 | `vsin_mode` | Ja — Teilabdeckung | — |
| 35 | `vql_attributes` | Ja — Teilabdeckung | — |
| 36 | `vqm_attributes` | Ja — Teilabdeckung | — |
| 37 | `vqf_attributes` | Ja — Teilabdeckung | — |
| 38 | `vqt_attributes` | Ja — Teilabdeckung | — |
| 39 | `vst_alignment` | Ja — Teilabdeckung | — |
| 100 | `v_opnvwk` | Ja — Teilabdeckung | [V14](V14.md) / [V15](V15.md) |
| 101 | `v_clsvwk` | Nein | [G-VDI-101](G-VDI-101.md) |
| 102 | `vq_extnd` | Ja — Teilabdeckung | — |
| 103 | `v_contourfill` | Ja — Teilabdeckung | — |
| 104 | `vsf_perimeter` | Ja — Teilabdeckung | — |
| 105 | `v_get_pixel` | Ja — Teilabdeckung | [V10](V10.md) / [V11](V11.md) / [V12](V12.md) |
| 106 | `vst_effects` | Ja — Teilabdeckung | — |
| 107 | `vst_point` | Ja — Teilabdeckung | — |
| 108 | `vsl_ends` | Ja — Teilabdeckung | — |
| 109 | `vro_cpyfm` | Ja — Teilabdeckung | [R01](R01.md) / [R02](R02.md) / [R03](R03.md) / [R04](R04.md) |
| 110 | `vr_trnfm` | Ja — Teilabdeckung | — |
| 111 | `vsc_form` | Ja — Teilabdeckung | — |
| 112 | `vsf_udpat` | Nein | [G-VDI-112](G-VDI-112.md) |
| 113 | `vsl_udsty` | Ja — Teilabdeckung | — |
| 114 | `vr_recfl` | Ja — Teilabdeckung | — |
| 115 | `vqin_mode` | Ja — Teilabdeckung | — |
| 116 | `vqt_extent` | Ja — Teilabdeckung | — |
| 117 | `vqt_width` | Ja — Teilabdeckung | — |
| 118 | `vex_timv` | Nein | [G-VDI-118](G-VDI-118.md) |
| 119 | `vst_load_fonts` | Nein | [G-VDI-119](G-VDI-119.md) |
| 120 | `vst_unload_fonts` | Nein | [G-VDI-120](G-VDI-120.md) |
| 121 | `vrt_cpyfm` | Ja — Teilabdeckung | [R05](R05.md) |
| 122 | `v_show_c` | Ja — Teilabdeckung | — |
| 123 | `v_hide_c` | Ja — Teilabdeckung | — |
| 124 | `vq_mouse` | Nein | [G-VDI-124](G-VDI-124.md) |
| 125 | `vex_butv` | Nein | [G-VDI-125](G-VDI-125.md) |
| 126 | `vex_motv` | Nein | [G-VDI-126](G-VDI-126.md) |
| 127 | `vex_curv` | Nein | [G-VDI-127](G-VDI-127.md) |
| 128 | `vq_key_s` | Nein | [G-VDI-128](G-VDI-128.md) |
| 129 | `vs_clip` | Ja — Teilabdeckung | — |
| 130 | `vqt_name` | Ja — Teilabdeckung | — |
| 131 | `vqt_fontinfo` | Ja — Teilabdeckung | — |
| 132 | `vqt_justified` | Ja — Teilabdeckung | — |
| 133 | `vs_grayoverride` | Ja — Teilabdeckung | — |
| 134 | `vex_wheelv` | Nein | [G-VDI-134](G-VDI-134.md) |

VDI-Quellinventar: `tosdump/cases/COVERAGE.md`; AES-Quellinventar: `docs/analysis/aes-audit/coverage-matrix.md`. Diese Inventare dienen der Namensliste; ihre alten Statusangaben wurden nicht übernommen. VDI escape/GDP-Unterfunktionen sind hier nur über ihren Hauptopcode erfasst; diese Matrix behauptet keine vollständige Subopcode-Abdeckung. QED-nahe Teilaufgaben stehen separat in Q01–Q05.
