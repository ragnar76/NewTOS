/*
 * cdefs-compat.h -- NewTOS SDK adapter (TASK/SDK01 J): OpenLibm's
 * include/openlibm_fenv_aarch64.h (reached through <fenv.h> ->
 * <openlibm_fenv.h>) does #include "cdefs-compat.h", a header that lives in
 * OpenLibm's src/, not in its include/. OpenLibm's own `make install-headers`
 * copies the headers of include/ AND of src/ into one directory, so an installed OpenLibm
 * finds it next to openlibm_fenv_aarch64.h; the SDK uses the extracted tree
 * in place (-I sdk/libm/include -I sdk/libm/openlibm-<ver>/include), where
 * it was not found and <fenv.h> did not compile. This file is found on that
 * include path and forwards to the one header -- not to all of src/, whose
 * other headers (math_private.h, ...) are libm internals.
 *
 * The path is relative to this file and carries the OpenLibm version that
 * sdk/libm/build-libm.sh extracts (VER there).
 */
#include "../openlibm-0.8.8/src/cdefs-compat.h"
