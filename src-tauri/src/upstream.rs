//! "Bring your own exit" - the user's own proxy in a permitted region.
//!
//! Everything else in this tool exists to make Google see a request from a
//! region it allows: an NRPT rule, a substituted address, a relay somebody else
//! runs. A proxy that already egresses in such a region does that directly, and
//! it was proven end to end rather than assumed - through a Dutch CONNECT proxy
//! the language server got a correct answer out of the model while our own relay
//! log stayed empty and not one DNS rule was involved (kb/dns.md).
//!
//! So if a user has one, it is the best route available and it is theirs, not a
//! third party we chose for them. It is asked for once, in menu 1, and may be
//! skipped with Enter.
//!
//! HTTP CONNECT only. That is what proxy clients in this space overwhelmingly
//! speak, it is the one protocol the rest of this file already implements for
//! the relay, and guessing wrong about SOCKS would fail in a way a user cannot
//! read. The prompt says so plainly.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use rustls::ClientConnection;

use crate::health::Health;

/// How long `open` may spend reaching a proxy and reading its answer for the
/// region check - which asks a different question from route health, runs rarely,
/// and is worth waiting out rather than repeating.
const PROBE_OPEN_BUDGET: Duration = Duration::from_secs(15);
/// The same for every path a request can take, and for the health probe that
/// stands in for one.
///
/// Much shorter, and the reason is not impatience. Falling from one route to the
/// next is free in correctness - nothing is committed before the `200` (I35) - but
/// it is not free in time, and the two were confused. A route that has not
/// answered a `CONNECT` in three seconds is not going to save this request: the
/// route below it is already there, and whether the slow one is really down is a
/// question for the warm loop, not for the person waiting. With the probe limit on
/// this path a flapping exit cost 15 s per attempt and 30 s before its second
/// failure benched it, which is most of what a long "Authenticating" was.
pub const LIVE_OPEN_BUDGET: Duration = Duration::from_secs(3);
/// Longest a probe's own request may take once the tunnel is open.
pub(crate) const PROBE_BUDGET: Duration = Duration::from_secs(20);

/// The host a probe asks for: the one the IDE actually uses, so a probe measures
/// the path a request will take rather than a neighbouring one.
const PROBE_HOST: &str = "daily-cloudcode-pa.googleapis.com";

/// One route's standing, and the only place the two reasons a route stands down
/// are kept apart (I44).
///
/// `health` is timed: a route that stopped answering is benched and the bench
/// doubles, because waiting is the right response to an outage. `bad_exit` is not
/// timed at all - a proxy that surfaces in the blocked region is useless *at that
/// exit*, and no amount of waiting moves it, only a new address does. So that
/// verdict is pinned to the address and released the moment it changes.
///
/// One of these per route, not one shared: the user's own proxy and each built-in
/// exit fail independently, and a single pair of statics would have one dead exit
/// bench every other route along with it.
pub struct Route {
    pub health: Health,
    bad_exit: Mutex<Option<String>>,
    /// How this route is named in the log - never its address. A built-in exit is
    /// private, and a log file is the one thing users paste into public chats.
    label: &'static str,
    /// Which row of the route table a probe of this route feeds. The user's own
    /// proxy has a row of its own; every built-in exit feeds the one `Exits` row,
    /// because the proxy treats the pool as a single route.
    kind: crate::routes::Kind,
}

impl Route {
    pub const fn new(label: &'static str, kind: crate::routes::Kind) -> Self {
        Route {
            health: Health::new(label),
            bad_exit: Mutex::new(None),
            label,
            kind,
        }
    }

    /// Whether this route is worth trying for the next connection: answering, and
    /// coming out somewhere Google will accept.
    pub fn usable(&self) -> bool {
        self.bad_exit().is_none() && !self.health.is_benched()
    }

