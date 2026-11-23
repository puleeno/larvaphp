//! FFI bindings to the Zend Engine (PHP C API).
//!
//! This crate provides raw, unsafe bindings to the PHP Zend Engine, mirroring
//! the C bridge in `frankenphp.c` and the `#cgo` directives in the Go source.
//!
//! When the `bindgen` feature is enabled and PHP headers are available at build
//! time (via `PHP_INCLUDES`), the bindings are generated automatically. Otherwise,
//! the manual declarations in [`ffi`] are used.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

pub mod ffi;

pub use ffi::*;

// ---------------------------------------------------------------------------
// Embedded PHP SAPI shim (larva_embed.c, compiled by build.rs).
//
// All PHP entry points are resolved dynamically from php8.dll inside the shim,
// so no import library or PHP headers are needed to link.
// ---------------------------------------------------------------------------
extern "C" {
    pub fn larva_embed_init(php_ini_path: *const std::os::raw::c_char, ini_entries: *const std::os::raw::c_char) -> std::os::raw::c_int;
    pub fn larva_embed_is_ready() -> std::os::raw::c_int;
    pub fn larva_embed_execute(
        script_path: *const std::os::raw::c_char,
        out_buf: *mut std::os::raw::c_char,
        out_cap: std::os::raw::c_int,
        out_len: *mut std::os::raw::c_int,
    ) -> std::os::raw::c_int;
    pub fn larva_embed_shutdown();

    // ---- Request context (HTTP serving) ----

    pub fn larva_embed_execute_request(
        script_path: *const std::os::raw::c_char,
        req: *const LarvaEmbedRequest,
        resp: *mut LarvaEmbedResponse,
    ) -> std::os::raw::c_int;

    pub fn larva_embed_response_free(resp: *mut LarvaEmbedResponse);
}

/// A single "KEY=VALUE" server variable for $_SERVER.
#[repr(C)]
pub struct LarvaEmbedServerVar {
    pub key: *const std::os::raw::c_char,
    pub value: *const std::os::raw::c_char,
}

/// Request description passed to `larva_embed_execute_request`.
#[repr(C)]
pub struct LarvaEmbedRequest {
    pub method: *const std::os::raw::c_char,
    pub request_uri: *const std::os::raw::c_char,
    pub query_string: *const std::os::raw::c_char,
    pub content_type: *const std::os::raw::c_char,
    pub content_length: std::os::raw::c_long,
    pub body: *const std::os::raw::c_char,
    pub body_len: std::os::raw::c_long,
    pub cookie_data: *const std::os::raw::c_char,
    pub server_vars: *const LarvaEmbedServerVar,
    pub server_vars_count: std::os::raw::c_int,
}

/// Response returned from `larva_embed_execute_request`.
#[repr(C)]
pub struct LarvaEmbedResponse {
    pub status: std::os::raw::c_int,
    pub headers: *mut std::os::raw::c_char,
    pub headers_len: std::os::raw::c_int,
    pub body: *mut std::os::raw::c_char,
    pub body_len: std::os::raw::c_int,
}

/// PHP version constants
pub const PHP_VERSION_ID: i32 = 80500;

/// zval type tags (mirror `IS_*` constants from Zend/zend_types.h)
pub mod zval_type {
    pub const UNDEF: u8 = 0;
    pub const NULL: u8 = 1;
    pub const FALSE: u8 = 2;
    pub const TRUE: u8 = 3;
    pub const LONG: u8 = 4;
    pub const DOUBLE: u8 = 5;
    pub const STRING: u8 = 6;
    pub const ARRAY: u8 = 7;
    pub const OBJECT: u8 = 8;
    pub const RESOURCE: u8 = 9;
    pub const REFERENCE: u8 = 10;
    pub const PTR: u8 = 13;
}

/// Syslog levels (mirror the syslogLevel enum in frankenphp.go)
pub mod syslog_level {
    pub const EMERG: i32 = 0;
    pub const ALERT: i32 = 1;
    pub const CRIT: i32 = 2;
    pub const ERR: i32 = 3;
    pub const WARN: i32 = 4;
    pub const NOTICE: i32 = 5;
    pub const INFO: i32 = 6;
    pub const DEBUG: i32 = 7;
}

/// SAPI header return codes
pub mod sapi_header {
    pub const SENT_SUCCESSFULLY: i32 = 0;
    pub const SEND_FAILED: i32 = 1;
}

/// Thread states (mirror internal/state/state.go)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ThreadState {
    Reserved = 0,
    Booting = 1,
    BootRequested = 2,
    ShuttingDown = 3,
    Done = 4,
    Inactive = 5,
    Ready = 6,
    TransitionRequested = 7,
    TransitionInProgress = 8,
    TransitionComplete = 9,
    Rebooting = 10,
    ForceRebooting = 11,
    RebootReady = 12,
    YieldingForReboot = 13,
}

impl ThreadState {
    pub fn name(&self) -> &'static str {
        match self {
            ThreadState::Reserved => "reserved",
            ThreadState::Booting => "booting",
            ThreadState::BootRequested => "boot requested",
            ThreadState::ShuttingDown => "shutting down",
            ThreadState::Done => "done",
            ThreadState::Inactive => "inactive",
            ThreadState::Ready => "ready",
            ThreadState::TransitionRequested => "transition requested",
            ThreadState::TransitionInProgress => "transition in progress",
            ThreadState::TransitionComplete => "transition complete",
            ThreadState::Rebooting => "rebooting",
            ThreadState::ForceRebooting => "rebooting (force)",
            ThreadState::RebootReady => "reboot ready",
            ThreadState::YieldingForReboot => "yielding for reboot",
        }
    }

    pub fn is_stable(&self) -> bool {
        matches!(self, ThreadState::Inactive | ThreadState::Ready)
    }
}