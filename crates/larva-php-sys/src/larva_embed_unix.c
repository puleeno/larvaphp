/* Real embedded PHP SAPI shim for Unix (Linux/macOS).
 *
 * Unlike the old Windows-only shim, this file uses the PHP development headers
 * directly instead of hardcoded ABI layouts. It is linked against the PHP
 * embed library (libphp8.x.so), so there are no no-op/stub implementations of
 * the PHP entry points: requests really run inside the Zend Engine.
 *
 * PHP's NTS embed library is not thread-safe. The Rust side serialises all
 * access through a mutex (see embed_php.rs).
 */

#include <php.h>
#include <php_main.h>
#include <SAPI.h>
#include <zend_call_stack.h>
#include <zend_stream.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "larva_embed.h"

/* ------------------------------------------------------------------------- */
/* Runtime state                                                             */
/* ------------------------------------------------------------------------- */

static int g_ready = 0;
static sapi_module_struct g_sapi;

static char *g_output = NULL;
static size_t g_out_cap = 0;
static size_t g_out_len = 0;

static char *g_headers = NULL;
static size_t g_hdr_cap = 0;
static size_t g_hdr_len = 0;
static int g_status = 200;
static int g_status_set = 0;

static const char *g_req_body = NULL;
static zend_long g_req_body_len = 0;
static zend_long g_req_body_read = 0;
static char *g_cookie_copy = NULL;

static const larva_embed_server_var *g_server_vars = NULL;
static int g_server_vars_count = 0;

/* ------------------------------------------------------------------------- */
/* SAPI callbacks                                                            */
/* ------------------------------------------------------------------------- */

static size_t larva_ub_write(const char *str, size_t len) {
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

static void larva_flush(void *server_context) {
    (void)server_context;
}

static int larva_send_headers(sapi_headers_struct *sapi_headers) {
    (void)sapi_headers;
    /* Make PHP call send_header() once per header. */
    return SAPI_HEADER_DO_SEND;
}

/* PHP's embed SAPI sends either "HTTP/1.x <code> <reason>" or
 * "Status: <code> <reason>". Some builds emit code 0 for the default; keep
 * 200 in that case. */
static void note_status(const char *line) {
    int code = 0;
    if (strncmp(line, "HTTP/", 5) == 0) {
        const char *sp = strchr(line, ' ');
        if (sp) {
            code = atoi(sp + 1);
        }
    } else if (strncmp(line, "Status:", 7) == 0) {
        code = atoi(line + 7);
    }
    if (code >= 100 && code <= 599 && (!g_status_set || code != 200)) {
        g_status = code;
        g_status_set = 1;
    }
}

static void larva_send_header(sapi_header_struct *header, void *server_context) {
    (void)server_context;
    if (!header || !header->header) {
        return;
    }

    note_status(header->header);

    if (g_hdr_len + header->header_len + 2 > g_hdr_cap) {
        size_t new_cap = (g_hdr_len + header->header_len + 2) * 2;
        char *p = (char *)realloc(g_headers, new_cap);
        if (!p) {
            return;
        }
        g_headers = p;
        g_hdr_cap = new_cap;
    }
    memcpy(g_headers + g_hdr_len, header->header, header->header_len);
    g_hdr_len += header->header_len;
    g_headers[g_hdr_len++] = '\n';
    g_headers[g_hdr_len] = '\0';
}

static size_t larva_read_post(char *buffer, size_t count_bytes) {
    if (!g_req_body || g_req_body_read >= g_req_body_len) {
        return 0;
    }
    size_t avail = (size_t)(g_req_body_len - g_req_body_read);
    size_t n = avail < count_bytes ? avail : count_bytes;
    memcpy(buffer, g_req_body + g_req_body_read, n);
    g_req_body_read += (zend_long)n;
    return n;
}

static char *larva_read_cookies(void) {
    char *ret = g_cookie_copy;
    g_cookie_copy = NULL;
    return ret;
}

static void larva_log_message(const char *message, int syslog_type_int) {
    (void)syslog_type_int;
    if (message) {
        fprintf(stderr, "[php] %s\n", message);
    }
}

#define ADD_STR(key, val)                                                     \
    do {                                                                      \
        if ((val)) {                                                          \
            add_assoc_stringl_ex(track_vars_array, (key), strlen(key),         \
                                 (char *)(val), strlen((val)));                \
        }                                                                     \
    } while (0)

static void larva_register_server_variables(zval *track_vars_array) {
    sapi_request_info *ri = &SG(request_info);

    ADD_STR("REQUEST_METHOD", ri->request_method);
    ADD_STR("QUERY_STRING", ri->query_string);
    ADD_STR("CONTENT_TYPE", ri->content_type);
    {
        char content_length[32];
        int n = snprintf(content_length, sizeof(content_length), "%ld",
                         (long)ri->content_length);
        if (n > 0) {
            add_assoc_stringl_ex(track_vars_array, "CONTENT_LENGTH",
                                 strlen("CONTENT_LENGTH"), content_length,
                                 (size_t)n);
        }
    }
    ADD_STR("PATH_TRANSLATED", ri->path_translated);
    ADD_STR("REQUEST_URI", ri->request_uri);
    ADD_STR("SCRIPT_FILENAME", ri->path_translated);

    for (int i = 0; i < g_server_vars_count; i++) {
        if (g_server_vars[i].key && g_server_vars[i].value) {
            add_assoc_stringl_ex(track_vars_array,
                                 g_server_vars[i].key,
                                 strlen(g_server_vars[i].key),
                                 (char *)g_server_vars[i].value,
                                 strlen(g_server_vars[i].value));
        }
    }
}

#undef ADD_STR

/* ------------------------------------------------------------------------- */
/* Helpers                                                                   */
/* ------------------------------------------------------------------------- */

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
    g_status_set = 0;
}

