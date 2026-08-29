/* tdtest.h -- minimal assert harness for the host-side tosdump tests. */
#ifndef TDTEST_H
#define TDTEST_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int td_test_failures = 0;

#define TD_CHECK(cond) \
	do { \
		if (!(cond)) { \
			printf("  FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
			td_test_failures++; \
		} \
	} while (0)

#define TD_CHECK_EQ(got, want) \
	do { \
		long g_ = (long)(got), w_ = (long)(want); \
		if (g_ != w_) { \
			printf("  FAIL %s:%d: %s == %s: got %ld (0x%lx), want %ld (0x%lx)\n", \
			       __FILE__, __LINE__, #got, #want, g_, (unsigned long)g_, \
			       w_, (unsigned long)w_); \
			td_test_failures++; \
		} \
	} while (0)

#define TD_CHECK_MEM(got, want, len) \
	do { \
		if (memcmp((got), (want), (len)) != 0) { \
			size_t i_; \
			printf("  FAIL %s:%d: %s != %s over %lu bytes\n", __FILE__, __LINE__, \
			       #got, #want, (unsigned long)(len)); \
			for (i_ = 0; i_ < (size_t)(len); i_++) { \
				if (((const unsigned char *)(got))[i_] != \
				    ((const unsigned char *)(want))[i_]) { \
					printf("    first difference at +%lu: got 0x%02x want 0x%02x\n", \
					       (unsigned long)i_, \
					       ((const unsigned char *)(got))[i_], \
					       ((const unsigned char *)(want))[i_]); \
					break; \
				} \
			} \
			td_test_failures++; \
		} \
	} while (0)

#define TD_TEST_RESULT() \
	(td_test_failures == 0 \
	 ? (printf("  ok\n"), 0) \
	 : (printf("  %d failure(s)\n", td_test_failures), 1))

#endif /* TDTEST_H */
