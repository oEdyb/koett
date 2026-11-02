#include <sqlite3.h>

// Copy Swift-owned buffers before their temporary pointer expires.
static inline int koett_bind_text(sqlite3_stmt *s, int i, const char *p, int n) {
    return sqlite3_bind_text(s, i, p, n, SQLITE_TRANSIENT);
}
static inline int koett_bind_blob(sqlite3_stmt *s, int i, const void *p, int n) {
    return sqlite3_bind_blob(s, i, p, n, SQLITE_TRANSIENT);
}