static void free_cookie(void) {
    free(g_cookie_copy);
    g_cookie_copy = NULL;
}

static int execute_script_internal(const char *script_path) {
    reset_buffers();

    /* PHP only parses POST bodies and cookies when a server context exists.
     * The Windows bridge already does this; the Unix bridge missed it, which
     * left $_POST empty even though php://input had the body. */
    static int dummy_server_context;
    SG(server_context) = &dummy_server_context;

    /* PHP's Zend stack guard is initialised once at module startup. Embedded
     * PHP requests may run on a different thread (HTTP/3 workers use a 64 MiB
     * stack), so the cached base/limit would be from another thread's stack and
     * every script would fail with a false "Maximum call stack size reached".
     * Recompute them for the current thread before each request. */
#ifdef ZEND_CHECK_STACK_LIMIT
    zend_call_stack_init();
#endif

    if (php_request_startup() != SUCCESS) {
        return -1;
    }

#ifdef ZEND_CHECK_STACK_LIMIT
    /* Request activation must not keep a stale base from module startup. */
    zend_call_stack_init();
#endif

    /* The embedded SAPI is persistent: php_request_shutdown() does not always
     * clear SG(sapi_headers).http_response_code. A redirect from an earlier
     * request (e.g. /wp-admin/ -> 302) therefore leaked into every following
     * response, including load-scripts.php bundles, which browsers refused to
     * execute as JavaScript. Reset it after sapi_activate() so the script can
     * still set its own status afterwards. */
    SG(sapi_headers).http_response_code = 200;

    zend_file_handle fh;
    memset(&fh, 0, sizeof(fh));
    zend_stream_init_filename(&fh, script_path);
    fh.primary_script = 1;
    php_execute_script(&fh);
    zend_destroy_file_handle(&fh);

    php_request_shutdown(NULL);
    free_cookie();
    return 0;
}

/* ------------------------------------------------------------------------- */
/* Public API                                                                */
/* ------------------------------------------------------------------------- */

int larva_embed_is_ready(void) {
    return g_ready;
}

int larva_embed_init(const char *php_ini_path, const char *ini_entries) {
    if (g_ready) {
        return 0;
    }

    memset(&g_sapi, 0, sizeof(g_sapi));
    g_sapi.name = "frankenphp";
    g_sapi.pretty_name = "LarvaPHP Embedded";
    g_sapi.ub_write = larva_ub_write;
    g_sapi.flush = larva_flush;
    g_sapi.send_headers = larva_send_headers;
    g_sapi.send_header = larva_send_header;
    g_sapi.read_post = larva_read_post;
    g_sapi.read_cookies = larva_read_cookies;
    g_sapi.log_message = larva_log_message;
    g_sapi.register_server_variables = larva_register_server_variables;

    if (php_ini_path) {
        g_sapi.php_ini_path_override = (char *)php_ini_path;
    }

    sapi_startup(&g_sapi);

    /* sapi_startup() clears sf->ini_entries; PHP copies the SAPI struct again
     * in php_module_startup(), so ini_entries must be set in between. */
    if (ini_entries) {
        g_sapi.ini_entries = ini_entries;
    }

    if (php_module_startup(&g_sapi, NULL) != SUCCESS) {
        return -1;
    }

    g_ready = 1;
    return 0;
}

