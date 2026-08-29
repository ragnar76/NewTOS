# Makefile -- cross builds of the TOS reference dumper.
#
#   make dumpvdi-m68k     DUMPVDI.TOS for TOS 3.06 on the TT
#   make host-test        the host-side suite (same as make -C host test)
#
# The NewTOS/AArch64 build lands with task T4.

CROSS_MINT ?= /opt/cross-mint/bin/m68k-atari-mint-
M68K_CC    := $(CROSS_MINT)gcc

# 4 bytes of the git hash, ASCII, as the record header carries it.
BUILD_ID := $(shell git rev-parse --short=4 HEAD 2>/dev/null || echo ----)

# The m68k is big-endian, so the record order is the machine order: no swap.
M68K_CFLAGS := -std=c99 -O2 -Wall -Wextra -Werror -Icore \
               -DTD_BIG_ENDIAN_TARGET=1 -DTD_BUILD_ID='"$(BUILD_ID)"'

CORE_SRC := core/record.c core/cases.c core/gem.c core/runner.c core/dump.c
VDI_SRC  := $(CORE_SRC) core/dumpvdi.c

BUILD := build

.PHONY: all dumpvdi-m68k host-test clean
all: dumpvdi-m68k host-test

$(BUILD):
	@mkdir -p $(BUILD)

dumpvdi-m68k: $(BUILD)/DUMPVDI.TOS
$(BUILD)/DUMPVDI.TOS: $(VDI_SRC) abi/m68k_trap.c $(wildcard core/*.h) | $(BUILD)
	$(M68K_CC) $(M68K_CFLAGS) $(VDI_SRC) abi/m68k_trap.c -o $@
	@$(CROSS_MINT)size $@

host-test:
	@$(MAKE) --no-print-directory -C host test

clean:
	rm -rf $(BUILD)
	@$(MAKE) --no-print-directory -C host clean
