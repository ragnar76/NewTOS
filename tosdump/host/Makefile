# Makefile -- host-side build and tests for the TOS reference dumper.
#
# `make test` is the T1 acceptance command (spec section 9).

CC      ?= cc
CFLAGS  ?= -std=c99 -O1 -g
CFLAGS  += -Wall -Wextra -Werror -Wno-unused-parameter
CFLAGS  += -I../core -I../host -I../tests

BUILD   := build
CORE    := ../core
TESTS   := ../tests

.PHONY: all test run-tests clean
all: $(BUILD)/tosdiff

$(BUILD):
	@mkdir -p $(BUILD)

$(BUILD)/record.o: $(CORE)/record.c $(CORE)/record.h $(CORE)/tosdump.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@

# The same writer, built for both target configurations (spec section 2.1).
$(BUILD)/record_be.o: $(CORE)/record.c $(CORE)/record.h $(CORE)/tosdump.h | $(BUILD)
	$(CC) $(CFLAGS) -DTD_BIG_ENDIAN_TARGET=1 -c $< -o $@
$(BUILD)/record_le.o: $(CORE)/record.c $(CORE)/record.h $(CORE)/tosdump.h | $(BUILD)
	$(CC) $(CFLAGS) -DTD_BIG_ENDIAN_TARGET=0 -c $< -o $@

$(BUILD)/casefile.o: casefile.c casefile.h $(CORE)/record.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/tosdiff.o: tosdiff.c casefile.h $(CORE)/record.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/tosdiff: $(BUILD)/tosdiff.o $(BUILD)/casefile.o $(BUILD)/record.o
	$(CC) $(CFLAGS) $^ -o $@

$(BUILD)/test_record.o: $(TESTS)/test_record.c $(TESTS)/tdtest.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@
$(BUILD)/test_record: $(BUILD)/test_record.o $(BUILD)/record.o
	$(CC) $(CFLAGS) $^ -o $@

$(BUILD)/mkdump.o: $(TESTS)/mkdump.c casefile.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@
$(BUILD)/mkdump: $(BUILD)/mkdump.o $(BUILD)/casefile.o $(BUILD)/record.o
	$(CC) $(CFLAGS) $^ -o $@

$(BUILD)/emit_record.o: $(TESTS)/emit_record.c | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@
$(BUILD)/emit_be: $(BUILD)/emit_record.o $(BUILD)/record_be.o
	$(CC) $(CFLAGS) $^ -o $@
$(BUILD)/emit_le: $(BUILD)/emit_record.o $(BUILD)/record_le.o
	$(CC) $(CFLAGS) $^ -o $@

# `make test` always rebuilds from scratch.  The make that ships with macOS
# compares modification times at one-second resolution, so an edit and a
# re-run inside the same second silently test the previous object file -- a
# false green, which is the one failure mode this whole exercise exists to
# rule out.  The full build takes well under a second; correctness wins.
test:
	@$(MAKE) --no-print-directory clean
	@$(MAKE) --no-print-directory run-tests

run-tests: $(BUILD)/test_record $(BUILD)/tosdiff $(BUILD)/mkdump \
      $(BUILD)/emit_be $(BUILD)/emit_le
	@echo "== record format"
	@$(BUILD)/test_record
	@echo "== byte order: both target configurations emit identical records"
	@$(BUILD)/emit_be $(BUILD)/be.bin
	@$(BUILD)/emit_le $(BUILD)/le.bin
	@cmp $(BUILD)/be.bin $(BUILD)/le.bin && echo "  ok"
	@echo "== comparator"
	@sh $(TESTS)/test_tosdiff.sh $(BUILD)

clean:
	rm -rf $(BUILD)
