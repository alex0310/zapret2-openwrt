// zapret2-finder (Rust reimplementation)
//
// Implements the CLI contract of the vendor's proprietary zapret2-finder and a
// practical subset of its strategy search.  Strategies are plain nfqws2
// argument tails; results are written both to stdout (which LuCI tees into
// /tmp/zapret2-finder.log) and to /tmp/finder.log in the "fixed=N/M <args>"
// format parsed by the LuCI frontend.
//
// Only std is used so the binary stays small and builds without crates.io.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{exit, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const RESULTS_FILE: &str = "/tmp/finder.log";
const STOP_FILE: &str = "/tmp/zapret2-finder.stop";
const DEFAULT_BASE: &str = "/opt/zapret2";
const MARK: u32 = 0x4000_0000;
const QNUM: u32 = 65300;
const VERSION: &str = "1.0.0-r1";

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn geteuid() -> u32;
    fn signal(sig: i32, handler: usize) -> usize;
}

extern "C" fn on_signal(_sig: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Proto {
    Http,
    Tls12,
    Tls13,
    Quic,
}

impl Proto {
    #[allow(dead_code)]
    fn label(self) -> &'static str {
        match self {
            Proto::Http => "http",
            Proto::Tls12 => "https_tls12",
            Proto::Tls13 => "https_tls13",
            Proto::Quic => "quic",
        }
    }
}

#[derive(Clone)]
struct Opts {
    base: String,
    domains: Vec<String>,
    domains_file: Vec<String>,
    ipv4: bool,
    ipv6: bool,
    http: bool,
    tls12: bool,
    tls13: bool,
    quic: bool,
    curl_get: bool,
    blob_dir: String,
    no_default_blob: bool,
    no_fake: bool,
    no_multisplit: bool,
    no_oob: bool,
    no_seqovl: bool,
    no_syndata: bool,
    no_faked: bool,
    no_hostfake: bool,
    no_combo_multi: bool,
    no_combo_faked: bool,
    no_combo_hostfake: bool,
    no_combo_syndata: bool,
    no_combo_oob: bool,
    no_combo_ackdrop: bool,
    no_combo_doublefake: bool,
    no_combo_seqovl: bool,
    no_combo_ipfrag: bool,
    combo_reverse: bool,
    combo_only: bool,
    no_tcpseg: bool,
    no_wssize: bool,
    no_md5: bool,
    no_badsum: bool,
    no_seq: bool,
    no_ack: bool,
    no_ts: bool,
    no_flags: bool,
    no_ttl: bool,
    no_autottl: bool,
    no_sni: bool,
    sni: String,
    min_kb: u64,
    http_port: u16,
    https_port: u16,
    quic_port: u16,
    jobs: usize,
    verbose: bool,
    simulate: bool,
    extra_pre: String,
    extra_post: String,
    max_found: usize,
    fake_repeats: u32,
    test_strategy: Option<String>,
    builtin: bool,
    timeout: u64,
    repeats: usize,
    repeat_parallel: bool,
    scan: String,
    json: bool,
    batch: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            base: DEFAULT_BASE.to_string(),
            domains: Vec::new(),
            domains_file: Vec::new(),
            ipv4: false,
            ipv6: false,
            http: true,
            tls12: true,
            tls13: false,
            quic: false,
            curl_get: false,
            blob_dir: String::new(),
            no_default_blob: false,
            no_fake: false,
            no_multisplit: false,
            no_oob: false,
            no_seqovl: false,
            no_syndata: false,
            no_faked: false,
            no_hostfake: false,
            no_combo_multi: false,
            no_combo_faked: false,
            no_combo_hostfake: false,
            no_combo_syndata: false,
            no_combo_oob: false,
            no_combo_ackdrop: false,
            no_combo_doublefake: false,
            no_combo_seqovl: false,
            no_combo_ipfrag: false,
            combo_reverse: false,
            combo_only: false,
            no_tcpseg: false,
            no_wssize: false,
            no_md5: false,
            no_badsum: false,
            no_seq: false,
            no_ack: false,
            no_ts: false,
            no_flags: false,
            no_ttl: false,
            no_autottl: false,
            no_sni: false,
            sni: String::new(),
            min_kb: 0,
            http_port: 80,
            https_port: 443,
            quic_port: 443,
            jobs: 8,
            verbose: false,
            simulate: false,
            extra_pre: String::new(),
            extra_post: String::new(),
            max_found: 0,
            fake_repeats: 1,
            test_strategy: None,
            builtin: false,
            timeout: 10,
            repeats: 1,
            repeat_parallel: false,
            scan: "standard".to_string(),
            json: false,
            batch: false,
        }
    }
}

impl Opts {
    fn protocols(&self) -> Vec<Proto> {
        let mut v = Vec::new();
        if self.tls12 {
            v.push(Proto::Tls12);
        }
        if self.tls13 {
            v.push(Proto::Tls13);
        }
        if self.http {
            v.push(Proto::Http);
        }
        if self.quic {
            v.push(Proto::Quic);
        }
        v
    }
}

fn usage(prog: &str) {
    println!("Usage: {} [options]", prog);
    println!("Options:");
    println!("  -d, --domain DOMAIN     Domain to test (can specify multiple)");
    println!("  -f, --domains-file PATH Load domains from text file");
    println!("  -b, --base PATH         Path to zapret2 directory (default: {})", DEFAULT_BASE);
    println!("  -4 / -6 / -46           Address family");
    println!("  --http / --no-http      Enable/disable HTTP (default on)");
    println!("  --tls12 / --no-tls12    Enable/disable TLS 1.2 (default on)");
    println!("  --tls13, --quic         Enable TLS 1.3 / HTTP3");
    println!("  --blob-dir PATH         Custom fake blob directory");
    println!("  --no-default-blob       Do not test fake_default_tls");
    println!("  --no-fake --no-multisplit --no-oob --no-seqovl --no-syndata");
    println!("  --no-faked --no-hostfake --no-tcpseg --no-wssize");
    println!("  --combo-only --combo-reverse --no-combo-*");
    println!("  --no-md5 --no-badsum --no-seq --no-ack --no-ts --no-flags");
    println!("  --no-ttl --no-autottl --sni DOMAIN --no-sni");
    println!("  --min-kb N --jobs N --timeout N -r N --rp N");
    println!("  --extra-pre ARGS --extra-post ARGS --max-found N --fake-repeats N");
    println!("  --test-strategy ARGS    Test a single strategy");
    println!("  --builtin               Use built-in host list (33 hosts)");
    println!("  -s, --scan LEVEL        quick|standard|force (default: standard)");
    println!("  -j, --json              JSON output");
    println!("  -h, --help              Show this help");
    println!("  --device-check          Device compatibility check");
}

