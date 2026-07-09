/**
 * anetd WASM API
 *
 * Replaces webui/src/api/anetd.ts.  Communicates with the anetd daemon via
 * KernelSU's ksu.exec() bridge using Emscripten EM_JS interop.
 *
 * Build: emcc -O3 -sEXPORTED_FUNCTIONS=... -o anetd_api.js anetd_api.c
 */

#include <emscripten.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <ctype.h>

#define MODDIR        "/data/adb/modules/anetd"
#define PID_FILE      MODDIR "/log/anetd.pid"
#define RULES_DIR    MODDIR "/rules"
#define CONFIG_FILE  MODDIR "/config.toml"
#define STATE_FILE    MODDIR "/log/dns_off"
#define LOG_FILE      MODDIR "/log/anetd.log"
#define TOGGLE_SCRIPT MODDIR "/toggle.sh"

EM_JS(char*, ksu_exec_json, (const char* cmd), {
    var cmdStr = UTF8ToString(cmd);
    var result = { errno: 0, stdout: "", stderr: "" };
    try {
        if (typeof globalThis !== 'undefined' && globalThis.ksu && globalThis.ksu.exec) {
            var r = globalThis.ksu.exec(cmdStr);
            result.errno = (r.errno != null) ? r.errno : -1;
            result.stdout = r.stdout || "";
            result.stderr = r.stderr || "";
        }
    } catch (e) {
        result.errno = -1;
        result.stderr = String(e);
    }
    // Return as JSON so C can parse individual fields
    var json = JSON.stringify(result);
    var len = lengthBytesUTF8(json) + 1;
    var ptr = _malloc(len);
    stringToUTF8(json, ptr, len);
    return ptr;
});

/** Extract a string field from {"errno":N,"stdout":"...","stderr":"..."} */
static char* json_field(char* json, const char* key) {
    char search[64];
    snprintf(search, sizeof(search), "\"%s\":\"", key);
    char* start = strstr(json, search);
    if (!start) return strdup("");
    start += strlen(search);
    char* end = start;
    while (*end && *end != '"') {
        if (*end == '\\' && *(end+1)) end++;
        end++;
    }
    size_t len = end - start;
    char* out = malloc(len + 1);
    if (!out) return strdup("");
    memcpy(out, start, len);
    out[len] = '\0';
    return out;
}

/** Extract an int field from ksu_exec_json result. */
static int json_int_field(char* json, const char* key) {
    char search[64];
    snprintf(search, sizeof(search), "\"%s\":", key);
    char* start = strstr(json, search);
    if (!start) return -1;
    start += strlen(search);
    return atoi(start);
}

/** Run a shell command via ksu.exec, return stdout (caller frees). */
static char* sh_stdout(const char* cmd) {
    char* json = ksu_exec_json(cmd);
    char* out = json_field(json, "stdout");
    free(json);
    return out;
}

/** Run a shell command via ksu.exec, return errno. */
static int sh_errno(const char* cmd) {
    char* json = ksu_exec_json(cmd);
    int e = json_int_field(json, "errno");
    free(json);
    return e;
}

/** Simple shell escape: wrap in single quotes after escaping embedded '. */
static char* shell_escape(const char* s) {
    size_t len = strlen(s);
    size_t extra = 0;
    for (size_t i = 0; i < len; i++) {
        if (s[i] == '\'') extra += 3;     /* '  ->  '\'' */
        if (s[i] == '\\') extra += 1;     /* \  ->  \\   */
    }
    char* out = malloc(len + extra + 8); /* quotes + nul */
    if (!out) return strdup("''");
    char* d = out;
    *d++ = '\'';
    for (size_t i = 0; i < len; i++) {
        if (s[i] == '\'') {
            *d++ = '\'';
            *d++ = '\\';
            *d++ = '\'';
            *d++ = '\'';
        } else if (s[i] == '\\') {
            *d++ = '\\';
            *d++ = '\\';
        } else {
            *d++ = s[i];
        }
    }
    *d++ = '\'';
    *d = '\0';
    return out;
}

typedef struct {
    char* buf;
    size_t len;
    size_t cap;
} StringBuilder;

static void sb_init(StringBuilder* sb, size_t initial) {
    sb->buf = malloc(initial);
    sb->cap = initial;
    sb->len = 0;
    if (sb->buf) sb->buf[0] = '\0';
}

static void sb_append(StringBuilder* sb, const char* s) {
    if (!sb->buf) return;
    size_t slen = strlen(s);
    if (sb->len + slen + 1 > sb->cap) {
        sb->cap = (sb->len + slen + 1) * 2;
        char* nb = realloc(sb->buf, sb->cap);
        if (!nb) { free(sb->buf); sb->buf = NULL; return; }
        sb->buf = nb;
    }
    memcpy(sb->buf + sb->len, s, slen);
    sb->len += slen;
    sb->buf[sb->len] = '\0';
}

