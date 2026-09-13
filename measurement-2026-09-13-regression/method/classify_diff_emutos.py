import sys, re

path = sys.argv[1]
# Known ap_id/ap_private/ap_planes/kernel-address/ap_3resv family, documented
# as expected NewTOS-vs-EmuTOS noise in every AES table (global[2]=ap_id,
# global[3]/[4]=ap_private half-words swapped, global[10]=ap_planes 16 vs 4,
# global[11]=kernel address, global[12]=ap_3resv uninitialised stack).
noise_globals = {"global[2]", "global[3]", "global[4]", "global[10]", "global[11]", "global[12]"}

with open(path, encoding="utf-8", errors="replace") as f:
    lines = f.readlines()

blocks = []
cur_header = None
cur_fields = []
for line in lines:
    if line.startswith("case "):
        if cur_header is not None:
            blocks.append((cur_header, cur_fields))
        cur_header = line.rstrip("\n")
        cur_fields = []
    elif line.strip().startswith(tuple(["global[", "int_out", "int_in", "addr_out", "addr_in", "control", "contrl"])):
        m = re.match(r"\s*(\S+)", line)
        if m:
            cur_fields.append((m.group(1), line.rstrip("\n")))
if cur_header is not None:
    blocks.append((cur_header, cur_fields))

real = []
for header, fields in blocks:
    non_noise = [f for f in fields if f[0] not in noise_globals]
    if non_noise:
        real.append((header, non_noise))

print(f"total case-blocks with any diff: {len(blocks)}")
print(f"case-blocks with a NON-noise field: {len(real)}")
for header, fields in real:
    print(header)
    for name, line in fields:
        print("   ", line.strip())