    /// How this route is named in the log. Only the built-in exits ask - every
    /// other route knows its own name at the call site - so a build without them
    /// is honest about it rather than carrying a warning.
    #[cfg_attr(not(exits), allow(dead_code))]
    pub fn label(&self) -> &'static str {
        self.label
    }

    /// The exit this route was last found unusable at, while it is standing down
    /// for that reason.
    fn bad_exit(&self) -> Option<String> {
        self.bad_exit.lock().ok().and_then(|g| g.clone())
    }

    /// Records that this exit is in a blocked region, so the route stands down
    /// until the address changes. Quiet when it is the same exit as last time -
    /// this runs on a timer and would otherwise repeat itself forever.
    fn note_blocked_exit(&self, ip: &str, loc: &str) {
        let Ok(mut pinned) = self.bad_exit.lock() else {
            return;
        };
        if pinned.as_deref() == Some(ip) {
            return;
        }
        *pinned = Some(ip.to_string());
        // The country, not the address: for the user's own proxy the exit is
        // their server, and for a built-in exit it is usually the exit itself -
        // neither belongs in a log that is pasted into a public chat.
        crate::dns_forwarder::log_proxy(&format!(
            "{} выходит в {} — это заблокированный регион, маршрут отключён до смены выхода",
            self.label, loc
        ));
    }

    /// Releases the pin, because the exit moved somewhere usable.
    fn note_usable_exit(&self, _ip: &str, loc: &str) {
        let Ok(mut pinned) = self.bad_exit.lock() else {
            return;
        };
        if pinned.is_none() {
            return;
        }
        *pinned = None;
        crate::dns_forwarder::log_proxy(&format!(
            "{} теперь выходит в {} — снова используем",
            self.label, loc
        ));
    }

    /// The warm loop's whole check for one route, run on our own time and never
    /// with somebody's request (I38).
    ///
    /// `check_region` is separate because the two halves cost differently. The
    /// carry probe is one tunnel to Google; the region check is another full TLS
    /// request, and for a built-in exit shared by every user of this tool that is
    /// load somebody else pays for. So the caller decides how often it is worth
    /// asking again where a route comes out.
    pub fn probe(&self, up: &Upstream, check_region: bool) {
        // Where it comes out is checked first, and separately, because it answers
        // a different question. A proxy can be perfectly responsive and still be
        // useless: if it surfaces in the blocked region Google refuses the request
        // with the very 400 this whole tool exists to remove - and that refusal is
        // invisible from here, because it arrives inside the client's own TLS. The
        // exit address is the one part of it we *can* see, so it is what the route
        // is judged on.
        if check_region {
            match exit_info(up) {
                Some((ip, loc)) if region_is_blocked(&loc) => {
                    self.note_blocked_exit(&ip, &loc);
                    // Nothing else is worth measuring: it is not coming back until
                    // the address changes, and this runs again shortly to see if
                    // it has.
                    return;
                }
                Some((ip, loc)) => self.note_usable_exit(&ip, &loc),
                // No trace host was reachable through it. That is a fault of its
                // own and the carry probe below will say so; it is not evidence
                // about the region, so an existing verdict is left standing.
                None => {}
            }
        }

        // Timed as a whole - reaching the proxy, its CONNECT, the TLS to Google
        // inside it and the answer - because that is what a request through it
        // costs, and the route table ranks routes by exactly that (routes.rs).
        self.probe_with(|| probe(up))
    }

    pub(crate) fn probe_with(&self, probe_fn: impl FnOnce() -> Result<(), String>) {
        let started = Instant::now();
        match probe_fn() {
            Ok(()) => {
                crate::routes::record(self.kind, started.elapsed());
                self.health.revive("проверка прошла");
            }
            Err(why) => {
                crate::dns_forwarder::log_proxy(&format!("{} не отвечает: {}", self.label, why));
                if self.kind != crate::routes::Kind::Exits {
                    crate::routes::record_failure(self.kind);
                }
                self.health.probe_failed();
            }
        }
    }
}

/// How many proxies of their own a user may give.
pub const MAX_OWN: usize = 4;

/// The user's own proxies, as routes, in the order they were given. One route
/// each, so a dead first server benches itself and the next connection goes to
/// the second, rather than one bench standing down every server at once. They
/// share the route table's single `Own` row: to the table they are one route
/// with spares.
pub static OWN: [Route; MAX_OWN] = [
    Route::new("свой прокси 1", crate::routes::Kind::Own),
    Route::new("свой прокси 2", crate::routes::Kind::Own),
    Route::new("свой прокси 3", crate::routes::Kind::Own),
    Route::new("свой прокси 4", crate::routes::Kind::Own),
];

/// A user-supplied HTTP CONNECT proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    pub host: String,
    pub port: u16,
    /// `user:pass`, already joined; `None` when the proxy needs no credential.
    pub auth: Option<String>,
    /// An HTTPS proxy (`https://`): TLS to the proxy itself, the `CONNECT`
    /// inside it. Over plain `http://` the credential and the host asked for
    /// cross the network readable by anyone on the path.
    pub tls: bool,
}

impl Upstream {
    /// How it is shown back to the user - without the password, which they do
    /// not need to see again and which has no business being on screen or in a
    /// screenshot attached to a bug report.
    pub fn display(&self) -> String {
        let bare = match &self.auth {
            Some(a) => {
                let user = a.split(':').next().unwrap_or("");
                format!("{}:{}@{}:{}", user, "***", self.host, self.port)
            }
            None => format!("{}:{}", self.host, self.port),
        };
        self.with_scheme(bare)
    }

    fn as_line(&self) -> String {
        let bare = match &self.auth {
            Some(a) => format!("{}@{}:{}", a, self.host, self.port),
            None => format!("{}:{}", self.host, self.port),
        };
        self.with_scheme(bare)
    }

    /// `https://` is written out; plain stays bare, the shape older builds
    /// wrote and read.
    fn with_scheme(&self, bare: String) -> String {
        if self.tls {
            format!("https://{bare}")
        } else {
            bare
        }
    }
}