fn main() {
    let argv: Vec<String> = env::args().collect();
    let prog = argv
        .get(0)
        .and_then(|p| Path::new(p).file_name())
        .and_then(|p| p.to_str())
        .unwrap_or("zapret2-finder")
        .to_string();

    // Split --opt=value forms early.
    let mut raw: Vec<String> = Vec::new();
    for a in argv.iter().skip(1) {
        if a.starts_with('-') && a.contains('=') && !a.starts_with("--extra-") {
            let (k, v) = a.split_once('=').unwrap();
            raw.push(k.to_string());
            raw.push(v.to_string());
        } else {
            raw.push(a.clone());
        }
    }

    let mut o = Opts::default();
    let mut i = 0usize;
    let next = |i: &mut usize| -> Option<String> {
        if *i + 1 < raw.len() {
            *i += 1;
            Some(raw[*i].clone())
        } else {
            None
        }
    };

    while i < raw.len() {
        let a = raw[i].clone();
        match a.as_str() {
            "--device-check" => {
                // The vendor hides the Finder tab on third-party boards.  A
                // rebuilt, open package should not enforce that restriction.
                exit(0);
            }
            "-h" | "--help" | "help" => {
                usage(&prog);
                exit(0);
            }
            "-v" | "--version" | "version" => {
                println!("zapret2-finder {}", VERSION);
                exit(0);
            }
            "-d" | "--domain" => {
                if let Some(v) = next(&mut i) {
                    o.domains.push(v);
                }
            }
            "-f" | "--domains-file" => {
                if let Some(v) = next(&mut i) {
                    o.domains_file.push(v);
                }
            }
            "-b" | "--base" => {
                if let Some(v) = next(&mut i) {
                    o.base = v;
                }
            }
            "-4" | "--ipv4" => {
                o.ipv4 = true;
                o.ipv6 = false;
            }
            "-6" | "--ipv6" => {
                o.ipv4 = false;
                o.ipv6 = true;
            }
            "-46" | "--ipv-any" => {
                o.ipv4 = true;
                o.ipv6 = true;
            }
            "--ipv" => {
                if let Some(v) = next(&mut i) {
                    o.ipv4 = v.contains('4');
                    o.ipv6 = v.contains('6');
                }
            }
            "--http" => o.http = true,
            "--no-http" => o.http = false,
            "--tls12" => o.tls12 = true,
            "--no-tls12" => o.tls12 = false,
            "--tls13" => o.tls13 = true,
            "--quic" => o.quic = true,
            "--curl-get" => o.curl_get = true,
            "--blob-dir" => {
                if let Some(v) = next(&mut i) {
                    o.blob_dir = v;
                }
            }
            "--no-default-blob" => o.no_default_blob = true,
            "--no-fake" => o.no_fake = true,
            "--no-multisplit" => o.no_multisplit = true,
            "--no-oob" => o.no_oob = true,
            "--no-seqovl" => o.no_seqovl = true,
            "--no-syndata" => o.no_syndata = true,
            "--no-faked" => o.no_faked = true,
            "--no-hostfake" => o.no_hostfake = true,
            "--no-combo-multi" => o.no_combo_multi = true,
            "--no-combo-faked" => o.no_combo_faked = true,
            "--no-combo-hostfake" => o.no_combo_hostfake = true,
            "--no-combo-syndata" => o.no_combo_syndata = true,
            "--no-combo-oob" => o.no_combo_oob = true,
            "--no-combo-ackdrop" => o.no_combo_ackdrop = true,
            "--no-combo-doublefake" => o.no_combo_doublefake = true,
            "--no-combo-seqovl" => o.no_combo_seqovl = true,
            "--no-combo-ipfrag" => o.no_combo_ipfrag = true,
            "--combo-reverse" => o.combo_reverse = true,
            "--combo-only" => o.combo_only = true,
            "--no-tcpseg" => o.no_tcpseg = true,
            "--no-wssize" => o.no_wssize = true,
            "--no-md5" => o.no_md5 = true,
            "--no-badsum" => o.no_badsum = true,
            "--no-seq" => o.no_seq = true,
            "--no-ack" => o.no_ack = true,
            "--no-ts" => o.no_ts = true,
            "--no-flags" => o.no_flags = true,
            "--no-ttl" => o.no_ttl = true,
            "--no-autottl" => o.no_autottl = true,
            "--no-sni" => o.no_sni = true,
            "--sni" => {
                if let Some(v) = next(&mut i) {
                    o.sni = v;
                }
            }
            "--min-kb" => {
                if let Some(v) = next(&mut i) {
                    o.min_kb = v.parse().unwrap_or(0);
                }
            }
            "--http-port" => {
                if let Some(v) = next(&mut i) {
                    o.http_port = v.parse().unwrap_or(80);
                }
            }
            "--https-port" => {
                if let Some(v) = next(&mut i) {
                    o.https_port = v.parse().unwrap_or(443);
                }
            }
            "--quic-port" => {
                if let Some(v) = next(&mut i) {
                    o.quic_port = v.parse().unwrap_or(443);
                }
            }
            "--jobs" => {
                if let Some(v) = next(&mut i) {
                    o.jobs = v.parse().unwrap_or(8).max(1);
                }
            }
            "--verbose" => o.verbose = true,
            "--simulate" => o.simulate = true,
            "--extra-pre" => {
                if let Some(v) = next(&mut i) {
                    o.extra_pre = v;
                }
            }
            "--extra-post" => {
                if let Some(v) = next(&mut i) {
                    o.extra_post = v;
                }
            }
            "--max-found" => {
                if let Some(v) = next(&mut i) {
                    o.max_found = v.parse().unwrap_or(0);
                }
            }
            "--fake-repeats" => {
                if let Some(v) = next(&mut i) {
                    o.fake_repeats = v.parse().unwrap_or(1).max(1);
                }
            }
            "--test-strategy" => {
                if let Some(v) = next(&mut i) {
                    o.test_strategy = Some(v);
                }
            }
            "--builtin" => o.builtin = true,
            "--builtin-timeout" => {
                let _ = next(&mut i);
            }
            "--timeout" => {
                if let Some(v) = next(&mut i) {
                    o.timeout = v.parse().unwrap_or(10).max(1);
                }
            }
            "-r" | "--repeats" => {
                if let Some(v) = next(&mut i) {
                    o.repeats = v.parse().unwrap_or(1).max(1);
                }
            }
            "--rp" | "--repeat-parallel" => {
                if let Some(v) = next(&mut i) {
                    o.repeats = v.parse().unwrap_or(1).max(1);
                    o.repeat_parallel = true;
                }
            }
            "-s" | "--scan" => {
                if let Some(v) = next(&mut i) {
                    o.scan = v;
                }
            }
            "-j" | "--json" => o.json = true,
            "--batch" => o.batch = true,
            _ => {
                if o.verbose {
                    eprintln!("unknown option: {}", a);
                }
            }
        }
        i += 1;
    }

    unsafe {
        signal(2, on_signal as *const () as usize);
        signal(15, on_signal as *const () as usize);
    }

    if unsafe { geteuid() } != 0 {
        eprintln!("this program requires root privileges");
        exit(1);
    }

    if !o.ipv4 && !o.ipv6 {
        o.ipv4 = true;
    }
    if !o.http && !o.tls12 && !o.tls13 && !o.quic {
        o.tls12 = true;
    }
    if o.domains.is_empty() && o.domains_file.is_empty() && !o.builtin {
        o.builtin = true;
    }

    let nfqws2 = format!("{}/nfq2/nfqws2", o.base);
    if !Path::new(&nfqws2).exists() {
        eprintln!("{} not found or not executable", nfqws2);
        exit(1);
    }
    if which("curl").is_none() {
        eprintln!("curl not found");
        exit(1);
    }
    if which("nft").is_none() {
        eprintln!("no supported firewall found (nftables required)");
        exit(1);
    }

    println!("=== zapret2-finder ===");
    println!("* system detection");
    println!("  OS: Linux");
    println!("  firewall: nftables");
    println!("  curl: {}", which("curl").unwrap_or_default());
    println!("  base: {}", o.base);

    // Remove leftovers from a previous interrupted run.
    cleanup_stale_tables();

    let (strategies, blob_args) = load_strategy_list(&o);

    if o.simulate {
        println!("strategies: {}", strategies.len());
        for (n, s) in strategies.iter().enumerate() {
            println!("  [{}] {}", n + 1, s);
        }
        exit(0);
    }

    // A fresh run must not show results left over from an earlier run.  Do
    // this before any early exit (e.g. all hosts already reachable).
    let _ = fs::write(RESULTS_FILE, "");

    let mut hosts = load_hosts(&o);
    if hosts.is_empty() {
        eprintln!("no hosts to test");
        exit(1);
    }
    resolve_hosts(&mut hosts, &o);

    if !hosts.iter().any(|h| !h.ips4.is_empty() || !h.ips6.is_empty()) {
        eprintln!("DNS resolution failed for all hosts");
        exit(1);
    }

    // Stop the running service so it does not steal NFQUEUE traffic.
    println!("* stopping running zapret2 service...");
    let stopped = Command::new("/etc/init.d/zapret2")
        .arg("stop")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if stopped {
        println!("  zapret2 stopped");
    } else {
        println!("!!! WARNING: could not stop zapret2, results may be inaccurate !!!");
    }

    let protos = o.protocols();
    println!(
        "* baseline: testing {} hosts without bypass ({} protocols)...",
        hosts.len(),
        protos.len()
    );
    let baseline = test_all(&hosts, &o, &protos);
    let blocked: Vec<Host> = hosts
        .iter()
        .zip(baseline.iter())
        .filter(|(_, ok)| !**ok)
        .map(|(h, _)| h.clone())
        .collect();
    println!("  {}/{} hosts blocked", blocked.len(), hosts.len());
    if blocked.is_empty() {
        println!("  all hosts accessible without bypass, nothing to test");
        exit(0);
    }

    println!(
        "* testing {} strategies against {} blocked hosts...",
        strategies.len(),
        blocked.len()
    );

    let ips = collect_ips(&blocked);
    if ips.is_empty() {
        eprintln!("no resolved addresses for blocked hosts");
        exit(1);
    }
    let ports_tcp = {
        let mut v: Vec<u16> = Vec::new();
        if o.http {
            v.push(o.http_port);
        }
        if o.tls12 || o.tls13 {
            v.push(o.https_port);
        }
        v.sort_unstable();
        v.dedup();
        v
    };
    let ports_udp = if o.quic { vec![o.quic_port] } else { Vec::new() };

    let mut results: Vec<(usize, usize, String)> = Vec::new();
    let mut found = 0usize;

    for (idx, strategy) in strategies.iter().enumerate() {
        if INTERRUPTED.load(Ordering::SeqCst) || Path::new(STOP_FILE).exists() {
            println!("\n* interrupted after testing {} strategies", idx);
            break;
        }

        let table = format!("zapret2_finder_{}", std::process::id());
        let fw = Firewall::setup(&table, &ips, &ports_tcp, &ports_udp);
        if !fw.active {
            eprintln!("failed to set up firewall rules, aborting");
            exit(1);
        }

        let mut args = base_args(&o, &blob_args);
        if !ports_tcp.is_empty() {
            args.push(format!(
                "--filter-tcp={}",
                ports_tcp
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !ports_udp.is_empty() {
            args.push(format!(
                "--filter-udp={}",
                ports_udp
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !o.extra_pre.is_empty() {
            args.extend(split_args(&o.extra_pre));
        }
        args.extend(split_args(strategy));
        if !o.extra_post.is_empty() {
            args.extend(split_args(&o.extra_post));
        }
        if o.fake_repeats > 1 {
            for a in args.iter_mut() {
                if a.contains("repeats=1") {
                    *a = a.replace("repeats=1", &format!("repeats={}", o.fake_repeats));
                }
            }
        }

        let mut child = match Command::new(&nfqws2)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("cannot start nfqws2: {}", e);
                drop(fw);
                continue;
            }
        };

        thread::sleep(Duration::from_millis(350));
        if let Ok(Some(st)) = child.try_wait() {
            if o.verbose {
                eprintln!(
                    "nfqws2 exited immediately ({}): {}",
                    st.code().unwrap_or(-1),
                    strategy
                );
            }
            let _ = child.kill();
            let _ = child.wait();
            drop(fw);
            continue;
        }

        let ok = test_all(&blocked, &o, &protos);
        let fixed = ok.iter().filter(|x| **x).count();

        let _ = child.kill();
        let _ = child.wait();
        drop(fw);

        let total = blocked.len();
        if fixed > 0 {
            found += 1;
            let line = format!("fixed={}/{} {}", fixed, total, strategy);
            println!("  [{}] {}", idx + 1, line);
            append_result(&line);
            results.push((fixed, total, strategy.clone()));
            if o.max_found > 0 && found >= o.max_found {
                println!("* reached --max-found {}", o.max_found);
                break;
            }
        } else if o.verbose {
            println!("  [{}] fixed=0/{} {}", idx + 1, total, strategy);
        }
    }

    results.sort_by(|a, b| b.0.cmp(&a.0));
    if o.json {
        print_json(&results, blocked.len());
    } else {
        println!("\n* SUMMARY ({} results)", results.len());
        for (fixed, total, s) in &results {
            println!("  {}/{} : {}", fixed, total, s);
        }
    }

    let _ = fs::remove_file(STOP_FILE);
}

fn append_result(line: &str) {
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(RESULTS_FILE)
    {
        let _ = writeln!(f, "{}", line);
    }
}

fn print_json(results: &[(usize, usize, String)], total_hosts: usize) {
    println!("{{");
    println!("  \"total_hosts\": {},", total_hosts);
    println!("  \"results\": [");
    for (i, (fixed, total, s)) in results.iter().enumerate() {
        println!(
            "    {{\"fixed\": {}, \"total\": {}, \"strategy\": \"{}\"}}{}",
            fixed,
            total,
            json_escape(s),
            if i + 1 < results.len() { "," } else { "" }
        );
    }
    println!("  ]");
    println!("}}");
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn which(prog: &str) -> Option<String> {
    if prog.contains('/') {
        return if Path::new(prog).exists() {
            Some(prog.to_string())
        } else {
            None
        };
    }
    let path = env::var("PATH").unwrap_or_else(|_| "/usr/sbin:/usr/bin:/sbin:/bin".into());
    for dir in path.split(':') {
        let p = format!("{}/{}", dir, prog);
        if let Ok(meta) = fs::metadata(&p) {
            if meta.permissions().mode() & 0o111 != 0 {
                return Some(p);
            }
        }
    }
    None
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Clone)]
struct Host {
    name: String,
    provider: String,
    domain: String,
    url: String,
    ips4: Vec<String>,
    ips6: Vec<String>,
}

fn builtin_hosts() -> Vec<Host> {
    let rows: &[(&str, &str, &str, &str)] = &[
        ("media.miele.com", "Akamai", "media.miele.com", "https://media.miele.com/images/2000015/200001503/20000150334.png"),
        ("www.roxio.com", "Akamai", "www.roxio.com", "https://www.roxio.com/static/roxio/videos/products/nxt9/lamp-magic.mp4"),
        ("www.getscope.com", "AWS", "www.getscope.com", "https://www.getscope.com/assets/fonts/fa-solid-900.woff2"),
        ("corp.kaltura.com", "AWS", "corp.kaltura.com", "https://corp.kaltura.com/wp-content/cache/min/1/wp-content/themes/airfleet/dist/styles/theme.css"),
        ("cdn.eso.org", "CDN77", "cdn.eso.org", "https://cdn.eso.org/images/banner1920/eso2520a.jpg"),
        ("www.bigcartel.com", "Cloudflare", "www.bigcartel.com", "https://www.bigcartel.com/_next/static/chunks/453-03e77cda85f8a09a.js"),
        ("aegis.audioeye.com", "Cloudflare", "aegis.audioeye.com", "https://aegis.audioeye.com/assets/index.js"),
        ("img.wzstats.gg", "Cloudflare", "img.wzstats.gg", "https://img.wzstats.gg/cleaver/gunFullDisplay"),
        ("esm.sh", "Cloudflare", "esm.sh", "https://esm.sh/gh/esm-dev/esm.sh@e7447dea04/server/embed/assets/sceenshot-deno-types.png"),
        ("www.cateringexner.cz", "Contabo", "www.cateringexner.cz", "https://www.cateringexner.cz/font/ebrima/ebrima.woff2"),
        ("findair.net", "Contabo", "findair.net", "https://findair.net/wp-content/uploads/2025/07/online-booking-2.jpeg"),
        ("carishealthcare.com", "DigitalOcean", "carishealthcare.com", "https://carishealthcare.com/content/uploads/2025/04/Rectangle-105.jpg"),
        ("ecomstal.com", "DigitalOcean", "ecomstal.com", "https://ecomstal.com/_next/static/css/73cc557714b4846b.css"),
        ("ssl.p.jwpcdn.com", "Fastly", "ssl.p.jwpcdn.com", "https://ssl.p.jwpcdn.com/player/v/8.40.5/bidding.js"),
        ("www.jetblue.com", "Fastly", "www.jetblue.com", "https://www.jetblue.com/footer/footer-element-es2015.js"),
        ("gcore.com", "Gcore", "gcore.com", "https://gcore.com/assets/fonts/Montserrat-Variable.woff2"),
        ("api.usercentrics.eu", "Google Cloud", "api.usercentrics.eu", "https://api.usercentrics.eu/gvl/v3/en.json"),
        ("widgets.reputation.com", "Google Cloud", "widgets.reputation.com", "https://widgets.reputation.com/fonts/Inter-Light.ttf"),
        ("apiwhatsapp-1000.zapipro.com", "Hetzner", "apiwhatsapp-1000.zapipro.com", "https://apiwhatsapp-1000.zapipro.com/libs/bootstrap/dist/css/bootstrap.min.css"),
        ("www.industrialport.net", "Hetzner", "www.industrialport.net", "https://www.industrialport.net/wp-content/uploads/custom-fonts/2022/10/Lato-Bold.ttf"),
        ("251b5cd9.nip.io", "Hetzner", "251b5cd9.nip.io", "https://251b5cd9.nip.io/1MB.bin"),
        ("nioges.com", "Hetzner", "nioges.com", "https://nioges.com/libs/fontawesome/webfonts/fa-solid-900.woff2"),
        ("5fd8bdae.nip.io", "Hetzner", "5fd8bdae.nip.io", "https://5fd8bdae.nip.io/1MB.bin"),
        ("5fd8bca5.nip.io", "Hetzner", "5fd8bca5.nip.io", "https://5fd8bca5.nip.io/1MB.bin"),
        ("twin.mentat.su", "Melbicom", "twin.mentat.su", "https://twin.mentat.su/assets/fonts/Inter-SemiBold.woff2"),
        ("plataforma.trackerintl.com", "Oracle", "plataforma.trackerintl.com", "https://plataforma.trackerintl.com/images/background.jpg"),
        ("global-seres.com.sg", "Oracle", "global-seres.com.sg", "https://global-seres.com.sg/wp-content/uploads/2024/02/SVG00732-scaled.jpg"),
        ("testing.symarobot.com", "OVH", "testing.symarobot.com", "https://testing.symarobot.com/content/images/logo.png"),
        ("filmoteka.net.pl", "OVH", "filmoteka.net.pl", "https://filmoteka.net.pl/css/bootstrap.min.css"),
        ("www.velivole.fr", "Scaleway", "www.velivole.fr", "https://www.velivole.fr/img/header.jpg"),
        ("static-cdn.play.date", "Vultr", "static-cdn.play.date", "https://static-cdn.play.date/static/js/model-viewer.min.js"),
        ("us.rudder.qntmnet.com", "Vultr", "us.rudder.qntmnet.com", "https://us.rudder.qntmnet.com/QN-CDN/images/qn_bg_.jpg"),
    ];
    rows.iter()
        .map(|(n, p, d, u)| Host {
            name: n.to_string(),
            provider: p.to_string(),
            domain: d.to_string(),
            url: u.to_string(),
            ips4: Vec::new(),
            ips6: Vec::new(),
        })
        .collect()
}

fn load_hosts(o: &Opts) -> Vec<Host> {
    let mut hosts: Vec<Host> = Vec::new();
    if o.builtin {
        hosts.extend(builtin_hosts());
    }
    if !o.domains.is_empty() {
        for d in &o.domains {
            let d = d.trim().trim_start_matches("http://").trim_start_matches("https://");
            let d = d.split('/').next().unwrap_or(d);
            if d.is_empty() {
                continue;
            }
            hosts.push(Host {
                name: d.to_string(),
                provider: "-".to_string(),
                domain: d.to_string(),
                url: format!("https://{}/", d),
                ips4: Vec::new(),
                ips6: Vec::new(),
            });
        }
    }
    for f in &o.domains_file {
        if let Ok(text) = fs::read_to_string(f) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let d = line.split_whitespace().next().unwrap_or(line);
                let d = d.trim_start_matches("http://").trim_start_matches("https://");
                let d = d.split('/').next().unwrap_or(d);
                if d.is_empty() {
                    continue;
                }
                hosts.push(Host {
                    name: d.to_string(),
                    provider: "-".to_string(),
                    domain: d.to_string(),
                    url: format!("https://{}/", d),
                    ips4: Vec::new(),
                    ips6: Vec::new(),
                });
            }
        } else {
            eprintln!("cannot open domains file: {}", f);
        }
    }
    let mut seen = HashSet::new();
    hosts.retain(|h| seen.insert(h.domain.clone()));
    hosts
}

fn resolve_hosts(hosts: &mut [Host], o: &Opts) {
    for h in hosts.iter_mut() {
        if o.ipv4 {
            h.ips4 = resolve(&h.domain, false);
        }
        if o.ipv6 {
            h.ips6 = resolve(&h.domain, true);
        }
        if h.ips4.is_empty() && h.ips6.is_empty() {
            println!("  {}: resolve failed", h.domain);
        } else {
            let n = h.ips4.len() + h.ips6.len();
            if o.verbose {
                println!("  {}: {} address(es)", h.domain, n);
            }
        }
    }
}

fn resolve(domain: &str, v6: bool) -> Vec<String> {
    let typ = if v6 { "AAAA" } else { "A" };
    // BusyBox nslookup (used on OpenWrt) understands -type= but not
    // -timeout=.  Only start parsing at the answer section: the "Address:"
    // line before "Name:" is the resolver itself, not the host.
    let out = Command::new("nslookup")
        .arg(format!("-type={}", typ))
        .arg(domain)
        .stderr(Stdio::null())
        .output();
    let text = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    };
    let mut ips = Vec::new();
    let mut after_name = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("Name:") {
            after_name = true;
            continue;
        }
        if !after_name {
            continue;
        }
        for tok in t.split(|c: char| c.is_whitespace() || c == ',' || c == '#') {
            let tok = tok.trim_matches(|c| c == '[' || c == ']' || c == '.');
            if v6 {
                let tok = tok.split('%').next().unwrap_or(tok);
                // Parsing is stricter than "contains a colon": nslookup's
                // "Address 1:" label must not be mistaken for an address.
                if tok.parse::<std::net::Ipv6Addr>().is_ok() && !ips.iter().any(|x| x == tok) {
                    ips.push(tok.to_string());
                }
            } else if is_ipv4(tok) && !ips.iter().any(|x| x == tok) {
                ips.push(tok.to_string());
            }
        }
    }
    ips
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u8>().is_ok()
        })
}

