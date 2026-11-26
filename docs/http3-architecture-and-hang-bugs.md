# HTTP/3 trong LarvaPHP: kiến trúc, bug treo request và cách phòng tránh

Tài liệu này mô tả cách LarvaPHP (fork của FrankenPHP, branch `v1`) triển khai HTTP/3/QUIC, vì sao một request có thể treo rất lâu (biểu hiện: Waterfox báo "waiting", có khi hàng chục phút), cách đã sửa và cách rollback an toàn.

Số liệu đo thực tế tại thời điểm viết (2026-11-25, `https://nghesachxua.vn/`):

| Đối tượng | Kết quả |
| --- | --- |
| `curl --http2` | `HTTP/2 200`, TTFB thường `0,05–0,25s` |
| `curl --http1.1` | `200`, HTTP/1.1 |
| Asset tĩnh / edge cache | `x-larva-edge-cache: HIT`, TTFB thường `< 0,15s` |
| Cert | `CN=nghesachxua.vn`, Let's Encrypt, `openssl verify` trả `0 (ok)` |
| TCP 80/443 | do tiến trình `larva-php` lắng nghe |
| UDP 443 | mở; từ 2026-11-26 16:00 đã **quảng cáo H3** qua `Alt-Svc` |

Kết luận: PHP, static file, edge cache, TLS và mạng TCP đều nhanh. Thời gian treo nằm ở **đường vào HTTP/3 (QUIC)** và cơ chế fallback của browser sau khi đọc `Alt-Svc`.

---

## 1. Kiến trúc HTTP/3

### 1.1. Hai transport dùng chung một core

```text
                    ┌──────────────────────────────────────────────┐
  Browser           │ TCP :80/:443          UDP :443               │
  (Alt-Svc: h3) ───►│ hyper (H1/H2)         quiche + h3 (H3)       │
                    │        │                    │                 │
                    │        ▼                    ▼                 │
                    │   Server::handle_request_parts()  ← core      │
                    │   (static / edge cache / PHP CGI)             │
                    └──────────────────────────────────────────────┘
```

- `crates/larva-php/src/http3.rs`: phần QUIC/h3, chỉ biên dịch khi bật feature `http3`.
- `crates/larva-php/src/server.rs`: core xử lý chung cho H1/H2/H3.
- `crates/larva-php/src/main.rs`: khởi tạo listener. Điều kiện vẫn là `args.http3 || args.lets_encrypt` để giữ tương thích, nhưng **chỉ gọi `set_http3_port()` khi có `--http3`**.

Hệ quả sau bản sửa:

- Chỉ `--lets-encrypt` (trạng thái trước 2026-11-26 16:00): UDP listener chạy, nhưng **không gửi** `Alt-Svc: h3`.
- `--lets-encrypt --http3` (production hiện tại): UDP listener **và** quảng cáo `Alt-Svc: h3=":443"; ma=86400`.

### 1.2. Thread model

`Http3Server::run()` chạy trên một thread riêng và là nơi duy nhất được chạm vào `quiche::Connection` / `h3::Connection`.

| Thành phần | Vai trò |
| --- | --- |
| `UdpSocket` | Nhận/gửi datagram; timeout đọc `100ms` |
| `clients: HashMap<Vec<u8>, Client>` | Mọi kết nối QUIC đang theo dõi, key là DCID |
| `Client { conn, http3_conn, partial_responses }` | Trạng thái quiche + h3 + response bị flow-control chặn |
| `job_tx / job_rx` | `crossbeam::channel::bounded`, tối đa `256` job |
| `result_tx / result_rx` | `crossbeam::channel::unbounded`, trả `CompletedRequest` về listener |
| Worker threads | `num_cpus::get().clamp(2, 8)` luồng, gọi `Server::handle_request_parts()` |

Hằng số quan trọng:

| Hằng | Giá trị | Ý nghĩa |
| --- | --- | --- |
| `MAX_DATAGRAM_SIZE` | 1350 byte | Tránh IP fragmentation |
| `TIMER_TICK` | 100 ms | Nhịp wall-clock chạy `conn.on_timeout()` |
| `MAX_CLIENTS` | 4096 | Giới hạn kết nối QUIC đang theo dõi |
| `REQUEST_QUEUE_CAP` | 256 | Giới hạn request chờ worker |
| QUIC idle timeout | 30.000 ms | Browser có thể chờ ~30s trước khi fallback |

