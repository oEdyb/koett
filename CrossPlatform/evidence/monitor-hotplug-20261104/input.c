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
    if (snprintf(socket, size, "%s/wayland-hotplug-sol", runtime) >= (int)size) {
        fputs("Private socket path is too long.\n", stderr); exit(2);
    }
}
static void tap_keys(int argc, char **argv) {
    for (int i = 2; i < argc; i++) {
        uint32_t key = (uint32_t)integer(argv[i], 1, KEY_MAX);
        org_kde_kwin_fake_input_keyboard_key(input, key, 1);
        org_kde_kwin_fake_input_keyboard_key(input, key, 0);
    }
}
static void chord(int argc, char **argv, struct wl_display *display) {
    for (int i = 2; i < argc; i++) integer(argv[i], 1, KEY_MAX);
    for (int i = 2; i < argc; i++)
        org_kde_kwin_fake_input_keyboard_key(input, (uint32_t)integer(argv[i], 1, KEY_MAX), 1);
    wl_display_roundtrip(display);
    const struct timespec held = {.tv_sec = 0, .tv_nsec = 50000000};
    nanosleep(&held, NULL);
    for (int i = argc - 1; i >= 2; i--)
        org_kde_kwin_fake_input_keyboard_key(input, (uint32_t)integer(argv[i], 1, KEY_MAX), 0);
}
static void text(const char *value) {
    const char *p = value;
    mbstate_t state = {0};
    while (*p) {
        char32_t codepoint;
        size_t count = mbrtoc32(&codepoint, p, strlen(p), &state);
        if (count == (size_t)-1 || count == (size_t)-2 || count == 0) {
            fputs("Invalid UTF-8 text.\n", stderr); exit(2);
        }
        xkb_keysym_t sym = xkb_utf32_to_keysym(codepoint);
        org_kde_kwin_fake_input_keyboard_keysym(input, sym, 1);
        org_kde_kwin_fake_input_keyboard_keysym(input, sym, 0);
        p += count;
    }
}
static void action(int argc, char **argv, struct wl_display *display) {
    if (!strcmp(argv[1], "move")) {
        arity(argc, 4, 4);
        org_kde_kwin_fake_input_pointer_motion_absolute(input,
            wl_fixed_from_int(integer(argv[2], 0, 16384)), wl_fixed_from_int(integer(argv[3], 0, 16384)));
        return;
    }
    if (!strcmp(argv[1], "click")) {
        arity(argc, 2, 3);
        uint32_t button = argc == 3 ? (uint32_t)integer(argv[2], BTN_LEFT, BTN_TASK) : BTN_LEFT;
        org_kde_kwin_fake_input_button(input, button, 1);
        org_kde_kwin_fake_input_button(input, button, 0);
        return;
    }
    if (!strcmp(argv[1], "key")) { arity(argc, 3, INT_MAX); tap_keys(argc, argv); return; }
    if (!strcmp(argv[1], "chord")) { arity(argc, 3, INT_MAX); chord(argc, argv, display); return; }
    if (!strcmp(argv[1], "text")) { arity(argc, 3, 3); text(argv[2]); return; }
    usage();
}
int main(int argc, char **argv) {
    setlocale(LC_CTYPE, "C.UTF-8");
    char socket[PATH_MAX]; private_socket(socket, sizeof(socket));
    if (argc < 2) usage();
    struct wl_display *display = wl_display_connect(socket);
    if (!display) { perror("Private lab Wayland connection"); return 1; }
    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !input) {
        fputs("Lab compositor has no supported test input interface.\n", stderr); return 1;
    }
    org_kde_kwin_fake_input_authenticate(input, "Agent Lab", "Input in an isolated invisible test desktop");
    if (!strcmp(argv[1], "hold")) {
        arity(argc, 2, 2);
        while (wl_display_dispatch(display) >= 0) {}
        wl_display_disconnect(display); return 0;
    }
    action(argc, argv, display);
    int result = wl_display_roundtrip(display);
    const struct timespec settle = {.tv_sec = 0, .tv_nsec = 100000000};
    nanosleep(&settle, NULL);
    wl_display_roundtrip(display);
    org_kde_kwin_fake_input_destroy(input);
    wl_registry_destroy(registry); wl_display_disconnect(display);
    return result < 0 ? 1 : 0;
}
