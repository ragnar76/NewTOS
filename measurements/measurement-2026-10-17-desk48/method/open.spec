# open.spec -- DESK48 regression basics: open a window from a disk icon, open a folder in a window (same window).
park
move 37 35
dclick           # Hard disk icon -> do_dopen -> new window C:\
wait 300
mark a-diskopen
wait 200
move 200 42
click            # W1 title -> top W1
wait 200
move 140 84
dclick           # SUB folder row in W1 -> do_fopen in the same window
wait 300
mark b-folderopen
wait 200
