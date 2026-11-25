//! Edge cache for pre-rendered WordPress pages.
//!
//! The Jankx theme stores rendered HTML in `wp-content/cache/jankx/jankx_page`
//! as PHP-serialized entries. PHP still has to bootstrap the whole CMS before
//! it can read that file, which costs tens of milliseconds. LarvaPHP can read
//! the same entry *before* starting PHP and return the HTML directly — the
//! equivalent of a reverse-proxy/edge cache, and the fastest possible "worker
//! mode" for a CMS that is not worker-aware.
//!
//! The cache is read-only here: the theme remains the single writer, so its
//! purge/invalidation logic keeps working unchanged.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::context::PhpContext;

/// Where the Jankx page cache lives relative to the document root.
const DEFAULT_CACHE_DIR: &str = "wp-content/cache/jankx/jankx_page";
/// Cookie folded into the Jankx page key (currency variants).
const VARY_COOKIE: &str = "jankx_current_currency";

/// Query arguments removed from the cache key (Jankx `strip_query_args`).
const STRIP_QUERY_ARGS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "msclkid",
    "ref",
];

/// Query arguments that make a request uncacheable (Jankx `exclude_query_args`).
const EXCLUDE_QUERY_ARGS: &[&str] = &[
    "mode",
    "preview",
    "customize_changeset_uuid",
    "add-to-cart",
    "remove-from-cart",
    "remove_item",
    "undo_item",
    "wc-ajax",
    "wc-api",
    "rest_route",
    "jankx_purge",
    "jankx_fast_ajax",
    "preview_id",
    "replytocom",
];

/// Path prefixes that are never cached (Jankx `exclude_paths`).
const EXCLUDE_PATHS: &[&str] = &[
    "/wp-admin",
    "/wp-login.php",
    "/wp-cron.php",
    "/xmlrpc.php",
    "/wp-json",
    "/jankx-ajax",
    "/cart",
    "/checkout",
    "/my-account",
];

/// Cookies indicating a personalised response (Jankx `exclude_cookies`).
/// A trailing `*` is a prefix wildcard.
const EXCLUDE_COOKIES: &[&str] = &[
    "wordpress_logged_in_*",
    "woocommerce_items_in_cart",
    "woocommerce_cart_hash",
    "wp_woocommerce_session_*",
    "jankx_session",
    "comment_author_*",
];

/// A decoded Jankx page-cache entry.
#[derive(Debug, Clone)]
pub struct CachedPage {
    /// Path of the source cache file (used as the gzip-cache key).
    pub file: String,
    pub html: Vec<u8>,
    pub status: u16,
    pub content_type: String,
    pub created_at: i64,
    pub ttl: i64,
    pub expires: i64,
}

/// Read-only Jankx page-cache reader.
#[derive(Debug, Clone)]
pub struct EdgeCache {
    dir: PathBuf,
    enabled: bool,
}