fn proto_port(proto: Proto, o: &Opts) -> u16 {
    match proto {
        Proto::Http => o.http_port,
        Proto::Tls12 | Proto::Tls13 => o.https_port,
        Proto::Quic => o.quic_port,
    }
}

fn protocol_url(h: &Host, proto: Proto, o: &Opts) -> String {
    let path = if let Some(rest) = h.url.splitn(2, "://").nth(1) {
        if let Some(pos) = rest.find('/') {
            &rest[pos..]
        } else {
            "/"
        }
    } else {
        "/"
    };
    let port = proto_port(proto, o);
    let default_port = matches!(proto, Proto::Http) && port == 80
        || !matches!(proto, Proto::Http) && port == 443;
    let authority = if default_port {
        h.domain.clone()
    } else {
        format!("{}:{}", h.domain, port)
    };
    match proto {
        Proto::Http => format!("http://{}{}", authority, path),
        _ => format!("https://{}{}", authority, path),
    }
}

fn probe(h: &Host, proto: Proto, o: &Opts) -> bool {
    let ips: &[String] = if !h.ips4.is_empty() && o.ipv4 {
        &h.ips4
    } else if o.ipv6 {
        &h.ips6
    } else {
        &h.ips4
    };
    if ips.is_empty() {
        return false;
    }
    let url = protocol_url(h, proto, o);
    let port = proto_port(proto, o);
    for ip in ips.iter().take(2) {
        // curl wants IPv6 literals in brackets in --resolve.
        let addr = if ip.contains(':') {
            format!("[{}]", ip)
        } else {
            ip.clone()
        };
        let mut cmd = Command::new("curl");
        cmd.args([
            "-sS",
            "-o",
            "/dev/null",
            "--max-time",
            &o.timeout.to_string(),
            "-A",
            "Mozilla/5.0",
            "--resolve",
            &format!("{}:{}:{}", h.domain, port, addr),
            "-w",
            "%{http_code} %{size_download}",
        ]);
        match proto {
            Proto::Tls12 => {
                cmd.arg("--tlsv1.2");
            }
            Proto::Tls13 => {
                cmd.arg("--tlsv1.3");
            }
            Proto::Quic => {
                cmd.arg("--http3-only");
            }
            Proto::Http => {}
        }
        cmd.arg(&url);
        cmd.stdout(Stdio::piped()).stderr(Stdio::null());
        if o.verbose {
            eprintln!("[curl] {:?}", cmd);
        }
        let out = cmd.output();
        if let Ok(out) = out {
            if out.status.success() {
                let txt = String::from_utf8_lossy(&out.stdout);
                let mut it = txt.split_whitespace();
                let code: u16 = it.next().unwrap_or("0").parse().unwrap_or(0);
                let size: u64 = it.next().unwrap_or("0").parse().unwrap_or(0);
                let size_ok = o.min_kb == 0 || size >= o.min_kb * 1024;
                if (200..400).contains(&code) && size_ok {
                    return true;
                }
            }
        }
    }
    false
}

