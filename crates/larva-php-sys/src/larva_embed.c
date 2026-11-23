/* Minimal embedding shim for PHP (SAPI), mirroring frankenphp.c.
 *
 * All PHP entry points are resolved dynamically from php8.dll at runtime, so
 * this file does not need PHP headers or an import library to build.
 *
 * Struct layouts are copied from PHP 8.3 (main/SAPI.h, Zend/zend_stream.h).
 * The NTS php8.dll is not thread-safe; callers must serialise PHP access.
 */
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
#include <stdint.h>

#include "larva_embed.h"

/* ============================ PHP 8.3 ABI ============================ */

/* ---- sapi_module_struct (main/SAPI.h) ---- */
typedef struct _sapi_module_struct sapi_module_struct;
struct _sapi_module_struct {
    char *name;
    char *pretty_name;
    int (*startup)(sapi_module_struct *);
    int (*shutdown)(sapi_module_struct *);
    int (*activate)(void);
    int (*deactivate)(void);
    size_t (*ub_write)(const char *, size_t);
    void (*flush)(void *);
    void *(*get_stat)(void);
    char *(*getenv)(const char *, size_t);
    void (*sapi_error)(int, const char *, ...);
    int (*header_handler)(void *, int, void *);
    int (*send_headers)(void *);
    void (*send_header)(void *, void *);
    size_t (*read_post)(char *, size_t);
    char *(*read_cookies)(void);
    void (*register_server_variables)(void *);
    void (*log_message)(const char *, int);
    int (*get_request_time)(double *);
    void (*terminate_process)(void);
    char *php_ini_path_override;
    void (*default_post_reader)(void);
    void (*treat_data)(int, char *, void *);
    char *executable_location;
    int php_ini_ignore;
    int php_ini_ignore_cwd;
    int (*get_fd)(int *);
    int (*force_http_10)(void);
    int (*get_target_uid)(unsigned int *);
    int (*get_target_gid)(unsigned int *);
    unsigned int (*input_filter)(int, const char *, char **, size_t, size_t *);
    void (*ini_defaults)(void *);
    int phpinfo_as_text;
    const char *ini_entries;
    const void *additional_functions;
    unsigned int (*input_filter_init)(void);
};

/* ---- sapi_request_info (main/SAPI.h) ---- */
typedef struct {
    const char *request_method;
    char *query_string;
    char *cookie_data;
    int64_t content_length;
    char *path_translated;
    char *request_uri;
    void *request_body;       /* php_stream * */
    const char *content_type;
    unsigned char headers_only;
    unsigned char no_headers;
    unsigned char headers_read;
    void *post_entry;
    char *content_type_dup;
    char *auth_user;
    char *auth_password;
    char *auth_digest;
    char *argv0;
    char *current_user;
    int current_user_length;
    int argc;
    char **argv;
    int proto_num;
} sapi_request_info;

/* ---- sapi_globals_struct (partial: only the fields we access) ---- */
typedef struct {
    void *server_context;
    sapi_request_info request_info;
    /* rest (sapi_headers, etc.) not accessed directly */
} sapi_globals_partial;

/* ---- zend_file_handle (Zend/zend_stream.h) ---- */
typedef struct {
    union {
        FILE *fp;
        struct {
            void *handle;
            int isatty;
            void *reader;
            void *fsizer;
            void *closer;
        } stream;
    } handle;
    void *filename;
    void *opened_path;
    unsigned char type;
    unsigned char primary_script;
    unsigned char in_list;
    char *buf;
    size_t len;
} zend_file_handle;

/* ---- sapi_header_struct (main/SAPI.h) ---- */
typedef struct {
    char *header;
    size_t header_len;
} sapi_header_struct;

#define SAPI_HEADER_DO_SEND 2

/* ============================ PHP entry points ============================ */

static HMODULE g_php = NULL;
static int g_ready = 0;

static void (*p_sapi_startup)(void *) = NULL;
static int (*p_php_module_startup)(void *, void *) = NULL;
static int (*p_php_request_startup)(void) = NULL;
static void (*p_php_request_shutdown)(void *) = NULL;
static void (*p_php_execute_script)(void *) = NULL;
static void (*p_zend_stream_init_filename)(void *, const char *) = NULL;
static void (*p_zend_stream_open)(void *) = NULL;
static void (*p_add_assoc_stringl_ex)(void *, const char *, size_t, const char *, size_t) = NULL;

/* sapi_globals is a data export; GetProcAddress returns its address. */
static sapi_globals_partial *g_sapi_globals = NULL;

/* ============================ per-request state ============================ */