/// Reads what the user typed.
///
/// Deliberately forgiving about the shapes people actually paste - with or
/// without a scheme, with or without a credential - and strict about the one
/// thing that must be right, the port. A silently mis-parsed proxy would look
/// exactly like a proxy that is down.
pub fn parse(input: &str) -> Result<Upstream, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err("пустая строка".to_string());
    }
    let (tls, rest) = match raw.strip_prefix("https://") {
        Some(rest) => (true, rest),
        None => (false, raw.strip_prefix("http://").unwrap_or(raw)),
    };
    if rest.contains("://") {
        return Err("поддерживаются только http:// и https://".to_string());
    }
    // Split on the LAST '@': a password may contain one, a hostname may not.
    let (auth, hostport) = match rest.rsplit_once('@') {
        Some((a, hp)) => {
            if !a.contains(':') {
                return Err("логин без пароля — нужно логин:пароль@хост:порт".to_string());
            }
            (Some(a.to_string()), hp)
        }
        None => (None, rest),
    };
    let hostport = hostport.trim_end_matches('/');
    let (host, port) = hostport
        .rsplit_once(':')
        .ok_or_else(|| "не указан порт — нужно хост:порт".to_string())?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return Err("не указан адрес".to_string());
    }
    let port: u16 = port
        .parse()
        .map_err(|_| format!("порт «{}» не число", port))?;
    if port == 0 {
        return Err("порт не может быть 0".to_string());
    }
    Ok(Upstream {
        host: host.to_string(),
        port,
        auth,
        tls,
    })
}

/// Several proxies, separated by spaces or one per line, in the order they are
/// to be tried. A comma or semicolon a pasted list leaves on an address is
/// dropped; the same proxy twice counts once. A bad one is named by its place
/// in the list, never by its text, which carries the password.
pub fn parse_list(input: &str) -> Result<Vec<Upstream>, String> {
    let mut out: Vec<Upstream> = Vec::new();
    let tokens = input
        .split_whitespace()
        .map(|t| t.trim_matches(|c| c == ',' || c == ';'))
        .filter(|t| !t.is_empty());
    for (i, token) in tokens.enumerate() {
        let up = parse(token).map_err(|e| format!("прокси №{}: {}", i + 1, e))?;
        if !out.contains(&up) {
            out.push(up);
        }
    }
    if out.is_empty() {
        return Err("пустая строка".to_string());
    }
    if out.len() > MAX_OWN {
        return Err(format!("не больше {} прокси", MAX_OWN));
    }
    Ok(out)
}

/// Where the address is kept.
///
/// Beside the relay's log, because both processes have to reach it: menu 1 runs
/// as the user and writes it, and the relay - which is what actually opens the
/// connections - runs under an S4U principal for that same user and reads it.
/// A registry value or an environment variable would need a relay restart to
/// take effect; a file is picked up on the next connection.
pub fn config_path() -> PathBuf {
    crate::dns_forwarder::log_dir().join("upstream.txt")
}

/// One proxy per line. The file holds passwords, so it is the user's alone.
pub fn save(ups: &[Upstream]) -> Result<(), String> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let body: Vec<String> = ups.iter().map(Upstream::as_line).collect();
    fs::write(&path, body.join("\n") + "\n").map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn clear() {
    fs::remove_file(config_path()).ok();
}

/// The configured proxy, or `None` when the user skipped the question.
///
/// Read from disk every time rather than cached: the relay runs for weeks, and a
/// user who changes or removes their proxy in menu 1 should not have to restart
/// a background service for it to matter. One small read per connection is
/// nothing beside the handshake that follows it - the same reasoning the old CA
/// lookup used.
pub fn configured() -> Option<Upstream> {
    configured_all().into_iter().next()
}

/// Every configured proxy, in the order to try them. A line that does not
/// parse is skipped rather than taking the rest down with it.
pub fn configured_all() -> Vec<Upstream> {
    let Ok(raw) = fs::read_to_string(config_path()) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|l| parse(l).ok())
        .take(MAX_OWN)
        .collect()
}