fn test_host(h: &Host, o: &Opts, protos: &[Proto]) -> bool {
    let mut ok = false;
    for _ in 0..o.repeats {
        for p in protos {
            if probe(h, *p, o) {
                ok = true;
                break;
            }
        }
        if ok {
            break;
        }
    }
    ok
}

fn test_all(hosts: &[Host], o: &Opts, protos: &[Proto]) -> Vec<bool> {
    let jobs = o.jobs.max(1).min(hosts.len().max(1));
    let mut res = vec![false; hosts.len()];
    let mut start = 0usize;
    while start < hosts.len() {
        let end = (start + jobs).min(hosts.len());
        let mut handles = Vec::new();
        for h in &hosts[start..end] {
            let h = h.clone();
            let o = o.clone();
            let protos = protos.to_vec();
            handles.push(thread::spawn(move || test_host(&h, &o, &protos)));
        }
        for (k, hd) in handles.into_iter().enumerate() {
            res[start + k] = hd.join().unwrap_or(false);
        }
        start = end;
    }
    res
}

fn collect_ips(hosts: &[Host]) -> Vec<String> {
    let mut v = Vec::new();
    for h in hosts {
        for ip in h.ips4.iter().chain(h.ips6.iter()) {
            if !v.iter().any(|x| x == ip) {
                v.push(ip.clone());
            }
        }
    }
    v
}