static char *g_output = NULL;
static size_t g_out_cap = 0;
static size_t g_out_len = 0;

static char *g_headers = NULL;
static size_t g_hdr_cap = 0;
static size_t g_hdr_len = 0;
static int g_status = 200;

static const char *g_req_body = NULL;
static long g_req_body_len = 0;
static long g_req_body_read = 0;
static char *g_cookie_copy = NULL;
static char *g_cookie_orig = NULL; /* tracked after read_cookies() takes ownership */

/* Extra $_SERVER variables passed from the Rust side. */
static const larva_embed_server_var *g_server_vars = NULL;
static int g_server_vars_count = 0;

/* ============================ SAPI callbacks ============================ */

static size_t my_ub_write(const char *str, size_t len) {
    if (!str || len == 0) {
        return len;
    }
    if (g_out_len + len + 1 > g_out_cap) {
        size_t new_cap = (g_out_len + len + 1) * 2;
        char *p = (char *)realloc(g_output, new_cap);
        if (!p) {
            return 0;
        }
        g_output = p;
        g_out_cap = new_cap;
    }
    memcpy(g_output + g_out_len, str, len);
    g_out_len += len;
    g_output[g_out_len] = '\0';
    return len;
}

static void my_flush(void *ctx) { (void)ctx; }

static int my_send_headers(void *h) {
    (void)h;
    /* Return SAPI_HEADER_DO_SEND so send_header is called for each header. */
    return SAPI_HEADER_DO_SEND;
}

static void my_send_header(void *hdr, void *ctx) {
    (void)ctx;
    sapi_header_struct *sh = (sapi_header_struct *)hdr;
    if (!sh || !sh->header) {
        return;
    }
    /* First header line is the status line (e.g. "HTTP/1.0 200 OK"). */
    if (g_status == 200 && g_hdr_len == 0) {
        const char *p = sh->header;
        if (p[0] == 'H' && p[1] == 'T' && p[2] == 'T' && p[3] == 'P' && p[4] == '/') {
            const char *sp = strchr(p, ' ');
            if (sp) {
                g_status = atoi(sp + 1);
            }
        }
    }
    if (g_hdr_len + sh->header_len + 2 > g_hdr_cap) {
        size_t new_cap = (g_hdr_len + sh->header_len + 2) * 2;
        char *p = (char *)realloc(g_headers, new_cap);
        if (!p) {
            return;
        }
        g_headers = p;
        g_hdr_cap = new_cap;
    }
    memcpy(g_headers + g_hdr_len, sh->header, sh->header_len);
    g_hdr_len += sh->header_len;
    g_headers[g_hdr_len++] = '\n';
    g_headers[g_hdr_len] = '\0';
}

static size_t my_read_post(char *buffer, size_t count_bytes) {
    if (!g_req_body || g_req_body_read >= g_req_body_len) {
        return 0;
    }
    size_t avail = (size_t)(g_req_body_len - g_req_body_read);
    size_t n = avail < count_bytes ? avail : count_bytes;
    memcpy(buffer, g_req_body + g_req_body_read, n);
    g_req_body_read += (long)n;
    return n;
}

static char *my_read_cookies(void) {
    /* Return the malloc'd cookie copy. PHP takes ownership (but may not free
     * it); we track the pointer so we can free it after request shutdown. */
    char *ret = g_cookie_copy;
    g_cookie_orig = g_cookie_copy;
    g_cookie_copy = NULL;
    return ret;
}

static void my_log_message(const char *msg, int t) {
    (void)t;
    if (msg) {
        fprintf(stderr, "[php] %s\n", msg);
    }
}

static void my_register_server_variables(void *track_vars) {
    if (!p_add_assoc_stringl_ex || !track_vars) {
        return;
    }
    sapi_request_info *ri = &g_sapi_globals->request_info;

#define ADD_STR(key, val) \
    do { \
        if (val) { \
            size_t _kl = strlen(key); \
            size_t _vl = strlen(val); \
            p_add_assoc_stringl_ex(track_vars, key, _kl, val, _vl); \
        } \
    } while (0)

    /* Variables derived from SG(request_info). */
    ADD_STR("REQUEST_METHOD", ri->request_method);
    ADD_STR("QUERY_STRING", ri->query_string);
    ADD_STR("CONTENT_TYPE", ri->content_type);
    ADD_STR("PATH_TRANSLATED", ri->path_translated);
    ADD_STR("REQUEST_URI", ri->request_uri);
    ADD_STR("SCRIPT_FILENAME", ri->path_translated);

    /* Extra server variables passed from the Rust side (HTTP_*, SCRIPT_NAME,
     * SERVER_NAME, etc.). */
    for (int i = 0; i < g_server_vars_count; i++) {
        if (g_server_vars[i].key && g_server_vars[i].value) {
            size_t _kl = strlen(g_server_vars[i].key);
            size_t _vl = strlen(g_server_vars[i].value);
            p_add_assoc_stringl_ex(track_vars, g_server_vars[i].key, _kl,
                                   g_server_vars[i].value, _vl);
        }
    }

#undef ADD_STR
}

