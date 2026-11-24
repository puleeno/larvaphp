/* Unix (Linux/macOS) compatibility stubs for LarvaPHP.
 *
 * The real embedded runtime (larva_embed.c) is Windows-only: it loads php8.dll
 * through the Win32 API and mirrors the Windows PHP 8.3 ABI. On Unix, the
 * server's default CGI/FastCGI transports do not call into the Zend Engine, so
 * we only need to satisfy the FFI symbols referenced by the Rust crate.
 *
 * These stubs deliberately fail at runtime: embedded PHP mode is not supported
 * on Unix by this crate. Building with `--features embedded` still works, but
 * calling the embedded executor returns an error instead of crashing.
 */

#include <stddef.h>
#include <string.h>

#include "larva_embed.h"

/* ---- PHP C API symbols referenced by stubs.c --------------------------- */

int php_module_startup(void *sapi_module, void *additional_modules, int num_additional_modules) {
    (void)sapi_module;
    (void)additional_modules;
    (void)num_additional_modules;
    return -1;
}

int php_module_shutdown(void) { return -1; }
void php_request_startup(void) {}
void php_request_shutdown(void) {}

int php_execute_script(void *file_handle) {
    (void)file_handle;
    return -1;
}

void php_register_variable_safe(const char *key, const char *val, size_t val_len, void *track_vars_array) {
    (void)key;
    (void)val;
    (void)val_len;
    (void)track_vars_array;
}

void php_register_variable(const char *key, const char *val, size_t val_len, void *track_vars_array) {
    (void)key;
    (void)val;
    (void)val_len;
    (void)track_vars_array;
}

void *php_output_get_contents(void) { return NULL; }
void php_output_activate(void) {}
void php_output_end_all(void) {}

int zend_stream_init_filename(void *handle, const char *filename) {
    (void)handle;
    (void)filename;
    return -1;
}

int zend_stream_open(void *handle) {
    (void)handle;
    return -1;
}

void zend_destroy_file_handle(void *handle) { (void)handle; }

/* ---- Embedded executor stubs ------------------------------------------- */

int larva_embed_init(const char *php_ini_path, const char *ini_entries) {
    (void)php_ini_path;
    (void)ini_entries;
    return -1;
}

int larva_embed_is_ready(void) { return 0; }

int larva_embed_execute(const char *script_path, char *out_buf, int out_cap, int *out_len) {
    (void)script_path;
    if (out_len) {
        *out_len = 0;
    }
    if (out_buf && out_cap > 0) {
        out_buf[0] = '\0';
    }
    return -1;
}

int larva_embed_execute_request(
    const char *script_path,
    const larva_embed_request *req,
    larva_embed_response *resp) {
    (void)script_path;
    (void)req;
    if (resp) {
        memset(resp, 0, sizeof(*resp));
        resp->status = 500;
    }
    return -1;
}

void larva_embed_response_free(larva_embed_response *resp) { (void)resp; }
void larva_embed_shutdown(void) {}