fn base_args(o: &Opts, blob_args: &[(String, String)]) -> Vec<String> {
    let mut a = vec![
        "--uid=1:3003".to_string(),
        format!("--fwmark=0x{:08x}", MARK),
        format!("--qnum={}", QNUM),
        format!("--lua-init=@{}/lua/zapret-lib.lua", o.base),
        format!("--lua-init=@{}/lua/zapret-antidpi.lua", o.base),
        format!("--lua-init=@{}/lua/zapret-auto.lua", o.base),
    ];
    for (name, path) in blob_args {
        a.push(format!("--blob={}:@{}", name, path));
    }
    a
}

fn blob_search_dir(o: &Opts) -> String {
    if o.blob_dir.is_empty() {
        format!("{}/files/fake", o.base)
    } else {
        o.blob_dir.clone()
    }
}

fn all_blob_args(o: &Opts) -> Vec<(String, String)> {
    let dir = blob_search_dir(o);
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let path = e.path();
            let fname = match path.file_name().and_then(|s| s.to_str()) {
                Some(f) => f.to_string(),
                None => continue,
            };
            if !fname.ends_with(".bin") {
                continue;
            }
            out.push((
                fname.trim_end_matches(".bin").to_string(),
                path.to_string_lossy().to_string(),
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn load_strategy_list(o: &Opts) -> (Vec<String>, Vec<(String, String)>) {
    if let Some(ts) = &o.test_strategy {
        let text = if let Some(path) = ts.strip_prefix('@') {
            fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("cannot open strategy file: {}: {}", path, e);
                String::new()
            })
        } else {
            ts.clone()
        };
        let s = text.trim().to_string();
        if s.is_empty() {
            eprintln!("empty strategy");
            exit(1);
        }
        // A saved strategy can reference any blob shipped with the package.
        (vec![s], all_blob_args(o))
    } else {
        build_strategies(o)
    }
}

fn build_strategies(o: &Opts) -> (Vec<String>, Vec<(String, String)>) {
    let dir = if o.blob_dir.is_empty() {
        format!("{}/files/fake", o.base)
    } else {
        o.blob_dir.clone()
    };
    let mut tls_files: Vec<(String, String)> = Vec::new();
    let mut quic_files: Vec<(String, String)> = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let path = e.path();
            let fname = match path.file_name().and_then(|s| s.to_str()) {
                Some(f) => f.to_string(),
                None => continue,
            };
            if !fname.ends_with(".bin") {
                continue;
            }
            let stem = fname.trim_end_matches(".bin").to_string();
            let p = path.to_string_lossy().to_string();
            if stem.starts_with("tls_clienthello_") {
                tls_files.push((stem, p));
            } else if stem.starts_with("quic") {
                quic_files.push((stem, p));
            }
        }
    }
    tls_files.sort_by(|a, b| a.0.cmp(&b.0));
    quic_files.sort_by(|a, b| a.0.cmp(&b.0));

    let (ttls, positions, tls_limit, quic_limit): (Vec<u32>, Vec<&str>, usize, usize) =
        match o.scan.as_str() {
            "quick" => (
                vec![2, 4],
                vec!["1", "midsld"],
                1,
                1,
            ),
            "force" => (
                vec![1, 2, 3, 4, 5, 6, 8, 10],
                vec![
                    "1", "2", "midsld", "1,midsld", "sniext", "sniext+1", "host+1", "endhost-1",
                    "10,sniext+1", "10,midsld",
                ],
                tls_files.len().max(1),
                quic_files.len().max(1),
            ),
            _ => (
                vec![2, 4, 6],
                vec!["1", "2", "midsld", "1,midsld"],
                4,
                2,
            ),
        };
    tls_files.truncate(tls_limit.min(tls_files.len()));
    quic_files.truncate(quic_limit.min(quic_files.len()));

    let mut blob_args: Vec<(String, String)> = Vec::new();
    for (n, p) in tls_files.iter().chain(quic_files.iter()) {
        blob_args.push((n.clone(), p.clone()));
    }

    let mut tls_names: Vec<String> = tls_files.iter().map(|(n, _)| n.clone()).collect();
    if !o.no_default_blob {
        tls_names.insert(0, "fake_default_tls".to_string());
    }
    let quic_names: Vec<String> = quic_files.iter().map(|(n, _)| n.clone()).collect();

    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !s.is_empty() {
            out.push(s);
        }
    };
    let standalone = !o.combo_only;

    if standalone {
        if !o.no_fake {
            for blob in &tls_names {
                for ttl in &ttls {
                    push(format!(
                        "--lua-desync=fake:blob={}:ip4_ttl={}:repeats=1",
                        blob, ttl
                    ));
                    if o.ipv6 {
                        push(format!(
                            "--lua-desync=fake:blob={}:ip6_ttl={}:repeats=1",
                            blob, ttl
                        ));
                    }
                }
                if !o.no_autottl {
                    for n in 1..=2 {
                        push(format!(
                            "--lua-desync=fake:blob={}:ip4_autottl=-{},3-20:repeats=1",
                            blob, n
                        ));
                    }
                }
            }
            if !o.no_ttl {
                for ttl in [1u32, 2, 4].iter() {
                    push(format!(
                        "--lua-desync=fake:blob=0x00000000:ip4_ttl={}:repeats=1",
                        ttl
                    ));
                }
            }
            if !o.no_md5 {
                push("--lua-desync=fake:blob=fake_default_tls:tcp_md5:repeats=1".into());
            }
            if !o.no_badsum {
                push("--lua-desync=fake:blob=fake_default_tls:badsum:repeats=1".into());
            }
            if !o.no_seq {
                push("--lua-desync=fake:blob=fake_default_tls:tcp_seq=-3000:repeats=1".into());
            }
            if !o.no_ack {
                push("--lua-desync=fake:blob=fake_default_tls:tcp_ack=-66000:tcp_ts_up:repeats=1".into());
            }
            if !o.no_ts {
                push("--lua-desync=fake:blob=fake_default_tls:tcp_ts=-1000:repeats=1".into());
            }
            if !o.no_flags {
                push("--lua-desync=fake:blob=fake_default_tls:tcp_flags_unset=ACK:repeats=1".into());
                push("--lua-desync=fake:blob=fake_default_tls:tcp_flags_set=SYN:repeats=1".into());
            }
        }

        if !o.no_multisplit {
            for pos in &positions {
                push(format!("--lua-desync=multisplit:pos={}", pos));
                push(format!("--lua-desync=multidisorder:pos={}", pos));
            }
        }
        if !o.no_oob {
            push("--in-range=-s1 --lua-desync=oob:urp=1".into());
            push("--in-range=-s1 --lua-desync=oob:urp=2".into());
            push("--lua-desync=multisplit:pos=midsld:oob".into());
        }
        if !o.no_seqovl {
            push("--lua-desync=multisplit:pos=1:seqovl=1".into());
            push("--lua-desync=multidisorder:pos=1:seqovl=1".into());
            push("--lua-desync=tcpseg:pos=0,-1:seqovl=1 --lua-desync=drop".into());
        }
        if !o.no_syndata {
            push("--lua-desync=syndata".into());
            if let Some(blob) = tls_names.first() {
                push(format!("--lua-desync=syndata:blob={}", blob));
            }
        }
        if !o.no_faked {
            for pos in ["2", "midsld", "1,midsld"] {
                for ttl in [2u32, 4, 6] {
                    push(format!(
                        "--lua-desync=fakedsplit:pos={}:ip4_ttl={}:repeats=1",
                        pos, ttl
                    ));
                    push(format!(
                        "--lua-desync=fakeddisorder:pos={}:ip4_ttl={}:repeats=1",
                        pos, ttl
                    ));
                }
            }
        }
        if !o.no_hostfake {
            for ttl in [2u32, 4, 6] {
                push(format!(
                    "--lua-desync=hostfakesplit:ip4_ttl={}:repeats=1",
                    ttl
                ));
                push(format!(
                    "--lua-desync=hostfakesplit:nofake1:ip4_ttl={}:repeats=1",
                    ttl
                ));
                push(format!(
                    "--lua-desync=hostfakesplit:nofake2:ip4_ttl={}:repeats=1",
                    ttl
                ));
                push(format!(
                    "--lua-desync=hostfakesplit:midhost=midsld:ip4_ttl={}:repeats=1",
                    ttl
                ));
                push(format!(
                    "--lua-desync=hostfakesplit:disorder_after:ip4_ttl={}:repeats=1",
                    ttl
                ));
            }
        }
        if !o.no_tcpseg {
            push("--lua-desync=tcpseg:pos=0,1:ip_id=rnd:repeats=1".into());
            push("--lua-desync=tcpseg:pos=0,method+2:ip_id=rnd:repeats=1".into());
        }
        if !o.no_wssize {
            push("--lua-desync=wssize:wsize=1:scale=6".into());
        }
    }

    // Combos.
    let fake = format!(
        "--lua-desync=fake:blob={}:ip4_ttl=4:repeats=1",
        tls_names.first().map(|s| s.as_str()).unwrap_or("fake_default_tls")
    );
    let mut combos: Vec<(String, String)> = Vec::new();
    if !o.no_combo_multi {
        combos.push((fake.clone(), "--lua-desync=multisplit:pos=1".into()));
        combos.push((fake.clone(), "--lua-desync=multidisorder:pos=midsld".into()));
    }
    if !o.no_combo_faked {
        combos.push((
            fake.clone(),
            "--lua-desync=fakedsplit:pos=2:ip4_ttl=4:repeats=1".into(),
        ));
        combos.push((
            fake.clone(),
            "--lua-desync=fakeddisorder:pos=midsld:ip4_ttl=4:repeats=1".into(),
        ));
    }
    if !o.no_combo_hostfake {
        combos.push((
            fake.clone(),
            "--lua-desync=hostfakesplit:ip4_ttl=4:repeats=1".into(),
        ));
    }
    if !o.no_combo_syndata {
        combos.push((
            "--lua-desync=syndata".to_string(),
            "--lua-desync=multisplit:pos=1".into(),
        ));
    }
    if !o.no_combo_oob {
        combos.push((
            "--lua-desync=multisplit:pos=midsld:oob".to_string(),
            "--lua-desync=multidisorder:pos=1".into(),
        ));
    }
    if !o.no_combo_ackdrop {
        combos.push((
            fake.clone(),
            "--payload=empty --out-range=s1<d1 --lua-desync=pktmod:ip4_ttl=1".into(),
        ));
    }
    if !o.no_combo_doublefake {
        combos.push((
            "--lua-desync=fake:blob=0x00000000:ip4_ttl=4:repeats=1".to_string(),
            format!("--lua-desync=fake:blob={}:ip4_ttl=4:repeats=1", tls_names.first().map(|s| s.as_str()).unwrap_or("fake_default_tls")),
        ));
    }
    if !o.no_combo_seqovl {
        combos.push((
            fake.clone(),
            "--lua-desync=multisplit:pos=1:seqovl=1".into(),
        ));
    }
    for (a, b) in combos {
        if o.combo_reverse {
            push(format!("{} {}", b, a));
        } else {
            push(format!("{} {}", a, b));
        }
    }

    if o.quic {
        for blob in &quic_names {
            push(format!(
                "--payload=quic_initial --lua-desync=fake:blob={}:badsum:repeats=1",
                blob
            ));
            if !o.no_combo_ipfrag {
                push(format!(
                    "--payload=quic_initial --lua-desync=fake:blob={}:repeats=1 --lua-desync=send:ipfrag:ipfrag_pos_udp=8 --lua-desync=drop",
                    blob
                ));
            }
        }
    }

    if o.http {
        push("--lua-desync=http_hostcase".into());
        push("--lua-desync=http_hostcase:spell=hoSt".into());
        push("--lua-desync=http_domcase".into());
        push("--lua-desync=http_methodeol".into());
        push("--lua-desync=http_unixeol".into());
    }

    // De-duplicate while keeping order.
    let mut seen = HashSet::new();
    out.retain(|s| seen.insert(s.clone()));
    let limit = match o.scan.as_str() {
        "quick" => 40,
        "force" => 600,
        _ => 160,
    };
    out.truncate(limit);
    (out, blob_args)
}