int larva_embed_execute(const char *script_path, char *out_buf, int out_cap, int *out_len) {
    if (!g_ready || !script_path) {
        return -1;
    }

    /* CLI-like execution: no request body, no extra server variables. */
    g_req_body = NULL;
    g_req_body_len = 0;
    g_req_body_read = 0;
    g_server_vars = NULL;
    g_server_vars_count = 0;

    SG(request_info).request_method = "GET";
    SG(request_info).query_string = NULL;
    SG(request_info).cookie_data = NULL;
    SG(request_info).content_length = 0;
    SG(request_info).content_type = NULL;
    SG(request_info).path_translated = (char *)script_path;
    SG(request_info).request_uri = (char *)script_path;
    SG(request_info).proto_num = 1001;

    int rc = execute_script_internal(script_path);
    if (rc != 0) {
        return rc;
    }

    int n = (int)g_out_len;
    if (out_len) {
        *out_len = n;
    }
    if (out_buf && out_cap > 0) {
        int copy = n < out_cap - 1 ? n : out_cap - 1;
        if (copy > 0) {
            memcpy(out_buf, g_output, (size_t)copy);
        }
        out_buf[copy] = '\0';
    }
    return 0;
}

int larva_embed_execute_request(
    const char *script_path,
    const larva_embed_request *req,
    larva_embed_response *resp) {
    if (!g_ready || !script_path || !req || !resp) {
        return -1;
    }

    char *path_copy = strdup(script_path);
    if (!path_copy) {
        return -2;
    }
    free_cookie();
    if (req->cookie_data) {
        g_cookie_copy = strdup(req->cookie_data);
    }

    g_req_body = req->body;
    g_req_body_len = req->body_len;
    g_req_body_read = 0;
    g_server_vars = req->server_vars;
    g_server_vars_count = req->server_vars_count;

    SG(request_info).request_method = req->method;
    SG(request_info).query_string = (char *)req->query_string;
    SG(request_info).cookie_data = g_cookie_copy;
    SG(request_info).content_length = (zend_long)req->content_length;
    SG(request_info).content_type = (char *)req->content_type;
    SG(request_info).path_translated = path_copy;
    SG(request_info).request_uri = (char *)req->request_uri;
    SG(request_info).proto_num = 1001;

    int rc = execute_script_internal(script_path);
    free(path_copy);
    free_cookie();
    if (rc != 0) {
        return rc;
    }

    resp->status = g_status;
    resp->headers_len = (int)g_hdr_len;
    resp->body_len = (int)g_out_len;
    resp->headers = NULL;
    resp->body = NULL;

    if (g_hdr_len > 0) {
        resp->headers = (char *)malloc(g_hdr_len + 1);
        if (!resp->headers) {
            return -3;
        }
        memcpy(resp->headers, g_headers, g_hdr_len);
        resp->headers[g_hdr_len] = '\0';
    }
    if (g_out_len > 0) {
        resp->body = (char *)malloc(g_out_len + 1);
        if (!resp->body) {
            free(resp->headers);
            resp->headers = NULL;
            return -3;
        }
        memcpy(resp->body, g_output, g_out_len);
        resp->body[g_out_len] = '\0';
    }
    return 0;
}

void larva_embed_response_free(larva_embed_response *resp) {
    if (!resp) {
        return;
    }
    free(resp->headers);
    free(resp->body);
    resp->headers = NULL;
    resp->body = NULL;
    resp->headers_len = 0;
    resp->body_len = 0;
}

void larva_embed_shutdown(void) {
    if (!g_ready) {
        return;
    }
    php_module_shutdown();
    sapi_shutdown();
    g_ready = 0;
    free(g_output);
    free(g_headers);
    g_output = NULL;
    g_headers = NULL;
    g_out_cap = 0;
    g_hdr_cap = 0;
}