/// How the configured proxies are shown: without their passwords.
pub fn display_all() -> String {
    configured_all()
        .iter()
        .map(Upstream::display)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether the route should be tried for this connection.
///
/// Three things have to hold: the user gave us one, it is answering, and it
/// comes out somewhere Google will accept. The last is checked against a pinned
/// address rather than a clock - see `Route`. Any one of the proxies will do.
pub fn available() -> bool {
    (0..configured_all().len()).any(|i| OWN[i].usable())
}

/// Whether any configured proxy is standing down, so the warm loop checks
/// it again on the next pass rather than on the slow clock.
pub fn any_benched() -> bool {
    (0..configured_all().len()).any(|i| OWN[i].health.is_benched())
}

fn basic(auth: &str) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let data = auth.as_bytes();
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | (*chunk.get(2).unwrap_or(&0) as u32);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Reaches the proxy itself, inside `deadline` whatever shape its address is in.
///
/// A hostname has to be resolved before `connect_timeout` can be used at all,
/// and the obvious `parse().unwrap_or_else(|_| TcpStream::connect(..))` silently
/// drops the budget for exactly that case: a named proxy whose address black-holes
/// then hangs on the OS default - about 21 s on Windows - with a live client
/// waiting behind it. The budget is spent across every candidate address rather
/// than granted to each, so the whole step is bounded however many a name has.
fn connect_within_budget(host: &str, port: u16, deadline: Instant) -> Result<TcpStream, String> {
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("адрес не разрешается: {}", e))?
        .collect();
    if addrs.is_empty() {
        return Err("адрес не разрешается".to_string());
    }
    let mut last = String::from("время вышло");
    for addr in addrs {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match TcpStream::connect_timeout(&addr, left) {
            Ok(sock) => return Ok(sock),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("недоступен: {}", last))
}

/// Opens a tunnel to `host:port` through the given proxy.
///
/// Everything that can fail happens here, before the caller has told its client
/// anything - the same rule the relay route follows (I35). An `Err` means the
/// caller still holds an untouched client socket and can take another route.
///
/// The error says only what went wrong, never which proxy it was: the caller
/// knows that and names the route itself, and a built-in exit must not put its
/// address in a log line (see `Route::label`).
///
/// `budget` covers the **whole** step - resolve, connect and read the answer -
/// because that is the thing a waiting client experiences. Bounding each part
/// separately is how three seconds of patience turns into fifteen (I43).
pub fn open(up: &Upstream, host: &str, port: u16, budget: Duration) -> Result<TcpStream, String> {
    if up.tls {
        return open_tls(up, host, port, budget);
    }
    let deadline = Instant::now() + budget;
    let mut sock = connect_within_budget(&up.host, up.port, deadline)?;
    let left = || deadline.saturating_duration_since(Instant::now());
    // A zero timeout means "block forever" to the OS, so a spent budget has to be
    // an error here rather than a socket option.
    if left().is_zero() {
        return Err("прокси не ответил вовремя".to_string());
    }
    sock.set_read_timeout(Some(left())).ok();
    sock.set_write_timeout(Some(left())).ok();

    // A proxy on this machine or the LAN (v2rayN, Privoxy, a router's proxy)
    // resolves the name itself, usually through the system resolver - which,
    // with the loopback door up, answers a gate host with *our* address. The
    // proxy would then dial us, we would dial it, and so on: an avalanche of
    // connections with no end (found in review before 2.14.0_1 shipped). Such a
    // proxy is handed Google's address instead of the name; the client's own
    // TLS still names the host, so nothing else changes.
    let target = crate::proxy::connect_target(up.host.as_str(), host);
    let host = target.as_str();
    let mut req = format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n");
    if let Some(a) = &up.auth {
        req.push_str(&format!("Proxy-Authorization: Basic {}\r\n", basic(a)));
    }
    req.push_str("\r\n");
    sock.write_all(req.as_bytes())
        .map_err(|e| format!("не отправить CONNECT: {}", e))?;

    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 8 * 1024 {
            return Err("прокси ответил чем-то очень длинным".to_string());
        }
        // The socket timeout bounds one `read`; this bounds the loop, so a proxy
        // dribbling a byte at a time cannot outlast the budget by repeating.
        if left().is_zero() {
            return Err("прокси не ответил вовремя".to_string());
        }
        match sock.read(&mut byte) {
            Ok(0) => return Err("прокси закрыл соединение".to_string()),
            Ok(_) => head.push(byte[0]),
            // The read timeout expiring: `TimedOut` on Windows, `WouldBlock` on
            // Linux, where it reads «Resource temporarily unavailable (os error
            // 11)» - which says nothing to a user about a proxy that let us
            // connect and then never answered the CONNECT.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err("соединение принято, но на запрос прокси не ответил вовремя".to_string())
            }
            Err(e) => return Err(format!("нет ответа: {}", e)),
        }
    }
    let status = String::from_utf8_lossy(&head)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    // Not `starts_with("HTTP/1.1 200")`: a CONNECT proxy is free to answer on
    // HTTP/1.0, and tinyproxy - which is what the built-in exits run - does.
    if let Some(why) = refusal(&status, host) {
        return Err(why);
    }
    // The budgets above bounded reaching the proxy and reading its answer. They
    // are mine, not the caller's, so the socket goes back clean - a caller that
    // wants one of its own (`probe`, `exit_info`) sets it immediately, and a
    // tunnel wants none (see `proxy::splice`, and I37).
    sock.set_read_timeout(None).ok();
    sock.set_write_timeout(None).ok();
    Ok(sock)
}

/// What a `CONNECT` answer that is not `200` means, or `None` for a `200`.
///
/// The statuses worth naming are the ones that are not an outage and do not fix
/// themselves however long the route is benched: 407 is a credential problem,
/// 403 a proxy that filters by destination and does not carry this host. A
/// redirect is what a probe-resistant proxy (Caddy's forwardproxy) answers a
/// `CONNECT` it did not authenticate - it sends the stranger to the website it
/// pretends to be - so it means the same as 407.
fn refusal(status: &str, host: &str) -> Option<String> {
    if status.contains(" 200") {
        return None;
    }
    Some(if status.contains(" 407") {
        "прокси требует логин и пароль".to_string()
    } else if status.contains(" 403") {
        format!("прокси не пропускает этот хост: {}", host)
    } else if [" 301", " 302", " 307", " 308"]
        .iter()
        .any(|c| status.contains(c))
    {
        "прокси не принял логин и пароль".to_string()
    } else {
        format!("прокси отказал: {}", status)
    })
}