### 1.3. Vòng đời một request HTTP/3

1. `recv_from()` nhận datagram.
2. Nếu là `Initial` và connection chưa tồn tại → `quiche::accept()`, lưu vào `clients[scid]`.
3. `conn.recv()` → khi established, tạo `h3::Connection`.
4. `h3c.poll()` nhận `Event::Headers` → `handle_request()`.
5. `handle_request()` shutdown chiều đọc stream, tách `:method`, `:path`, header, đóng gói `PendingRequest` và `try_send()` vào `job_tx`. Nếu queue đầy → trả `503` ngay.
6. Worker gọi `server.handle_request_parts(...)`, gửi `CompletedRequest` qua `result_tx`.
7. **Listener thread** gọi `drain_completed_results()` trên mỗi lần lặp, ghi response vào quiche stream.
8. `flush_writable_streams()` tiếp tục các response bị `StreamBlocked`.
9. `send_pending()` đổ datagram ra socket và dọn connection đã đóng.

Nguyên tắc cốt lõi: **listener không chạy PHP; worker không chạm quiche.**

---

## 2. BUG-01: response chỉ được lấy khỏi channel khi có datagram mới

### Triệu chứng

- Browser đã upgrade sang H3 qua `Alt-Svc` thì tải rất lâu, có request đứng ở trạng thái pending ~27s.
- `curl` HTTP/1.1/HTTP/2 vẫn `200` trong ~0,1s.
- Restart server chỉ tốt vài request đầu, rồi treo lại khi browser giữ connection H3.

### Code cũ (rút gọn)

```rust
loop {
    let (len, from) = match socket.recv_from(&mut buf) {
        Ok(v) => v,
        Err(WouldBlock | TimedOut) => {
            for c in clients.values_mut() { c.conn.on_timeout(); }
            send_pending(&socket, &mut clients, &mut out);
            continue;                       // ◄── KHÔNG đọc result_rx ở đây
        }
        ...
    };

    ... xử lý datagram, h3.poll(), handle_request() ...

    while let Ok(done) = result_rx.try_recv() {   // ◄── CHỈ ở đây
        send_h3_response(...);
    }
    send_pending(...);
}
```

### Nguyên nhân

Worker đã xong và gửi `CompletedRequest` vào `result_tx`, nhưng listener đang ở nhánh timeout của `recv_from()` rồi `continue`. Không có datagram mới nên `result_rx` không bao giờ được drain. Client thì đang chờ response nên không gửi thêm gói → deadlock kinh điển giữa event loop và worker channel.

```text
T+0ms    Browser gửi H3 GET / qua UDP
T+2ms    Worker nhận job, chạy PHP (~60ms)
T+62ms   Worker gửi CompletedRequest vào result_tx
T+62ms   Server chờ recv_from() → timeout → continue (result nằm trong queue)
T+162ms  Lại timeout → continue (vẫn chưa drain)
T+30s    QUIC idle timeout / browser fallback sang TCP
T+...    Lặp lại cho từng stream → tổng thời gian bị nhân lên
```

### Cách sửa (đã áp dụng)

Trích hai helper và gọi ở **mọi nhánh** của vòng lặp:

```rust
fn drain_completed_results(
    clients: &mut HashMap<Vec<u8>, Client>,
    result_rx: &Receiver<CompletedRequest>,
) { ... }

fn flush_writable_streams(clients: &mut HashMap<Vec<u8>, Client>) { ... }
```

Gọi tại:

- đầu vòng lặp (trước `recv_from()`),
- nhánh timeout **trước `continue`**,
- sau khi xử lý datagram.

Sau mỗi lần drain/flush luôn gọi `send_pending()` để đẩy UDP ngay. Ngoài ra:

- `on_timeout()` chạy theo `Instant` + `TIMER_TICK`, không còn phụ thuộc vào `recv_from()` timeout.
- PHP/static file chuyển sang worker threads; event loop không bị một request chậm chặn toàn bộ process.
- Response bị bỏ rơi (client đã đóng / `http3_conn` None) được log rõ ràng.
- Queue full trả `503` thay vì âm thầm drop.

---

## 3. Bằng chứng từ Chrome thật

