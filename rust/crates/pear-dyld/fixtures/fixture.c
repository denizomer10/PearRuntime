// A tiny iOS dylib exercising what a loader must do: rebases (pointers to
// its own data), binds (an imported function called through a stub and held
// in data, an imported data symbol), a weak import that is missing, and an
// initializer. Built by build-fixtures.sh for two deployment targets, so it
// carries dyld-info opcodes (iOS 12) and chained fixups (iOS 15).
extern int host_add(int a, int b);            // from libpearhost.dylib
extern const char *host_name;                 // data symbol from libpearhost.dylib
extern int host_missing(void) __attribute__((weak_import));

static int counter;
static int table[4] = {1, 2, 3, 4};
const char *greeting = "hello from an iOS image";
int *table_ptr = &table[2];
int (*add_ptr)(int, int) = host_add;

// Calls an import, so the compiler cannot evaluate it at build time.
__attribute__((constructor)) static void fixture_init(void) { counter = host_add(40, 1); }

int fixture_value(void) { return counter + 1; }
int fixture_call(int a, int b) { return add_ptr(a, b) * 2 + host_add(1, 1); }
const char *fixture_greeting(void) { return greeting; }
const char *fixture_host_name(void) { return host_name; }
int fixture_table(void) { return *table_ptr; }
int fixture_has_missing(void) { return host_missing != 0; }

// Thread-local variables (__thread_vars, with __thread_data and __thread_bss
// templates): each thread starts from the initial values.
__thread int tls_counter = 5;
__thread int tls_zero;
__thread const char *tls_text = "per-thread";
int fixture_tls_bump(void) { tls_zero += 10; return ++tls_counter + tls_zero; }
const char *fixture_tls_text(void) { return tls_text; }