static void sb_append_escaped(StringBuilder* sb, const char* s) {
    if (!sb->buf) return;
    // Reserve space: worst case every char escapes → 2x + 2 quotes + \0
    size_t slen = strlen(s);
    size_t needed = sb->len + slen * 2 + 4;
    if (needed > sb->cap) {
        sb->cap = needed * 2;
        char* nb = realloc(sb->buf, sb->cap);
        if (!nb) { free(sb->buf); sb->buf = NULL; return; }
        sb->buf = nb;
    }
    char* d = sb->buf + sb->len;
    *d++ = '"';
    for (size_t i = 0; i < slen; i++) {
        switch (s[i]) {
        case '"':  *d++ = '\\'; *d++ = '"';  break;
        case '\\': *d++ = '\\'; *d++ = '\\'; break;
        case '\n': *d++ = '\\'; *d++ = 'n';  break;
        case '\r': *d++ = '\\'; *d++ = 'r';  break;
        case '\t': *d++ = '\\'; *d++ = 't';  break;
        default:   *d++ = s[i]; break;
        }
    }
    *d++ = '"';
    *d = '\0';
    sb->len = d - sb->buf;
}

static char* sb_detach(StringBuilder* sb) {
    char* out = sb->buf;
    sb->buf = NULL;
    sb->len = 0;
    sb->cap = 0;
    return out;
}

EMSCRIPTEN_KEEPALIVE
char* get_status(void) {
    char* pid_raw = sh_stdout("cat \"" PID_FILE "\" 2>/dev/null");
    int pid = atoi(pid_raw);

    int running = 0;
    char uptime[64] = "\u2014"; // em-dash
    if (pid > 0) {
        char cmd[128];
        snprintf(cmd, sizeof(cmd), "ps -p %d -o pid= 2>/dev/null", pid);
        char* ps_out = sh_stdout(cmd);
        // trim whitespace
        char* t = ps_out;
        while (*t == ' ' || *t == '\t' || *t == '\n') t++;
        int found_pid = atoi(t);
        free(ps_out);
        if (found_pid == pid) {
            running = 1;
            snprintf(cmd, sizeof(cmd), "ps -p %d -o etime= 2>/dev/null", pid);
            char* et = sh_stdout(cmd);
            t = et;
            while (*t == ' ') t++;
            char* e = t + strlen(t) - 1;
            while (e > t && (*e == '\n' || *e == ' ')) { *e = '\0'; e--; }
            snprintf(uptime, sizeof(uptime), "%s", *t ? t : "running");
            free(et);
        }
    }
    int dns_off = 0;
    char* off_check = sh_stdout("[ -f \"" STATE_FILE "\" ] && echo 1 || echo 0");
    if (off_check && off_check[0] == '1') dns_off = 1;
    free(off_check);

    char buf[512];
    snprintf(buf, sizeof(buf),
        "{\"running\":%s,\"pid\":%s,\"dnsFilterEnabled\":%s,\"uptime\":\"%s\"}",
        running ? "true" : "false",
        running ? pid_raw : "null",
        dns_off ? "false" : "true",
        uptime);
    free(pid_raw);

    return strdup(buf);
}

static const char* classify_rule(const char* line, const char* trimmed) {
    if (!trimmed[0]) return "blank";
    if (trimmed[0] == '!') return "comment";
    if (trimmed[0] == '[') return "header";
    // inline comment: rule followed by " !"
    {
        const char* excl = strstr(line, " !");
        if (excl && excl > line && *(excl - 1) == ' ') return "inline-comment";
    }
    if (strncmp(trimmed, "@@||", 4) == 0) return "allow";
    if (strncmp(trimmed, "||", 2) == 0) return "block";
    return "comment";
}

static char* trim_in_place(char* s) {
    while (isspace((unsigned char)*s)) s++;
    if (*s == 0) return s;
    char* end = s + strlen(s) - 1;
    while (end > s && isspace((unsigned char)*end)) end--;
    end[1] = '\0';
    return s;
}

EMSCRIPTEN_KEEPALIVE
char* load_rules(void) {
    char* listing = sh_stdout("ls \"" RULES_DIR "\" 2>/dev/null");
    if (!listing || !listing[0]) {
        free(listing);
        return strdup("[]");
    }

    StringBuilder sb;
    sb_init(&sb, 4096);
    sb_append(&sb, "[");

    int first_entry = 1;
    char* saveptr;
    char* file = strtok_r(listing, "\n", &saveptr);
    while (file) {
        char filepath[1024];
        snprintf(filepath, sizeof(filepath), RULES_DIR "/%s", file);

        char cmd[1152];
        snprintf(cmd, sizeof(cmd), "cat \"%s\" 2>/dev/null", filepath);
        char* content = sh_stdout(cmd);
        if (content && content[0]) {
            char* line_save;
            char* line = strtok_r(content, "\n", &line_save);
            while (line) {
                char line_copy[4096];
                strncpy(line_copy, line, sizeof(line_copy) - 1);
                line_copy[sizeof(line_copy) - 1] = '\0';
                char* trimmed = trim_in_place(line_copy);
                const char* rtype = classify_rule(line, trimmed);

                if (!first_entry) sb_append(&sb, ",");
                first_entry = 0;

                sb_append(&sb, "{\"raw\":");
                sb_append_escaped(&sb, line);
                sb_append(&sb, ",\"type\":\"");
                sb_append(&sb, rtype);
                sb_append(&sb, "\",\"original\":");
                sb_append_escaped(&sb, line);
                sb_append(&sb, "}");

                line = strtok_r(NULL, "\n", &line_save);
            }
        }
        free(content);
        file = strtok_r(NULL, "\n", &saveptr);
    }

    sb_append(&sb, "]");
    free(listing);
    return sb_detach(&sb);
}