Môi trường: `google-chrome-stable 158.0.8127.88`, headless + CDP, profile sạch. Script đo nằm ở `/tmp/opencode/browser_test.py` và `/tmp/opencode/lcp_test.py` (không nằm trong repo).

### 3.1. Binary cũ – load bình thường

```text
WALL TIME: 9.93s
REQUESTS: 12
protocols: h2 x11, h3 x1
document: h2, 0.095s
```

Lần load ấm:

```text
WALL TIME: 3.57s
document: h2, 0.093s
assets: ~0,1s
x-larva-edge-cache: HIT
```

### 3.2. Binary cũ – ép H3 (`--disable-http2 --enable-quic`)

```text
WALL TIME: 30.22s
protocols: http/1.1 x10, h3 x2
2 asset JS: ~26.8s
```

Netlog `/tmp/opencode/netlog.json` cho thấy:

- Chrome nhận `Alt-Svc: h3=":443"; ma=86400`.
- QUIC handshake thành công, cert verified.
- H3 GET gửi lúc ~`51.009s` trên stream `0` và `4`.
- Server ACK gói nhưng **không gửi response headers**.
- Chrome gửi `RST_STREAM` ~`51.021s`.
- Kết thúc: `No recent network activity after 29017571us. Timeout:29s`.

Đây chính là BUG-01.

### 3.3. Biện pháp chặn tạm (trước khi có binary sửa)

Rule nft tạm:

```text
table inet larva_temp_http3
  udp dport 443 reject with icmp port-unreachable
```

Kết quả đo lại với điều kiện ép H3:

```text
WALL TIME: 3.66s | loadEventFired: yes
REQUESTS: 12 | protocols: http/1.1 x12
DOC: 200, http/1.1, 0.073s
Slowest: 0.48s (ảnh external)
```

HTTPS TCP không bị ảnh hưởng; browser đã cache `Alt-Svc` fail QUIC rất nhanh rồi fallback sang TCP.

### 3.4. Sau khi deploy binary đã sửa

- Build đúng feature: `embedded,http3,acme,mercure,tls`.
  - Lần đầu thiếu `tls` → lỗi `cannot find tls in larva_php` vì `pub mod tls` nằm sau `#[cfg(feature = "tls")]`.
- Backup binary cũ: `/usr/local/bin/larva-php.bak-20261125-143820`.
- Restart qua `larva-php@nghesachxua.service`.
- Log service xác nhận:

```text
HTTP/3 listener active but Alt-Svc advertisement disabled; TCP clients use h2/http/1.1
HTTP/3 (QUIC) listening on 0.0.0.0:443
```

Dòng `Alt-Svc advertisement disabled` chỉ đúng cho giai đoạn chưa bật `--http3`;
sau 2026-11-26 16:00 chỉ còn dòng `HTTP/3 (QUIC) listening on 0.0.0.0:443`.

Kiểm tra header:

```bash
curl -sSI https://nghesachxua.vn/ | grep -iE 'HTTP/|server:|alt-svc'
# HTTP/2 200
# server: LarvaPHP
# alt-svc: h3=":443"; ma=86400   (sau khi bật --http3)
```

ALPN và fallback:

```bash
openssl s_client -alpn h2       -connect nghesachxua.vn:443  # ALPN protocol: h2
openssl s_client -alpn http/1.1 -connect nghesachxua.vn:443  # ALPN protocol: http/1.1
curl --http2   -I https://nghesachxua.vn/  # 200
curl --http1.1 -I https://nghesachxua.vn/  # 200
```

Rule nft tạm vẫn được giữ làm safety net cho browser đã cache `Alt-Svc` từ bản cũ. Có thể gỡ sau khi chắc chắn mọi client đã hết cache:

```bash
nft delete table inet larva_temp_http3
```

---

## 4. Vì sao không "đọc header request để chọn HTTP version"

Phiên bản HTTP không nằm trong request header thông thường và không đổi được sau khi connection established:

- Với HTTPS, browser và server thương lượng trong TLS **ALPN** trước khi gửi request đầu tiên:
  `h2` → HTTP/2, `http/1.1` → HTTP/1.1.
