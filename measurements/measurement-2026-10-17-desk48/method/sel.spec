# sel.spec -- DESK48 places 2 (desk_clear_all), 3 (fun_selectall) and the WM_TOPPED desk_clear(G.g_cwin) that gains its
# drawing half. Layout of NEWDESK.INF (cal-1-desk.png): W1 C:\ (80..352, top 32), W2 C:\ (368..640), W3 C:\SUB\ (80..352, top 256, topped).
park
move 200 42
click            # W1 title bar -> WM_TOPPED W1
wait 100
move 140 120
click            # README row in W1 -> selected
wait 200
mark a-selected
wait 200
move 500 400
click            # desktop surface: wind_find = DESKWH != G.g_cwin -> desk_clear_all
wait 200
mark b-cleared
wait 200
move 200 170
click            # empty area of W1 -> G.g_cwin = W1
wait 100
key 0x1D 0x1E 0x9E 0x9D   # Ctrl+A -> SLCTITEM -> fun_selectall
wait 200
mark c-selall
wait 200
move 500 42
click            # W2 title bar -> WM_TOPPED W2 -> desk_clear(G.g_cwin = W1)
wait 200
mark d-topped
wait 100