/// `open` through an HTTPS proxy: TLS to the proxy first, then the `CONNECT`
/// inside it, so neither the password nor the host asked for crosses the
/// network in the clear. The proxy's certificate is checked against the public
/// roots like any website's.
///
/// The caller still gets a plain socket, the shape `open` has always handed
/// back - the probe, the trace and the splice need no second one. It is one end
/// of a loopback pair; a thread pumps the other end through the TLS session
/// with the pump the group route already carries its tunnels with.
///
/// Everything that can fail - the handshake, a refused credential, a filtered
/// host - fails before this returns, so the caller has still said nothing to
/// its client (I35).
fn open_tls(up: &Upstream, host: &str, port: u16, budget: Duration) -> Result<TcpStream, String> {
    let deadline = Instant::now() + budget;
    let mut sock = connect_within_budget(&up.host, up.port, deadline)?;
    let left = || deadline.saturating_duration_since(Instant::now());
    if left().is_zero() {
        return Err("прокси не ответил вовремя".to_string());
    }
    sock.set_read_timeout(Some(left())).ok();
    sock.set_write_timeout(Some(left())).ok();

    let name = ServerName::try_from(up.host.clone())
        .map_err(|_| "адрес прокси не годится для TLS".to_string())?;
    let mut tls = ClientConnection::new(proxy_tls_config(), name).map_err(|e| e.to_string())?;

    let target = crate::proxy::connect_target(up.host.as_str(), host);
    let target = target.as_str();
    let mut req = format!("CONNECT {target}:{port} HTTP/1.1\r\nHost: {target}:{port}\r\n");
    if let Some(a) = &up.auth {
        req.push_str(&format!("Proxy-Authorization: Basic {}\r\n", basic(a)));
    }
    req.push_str("\r\n");

    let (status, leftover) = {
        let mut stream = rustls::Stream::new(&mut tls, &mut sock);
        // The handshake happens inside this first write, so a certificate that
        // does not check out fails here, before a byte of the request is sent.
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("TLS до прокси: {}", e))?;
        let mut acc: Vec<u8> = Vec::new();
        let mut buf = [0u8; 1024];
        let end = loop {
            if let Some(pos) = acc.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
            if acc.len() > 8 * 1024 {
                return Err("прокси ответил чем-то очень длинным".to_string());
            }
            if left().is_zero() {
                return Err("прокси не ответил вовремя".to_string());
            }
            match stream.read(&mut buf) {
                Ok(0) => return Err("прокси закрыл соединение".to_string()),
                Ok(n) => acc.extend_from_slice(&buf[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(
                        "соединение принято, но на запрос прокси не ответил вовремя".to_string()
                    )
                }
                Err(e) => return Err(format!("нет ответа: {}", e)),
            }
        };
        let status = String::from_utf8_lossy(&acc[..end])
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        (status, acc[end..].to_vec())
    };
    if let Some(why) = refusal(&status, host) {
        return Err(why);
    }
    sock.set_read_timeout(None).ok();
    sock.set_write_timeout(None).ok();

    let (ours, theirs) = local_pair()?;
    std::thread::Builder::new()
        .name("own-proxy-tls".to_string())
        .spawn(move || crate::group::tls_pump(theirs, tls, sock, leftover))
        .map_err(|e| format!("поток туннеля: {}", e))?;
    Ok(ours)
}

/// TLS settings for an HTTPS proxy: the public roots, and HTTP/1.1 only, since
/// `CONNECT` is spoken as HTTP/1.1 here.
fn proxy_tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let mut config = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            Arc::new(config)
        })
        .clone()
}

/// Two ends of one loopback connection. The accepted end is checked to be the
/// one just dialled, so nothing else on the machine can slip in between the
/// bind and the accept.
fn local_pair() -> Result<(TcpStream, TcpStream), String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("локальный сокет: {}", e))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let ours = TcpStream::connect(addr).map_err(|e| format!("локальный сокет: {}", e))?;
    let want = ours.local_addr().map_err(|e| e.to_string())?;
    loop {
        let (theirs, from) = listener
            .accept()
            .map_err(|e| format!("локальный сокет: {}", e))?;
        if from == want {
            ours.set_nodelay(true).ok();
            theirs.set_nodelay(true).ok();
            return Ok((ours, theirs));
        }
    }
}

/// A full request through the proxy: TLS to Google inside the tunnel, and an
/// answer back. What a probe has to prove is that the route *carries* something,
/// not merely that the proxy accepts a CONNECT - the relay taught that lesson by
/// accepting tunnels for an hour and cutting every one at the handshake.
pub fn probe(up: &Upstream) -> Result<(), String> {
    // Opened on the **live** budget, not the generous one. A probe exists to
    // predict what a request will meet, so the two must agree on what "reachable"
    // means: judge on 15 s and serve on 3 s and a merely-slow proxy flaps forever -
    // revived by every probe, benched by the next two requests. The generosity
    // belongs in the request below, which is measuring something else.
    let mut sock = open(up, PROBE_HOST, 443, LIVE_OPEN_BUDGET)?;
    sock.set_read_timeout(Some(PROBE_BUDGET)).ok();
    sock.set_write_timeout(Some(PROBE_BUDGET)).ok();
    let name = ServerName::try_from(PROBE_HOST).map_err(|e| e.to_string())?;
    let mut tls =
        ClientConnection::new(crate::proxy::probe_config(), name).map_err(|e| e.to_string())?;
    let mut stream = rustls::Stream::new(&mut tls, &mut sock);
    let req = format!(
        "GET /v1internal:probe HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        PROBE_HOST
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("рукопожатие: {}", e))?;
    let mut buf = [0u8; 64];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("ответа нет: {}", e))?;
    // Any status line means the tunnel carried a request end to end; which status
    // it is says nothing, since the path is deliberately not a real API call.
    if n > 0 && buf.starts_with(b"HTTP/") {
        Ok(())
    } else {
        Err("ответ не похож на HTTP".to_string())
    }
}

