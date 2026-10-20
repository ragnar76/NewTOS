# newtos.mk -- Makefile include for NewTOS programs (TASK/SDK01 D).
#
# Usage, in your project's Makefile:
#
#     PROG = HELLO                 # program name without extension (8.3!)
#     KIND = nap                   # ncp | ntp | nap | nac | nld   (default ncp)
#     SRCS = hello.c util.c        # .c / .S sources, relative to the Makefile
#     LIBS = -lcflib -lm           # SDK libraries (newtos-cc --help); libc always
#     RSC  = hello.rsc             # optional: resource file(s) copied by `make card`
#     include /path/to/sdk/mk/newtos.mk
#
# Optional: CFLAGS (extra compiler flags, e.g. -Os -Wall), LDFLAGS (-Wl,...),
# PRGFLAGS (e.g. 0x7), PROT (private | global | super | readable), BARE = 1
# (no libc, sdk/crt0.S), EXTRA (more files `make card` copies), BUILD (object
# directory, default build), NEWTOS_CC (default: the newtos-cc next to this file).
#
# Targets:
#     make                      build $(PROG).<KIND in upper case>
#     make card DEST=<folder>   copy the program, $(RSC) and $(EXTRA) into <folder>
#                               (e.g. a folder on a mounted SD card)
#     make clean                remove $(BUILD) and the program
#
# GNU make 3.81 or newer (macOS ships 3.81). Reference: docs/sdk/16-newtos-cc.md.

# Captured before the dependency files are included: the user's Makefile and
# this file. A change in either rebuilds (flags, PRGFLAGS, LIBS live there).
_NEWTOS_MAKEFILES := $(MAKEFILE_LIST)
NEWTOS_SDK := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))..)
NEWTOS_CC  ?= $(NEWTOS_SDK)/bin/newtos-cc

ifeq ($(strip $(PROG)),)
$(error newtos.mk: set PROG (program name without extension) before the include)
endif
ifeq ($(strip $(SRCS)),)
$(error newtos.mk: set SRCS (the program's .c/.S sources) before the include)
endif

KIND  ?= ncp
BUILD ?= build

_NEWTOS_EXT_ncp := NCP
_NEWTOS_EXT_ntp := NTP
_NEWTOS_EXT_nap := NAP
_NEWTOS_EXT_nac := NAC
_NEWTOS_EXT_nld := NLD
_NEWTOS_EXT := $(_NEWTOS_EXT_$(strip $(KIND)))
ifeq ($(_NEWTOS_EXT),)
$(error newtos.mk: KIND must be one of ncp ntp nap nac nld, not '$(KIND)')
endif

TARGET := $(strip $(PROG)).$(_NEWTOS_EXT)
OBJS   := $(addprefix $(BUILD)/,$(addsuffix .o,$(basename $(SRCS))))

# Deferred (=): BARE, PRGFLAGS, PROT, LIBS, CFLAGS may also be set after the
# include; KIND, PROG and SRCS must come before it (they name the targets).
_NEWTOS_KIND    = -$(strip $(KIND)) $(if $(strip $(BARE)),-bare)
_NEWTOS_PFLAGS  = $(if $(strip $(PRGFLAGS)),-prgflags $(strip $(PRGFLAGS))) \
                  $(if $(strip $(PROT)),-prot $(strip $(PROT)))
# Relink when an SDK library was rebuilt.
_NEWTOS_ARCHIVES := $(wildcard $(NEWTOS_SDK)/*/*.a)

.PHONY: all card clean
all: $(TARGET)

$(TARGET): $(OBJS) $(_NEWTOS_MAKEFILES) $(_NEWTOS_ARCHIVES)
	$(NEWTOS_CC) $(_NEWTOS_KIND) $(_NEWTOS_PFLAGS) $(LIBS) $(LDFLAGS) $(OBJS) -o $@

$(BUILD)/%.o: %.c $(_NEWTOS_MAKEFILES)
	@mkdir -p $(dir $@)
	$(NEWTOS_CC) $(_NEWTOS_KIND) $(LIBS) $(CFLAGS) -MMD -MP -c $< -o $@

$(BUILD)/%.o: %.S $(_NEWTOS_MAKEFILES)
	@mkdir -p $(dir $@)
	$(NEWTOS_CC) $(_NEWTOS_KIND) $(LIBS) $(CFLAGS) -MMD -MP -c $< -o $@

-include $(OBJS:.o=.d)

# `make card DEST=...`: copy without extended attributes (cp -X on macOS), and
# delete a ._<name> AppleDouble file should one appear anyway (for each copied
# file, and for DEST itself when this rule created it) -- on a FAT card such a
# file ends in the program's extension, so the desktop would list it
# (docs/sdk/06-image-and-testing.md, "AppleDouble files").
card: $(TARGET)
	@if [ -z "$(DEST)" ]; then echo "usage: make card DEST=<folder>" >&2; exit 1; fi
	@set -e; d="$(DEST)"; d=$${d%/}; \
	if [ ! -d "$$d" ]; then echo "mkdir -p $$d"; mkdir -p "$$d"; \
	  rm -f "$$(dirname "$$d")/._$$(basename "$$d")"; fi; \
	cpx=cp; if [ "$$(uname -s)" = Darwin ]; then cpx="cp -X"; fi; \
	for f in $(TARGET) $(RSC) $(EXTRA); do \
	  echo "$$cpx $$f $$d/"; $$cpx "$$f" "$$d/"; \
	  rm -f "$$d/._$$(basename "$$f")"; \
	done

clean:
	rm -rf $(BUILD) $(TARGET)
