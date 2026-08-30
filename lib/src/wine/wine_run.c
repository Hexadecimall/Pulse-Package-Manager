/*
 * pulse_wine_run — run a Windows executable through libwine, loaded as a dylib.
 *
 * This is how Wine's own `wine` loader stub works: it dlopen's libwine and
 * calls wine_init(argc, argv, error, error_size), which sets up the Wine
 * environment and hands control to the Windows program. wine_init normally
 * does not return; if it does, it failed and left a message in `error`.
 *
 * Pulse links this via FFI (see wine/mod.rs) so we can drive Wine without
 * shelling out to the `wine` binary. libwine is loaded at runtime, so the build
 * doesn't require Wine to be present.
 */
#include <stddef.h>
#include <stdio.h>

#ifndef _WIN32
#include <dlfcn.h>

typedef void (*wine_init_fn)(int argc, char **argv, char *error, int error_size);

/* libwine names/paths to try when the caller doesn't give an explicit one. */
static const char *CANDIDATES[] = {
#ifdef __APPLE__
    "libwine.1.dylib",
    "libwine.dylib",
    "/usr/local/lib/libwine.dylib",
    "/opt/homebrew/lib/libwine.dylib",
#else
    "libwine.so.1",
    "libwine.so",
    "/usr/lib/libwine.so.1",
    "/usr/lib/x86_64-linux-gnu/libwine.so.1",
#endif
    NULL};

/*
 * Load libwine and run `exe_path` under it. `wine_lib` may be an explicit path
 * to libwine, or NULL to search the candidates. On success control does not
 * return here (the process becomes the Wine process). On failure a message is
 * written to `err` and a negative code is returned.
 */
int pulse_wine_run(const char *wine_lib, const char *exe_path, char *err, int err_size)
{
    void *handle = NULL;

    if (wine_lib && wine_lib[0]) {
        handle = dlopen(wine_lib, RTLD_NOW | RTLD_GLOBAL);
    } else {
        for (int i = 0; CANDIDATES[i]; i++) {
            handle = dlopen(CANDIDATES[i], RTLD_NOW | RTLD_GLOBAL);
            if (handle)
                break;
        }
    }
    if (!handle) {
        snprintf(err, err_size, "could not load libwine (%s)", dlerror());
        return -1;
    }

    wine_init_fn wine_init = (wine_init_fn)dlsym(handle, "wine_init");
    if (!wine_init) {
        snprintf(err, err_size, "libwine has no wine_init symbol (%s)", dlerror());
        return -2;
    }

    char *argv[] = {(char *)"wine", (char *)exe_path, NULL};
    char winerr[1024];
    winerr[0] = '\0';
    wine_init(2, argv, winerr, (int)sizeof(winerr));

    /* wine_init normally never returns; reaching here means it failed. */
    snprintf(err, err_size, "wine_init returned: %s",
             winerr[0] ? winerr : "unknown error");
    return -3;
}

#else /* _WIN32 */

int pulse_wine_run(const char *wine_lib, const char *exe_path, char *err, int err_size)
{
    (void)wine_lib;
    (void)exe_path;
    snprintf(err, err_size, "wine-run is not applicable on Windows");
    return -10;
}

#endif