/// Checked on the warm loop, so the route is judged on our time and never with
/// somebody's request. A no-op when the user never gave us a proxy.
///
/// Carry only, never the region: the user's proxy has its exit country checked
/// once, when it is added (`ops::set_own_proxy`), and after that the region-400
/// watch is what stands it down - it sees the refusal Google actually gave,
/// where the trace only guesses from Cloudflare's geolocation, and a benched
/// Own gives way like any route (G81). Owner's decision: no trace traffic to
/// third-party hosts through their proxy every two minutes.
pub fn probe_health() {
    for (i, up) in configured_all().iter().enumerate() {
        OWN[i].probe(up, false);
    }
}

/// The address the proxy comes out on, and the country Cloudflare puts it in.
///
/// The address matters as much as the country: a proxy that surfaces somewhere
/// blocked is unusable *at that exit*, and the way it becomes usable again is by
/// rotating to another one. Watching the address is how we notice that without
/// asking the user to press anything.
pub fn exit_info(up: &Upstream) -> Option<(String, String)> {
    TRACE_HOSTS.iter().find_map(|h| trace_through(up, h))
}

/// Hosts that will report an exit back, in the order they are tried.
///
/// `www.cloudflare.com` first: it is the canonical one, and it is not something a
/// user's own proxy is likely to have an opinion about. The rest exist because a
/// *filtering* proxy answers `403 Filtered` for it while carrying them perfectly
/// well - which is exactly what the built-in exits are, somebody's AI-unblocking
/// service with a whitelist. Measured through one: cloudflare.com refused, both
/// of the others return a full trace.
///
/// Getting this wrong is not a small thing. A `None` from here leaves the previous
/// region verdict standing, so a route whose exit had quietly moved into the
/// blocked region would keep taking traffic and keep failing inside the client's
/// own TLS, where none of it is visible to us.
const TRACE_HOSTS: &[&str] = &["www.cloudflare.com", "chatgpt.com", "claude.ai"];

fn trace_through(up: &Upstream, host: &'static str) -> Option<(String, String)> {
    let mut sock = open(up, host, 443, PROBE_OPEN_BUDGET).ok()?;
    sock.set_read_timeout(Some(PROBE_BUDGET)).ok();
    sock.set_write_timeout(Some(PROBE_BUDGET)).ok();
    let name = ServerName::try_from(host).ok()?;
    let mut tls = ClientConnection::new(crate::proxy::probe_config(), name).ok()?;
    let mut stream = rustls::Stream::new(&mut tls, &mut sock);
    stream
        .write_all(
            format!(
                "GET /cdn-cgi/trace HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                host
            )
            .as_bytes(),
        )
        .ok()?;
    let mut body = Vec::new();
    let mut chunk = [0u8; 2048];
    while let Ok(n) = stream.read(&mut chunk) {
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > 8 * 1024 {
            break;
        }
    }
    let text = String::from_utf8_lossy(&body);
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name).map(|v| v.trim().to_string()))
            .filter(|v| !v.is_empty())
    };
    Some((field("ip=")?, field("loc=")?))
}

/// The machine's OWN egress, ip and Cloudflare's country for it, over the
/// default route (no proxy). This is the P16 primitive: menu 1 uses it to tell
/// the user whether their exit already lifts the gate, so the DNS layer is
/// insurance rather than load-bearing (G26). Advisory only - it never decides
/// what to install, because the 400 is invisible out of band and a permitted
/// exit is inferred, not proven.
///
/// Same trace hosts and parsing as `exit_info`, just dialed directly. Bounded so
/// a dead network cannot hang the menu.
#[allow(dead_code)]
pub fn machine_exit() -> Option<(String, String)> {
    TRACE_HOSTS.iter().find_map(|h| trace_direct(h))
}

#[allow(dead_code)]
fn trace_direct(host: &'static str) -> Option<(String, String)> {
    let addrs: Vec<_> = (host, 443u16).to_socket_addrs().ok()?.collect();
    let mut sock = None;
    for addr in addrs {
        if let Ok(s) = TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            sock = Some(s);
            break;
        }
    }
    let mut sock = sock?;
    sock.set_read_timeout(Some(PROBE_BUDGET)).ok();
    sock.set_write_timeout(Some(PROBE_BUDGET)).ok();
    let name = ServerName::try_from(host).ok()?;
    let mut tls = ClientConnection::new(crate::proxy::probe_config(), name).ok()?;
    let mut stream = rustls::Stream::new(&mut tls, &mut sock);
    stream
        .write_all(
            format!(
                "GET /cdn-cgi/trace HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                host
            )
            .as_bytes(),
        )
        .ok()?;
    let mut body = Vec::new();
    let mut chunk = [0u8; 2048];
    while let Ok(n) = stream.read(&mut chunk) {
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > 8 * 1024 {
            break;
        }
    }
    let text = String::from_utf8_lossy(&body);
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name).map(|v| v.trim().to_string()))
            .filter(|v| !v.is_empty())
    };
    Some((field("ip=")?, field("loc=")?))
}

