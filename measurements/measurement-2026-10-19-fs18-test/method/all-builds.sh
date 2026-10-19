#!/usr/bin/env bash
# all-builds.sh -- fresh release rpi4 kernels head + wt for all variants used in this run (sequential)
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for s in head wt; do KERNELS="plain drn trn qsv s2s tdq dcm tcp dsk tdd" bash "$HERE/build-kernels.sh" $s; done
echo ALLBUILDS-DONE
