// zapret2 management CLI (Rust reimplementation)
//
// Drop-in replacement for the proprietary /usr/bin/zapret2 helper shipped in
// the vendor's zapret2 package.  Provides:
//   status, genconfig, dump, reset, patch-apply, patch-revert
//
// Only std is used on purpose: the binary has to be small and cross-compile
// without a crates.io dependency tree.

use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::{exit, Command, Stdio};

const VERSION: &str = "1.0.0-r1";
const ZAPRET_BASE: &str = "/opt/zapret2";
const UCI2CONFIG: &str = "/opt/zapret2/init.d/openwrt/uci2config";
const INIT_SCRIPT: &str = "/opt/zapret2/init.d/openwrt/zapret2";
const CUSTOM_SH: &str = "/opt/zapret2/common/custom.sh";
const DEFAULTS: &str = "/opt/zapret2/defaults/zapret2.uci";
const LOCK_FILE: &str = "/var/lock/zapret2_genconfig.lock";

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
const LOCK_EX: i32 = 2;
const LOCK_UN: i32 = 8;

const INIT_PATCH: &str = include_str!("../assets/init.sh");
const CUSTOM_PATCH: &str = include_str!("../assets/custom.sh");

fn usage(prog: &str) {
    println!("Usage: {} <command> [options]", prog);
    println!("Commands:");
    println!("  status      Show daemon status");
    println!("  genconfig   Generate shell config from UCI");
    println!("  dump        Dump configuration as JSON with validation");
    println!("  reset       Reset configuration to defaults");
    println!("  patch-apply   Apply embedded shell patches (creates .orig backups)");
    println!("  patch-revert  Revert shell patches (restores .orig files)");
    println!("Daemon management (use init script):");
    println!("  /etc/init.d/zapret2 start|stop|restart|reload");
    println!("Options:");
    println!("  -v, --version  Show version");
    println!("  -h, --help     Show this help");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let prog = args
        .get(0)
        .and_then(|s| Path::new(s).file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("zapret2")
        .to_string();

    if args.len() < 2 {
        usage(&prog);
        exit(1);
    }

    match args[1].as_str() {
        "-v" | "--version" | "version" => {
            println!("zapret2 version {}", VERSION);
        }
        "-h" | "--help" | "help" => usage(&prog),
        "status" => cmd_status(),
        "genconfig" => cmd_genconfig(),
        "dump" => cmd_dump(),
        "reset" => cmd_reset(),
        "patch-apply" => cmd_patch(true),
        "patch-revert" => cmd_patch(false),
        other => {
            eprintln!("Unknown command: {}", other);
            usage(&prog);
            exit(1);
        }
    }
}

/// Return (instances, first_pid) for nfqws2 processes.
fn nfqws2_instances() -> (usize, Option<u32>) {
    let mut pids: Vec<u32> = Vec::new();
    if let Ok(rd) = fs::read_dir("/proc") {
        for entry in rd.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let pid: u32 = match name.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let comm = fs::read_to_string(format!("/proc/{}/comm", pid));
            if let Ok(comm) = comm {
                if comm.trim() == "nfqws2" {
                    pids.push(pid);
                }
            }
        }
    }
    pids.sort_unstable();
    (pids.len(), pids.first().copied())
}

fn uci_show() -> Vec<String> {
    let out = Command::new("uci")
        .args(["-q", "show", "zapret2"])
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn unquote_uci(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        return v[1..v.len() - 1].replace("'\\''", "'");
    }
    v.to_string()
}

#[derive(Default)]
struct Section {
    name: String,
    typ: String,
    opts: Vec<(String, String)>,
}

fn parse_uci(lines: &[String]) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for line in lines {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let mut parts = key.split('.');
        if parts.next() != Some("zapret2") {
            continue;
        }
        let Some(sec) = parts.next() else { continue };
        match parts.next() {
            None => sections.push(Section {
                name: sec.to_string(),
                typ: value.to_string(),
                opts: Vec::new(),
            }),
            Some(opt) => {
                if let Some(s) = sections.iter_mut().find(|s| s.name == sec) {
                    s.opts.push((opt.to_string(), unquote_uci(value)));
                }
            }
        }
    }
    sections
}

fn strategy_count(sections: &[Section]) -> usize {
    sections
        .iter()
        .filter(|s| s.typ == "strategy")
        .filter(|s| {
            s.opts
                .iter()
                .find(|(k, _)| k == "enabled")
                .map(|(_, v)| v != "0")
                .unwrap_or(true)
        })
        .count()
}

fn cmd_status() {
    let (instances, pid) = nfqws2_instances();
    if instances == 0 {
        println!("zapret2 inactive");
        return;
    }
    let sections = parse_uci(&uci_show());
    println!("zapret2 is running");
    println!("  Instances: {}", instances);
    if let Some(pid) = pid {
        println!("  First PID: {}", pid);
    }
    println!("  Strategies: {} enabled", strategy_count(&sections));
}