impl EdgeCache {
    /// Build the reader for a document root.
    ///
    /// `LARVA_EDGE_CACHE=0` disables it; `LARVA_EDGE_CACHE_DIR` overrides the
    /// cache directory. Missing directories simply disable the fast path.
    pub fn new(root: &str) -> Self {
        let env_disabled = matches!(
            std::env::var("LARVA_EDGE_CACHE").as_deref(),
            Ok("0") | Ok("off") | Ok("false") | Ok("no")
        );
        let dir = std::env::var("LARVA_EDGE_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(root).join(DEFAULT_CACHE_DIR));
        let enabled = !env_disabled && dir.is_dir();
        Self { dir, enabled }
    }

    /// Whether a usable cache directory exists.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Return a fresh cached page for `ctx`, if one exists.
    pub fn lookup(&self, ctx: &PhpContext) -> Option<CachedPage> {
        if !self.enabled || !cacheable_method(ctx) || !cacheable_request(ctx) {
            return None;
        }

        let file = self.dir.join(key_for_request(ctx)?);
        let raw = std::fs::read(&file).ok()?;
        let page = decode_entry(&raw)?;

        let now = now_secs();
        if page.expires > 0 && page.expires <= now {
            return None;
        }
        if page.ttl > 0 && page.created_at + page.ttl < now {
            return None;
        }
        if page.html.is_empty() || page.status != 200 {
            return None;
        }

        Some(CachedPage {
            file: file.to_string_lossy().into_owned(),
            ..page
        })
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn cacheable_method(ctx: &PhpContext) -> bool {
    ctx.request_method.eq_ignore_ascii_case("GET") || ctx.request_method.eq_ignore_ascii_case("HEAD")
}

fn header<'a>(ctx: &'a PhpContext, name: &str) -> Option<&'a str> {
    ctx.env.iter().find_map(|(k, v)| {
        let key = std::str::from_utf8(k).ok()?;
        if !key.eq_ignore_ascii_case(name) {
            return None;
        }
        std::str::from_utf8(v).ok()
    })
}

fn combined_header(ctx: &PhpContext, name: &str) -> String {
    let mut out = String::new();
    for (k, v) in &ctx.env {
        let key = std::str::from_utf8(k).ok().unwrap_or_default();
        if key.eq_ignore_ascii_case(name) {
            if !out.is_empty() {
                out.push_str("; ");
            }
            out.push_str(std::str::from_utf8(v).unwrap_or_default());
        }
    }
    out
}

fn cookie_value(cookies: &[(String, String)], name: &str) -> String {
    cookies
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn parse_cookies(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

fn cacheable_request(ctx: &PhpContext) -> bool {
    let path = ctx.request_uri.split('?').next().unwrap_or("");
    if EXCLUDE_PATHS.iter().any(|p| path.starts_with(p)) {
        return false;
    }

    let query = parse_query(ctx.request_uri.split_once('?').map(|(_, q)| q).unwrap_or(""));
    if query.iter().any(|(name, _)| EXCLUDE_QUERY_ARGS.contains(&name.as_str())) {
        return false;
    }

    let cookies = parse_cookies(&combined_header(ctx, "cookie"));
    !cookies.iter().any(|(name, _)| {
        EXCLUDE_COOKIES.iter().any(|pattern| match pattern.strip_suffix('*') {
            Some(prefix) => name.starts_with(prefix),
            None => name == *pattern,
        })
    })
}

/// Reproduce `UrlKeyGenerator::generate()` from the Jankx theme.
fn key_for_request(ctx: &PhpContext) -> Option<String> {
    let raw_path = ctx.request_uri.split('?').next().unwrap_or("");
    let path = if raw_path.is_empty() || raw_path == "/" {
        "/".to_string()
    } else {
        raw_path.trim_end_matches('/').to_string()
    };

    let host = header(ctx, "host")
        .or(Some(ctx.server_name.as_str()))
        .unwrap_or("localhost")
        .to_ascii_lowercase();
    let secure = ctx.https == "on" || ctx.server_port == "443";
    let scheme = if secure { "https" } else { "http" };

    let query: Vec<(String, String)> = parse_query(ctx.request_uri.split_once('?').map(|(_, q)| q).unwrap_or(""))
        .into_iter()
        .filter(|(name, _)| !STRIP_QUERY_ARGS.contains(&name.as_str()) && !name.starts_with("utm_"))
        .collect();
    let mut sorted = query;
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut url = format!("{}://{}{}", scheme, host, path);
    if !sorted.is_empty() {
        url.push('?');
        url.push_str(&http_build_query(&sorted));
    }

    let cookies = parse_cookies(&combined_header(ctx, "cookie"));
    let vary = format!("{}={}", VARY_COOKIE, cookie_value(&cookies, VARY_COOKIE));
    let raw = format!("{}|{}|{}", ctx.request_method.to_uppercase(), url, vary);

    let key = format!("page_{}", md5_hex(raw.as_bytes()));
    let suffix = &sha1_hex(key.as_bytes())[..8];
    Some(format!("{}_{}.cache", key, suffix))
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    // PHP parse_str() keeps the last value for repeated scalar keys.
    let mut out: Vec<(String, String)> = Vec::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let (name, value) = (url_decode(name), url_decode(value));
        if let Some(existing) = out.iter_mut().find(|(k, _)| k == &name) {
            existing.1 = value;
        } else {
            out.push((name, value));
        }
    }
    out
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_val(bytes[i + 1]);
                let lo = hex_val(bytes[i + 2]);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push(hi * 16 + lo);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// PHP `http_build_query()` with the default RFC1738 encoding.
fn http_build_query(pairs: &[(String, String)]) -> String {
    let mut sorted = pairs.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for (i, (name, value)) in sorted.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        out.push_str(&url_encode(name));
        out.push('=');
        out.push_str(&url_encode(value));
    }
    out
}

fn url_encode(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Minimal PHP-serialization reader (the cache format used by the Jankx theme)
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum PhpValue {
    Array(Vec<(Vec<u8>, PhpValue)>),
    Bytes(Vec<u8>),
    Int(i64),
    Other,
}

impl PhpValue {
    fn get(&self, key: &str) -> Option<&PhpValue> {
        match self {
            PhpValue::Array(items) => items
                .iter()
                .find(|(k, _)| k.as_slice() == key.as_bytes())
                .map(|(_, v)| v),
            _ => None,
        }
    }

    fn bytes(&self) -> Option<&[u8]> {
        match self {
            PhpValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    fn int(&self) -> Option<i64> {
        match self {
            PhpValue::Int(i) => Some(*i),
            _ => None,
        }
    }
}

struct PhpParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> PhpParser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn eat(&mut self, token: &[u8]) -> bool {
        if self.bytes[self.pos..].starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    fn number(&mut self) -> Option<i64> {
        let start = self.pos;
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'-')
        {
            self.pos += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.pos])
            .ok()?
            .parse()
            .ok()
    }

    fn value(&mut self) -> Option<PhpValue> {
        match self.bytes.get(self.pos)? {
            b'a' => {
                self.eat(b"a:");
                let len = self.number()? as usize;
                if !self.eat(b":{") {
                    return None;
                }
                let mut items = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    let key = match self.value()? {
                        PhpValue::Bytes(b) => b,
                        PhpValue::Int(i) => i.to_string().into_bytes(),
                        _ => return None,
                    };
                    let val = self.value()?;
                    items.push((key, val));
                }
                if !self.eat(b"}") {
                    return None;
                }
                Some(PhpValue::Array(items))
            }
            b's' => {
                self.eat(b"s:");
                let len = self.number()? as usize;
                if !self.eat(b":\"") || self.pos + len + 2 > self.bytes.len() {
                    return None;
                }
                let data = self.bytes[self.pos..self.pos + len].to_vec();
                self.pos += len;
                if !self.eat(b"\";") {
                    return None;
                }
                Some(PhpValue::Bytes(data))
            }
            b'i' => {
                self.eat(b"i:");
                let n = self.number()?;
                if !self.eat(b";") {
                    return None;
                }
                Some(PhpValue::Int(n))
            }
            b'b' => {
                self.eat(b"b:");
                let b = self.bytes.get(self.pos).copied()?;
                self.pos += 1;
                if !self.eat(b";") {
                    return None;
                }
                Some(PhpValue::Int(if b == b'1' { 1 } else { 0 }))
            }
            b'N' => {
                self.eat(b"N;");
                Some(PhpValue::Other)
            }
            b'd' => {
                self.eat(b"d:");
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b';' {
                    self.pos += 1;
                }
                self.eat(b";");
                Some(PhpValue::Other)
            }
            _ => None,
        }
    }
}

fn decode_entry(raw: &[u8]) -> Option<CachedPage> {
    let root = PhpParser::new(raw).value()?;
    let expires = root.get("expires").and_then(PhpValue::int).unwrap_or(0);
    let value = root.get("value")?;
    let html = value.get("html")?.bytes()?.to_vec();
    let status = value
        .get("status")
        .and_then(PhpValue::int)
        .unwrap_or(200)
        .clamp(100, 599) as u16;
    let content_type = value
        .get("content_type")
        .and_then(PhpValue::bytes)
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_else(|| "text/html; charset=UTF-8".to_string());
    let created_at = value.get("created_at").and_then(PhpValue::int).unwrap_or(0);
    let ttl = value.get("ttl").and_then(PhpValue::int).unwrap_or(0);

    Some(CachedPage {
        file: String::new(),
        html,
        status,
        content_type,
        created_at,
        ttl,
        expires,
    })
}

// ---------------------------------------------------------------------------
// Hashes (kept local so the fast path has no extra dependencies)
// ---------------------------------------------------------------------------

fn md5_hex(data: &[u8]) -> String {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[u32; 64]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 64];
        for (i, slot) in t.iter_mut().enumerate() {
            *slot = (((i + 1) as f64).sin().abs() * 4_294_967_296.0) as u32;
        }
        t
    });

    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20,
        5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let mut msg = padded_le(data);
    let mut a0 = 0x6745_2301u32;
    let mut b0 = 0xefcd_ab89u32;
    let mut c0 = 0x98ba_dcfeu32;
    let mut d0 = 0x1032_5476u32;

    for chunk in msg.chunks_exact_mut(64) {
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            let j = i * 4;
            *word = u32::from_le_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f
                .wrapping_add(a)
                .wrapping_add(table[i])
                .wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = Vec::with_capacity(16);
    for word in [a0, b0, c0, d0] {
        out.extend_from_slice(&word.to_le_bytes());
    }
    hex(&out)
}

