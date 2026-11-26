//! Shared HTML error page for 4xx/5xx responses.
//!
//! The page is intentionally self-contained: one inline `<style>` block, no
//! external CSS, fonts, scripts or images, and no per-request data. That keeps
//! it fast even when the origin is unhealthy and prevents request values from
//! leaking into an error response.

use hyper::StatusCode;

/// Inline styles. Kept separate from the markup so `format!` never has to
/// escape CSS braces.
const STYLES: &str = r#"
:root{color-scheme:dark}
*{box-sizing:border-box}
html{min-height:100%;background:#05070c}
body{min-height:100vh;margin:0;display:grid;place-items:center;padding:clamp(16px,4vw,40px);background:radial-gradient(circle at 12% 8%,rgba(54,239,197,.10),transparent 38%),radial-gradient(circle at 88% 92%,rgba(131,169,255,.09),transparent 36%),#05070c;color:#e8f0fb;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,"Liberation Mono",monospace;line-height:1.55}
body::before{content:"";position:fixed;inset:0;pointer-events:none;background:repeating-linear-gradient(180deg,rgba(255,255,255,.018) 0 1px,transparent 1px 4px)}
.shell{--accent:#36efc5;--accent-2:#83a9ff;--glow:rgba(54,239,197,.14);width:min(760px,100%);position:relative;overflow:hidden;border:1px solid #1e2c3d;border-radius:18px;background:linear-gradient(180deg,rgba(13,20,30,.98),rgba(6,10,16,.99));box-shadow:0 30px 90px rgba(0,0,0,.6),0 0 70px rgba(54,239,197,.08)}
.shell.is-server{--accent:#ff6f91;--accent-2:#ffc95c;--glow:rgba(255,111,145,.14);box-shadow:0 30px 90px rgba(0,0,0,.6),0 0 70px rgba(255,111,145,.1)}
.bar{display:flex;align-items:center;gap:8px;padding:12px 16px;border-bottom:1px solid #1e2c3d;background:rgba(255,255,255,.025);font-size:12px;color:#a8bbd0}
.dot{width:10px;height:10px;border-radius:50%;flex:0 0 auto}
.dot-red{background:#ff6f91}.dot-amber{background:#ffc95c}.dot-green{background:#36efc5}
.path{color:#dbe8f7;margin-left:6px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.meta{margin-left:auto;white-space:nowrap;color:#91a7bf}
main{padding:clamp(24px,5vw,48px)}
.kicker{margin:0 0 10px;font-size:12px;letter-spacing:.16em;text-transform:uppercase;color:var(--accent)}
.caret{display:inline-block;margin-right:6px;animation:blink 1.1s steps(1,end) infinite}
.sep{color:#7389a2;margin:0 6px}
.code{margin:0;font-size:clamp(68px,17vw,132px);font-weight:800;letter-spacing:-.09em;line-height:.92;color:var(--accent);background:linear-gradient(135deg,var(--accent),var(--accent-2));-webkit-background-clip:text;background-clip:text;-webkit-text-fill-color:transparent}
h1{font-size:clamp(20px,4.5vw,30px);line-height:1.2;margin:18px 0 10px;color:#f3f7ff}
.desc{margin:0;max-width:58ch;color:#c6d5e8;font-size:15px}
.trace{display:flex;flex-wrap:wrap;gap:8px;margin:24px 0;padding:0;list-style:none}
.trace li{display:flex;gap:8px;align-items:baseline;padding:8px 10px;border:1px solid #223349;border-radius:9px;background:rgba(255,255,255,.018);font-size:12px;color:#9db2ca}
.trace span{color:#91a7bf;text-transform:uppercase;letter-spacing:.08em}
.trace strong{color:#e4eefb;font-weight:600}
.actions{display:flex;flex-wrap:wrap;gap:12px}
.btn{display:inline-flex;align-items:center;justify-content:center;min-height:48px;min-width:48px;padding:0 18px;border:1px solid #36506c;border-radius:10px;color:#eaf2ff;background:transparent;text-decoration:none;font-weight:700;font-size:14px}
.btn.primary{border-color:var(--accent);background:var(--accent);color:#04120f}
.shell.is-server .btn.primary{color:#1a0409}
.btn:hover{border-color:var(--accent);box-shadow:0 0 0 3px var(--glow)}
.btn:focus-visible,a:focus-visible{outline:3px solid #fff;outline-offset:3px}
footer{display:flex;flex-wrap:wrap;justify-content:space-between;gap:8px;padding:14px 16px;border-top:1px solid #1e2c3d;color:#9db2ca;font-size:11px;letter-spacing:.04em}
.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}
@keyframes blink{50%{opacity:.15}}
@media(max-width:520px){.meta{display:none}.bar{padding:10px 12px}main{padding:24px 18px}.trace li{width:100%}.btn{flex:1 1 140px}}
@media(prefers-reduced-motion:reduce){.caret{animation:none}}
"#;

struct Copy {
    title: &'static str,
    message: &'static str,
}

fn copy_for(code: u16) -> Copy {
    let copy = match code {
        400 => Copy {
            title: "Yêu cầu không hợp lệ",
            message: "Yêu cầu gửi lên chưa đúng cấu trúc. Hãy kiểm tra lại đường dẫn, cookie và thử lại.",
        },
        401 => Copy {
            title: "Cần xác thực",
            message: "Bạn cần đăng nhập để xem nội dung này.",
        },
        403 => Copy {
            title: "Truy cập bị từ chối",
            message: "Máy chủ đã từ chối yêu cầu này. Kiểm tra quyền truy cập hoặc đường dẫn.",
        },
        404 => Copy {
            title: "Không tìm thấy trang",
            message: "Đường dẫn này không tồn tại hoặc đã được di chuyển. Quay lại trang chủ để tiếp tục.",
        },
        405 => Copy {
            title: "Phương thức không được phép",
            message: "Phương thức HTTP này không hợp lệ cho tài nguyên hiện tại.",
        },
        408 => Copy {
            title: "Yêu cầu hết thời gian chờ",
            message: "Yêu cầu không hoàn tất trong thời gian cho phép. Hãy thử lại.",
        },
        409 => Copy {
            title: "Yêu cầu xung đột",
            message: "Yêu cầu xung đột với trạng thái hiện tại của tài nguyên.",
        },
        410 => Copy {
            title: "Tài nguyên đã bị gỡ",
            message: "Tài nguyên này đã bị xóa vĩnh viễn và không còn đường dẫn thay thế.",
        },
        411 => Copy {
            title: "Thiếu Content-Length",
            message: "Yêu cầu cần có trường Content-Length trước khi được xử lý.",
        },
        412 => Copy {
            title: "Điều kiện không thỏa mãn",
            message: "Một điều kiện trong yêu cầu không còn đúng với tài nguyên hiện tại.",
        },
        413 => Copy {
            title: "Dữ liệu quá lớn",
            message: "Dữ liệu gửi lên vượt quá giới hạn cho phép của máy chủ.",
        },
        414 => Copy {
            title: "URL quá dài",
            message: "Đường dẫn yêu cầu quá dài. Hãy rút gọn rồi thử lại.",
        },
        415 => Copy {
            title: "Định dạng không được hỗ trợ",
            message: "Máy chủ không hỗ trợ định dạng dữ liệu của yêu cầu này.",
        },
        416 => Copy {
            title: "Phạm vi không hợp lệ",
            message: "Phạm vi byte bạn yêu cầu không nằm trong tệp hiện tại.",
        },
        418 => Copy {
            title: "Máy chủ là ấm trà",
            message: "Không thể pha cà phê từ ấm trà. Đây vẫn là lỗi 418 thật, không phải lỗi hiển thị.",
        },
        422 => Copy {
            title: "Không thể xử lý nội dung",
            message: "Nội dung yêu cầu đúng cú pháp nhưng sai ngữ nghĩa, nên không thể hoàn tất.",
        },
        429 => Copy {
            title: "Quá nhiều yêu cầu",
            message: "Bạn đã gửi quá nhiều yêu cầu trong thời gian ngắn. Vui lòng đợi một chút rồi thử lại.",
        },
        451 => Copy {
            title: "Không khả dụng vì pháp lý",
            message: "Nội dung này không thể hiển thị do giới hạn pháp lý.",
        },
        500 => Copy {
            title: "Lỗi máy chủ nội bộ",
            message: "Có lỗi bên trong máy chủ. Sự cố đã được ghi nhận; vui lòng thử lại sau ít phút.",
        },
        501 => Copy {
            title: "Chưa được hỗ trợ",
            message: "Máy chủ chưa hỗ trợ phương thức hoặc tính năng này.",
        },
        502 => Copy {
            title: "Phản hồi không hợp lệ",
            message: "Máy chủ trung gian nhận được phản hồi không hợp lệ từ nguồn phía sau.",
        },
        503 => Copy {
            title: "Dịch vụ tạm ngưng",
            message: "Dịch vụ đang bảo trì hoặc quá tải. Vui lòng thử lại trong ít phút.",
        },
        504 => Copy {
            title: "Nguồn phía sau chậm",
            message: "Nguồn phía sau không phản hồi kịp thời. Hãy thử lại sau.",
        },
        505 => Copy {
            title: "Phiên bản HTTP không được hỗ trợ",
            message: "Phiên bản HTTP trong yêu cầu này không được máy chủ hỗ trợ.",
        },
        507 => Copy {
            title: "Không đủ dung lượng",
            message: "Máy chủ không đủ dung lượng để lưu nội dung yêu cầu.",
        },
        _ if code >= 500 => Copy {
            title: "Lỗi máy chủ",
            message: "Máy chủ đang gặp sự cố khi xử lý yêu cầu. Vui lòng thử lại sau.",
        },
        _ => Copy {
            title: "Yêu cầu không thể hoàn tất",
            message: "Yêu cầu không thể được xử lý. Hãy kiểm tra lại rồi thử lại.",
        },
    };
    copy
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Render the complete HTML document for a 4xx/5xx status.
///
/// The output contains no request path, host, user agent, cookie or any other
/// caller-controlled value, so it can be shared across connections safely.
pub fn html(status: StatusCode) -> Vec<u8> {
    use std::fmt::Write as _;

    let code = status.as_u16();
    let copy = copy_for(code);
    let reason = status.canonical_reason().map(escape).unwrap_or_else(|| {
        if status.is_server_error() {
            "Lỗi máy chủ".to_string()
        } else {
            "Lỗi yêu cầu".to_string()
        }
    });
    let is_server = status.is_server_error();
    let shell_class = if is_server { "is-server" } else { "is-client" };
    let family = if is_server {
        "5xx / server"
    } else {
        "4xx / client"
    };
    let trace = format!("err-{code:04x}");

    let mut out = String::with_capacity(STYLES.len() + 1800);
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"vi\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta name=\"color-scheme\" content=\"dark\">\
         <meta name=\"robots\" content=\"noindex,nofollow\">\
         <title>{code} · {title}</title><style>",
        title = escape(copy.title)
    );
    out.push_str(STYLES);
    let _ = write!(
        out,
        r#"</style></head><body><div class="shell {shell_class}">
<div class="bar">
<span class="dot dot-red" aria-hidden="true"></span><span class="dot dot-amber" aria-hidden="true"></span><span class="dot dot-green" aria-hidden="true"></span>
<span class="path">larva://error/{code}</span><span class="meta">status · {family}</span>
</div>
<main>
<p class="kicker"><span class="caret" aria-hidden="true">▍</span> request terminated <span class="sep">/</span> {family}</p>
<p class="code" aria-hidden="true">{code}</p>
<h1><span class="sr-only">Mã lỗi {code}: </span>{title}</h1>
<p class="desc">{message}</p>
<ul class="trace" aria-label="Thông tin kỹ thuật">
<li><span>status</span><strong>{code} {reason}</strong></li>
<li><span>layer</span><strong>larva-php / edge</strong></li>
<li><span>trace</span><strong>{trace}</strong></li>
</ul>
<div class="actions">
<a class="btn primary" href="/">← Về trang chủ</a>
<a class="btn" href="javascript:history.back()" rel="nofollow">↩ Quay lại</a>
</div>
</main>
<footer><span>HTTP error response</span><span>inline css · no external assets</span></footer>
</div></body></html>"#,
        title = escape(copy.title),
        message = escape(copy.message),
    );

    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_is_self_contained_and_shows_status() {
        for status in [
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let page = String::from_utf8(html(status)).unwrap();
            assert!(page.starts_with("<!doctype html>"));
            assert!(page.contains(&format!("<title>{}", status.as_u16())));
            assert!(page.contains("<style>"));
            assert!(page.contains("lang=\"vi\""));
            assert!(page.contains("<h1>"));
            assert!(!page.contains("<link"));
            assert!(!page.contains("<script"));
            assert!(!page.contains("url(http"));
            assert!(!page.contains("url('http"));
        }
    }

    #[test]
    fn client_and_server_errors_use_different_visuals() {
        let not_found = String::from_utf8(html(StatusCode::NOT_FOUND)).unwrap();
        let server_error = String::from_utf8(html(StatusCode::INTERNAL_SERVER_ERROR)).unwrap();

        assert!(not_found.contains("shell is-client"));
        assert!(server_error.contains("shell is-server"));
        assert!(not_found.contains("Không tìm thấy trang"));
        assert!(server_error.contains("Lỗi máy chủ nội bộ"));
    }

    #[test]
    fn escape_does_not_allow_html_injection() {
        assert_eq!(escape("<b>&\"</b>"), "&lt;b&gt;&amp;&quot;&lt;/b&gt;");
    }
}