- Với HTTP/3, ALPN là `h3` trên QUIC/UDP. Browser chỉ biết có H3 nhờ `Alt-Svc` từ một response TCP trước đó.
- HTTP/1.1 không có header "version". HTTP/2/3 dùng pseudo-header (`:method`, `:scheme`, `:authority`, `:path`) nhưng **không có** `:version` dùng để thương lượng. Chính framing của request xác định phiên bản.
- `Upgrade: h2c` chỉ áp dụng cho HTTP/1.1 cleartext; browser HTTPS không dùng.

Cách adapt đúng: giữ ALPN `h2, http/1.1`, chỉ gửi `Alt-Svc: h3` khi operator bật rõ `--http3`. Browser hỗ trợ h2 dùng h2; browser không hỗ trợ h2 tự dùng http/1.1.

---

## 5. Các rủi ro đã biết nhưng chưa sửa trong commit này

Các mục dưới đây được ghi lại để tránh tưởng rằng bản sửa đã làm hết mọi thứ.

| ID | Vấn đề | Trạng thái |
| --- | --- | --- |
| BUG-02 | Client đã đóng khi response xong → response bị drop | **Đã log** (`drain_completed_results`), vẫn chưa đóng stream bằng reset |
| BUG-03 | `result_tx` là unbounded | Chưa đổi; đã có log và `MAX_CLIENTS` nhưng vẫn cần bounded/drop có chủ đích |
| BUG-04 | H3 dùng `Server::new()` riêng, không chia `Arc<Server>` với TCP | Chưa sửa; cache/php_ini giữa H1/H2 và H3 chưa dùng chung |
| BUG-05 | `:authority` chưa được dịch sang `host` | Chưa sửa; có thể làm WordPress thấy `SERVER_NAME=localhost` khi thử H3 thật |
| BUG-06 | Request body truyền `&[]` cho worker | Chưa sửa; POST/PUT qua H3 chưa đúng |
| BUG-07 | `SERVER_PROTOCOL` hardcode `HTTP/1.1` | Chưa sửa; chỉ ảnh hưởng log/env PHP |
| BUG-08 | `on_timeout()`/`send_pending()` duyệt toàn bộ client | Chưa tối ưu; có `MAX_CLIENTS` làm trần |
| BUG-09 | Worker panic → job mất, stream treo | Chưa có `catch_unwind`/timeout |
| BUG-11 | Cert ACME không reload trong H3 | Chưa sửa; cần restart sau renewal hoặc rebuild `quiche::Config` |
| BUG-13 | 1-RTT H3 request bị drop do version check / sai độ dài DCID | **Đã sửa + có test E2E** (xem mục 11) |

Vì các rủi ro trên, production đã **chủ động không quảng cáo H3** cho đến 2026-11-26 16:00. Sau khi sửa BUG-13 và verify forced QUIC, đã bật rõ `--http3`; vẫn giữ các rủi ro BUG-02/03/05/06/07/11 trong diện theo dõi.

---

## 6. Invariant khi sửa HTTP/3

1. Chỉ listener thread được chạm `quiche::Connection` và `h3::Connection`.
2. Timer chạy theo wall-clock (`Instant` + `TIMER_TICK`), không gắn vào `recv_from()`.
3. Mọi channel có đường ra trên mỗi lần lặp, kể cả khi client im lặng.
4. Mọi stream phải kết thúc bằng `fin=true` hoặc reset; không im lặng ở `Err(_) => return`.
5. `client_key` phải ổn định trong suốt connection (hiện dùng DCID + `set_disable_active_migration(true)`).
6. Không chạy PHP/đọc file trong event loop.
7. H3 và TCP nên chia sẻ cùng `Arc<Server>` (việc này còn phải làm).
8. Pseudo-header H3 phải được dịch sang CGI env (`:authority` → `host`, `:scheme` → HTTPS) và body phải được truyền cho worker.
9. Bound mọi thứ: clients, queue, result channel, partial responses.
10. Log mọi response bị bỏ rơi — im lặng là lý do bug tồn tại lâu.

---

## 7. Deploy, kiểm tra và rollback

### Build

```bash
export PHP_LIB_DIR=/usr/lib
export PHP_EMBED_LIB=php8.3
export PHP_INCLUDES="$(php-config --includes)"

cargo build --release -p larva-php --bin larva-php \
  --features embedded,http3,acme,mercure,tls
```

Feature `tls` là bắt buộc nếu binary có `--tls`/`--lets-encrypt`; thiếu feature này sẽ lỗi `cannot find tls in larva_php`.