/// Regions where a proxy is pointless, because they are the ones being blocked.
/// Not a complete list and not meant to be - it exists to catch the common case
/// of someone pointing this at a VPN that surfaces next door.
/// Where the gate refuses. Russia and Belarus are the ones measured; the rest are
/// the countries Google lists no Gemini service for at all, which is the same
/// refusal for the same reason - and a VPN or proxy exiting in one of them must
/// not be offered as a way around it.
const BLOCKED_REGIONS: &[&str] = &["RU", "BY", "CN", "HK", "MO", "IR", "KP", "SY", "CU"];

pub fn region_is_blocked(loc: &str) -> bool {
    BLOCKED_REGIONS.iter().any(|r| r.eq_ignore_ascii_case(loc))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live: the P16 primitive reaches Cloudflare directly and reads the exit
    /// country. Ignored (needs the network); the point is that the direct
    /// rustls path works, not the value.
    #[test]
    #[ignore = "needs a live network; run with --ignored"]
    fn machine_exit_reports_a_country() {
        let (ip, loc) = machine_exit().expect("a trace");
        println!("exit ip={} loc={}", ip, loc);
        assert!(!ip.is_empty() && loc.len() == 2, "ip={} loc={}", ip, loc);
    }

    #[test]
    fn reads_the_shapes_people_actually_paste() {
        let plain = Upstream {
            host: "127.0.0.1".into(),
            port: 1371,
            auth: None,
            tls: false,
        };
        assert_eq!(parse("127.0.0.1:1371").unwrap(), plain);
        assert_eq!(parse("http://127.0.0.1:1371").unwrap(), plain);
        assert_eq!(parse("  http://127.0.0.1:1371/  ").unwrap(), plain);
        assert_eq!(
            parse("http://user:pw@proxy.example.com:8080").unwrap(),
            Upstream {
                host: "proxy.example.com".into(),
                port: 8080,
                auth: Some("user:pw".into()),
                tls: false,
            }
        );
        // A password may contain '@'; a hostname may not, so the last one wins.
        assert_eq!(
            parse("user:p@ss@proxy.example.com:8080").unwrap().auth,
            Some("user:p@ss".into())
        );
    }

    /// A mis-parsed proxy is indistinguishable from a proxy that is down, so
    /// anything ambiguous has to be refused while the user is still looking.
    #[test]
    fn refuses_what_it_cannot_be_sure_of() {
        for bad in [
            "",
            "   ",
            "proxy.example.com",
            "proxy.example.com:",
            "proxy.example.com:port",
            "proxy.example.com:0",
            "socks5://127.0.0.1:1080",
            "user@proxy.example.com:8080",
        ] {
            assert!(parse(bad).is_err(), "should have been refused: {:?}", bad);
        }
    }

    /// `https://` is an HTTPS proxy and stays one through the file: it is the
    /// difference between a password on the wire and one inside TLS.
    #[test]
    fn an_https_proxy_keeps_its_scheme() {
        let up = parse("https://u:p@edge.example.com:8443").unwrap();
        assert!(up.tls);
        assert_eq!(up.as_line(), "https://u:p@edge.example.com:8443");
        assert_eq!(parse(&up.as_line()).unwrap(), up);
        assert_eq!(up.display(), "https://u:***@edge.example.com:8443");
        assert!(!parse("u:p@edge.example.com:8443").unwrap().tls);
    }

    /// Several proxies in the order given, however they were separated; the
    /// same one twice counts once; a bad one is named by number only.
    #[test]
    fn a_list_keeps_its_order_and_never_echoes_a_password() {
        let ups = parse_list(
            "https://a:1@one.example.com:8443,\nhttps://b:2@two.example.com:8443;  https://a:1@one.example.com:8443",
        )
        .unwrap();
        assert_eq!(ups.len(), 2);
        assert_eq!(ups[0].host, "one.example.com");
        assert_eq!(ups[1].host, "two.example.com");

        let err = parse_list("https://a:secret@one.example.com:8443 https://b:hush@two.example.com:x")
            .unwrap_err();
        assert!(err.contains("№2"), "{err}");
        assert!(!err.contains("hush") && !err.contains("secret"), "{err}");

        let five = "h1:1 h2:2 h3:3 h4:4 h5:5";
        assert!(parse_list(five).is_err());
        assert!(parse_list("  ").is_err());
    }

    /// Live, against a real HTTPS proxy named in `AG_TEST_PROXY`
    /// (`https://host:port`, no credential): the TLS to the proxy and the
    /// `CONNECT` inside it work, and a probe-resistant proxy's refusal reads as
    /// a credential problem rather than an outage.
    #[test]
    #[ignore = "needs a live HTTPS proxy in AG_TEST_PROXY; run with --ignored"]
    fn an_https_proxy_refuses_a_stranger_inside_tls() {
        let Ok(addr) = std::env::var("AG_TEST_PROXY") else {
            return;
        };
        let up = parse(&addr).expect("AG_TEST_PROXY parses");
        assert!(up.tls && up.auth.is_none());
        let err = open(&up, PROBE_HOST, 443, LIVE_OPEN_BUDGET).expect_err("no credential");
        assert_eq!(err, "прокси не принял логин и пароль");
    }

    /// Live: every proxy the window saved carries a request to Google. Reads
    /// the saved file, so no password passes through the test's own text.
    #[test]
    #[ignore = "needs saved proxies; run with --ignored"]
    fn every_saved_proxy_carries_a_request() {
        let ups = configured_all();
        assert!(!ups.is_empty(), "no proxies saved");
        for (i, up) in ups.iter().enumerate() {
            let t = Instant::now();
            probe(up).unwrap_or_else(|e| panic!("proxy {}: {}", i + 1, e));
            println!("proxy {} ok in {} ms", i + 1, t.elapsed().as_millis());
        }
    }

    /// Caddy's probe-resistant proxy answers a `CONNECT` it did not
    /// authenticate with a redirect to its cover site, never a 407.
    #[test]
    fn a_redirect_on_connect_reads_as_a_refused_credential() {
        assert_eq!(refusal("HTTP/1.1 200 OK", "h"), None);
        assert_eq!(refusal("HTTP/1.0 200 Connection established", "h"), None);
        assert_eq!(
            refusal("HTTP/1.1 308 Permanent Redirect", "h").as_deref(),
            Some("прокси не принял логин и пароль")
        );
        assert!(refusal("HTTP/1.1 403 Forbidden", "x.example.com")
            .unwrap()
            .contains("x.example.com"));
        assert!(refusal("HTTP/1.1 407 Proxy Authentication Required", "h").is_some());
    }

    /// A password must not come back out onto the screen.
    #[test]
    fn the_password_is_never_displayed() {
        let up = parse("bob:hunter2@proxy.example.com:8080").unwrap();
        let shown = up.display();
        assert!(!shown.contains("hunter2"), "{}", shown);
        assert!(shown.contains("bob") && shown.contains("proxy.example.com:8080"));
    }

    /// Round-trip through the file format, since that is what the relay reads.
    #[test]
    fn survives_the_trip_through_the_config_line() {
        for raw in [
            "127.0.0.1:1371",
            "bob:hunter2@proxy.example.com:8080",
            "[::1]:3128",
        ] {
            let up = parse(raw).unwrap();
            assert_eq!(parse(&up.as_line()).unwrap(), up, "{}", raw);
        }
    }

    /// A blocked exit stands the route down, and only a *different* address
    /// brings it back - not a timer, because waiting does not move a proxy.
    ///
    /// On its own `Route` rather than a global, which is the point of there being
    /// one per route: two of these could now run in parallel without fighting.
    #[test]
    fn a_blocked_exit_stands_the_route_down_until_the_address_changes() {
        let r = Route::new("тест", crate::routes::Kind::Own);

        r.note_blocked_exit("203.0.113.7", "RU");
        assert_eq!(r.bad_exit().as_deref(), Some("203.0.113.7"));
        assert!(!r.usable(), "a blocked exit takes the route out of service");

        // Same exit seen again on the next pass: still down, and it must not
        // announce itself a second time.
        r.note_blocked_exit("203.0.113.7", "RU");
        assert_eq!(r.bad_exit().as_deref(), Some("203.0.113.7"));

        // Rotated, still blocked: down, now pinned to the new address.
        r.note_blocked_exit("198.51.100.4", "BY");
        assert_eq!(r.bad_exit().as_deref(), Some("198.51.100.4"));

        // Rotated somewhere usable: back in service.
        r.note_usable_exit("203.0.113.9", "NL");
        assert_eq!(r.bad_exit(), None);
        assert!(r.usable());
        r.note_usable_exit("203.0.113.9", "NL");
        assert_eq!(r.bad_exit(), None);
    }

    /// One route standing down must not take another with it. This is the whole
    /// reason `Route` exists rather than a pair of statics: with a shared pin, one
    /// dead built-in exit would have benched the user's own proxy too.
    #[test]
    fn routes_stand_down_independently() {
        let a = Route::new("тест A", crate::routes::Kind::Own);
        let b = Route::new("тест B", crate::routes::Kind::Own);
        a.note_blocked_exit("203.0.113.7", "RU");
        a.health.note(false);
        a.health.note(false);
        assert!(!a.usable());
        assert!(
            b.usable(),
            "B was never at fault and must still be in service"
        );
    }

    /// Live: the whole path against a real proxy, which is the only way to know
    /// the parser, the CONNECT and the probe agree with each other. Ignored by
    /// default - it needs an HTTP proxy on `127.0.0.1:1371` and a network.
    ///
    ///     cargo test upstream_route_works_against_a_real_proxy -- --ignored --nocapture
    #[test]
    #[ignore = "needs an HTTP proxy on 127.0.0.1:1371 and a live network"]
    fn upstream_route_works_against_a_real_proxy() {
        let up = parse("127.0.0.1:1371").expect("parsed");
        probe(&up).expect("the proxy carried a request to Google");
        let (_, loc) = exit_info(&up).expect("exit country");
        println!(
            "exit country: {} (blocked: {})",
            loc,
            region_is_blocked(&loc)
        );
        assert!(!loc.is_empty());
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(basic("f"), "Zg==");
        assert_eq!(basic("fo"), "Zm8=");
        assert_eq!(basic("foo"), "Zm9v");
        assert_eq!(basic("foobar"), "Zm9vYmFy");
    }

    /// The check that saves a user believing a same-region VPN will help.
    #[test]
    fn a_proxy_that_surfaces_in_the_blocked_region_is_recognised() {
        assert!(region_is_blocked("RU"));
        assert!(region_is_blocked("ru"));
        assert!(!region_is_blocked("NL"));
        assert!(!region_is_blocked(""));
    }
}