EMSCRIPTEN_KEEPALIVE
int reload_rules(void) {
    char* pid_raw = sh_stdout("cat \"" PID_FILE "\" 2>/dev/null");
    int pid = atoi(pid_raw);
    free(pid_raw);
    if (pid <= 0) return 0;

    char cmd[64];
    snprintf(cmd, sizeof(cmd), "kill -HUP %d 2>/dev/null", pid);
    return sh_errno(cmd) == 0;
}

EMSCRIPTEN_KEEPALIVE
int toggle_filter(void) {
    char cmd[512];
    snprintf(cmd, sizeof(cmd), "sh \"" TOGGLE_SCRIPT "\"");
    return sh_errno(cmd) == 0;
}

EMSCRIPTEN_KEEPALIVE
char* load_config(void) {
    return sh_stdout("cat \"" CONFIG_FILE "\" 2>/dev/null");
}

EMSCRIPTEN_KEEPALIVE
int save_config(const char* content) {
    char* escaped = shell_escape(content);
    char tmp[32];
    snprintf(tmp, sizeof(tmp), CONFIG_FILE ".tmp");
    // printf '%s' 'escaped' > tmp && mv tmp config
    size_t cmd_len = strlen(escaped) + 128;
    char* cmd = malloc(cmd_len);
    if (!cmd) { free(escaped); return 0; }
    snprintf(cmd, cmd_len,
        "printf '%%s' %s > \"%s\" && mv \"%s\" \"" CONFIG_FILE "\"",
        escaped, tmp, tmp);
    free(escaped);

    int ok = (sh_errno(cmd) == 0);
    free(cmd);
    if (ok) ok = reload_rules();
    return ok;
}

EMSCRIPTEN_KEEPALIVE
char* load_logs(int lines) {
    if (lines <= 0) lines = 100;
    char cmd[128];
    snprintf(cmd, sizeof(cmd), "tail -n %d \"" LOG_FILE "\" 2>/dev/null", lines);
    char* content = sh_stdout(cmd);
    if (!content || !content[0]) {
        free(content);
        return strdup("[]");
    }

    StringBuilder sb;
    sb_init(&sb, 4096);
    sb_append(&sb, "[");

    int first = 1;
    char* saveptr;
    char* line = strtok_r(content, "\n", &saveptr);
    while (line) {
        if (!first) sb_append(&sb, ",");
        first = 0;
        sb_append_escaped(&sb, line);
        line = strtok_r(NULL, "\n", &saveptr);
    }

    sb_append(&sb, "]");
    free(content);
    return sb_detach(&sb);
}

/**
 * Extract hostname from a pseudo-URL.
 * "https://example.com/" -> "example.com"
 */
static const char* extract_hostname(const char* url) {
    if (strncmp(url, "https://", 8) == 0) url += 8;
    else if (strncmp(url, "http://", 7) == 0) url += 7;

    // find end of hostname (/, ?, :)
    size_t i;
    for (i = 0; url[i]; i++) {
        if (url[i] == '/' || url[i] == '?' || url[i] == ':') break;
    }
    static char host[256];
    size_t n = i < sizeof(host) - 1 ? i : sizeof(host) - 1;
    memcpy(host, url, n);
    host[n] = '\0';
    // lowercase
    for (size_t j = 0; host[j]; j++) host[j] = tolower((unsigned char)host[j]);
    return host;
}

/**
 * Check if hostname is covered by a suffix-set entry.
 * "example.com" matches "example.com", "ads.example.com", etc.
 */
__attribute__((unused))
static int domain_in_set(const char* hostname, const char* suffix) {
    size_t hlen = strlen(hostname);
    size_t slen = strlen(suffix);
    if (hlen < slen) return 0;
    if (hlen == slen) return strcmp(hostname, suffix) == 0;
    // subdomain match: hostname ends with "." suffix
    if (hostname[hlen - slen - 1] == '.') {
        return strcmp(hostname + hlen - slen, suffix) == 0;
    }
    return 0;
}

EMSCRIPTEN_KEEPALIVE
int check_url(const char* url, const char* rules_json) {
    (void)rules_json; // reserved for future use
    // For now, just parse the URL
    const char* host = extract_hostname(url);
    if (!host[0]) return 0;
    // TODO: integrate with rule engine
    return 0;
}
