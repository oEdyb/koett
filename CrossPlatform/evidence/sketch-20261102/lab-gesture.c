#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <limits.h>
#include <linux/input-event-codes.h>
#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <uchar.h>
#include <unistd.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "fake-input-client.h"

static struct org_kde_kwin_fake_input *input;
static void usage(void) {
    fputs("Usage: input move X Y | click [BUTTON] | key CODE... | chord CODE... | text STRING\n", stderr);
    exit(2);
}
static void arity(int argc, int minimum, int maximum) {
    if (argc < minimum || argc > maximum) usage();
}
static void global(void *data, struct wl_registry *registry, uint32_t name, const char *interface, uint32_t version) {
    (void)data;
    if (strcmp(interface, "org_kde_kwin_fake_input") == 0 && version >= 6)
        input = wl_registry_bind(registry, name, &org_kde_kwin_fake_input_interface, 6);
}
static void removed(void *data, struct wl_registry *registry, uint32_t name) {
    (void)data; (void)registry; (void)name;
}
static const struct wl_registry_listener listener = {global, removed};
static int integer(const char *s, int minimum, int maximum) {
    char *end = NULL; errno = 0;
    long value = strtol(s, &end, 10);
    if (errno || !*s || *end || value < minimum || value > maximum) {
        fprintf(stderr, "Invalid input value: %s\n", s); exit(2);
    }
    return (int)value;
}
static void private_socket(char *socket, size_t size) {
    const char *runtime = getenv("AGENT_LAB_RUNTIME");
    const char *xdg = getenv("XDG_RUNTIME_DIR");
    char prefix[128]; struct stat st;
    snprintf(prefix, sizeof(prefix), "/run/user/%u/agent-lab/", (unsigned)getuid());
    if (!runtime || !xdg || strcmp(runtime, xdg) || strncmp(runtime, prefix, strlen(prefix)) ||
        strstr(runtime, "/../") || lstat(runtime, &st) || !S_ISDIR(st.st_mode) ||
        st.st_uid != getuid() || (st.st_mode & 077)) {
        fputs("Refusing input outside a private agent-lab runtime.\n", stderr); exit(2);
    }
    if (snprintf(socket, size, "%s/wayland-agent-lab", runtime) >= (int)size) {
        fputs("Private socket path is too long.\n", stderr); exit(2);
    }
}

#include <math.h>
static void pause_ms(long ms) { struct timespec t = {ms / 1000, (ms % 1000) * 1000000L}; nanosleep(&t, NULL); }
/* gesture KEY CX CY RADIUS STEPS: hold KEY, trace a circle, release KEY. */
int main(int argc, char **argv) {
    char socket[PATH_MAX]; private_socket(socket, sizeof(socket));
    if (argc != 6) { fputs("Usage: gesture KEY CX CY RADIUS STEPS\n", stderr); return 2; }
    uint32_t key = (uint32_t)integer(argv[1], 1, KEY_MAX);
    int cx = integer(argv[2], 0, 16384), cy = integer(argv[3], 0, 16384);
    int r = integer(argv[4], 1, 4000), steps = integer(argv[5], 3, 2000);
    struct wl_display *display = wl_display_connect(socket);
    if (!display) { perror("Private lab Wayland connection"); return 1; }
    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !input) { fputs("No fake input\n", stderr); return 1; }
    org_kde_kwin_fake_input_authenticate(input, "Agent Lab", "Koett sketch gesture in an isolated test desktop");
    org_kde_kwin_fake_input_pointer_motion_absolute(input, wl_fixed_from_int(cx + r), wl_fixed_from_int(cy));
    wl_display_roundtrip(display); pause_ms(150);
    org_kde_kwin_fake_input_keyboard_key(input, key, 1);
    wl_display_roundtrip(display); pause_ms(150);
    for (int i = 0; i <= steps; i++) {
        double a = 6.283185307179586 * i / steps;
        org_kde_kwin_fake_input_pointer_motion_absolute(input,
            wl_fixed_from_double(cx + r * cos(a)), wl_fixed_from_double(cy + r * sin(a)));
        wl_display_roundtrip(display); pause_ms(12);
    }
    pause_ms(100);
    org_kde_kwin_fake_input_keyboard_key(input, key, 0);
    wl_display_roundtrip(display); pause_ms(100);
    org_kde_kwin_fake_input_destroy(input);
    wl_display_disconnect(display);
    return 0;
}