### Deploy có rollback

```bash
STAMP=$(date +%Y%m%d-%H%M%S)
cp -a /usr/local/bin/larva-php /usr/local/bin/larva-php.bak-$STAMP
install -o root -g root -m 0755 target/release/larva-php /usr/local/bin/larva-php
systemctl restart larva-php@nghesachxua.service
```

Backup thực tế của lần này:

```text
/usr/local/bin/larva-php.bak-20261125-143820
```

Rollback nếu có sự cố:

```bash
cp -a /usr/local/bin/larva-php.bak-20261125-143820 /usr/local/bin/larva-php
systemctl restart larva-php@nghesachxua.service
```

### Kiểm tra sau deploy

```bash
systemctl is-active larva-php@nghesachxua.service
curl -sSI https://nghesachxua.vn/ | grep -iE 'HTTP/|server:|alt-svc|x-larva-edge-cache'
curl --http2   -sS -o /dev/null -w 'h2=%{http_code} %{http_version}\n' https://nghesachxua.vn/
curl --http1.1 -sS -o /dev/null -w 'h1=%{http_code} %{http_version}\n' https://nghesachxua.vn/
journalctl -u larva-php@nghesachxua -n 50 --no-pager
```

Kỳ vọng:

- service `active`,
- `HTTP/2 200`,
- `server: LarvaPHP`,
- `--http1.1` cũng trả `200`,
- có `alt-svc: h3=":443"; ma=86400` (từ khi bật `--http3`),
- log có `HTTP/3 (QUIC) listening on 0.0.0.0:443`.

Khi test H3, reload trang thứ hai bằng Chrome profile sạch sẽ thấy document và asset là `h3`.

---

## 8. Tóm tắt một câu

Server không chậm; **luồng HTTP/3 chỉ gửi response khi nhận thêm gói từ client, trong khi client đang chờ response**. Sửa bằng cách drain/ghi/flush result trên mọi lần lặp, chạy timer theo wall-clock, đưa PHP sang worker, và chỉ quảng cáo `Alt-Svc: h3` khi operator bật rõ `--http3`.

---

## 9. Phạm vi thay đổi ngoài repo này

Tối ưu LCP/Network dependency tree cho `nghesachxua.vn` (`fetchpriority="high"`, preload ảnh + `scroll.js`, defer `scroll.js`, preconnect `images.unsplash.com`) nằm ở document root WordPress `/home/nghesachxua/nghesachxua.vn`, **không nằm trong repo `larvaphp`** và không được commit cùng 2 commit dưới đây.

---

## 10. BUG-12: TLS handshake chặn vòng accept TCP

### Triệu chứng

- `systemctl status larva-php@nghesachxua` vẫn `active (running)`, CPU tích lũy thấp, RAM ~350 MB, `free -m` thoải mái.
- HTTP/1.1 qua cổng **80** vẫn trả `301` bình thường.
- Mọi kết nối tới cổng **443** (kể cả `https://127.0.0.1/`) đều `Connection timed out`.
- `ss -tulpn` cho thấy hàng đợi accept đầy: `LISTEN 129 128 0.0.0.0:443`.
- Một vài connection `ESTABLISHED` tới `:443` có `Recv-Q` lớn (`1479–1534` byte) nhưng `Send-Q = 0`.
- Journal không có log mới; dòng cuối cùng là `Connection error: received fatal alert: BadCertificate` từ ~9 giờ trước.

### Nguyên nhân

Trước bản sửa, `Server::start_server_inner()` chạy TLS handshake **ngay trong vòng lặp accept**:

```rust
loop {
    let (stream, _) = listener.accept().await?;
    let stream = acceptor.accept(stream).await?;   // ◄── chặn toàn bộ vòng lặp
    tokio::spawn(async move { /* serve_connection */ });
}
```

Chỉ cần một client hoàn tất TCP nhưng không gửi ( hoặc gửi chậm ) `ClientHello` là `acceptor.accept()` treo vô thời hạn. Vòng lặp không gọi `accept()` nữa → kernel đổ đầy backlog (128) → SYNs bị drop → mọi client mới thấy timeout. Process vẫn `active`, không crash, không tăng CPU/RAM, nên `htop` và `free -m` nhìn rất "bình thường".

Chuỗi sự kiện thực tế tại production:

```text
22:19:37  service start, listener active
22:19:54 → 22:53:58  bot gửi "fatal alert: BadCertificate" mỗi ~2 phút
22:53:58  connection cuối cùng làm handshake treo; không còn log nào sau đó
07:54    ss: LISTEN Recv-Q=129/128, curl 127.0.0.1:443 timeout
```

### Cách sửa (đã deploy)

Chuyển handshake vào task riêng và giới hạn thời gian 10 giây:

```rust
let tls_acceptor = tls_acceptor.clone();
tokio::spawn(async move {
    let stream = match tokio::time::timeout(
        Duration::from_secs(10),
        acceptor.accept(stream),
    ).await {
        Ok(Ok(s)) => ConnectionStream::Tls(s),
        Ok(Err(e)) => { debug!("TLS handshake failed: {}", e); return; }
        Err(_) => { debug!("TLS handshake timed out"); return; }
    };
    /* serve_connection như cũ */
});
```

Bất kỳ client nào treo handshake cũng chỉ làm chậm chính connection của nó. Vòng `accept()` luôn chạy, có thể phục vụ các client khỏe mạnh khác.

### Kiểm tra sau deploy

```bash
# Client giữ TCP nhưng không gửi ClientHello
python3 -c 'import socket,time; s=socket.create_connection(("127.0.0.1",443)); time.sleep(20)' &
curl -sS -o /dev/null -w '%{http_version} %{http_code}\n' https://127.0.0.1/ -k
curl -sS -o /dev/null -w '%{http_version} %{http_code}\n' https://nghesachxua.vn/
```

Kết quả mong đợi: cả hai vẫn `2 200` trong khi connection stall còn mở. Ngoài ra vẫn giữ `h2=200`, `h1=200`, `x-larva-edge-cache: HIT` và (sau khi bật `--http3`) có `alt-svc`.

### Invariant bổ sung

11. Không `await` bất kỳ thao tác handshake/I/O nào của một connection trong vòng lặp `accept()`; mọi connection chậm phải tách sang task riêng có timeout.

---

## 11. BUG-13: H3 handshake xong nhưng request 1-RTT bị drop

### Triệu chứng

- Chrome ép QUIC (`--origin-to-force-quic-on=nghesachxua.vn:443`) cho thấy:
  `QUIC_SESSION_VERSION_NEGOTIATED {"version":"RFCv1"}`, cert hợp lệ,
  `QUIC_SESSION_HANDSHAKE_DONE_FRAME_RECEIVED`, `HTTP3_HEADERS_SENT stream_id 0`.
- Server ACK các gói nhưng **không bao giờ có `:status` / response headers**.
- Sau ~31 gói lặp lại, Chrome báo `Network blackhole detected` (`quic_error 85`).
- Trafic TCP vẫn `h2 200`; `curl 7.88.1` không có HTTP/3 nên mọi kết luận trước đó chỉ phản ánh h2/h1.

### Nguyên nhân

Có hai lỗi cùng nằm ở đầu vòng lặp UDP trong `http3.rs`:

1. **Version negotiation chạy cho mọi packet.** Với short-header packet (1-RTT),
   quiche trả `hdr.version = 0`. Đoạn kiểm tra
   `!quiche::version_is_supported(hdr.version)` vì thế gọi `negotiate_version()`
   và `continue` — drop chính gói chứa request H3.

2. **Parse short header với độ dài CID sai.** `quiche::Header::from_slice(buf, MAX_CONN_ID_LEN)`
   chỉ đúng nếu local connection ID dài 20 byte. Server lại gọi `quiche::accept()`
   với SCID = DCID do client chọn trong Initial (Chrome thường 8 hoặc 16 byte).
   Header parser vì thế trả `Err`, datagram bị bỏ qua im lặng.

```text
Initial (long header, version RFCv1)  ──► accept() OK, h3 layer OK
1-RTT (short header, version 0)       ──► version check / CID parse drop
                                          ► HTTP3_HEADERS_SENT nhưng không có response
```

### Cách sửa (đã deploy)

- Version negotiation chỉ chạy khi **connection chưa tồn tại và packet là `Initial`**.
- Với short header, thử các độ dài DCID `0..=quiche::MAX_CONN_ID_LEN` và chọn
  bản parse có DCID trùng một connection đã accept. Long header giữ nguyên
  `MAX_CONN_ID_LEN` vì độ dài nằm trong packet.