fn cleanup_stale_tables() {
    let out = Command::new("nft").arg("list").arg("tables").output();
    let text = match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return,
    };
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "table" && parts[2].starts_with("zapret2_finder_") {
            let _ = Command::new("nft")
                .arg(format!("delete table {} {}", parts[1], parts[2]))
                .status();
        }
    }
}

struct Firewall {
    table: String,
    active: bool,
}

impl Firewall {
    fn setup(table: &str, ips: &[String], ports_tcp: &[u16], ports_udp: &[u16]) -> Firewall {
        let mut fw = Firewall {
            table: table.to_string(),
            active: false,
        };
        let v4: Vec<&String> = ips.iter().filter(|ip| !ip.contains(':')).collect();
        let v6: Vec<&String> = ips.iter().filter(|ip| ip.contains(':')).collect();
        if v4.is_empty() && v6.is_empty() {
            return fw;
        }
        let mut ok = nft(&format!("add table inet {}", table));
        ok &= nft(&format!(
            "add chain inet {} postnat {{ type filter hook postrouting priority 102; }}",
            table
        ));
        ok &= nft(&format!(
            "add chain inet {} prenat {{ type filter hook prerouting priority -102; }}",
            table
        ));
        ok &= nft(&format!(
            "add chain inet {} predefrag {{ type filter hook output priority -402; }}",
            table
        ));
        if !ok {
            fw.cleanup();
            return fw;
        }

        let tcp_ports = ports_tcp
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let udp_ports = ports_udp
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        for (family, list) in [("ipv4", &v4), ("ipv6", &v6)] {
            if list.is_empty() {
                continue;
            }
            let ipset = list
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let daddr = if family == "ipv4" { "ip" } else { "ip6" };
            let saddr = daddr;
            if !tcp_ports.is_empty() {
                nft(&format!(
                    "add rule inet {} postnat meta nfproto {} tcp dport {{ {} }} mark and 0x{:08x} == 0 {} daddr {{ {} }} ct mark set ct mark or 0x{:08x} queue num {}",
                    table, family, tcp_ports, MARK, daddr, ipset, MARK, QNUM
                ));
                nft(&format!(
                    "add rule inet {} prenat meta nfproto {} tcp sport {{ {} }} tcp flags & (syn | ack) == (syn | ack) {} saddr {{ {} }} queue num {}",
                    table, family, tcp_ports, saddr, ipset, QNUM
                ));
            }
            if !udp_ports.is_empty() {
                nft(&format!(
                    "add rule inet {} postnat meta nfproto {} udp dport {{ {} }} mark and 0x{:08x} == 0 {} daddr {{ {} }} ct mark set ct mark or 0x{:08x} queue num {}",
                    table, family, udp_ports, MARK, daddr, ipset, MARK, QNUM
                ));
            }
            nft(&format!(
                "add rule inet {} predefrag meta nfproto {} mark and 0x{:08x} != 0 notrack",
                table, family, MARK
            ));
            if family == "ipv4" {
                nft(&format!(
                    "add rule inet {} prenat icmp type time-exceeded ct mark and 0x{:08x} != 0 drop",
                    table, MARK
                ));
            } else {
                nft(&format!(
                    "add rule inet {} prenat icmpv6 type time-exceeded ct mark and 0x{:08x} != 0 drop",
                    table, MARK
                ));
            }
        }
        fw.active = true;
        fw
    }

    fn cleanup(&mut self) {
        if !self.table.is_empty() {
            let _ = Command::new("nft")
                .arg(format!("delete table inet {}", self.table))
                .status();
        }
        self.active = false;
    }
}

impl Drop for Firewall {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn nft(rule: &str) -> bool {
    Command::new("nft")
        .arg(rule)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
