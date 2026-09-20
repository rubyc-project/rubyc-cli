/* main.c — C host executable: consumes BOTH libraries.
 * - native_add.so (made in C) via dlopen/dlsym
 * - libtestns.so  (made by RubyC) via dlopen/dlsym
 */
#include <stdint.h>
#include <stdio.h>
#include <dlfcn.h>

typedef int32_t (*add_fn)(int32_t, int32_t);
typedef int32_t (*inc_fn)(int32_t);

static void *load_handle(const char *path) {
    void *handle = dlopen(path, RTLD_NOW);
    if (!handle) {
        fprintf(stderr, "dlopen %s failed: %s\n", path, dlerror());
        return NULL;
    }
    return handle;
}

static void *sym(void *handle, const char *symbol, const char *path) {
    void *fn = dlsym(handle, symbol);
    if (!fn) {
        fprintf(stderr, "dlsym %s in %s failed: %s\n", symbol, path, dlerror());
    }
    return fn;
}

int main(void) {
    /* The library made in C. */
    void *native = load_handle("./libnative_add.so");
    if (!native) return 1;

    /* The library made by RubyC. */
    void *rclib = load_handle("./libtestns.so");
    if (!rclib) return 1;

    add_fn c_add = (add_fn)sym(native, "add", "./libnative_add.so");
    inc_fn c_inc = (inc_fn)sym(native, "inc", "./libnative_add.so");
    add_fn rc_add = (add_fn)sym(rclib, "add", "./libtestns.so");
    inc_fn rc_inc = (inc_fn)sym(rclib, "inc", "./libtestns.so");
    if (!c_add || !c_inc || !rc_add || !rc_inc) return 1;

    printf("%d %d %d %d\n", c_add(2, 3), rc_add(40, 2), c_inc(9), rc_inc(41));
    return 0;
}
