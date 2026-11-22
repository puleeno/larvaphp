/* Minimal embedding shim for PHP (SAPI), mirroring frankenphp.c.
 *
 * All PHP entry points are resolved dynamically from php8.dll at runtime, so
 * this shim does not need PHP headers or an import library to build. It is
 * compiled into the larva-php binary by larva-php-sys/build.rs.
 *
 * The NTS php8.dll is not thread-safe; callers must serialize PHP access.
 */
#ifndef LARVA_EMBED_H
#define LARVA_EMBED_H

#ifdef __cplusplus
extern "C" {
#endif

/* Initialise the embedded PHP runtime.
 *   php_ini_path : full path to php.ini, or NULL to use the default lookup.
 *   ini_entries  : extra INI directives (newline separated), or NULL.
 * Returns 0 on success, non-zero on error. */
int larva_embed_init(const char *php_ini_path, const char *ini_entries);

/* 1 if the runtime has been initialised successfully. */
int larva_embed_is_ready(void);

/* Execute a PHP script (CLI-like: no request context). The captured output is
 * copied into out_buf (NUL terminated when it fits) and its length stored in
 * *out_len. Returns 0 on success, non-zero on error. */
int larva_embed_execute(const char *script_path, char *out_buf, int out_cap, int *out_len);

/* ---- Request context (HTTP serving) ---- */

/* A single "KEY=VALUE" server variable for $_SERVER. */
typedef struct {
    const char *key;
    const char *value;
} larva_embed_server_var;

/* Request description passed to larva_embed_execute_request. */
typedef struct {
    const char *method;          /* "GET", "POST", ...                   */
    const char *request_uri;     /* "/index.php?foo=bar"                 */
    const char *query_string;    /* "foo=bar" (may be NULL)              */
    const char *content_type;    /* request Content-Type, or NULL        */
    long content_length;         /* request Content-Length, or 0         */
    const char *body;            /* request body, or NULL                */
    long body_len;               /* request body length                  */
    const char *cookie_data;     /* Cookie header value, or NULL         */
    const larva_embed_server_var *server_vars; /* extra $_SERVER entries */
    int server_vars_count;
} larva_embed_request;

/* Response returned from larva_embed_execute_request. Caller owns headers
 * and body buffers (free with larva_embed_response_free). */
typedef struct {
    int status;
    char *headers;    /* captured header lines, newline-separated */
    int headers_len;
    char *body;       /* captured body (NUL terminated) */
    int body_len;
} larva_embed_response;

/* Execute a PHP script with full request context. Sets up $_SERVER, $_GET,
 * $_POST, $_COOKIE, php://input, then runs the script and captures the
 * response (status, headers, body).
 * Returns 0 on success, non-zero on error. */
int larva_embed_execute_request(
    const char *script_path,
    const larva_embed_request *req,
    larva_embed_response *resp);

/* Free buffers allocated by larva_embed_execute_request. */
void larva_embed_response_free(larva_embed_response *resp);

/* Tear the runtime down (optional). */
void larva_embed_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* LARVA_EMBED_H */