/* ============================ symbol resolution ============================ */

static void *resolve(HMODULE m, const char *name) {
    return (void *)GetProcAddress(m, name);
}

#define FAIL_ON_NULL(p)                  \
    do {                                 \
        if (!(p)) {                      \
            return -2;                   \
        }                                \
    } while (0)

/* ============================ public API ============================ */

int larva_embed_is_ready(void) { return g_ready; }

int larva_embed_init(const char *php_ini_path, const char *ini_entries) {
    if (g_ready) {
        return 0;
    }

    g_php = LoadLibraryA("php8.dll");
    if (!g_php) {
        g_php = LoadLibraryA("php8ts.dll");
    }
    if (!g_php) {
        return -1;
    }

    p_sapi_startup = (void (*)(void *))resolve(g_php, "sapi_startup");
    FAIL_ON_NULL(p_sapi_startup);
    p_php_module_startup = (int (*)(void *, void *))resolve(g_php, "php_module_startup");
    FAIL_ON_NULL(p_php_module_startup);
    p_php_request_startup = (int (*)(void))resolve(g_php, "php_request_startup");
    FAIL_ON_NULL(p_php_request_startup);
    p_php_request_shutdown = (void (*)(void *))resolve(g_php, "php_request_shutdown");
    FAIL_ON_NULL(p_php_request_shutdown);
    p_php_execute_script = (void (*)(void *))resolve(g_php, "php_execute_script");
    FAIL_ON_NULL(p_php_execute_script);
    p_zend_stream_init_filename = (void (*)(void *, const char *))resolve(g_php, "zend_stream_init_filename");
    FAIL_ON_NULL(p_zend_stream_init_filename);
    p_zend_stream_open = (void (*)(void *))resolve(g_php, "zend_stream_open");
    p_add_assoc_stringl_ex = (void (*)(void *, const char *, size_t, const char *, size_t))resolve(g_php, "add_assoc_stringl_ex");

    /* sapi_globals is a data export (main/SAPI.h: extern sapi_globals_struct). */
    g_sapi_globals = (sapi_globals_partial *)resolve(g_php, "sapi_globals");

    sapi_module_struct sapi;
    memset(&sapi, 0, sizeof(sapi));
    /* The SAPI name must be one of the names in opcache's supported_sapis
     * list (ext/opcache/ZendAccelerator.c: accel_find_sapi) or opcache will
     * refuse to start with "Opcode Caching is only supported in ... SAPIs".
     * FrankenPHP uses "frankenphp", which is in the list. */
    sapi.name = "frankenphp";
    sapi.pretty_name = "LarvaPHP Embedded";
    sapi.ub_write = my_ub_write;
    sapi.flush = my_flush;
    sapi.send_headers = my_send_headers;
    sapi.send_header = my_send_header;
    sapi.read_post = my_read_post;
    sapi.read_cookies = my_read_cookies;
    sapi.log_message = my_log_message;
    sapi.register_server_variables = my_register_server_variables;

    if (php_ini_path) {
        sapi.php_ini_path_override = (char *)php_ini_path;
    }

    p_sapi_startup(&sapi);

    /* sapi_startup() NULLs sf->ini_entries before copying the struct to the
     * global sapi_module. php_module_startup() later re-copies the struct
     * (`sapi_module = *sf`), so ini_entries must be set AFTER sapi_startup
     * but BEFORE php_module_startup. This is how FrankenPHP does it. */
    if (ini_entries) {
        sapi.ini_entries = ini_entries;
    }

    int rc = p_php_module_startup(&sapi, NULL);
    if (rc != 0) {
        return rc;
    }

    g_ready = 1;
    return 0;
}

/* Reset the output/headers buffers before each request. */
static void reset_buffers(void) {
    g_out_len = 0;
    if (g_output) {
        g_output[0] = '\0';
    }
    g_hdr_len = 0;
    if (g_headers) {
        g_headers[0] = '\0';
    }
    g_status = 200;
}

/* ---- Simple (CLI-like) execution ---- */