- Thêm test end-to-end `h3_request_returns_its_own_static_body`: dựng listener
  H3 thật bằng cert tự ký, dùng `quiche::connect()` + `h3::send_request()`,
  rồi assert `200` và đúng body. Test này đã chứng minh request 1-RTT bị drop
  trước khi sửa và pass sau khi sửa.

Kết quả test: `237 passed` (default), `243 passed`
(`embedded,http3,acme,mercure,tls`).

### Kiểm tra bằng Chrome sau deploy

```bash
google-chrome --headless=new --no-sandbox --disable-gpu --disable-dev-shm-usage \
  --disable-background-networking --disable-extensions --disable-sync \
  --enable-quic --quic-version=h3 \
  --origin-to-force-quic-on=nghesachxua.vn:443 \
  --log-net-log=/tmp/opencode/quic-netlog-after.json \
  --net-log-capture-mode=Default \
  --virtual-time-budget=5000 --dump-dom https://nghesachxua.vn/
```

Kết quả đo sau khi deploy binary `4cae7221c6c282808d6ddb32ecdb6d8b`
(backup: `/usr/local/bin/larva-php.bak-20261126-113534`):

```text
HTTP3_HEADERS_SENT: :authority nghesachxua.vn, :path /
HTTP3_HEADERS_DECODED: :status 200, server: LarvaPHP,
                        x-larva-edge-cache: HIT, content-length: 124887
peer_address: 103.238.234.195:443
Response có header LarvaPHP: 12
Network blackhole detected: 0
```

`curl` thường vẫn báo `h2` vì không hỗ trợ HTTP/3; điều kiện ép QUIC ở trên
là phép thử H3 thật. Tại thời điểm đo đó production **chưa gửi `Alt-Svc`**;
sau khi bật `--http3` lúc 16:00, client hỗ trợ QUIC dùng h3, client còn lại
vẫn h2/h1 qua ALPN.

### Invariant bổ sung

12. Không áp dụng version negotiation cho packet thuộc connection đã biết;
    `version = 0` của short-header không phải "unsupported version".
13. Khi parse short header, độ dài DCID phải là độ dài SCID local đã dùng lúc
    `quiche::accept()`, không phải `MAX_CONN_ID_LEN` mặc định.

---

## 12. Bật H3 mặc định cho production (2026-11-26 16:00)

### Thay đổi

Chỉ thêm `--http3` vào ExecStart trong
`/etc/systemd/system/larva-php@nghesachxua.service.d/override.conf`:

```ini
ExecStart=/usr/local/bin/larva-php \
    ...
    --lets-encrypt \
    --http3 \
    ...
```

Backup config: `/etc/systemd/system/larva-php@nghesachxua.service.d/override.conf.bak-20261126-160028`.

Không rebuild binary, không sửa code timeout/TLS. Lý do an toàn:

- UDP listener đã chạy sẵn vì có `--lets-encrypt`.
- Flag chỉ gọi `set_http3_port()` để thêm `Alt-Svc`; ALPN vẫn là `h2, http/1.1`.
- Vòng `accept()` TCP và handshake 10s (BUG-12) không bị đụng tới.
- H3 chạy thread riêng, idle timeout 30s riêng, worker riêng.

### Kết quả verify

```text
systemctl is-active        → active
curl -sSI                  → HTTP/2 200 + alt-svc: h3=":443"; ma=86400
curl --http2 / --http1.1   → 200 / 200
ss -lntup                  → TCP + UDP 443 do larva-php giữ
Stalled TCP client 15s     → curl vẫn 2 200
Chrome profile sạch, load 2:
  HTTP3_HEADERS_SENT       → :authority nghesachxua.vn, :path /
  HTTP3_HEADERS_DECODED    → :status 200, server: LarvaPHP,
                             x-larva-edge-cache: HIT
  Network blackhole        → 0
```

Netlog: `/tmp/opencode/quic-default-netlog.json`.

### Rollback

Bỏ flag `--http3`, `daemon-reload`, `restart`. Do `Alt-Svc` được browser cache
đến `ma=86400`, nếu cần fallback ngay thì chặn UDP/443 tạm bằng nft rồi gỡ sau
khi cache hết hạn.
