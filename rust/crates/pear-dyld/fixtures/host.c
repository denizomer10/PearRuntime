// Link-time stand-in for the library the fixture imports from; the loader's
// resolver supplies the real definitions.
int host_add(int a, int b) { return a + b; }
const char *host_name = "stand-in";
int host_missing(void) { return 0; }