fn cmd_genconfig() {
    println!("Generating config from UCI...");

    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(LOCK_FILE);
    let lock = match lock {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open lock file: {}", e);
            exit(1);
        }
    };
    let rc = unsafe { flock(lock.as_raw_fd(), LOCK_EX) };
    if rc != 0 {
        eprintln!("Failed to acquire lock: {}", std::io::Error::last_os_error());
        exit(1);
    }

    let status = Command::new(UCI2CONFIG).status();
    // closing the fd releases the lock
    drop(lock);

    match status {
        Ok(s) if s.success() => {
            println!("Config generated successfully");
        }
        Ok(s) => {
            eprintln!("Config generation failed (exit {})", s.code().unwrap_or(-1));
            exit(1);
        }
        Err(e) => {
            eprintln!("Config generation failed: {}", e);
            exit(1);
        }
    }
}

fn cmd_reset() {
    if !Path::new(DEFAULTS).exists() {
        eprintln!("Defaults file not found: {}", DEFAULTS);
        exit(1);
    }
    let script = format!(
        "uci import zapret2 < '{}' && uci commit zapret2",
        DEFAULTS
    );
    let st = Command::new("/bin/sh").arg("-c").arg(&script).status();
    match st {
        Ok(s) if s.success() => println!("Configuration reset to defaults"),
        _ => {
            eprintln!("Failed to reset configuration");
            exit(1);
        }
    }
}

fn cmd_patch(apply: bool) {
    let targets: [(&str, &str); 2] = [
        (CUSTOM_SH, CUSTOM_PATCH),
        (INIT_SCRIPT, INIT_PATCH),
    ];
    for (path, content) in targets {
        let p = Path::new(path);
        let orig = format!("{}.orig", path);
        if apply {
            if !p.exists() {
                eprintln!("patches: target missing, skipping: {}", path);
                continue;
            }
            if !Path::new(&orig).exists() {
                if let Err(e) = fs::copy(p, &orig) {
                    eprintln!("patches: backup {} -> {} failed: {}", path, orig, e);
                    continue;
                }
            }
            let tmp = format!("{}.tmp", path);
            match fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp)
            {
                Ok(mut f) => {
                    if let Err(e) = f.write_all(content.as_bytes()) {
                        eprintln!("patches: write {} failed: {}", tmp, e);
                        let _ = fs::remove_file(&tmp);
                        continue;
                    }
                }
                Err(e) => {
                    eprintln!("patches: open {} failed: {}", tmp, e);
                    continue;
                }
            }
            if let Ok(meta) = fs::metadata(&orig) {
                let _ = fs::set_permissions(&tmp, meta.permissions());
            } else {
                let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
            }
            match fs::rename(&tmp, p) {
                Ok(_) => println!("patches: applied {}", path),
                Err(e) => {
                    eprintln!("patches: rename {} -> {} failed: {}", tmp, path, e);
                    let _ = fs::remove_file(&tmp);
                }
            }
        } else if Path::new(&orig).exists() {
            match fs::rename(&orig, p) {
                Ok(_) => println!("patches: reverted {}", path),
                Err(e) => {
                    eprintln!("patches: restore {} -> {} failed: {}", orig, path, e);
                }
            }
        }
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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

fn json_obj(pairs: &[(String, String)]) -> String {
    let mut s = String::from("{");
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('"');
        s.push_str(&json_escape(k));
        s.push_str("\":\"");
        s.push_str(&json_escape(v));
        s.push('"');
    }
    s.push('}');
    s
}

fn cmd_dump() {
    let sections = parse_uci(&uci_show());
    let (instances, pid) = nfqws2_instances();

    println!("{{");
    println!("  \"version\": \"{}\",", json_escape(VERSION));
    println!("  \"config\": {{");
    if let Some(main) = sections.iter().find(|s| s.typ == "main") {
        let pairs = &main.opts;
        for (i, (k, v)) in pairs.iter().enumerate() {
            println!(
                "    \"{}\": \"{}\"{}",
                json_escape(k),
                json_escape(v),
                if i + 1 < pairs.len() { "," } else { "" }
            );
        }
    }
    println!("  }},");
    println!("  \"daemon\": {{");
    println!("    \"running\": {},", instances > 0);
    println!("    \"instances\": {},", instances);
    println!("    \"pid\": {},", pid.unwrap_or(0));
    println!("    \"strategies\": {}", strategy_count(&sections));
    println!("  }},");

    for (typ, key) in [
        ("strategy", "strategies"),
        ("hostlist", "hostlists"),
        ("blob", "blobs"),
        ("luascript", "luascripts"),
    ] {
        let items: Vec<&Section> = sections.iter().filter(|s| s.typ == typ).collect();
        println!("  \"{}\": [", key);
        for (i, sec) in items.iter().enumerate() {
            let mut pairs: Vec<(String, String)> =
                vec![("name".to_string(), sec.name.clone())];
            pairs.extend(sec.opts.iter().cloned());
            println!(
                "    {}{}",
                json_obj(&pairs),
                if i + 1 < items.len() { "," } else { "" }
            );
        }
        println!("  ],");
    }

    println!("  \"base\": \"{}\"", ZAPRET_BASE);
    println!("}}");
}