int larva_embed_execute(const char *script_path, char *out_buf, int out_cap, int *out_len) {
    if (!g_ready || !script_path) {
        return -1;
    }

    reset_buffers();

    int rc = p_php_request_startup();
    if (rc != 0) {
        return rc;
    }

    zend_file_handle fh;
    memset(&fh, 0, sizeof(fh));
    p_zend_stream_init_filename(&fh, script_path);
    fh.primary_script = 1;

    if (p_zend_stream_open) {
        p_zend_stream_open(&fh);
    }

    p_php_execute_script(&fh);

    p_php_request_shutdown(NULL);

    int n = (int)g_out_len;
    if (out_buf && out_cap > 0) {
        int copy = n;
        if (copy > out_cap - 1) {
            copy = out_cap - 1;
        }
        if (copy > 0) {
            memcpy(out_buf, g_output, (size_t)copy);
        }
        out_buf[copy] = '\0';
    }
    if (out_len) {
        *out_len = n;
    }
    return 0;
}

/* ---- Full request execution ---- */

int larva_embed_execute_request(
    const char *script_path,
    const larva_embed_request *req,
    larva_embed_response *resp)
{
    if (!g_ready || !script_path || !req || !resp) {
        return -1;
    }
    if (!g_sapi_globals) {
        return -3;
    }

    reset_buffers();

    /* Populate SG(request_info) before php_request_startup. */
    sapi_request_info *ri = &g_sapi_globals->request_info;

    ri->request_method = req->method;
    ri->query_string = req->query_string ? (char *)req->query_string : NULL;
    ri->request_uri = req->request_uri;
    ri->path_translated = (char *)script_path;
    ri->content_type = req->content_type;
    ri->content_length = req->content_length;
    ri->headers_read = 0;
    ri->headers_only = 0;
    ri->no_headers = 0;

    /* Store request body for the read_post callback. */
    g_req_body = req->body;
    g_req_body_len = req->body_len;
    g_req_body_read = 0;

    /* Store extra server variables for register_server_variables. */
    g_server_vars = req->server_vars;
    g_server_vars_count = req->server_vars_count;

    /* Prepare cookie data: read_cookies returns a malloc'd copy. */
    if (req->cookie_data) {
        g_cookie_copy = (char *)malloc(strlen(req->cookie_data) + 1);
        if (g_cookie_copy) {
            strcpy(g_cookie_copy, req->cookie_data);
        }
    } else {
        g_cookie_copy = NULL;
    }

    /* Set server_context to non-NULL so sapi_activate reads POST + cookies. */
    static int dummy_server_context;
    g_sapi_globals->server_context = &dummy_server_context;

    int rc = p_php_request_startup();
    if (rc != 0) {
        /* Clean up cookie copy if request startup failed. */
        if (g_cookie_copy) { free(g_cookie_copy); g_cookie_copy = NULL; }
        return rc;
    }

    zend_file_handle fh;
    memset(&fh, 0, sizeof(fh));
    p_zend_stream_init_filename(&fh, script_path);
    fh.primary_script = 1;

    if (p_zend_stream_open) {
        p_zend_stream_open(&fh);
    }

    p_php_execute_script(&fh);

    p_php_request_shutdown(NULL);

    /* Free cookie copy. If read_cookies() was called, g_cookie_orig holds the
     * pointer PHP now owns; otherwise g_cookie_copy still holds it. */
    if (g_cookie_orig) { free(g_cookie_orig); g_cookie_orig = NULL; }
    if (g_cookie_copy) { free(g_cookie_copy); g_cookie_copy = NULL; }

    /* Build the response. */
    resp->status = g_status;
    resp->headers_len = (int)g_hdr_len;
    resp->headers = g_headers;
    g_headers = NULL;
    g_hdr_cap = 0;
    g_hdr_len = 0;

    resp->body_len = (int)g_out_len;
    resp->body = g_output;
    g_output = NULL;
    g_out_cap = 0;
    g_out_len = 0;

    return 0;
}

void larva_embed_response_free(larva_embed_response *resp) {
    if (!resp) {
        return;
    }
    if (resp->headers) {
        free(resp->headers);
        resp->headers = NULL;
    }
    if (resp->body) {
        free(resp->body);
        resp->body = NULL;
    }
    resp->headers_len = 0;
    resp->body_len = 0;
}

void larva_embed_shutdown(void) {
    if (!g_ready) {
        return;
    }
    g_ready = 0;
    if (g_output) { free(g_output); g_output = NULL; }
    if (g_headers) { free(g_headers); g_headers = NULL; }
    g_out_cap = g_out_len = 0;
    g_hdr_cap = g_hdr_len = 0;
    g_php = NULL;
    g_sapi_globals = NULL;
}