fn sha1_hex(data: &[u8]) -> String {
    let msg = padded_be(data);
    let mut h = [
        0x6745_2301u32,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            let j = i * 4;
            w[i] = u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = Vec::with_capacity(20);
    for word in h {
        out.extend_from_slice(&word.to_be_bytes());
    }
    hex(&out)
}

fn padded_le(data: &[u8]) -> Vec<u8> {
    let mut msg = data.to_vec();
    let bits = (msg.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_le_bytes());
    msg
}

fn padded_be(data: &[u8]) -> Vec<u8> {
    let mut msg = data.to_vec();
    let bits = (msg.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    msg
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_known_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            sha1_hex(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn homepage_key_matches_jankx_filename() {
        let raw = "GET|https://nghesachxua.vn/|jankx_current_currency=";
        let key = format!("page_{}", md5_hex(raw.as_bytes()));
        let file = format!("{}_{}.cache", key, &sha1_hex(key.as_bytes())[..8]);
        assert_eq!(file, "page_134bd15eddc961643a09df9ce75142c1_5953941b.cache");
    }

    #[test]
    fn decodes_php_serialized_entry() {
        let raw = br#"a:2:{s:7:"expires";i:4102444800;s:5:"value";a:6:{s:4:"html";s:5:"hello";s:6:"status";i:200;s:12:"content_type";s:24:"text/html; charset=UTF-8";s:4:"tags";a:0:{}s:10:"created_at";i:1700000000;s:3:"ttl";i:3600;}}"#;
        let page = decode_entry(raw).expect("decode");
        assert_eq!(page.html, b"hello");
        assert_eq!(page.status, 200);
        assert_eq!(page.content_type, "text/html; charset=UTF-8");
        assert_eq!(page.expires, 4_102_444_800);
    }

    #[test]
    fn http_build_query_sorts_and_encodes_like_php() {
        let pairs = vec![
            ("b".to_string(), "a b".to_string()),
            ("a".to_string(), "x/y".to_string()),
        ];
        assert_eq!(http_build_query(&pairs), "a=x%2Fy&b=a+b");
    }
}
