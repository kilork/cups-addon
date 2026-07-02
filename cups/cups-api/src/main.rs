use axum::{routing::get, Json, Router};
use std::os::unix::fs::symlink;
use regex::Regex;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time;

// ── Config (from env / supervisor API) ───────────────────────

static PORT: LazyLock<u16> =
    LazyLock::new(|| std::env::var("CUPS_API_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8000));

#[derive(Clone)]
struct MqttConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
}

/// Try to detect MQTT settings. Priority:
/// 1. Manual env vars (MQTT_HOST etc.)
/// 2. HA Supervisor API (http://supervisor/services/mqtt)
async fn detect_mqtt() -> Option<MqttConfig> {
    // 1. Manual env var
    let env_host = std::env::var("MQTT_HOST").unwrap_or_default();
    if !env_host.is_empty() {
        return Some(MqttConfig {
            host: env_host,
            port: std::env::var("MQTT_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(1883),
            username: std::env::var("MQTT_USERNAME").unwrap_or_default(),
            password: std::env::var("MQTT_PASSWORD").unwrap_or_default(),
        });
    }

    // 2. HA Supervisor API (requires services: [mqtt:want] in config.yaml)
    let token = std::env::var("SUPERVISOR_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return None;
    }

    eprintln!("detecting MQTT via supervisor API...");

    let client = reqwest::Client::new();
    let resp = client
        .get("http://supervisor/services/mqtt")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            #[derive(Deserialize)]
            struct MqttPayload {
                result: String,
                data: MqttData,
            }
            #[derive(Deserialize)]
            struct MqttData {
                host: String,
                port: u16,
                username: Option<String>,
                password: Option<String>,
            }
            match r.json::<MqttPayload>().await {
                Ok(payload) if payload.result == "ok" => Some(MqttConfig {
                    host: payload.data.host,
                    port: payload.data.port,
                    username: payload.data.username.unwrap_or_default(),
                    password: payload.data.password.unwrap_or_default(),
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

// ── Data types ────────────────────────────────────────────────

#[derive(Serialize)]
struct StatusResponse {
    cups: CupsInfo,
    printers: Vec<PrinterInfo>,
    jobs_completed_total: u64,
    avahi_running: bool,
}

#[derive(Serialize)]
struct CupsInfo {
    is_running: bool,
    version: String,
}

#[derive(Serialize)]
struct PrinterInfo {
    name: String,
    state: String,
    is_accepting_jobs: bool,
    is_enabled: bool,
    make_and_model: String,
    device_uri: String,
    state_reasons: Vec<String>,
    jobs_in_queue: usize,
}

#[derive(Serialize)]
struct MqttState {
    state: String,
    printer_name: String,
}

// ── CUPS helpers ─────────────────────────────────────────────

fn run(args: &[&str]) -> (String, String) {
    let output = Command::new("lpstat").args(args).output();
    match output {
        Ok(o) => (
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        ),
        Err(e) => (String::new(), format!("lpstat error: {e}")),
    }
}

fn run_piped(cmd: &str, args: &[&str]) -> (String, String) {
    let output = Command::new(cmd).args(args).output();
    match output {
        Ok(o) => (
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        ),
        Err(e) => (String::new(), format!("{cmd} error: {e}")),
    }
}

// ── Setup ──────────────────────────────────────────────────

fn setup() {
    // Directories
    let dirs = [
        "/share/cups/cache",
        "/share/cups/logs",
        "/share/cups/state",
        "/share/cups/config",
        "/share/cups/config/ppd",
        "/share/cups/config/ssl",
    ];
    for d in &dirs {
        std::fs::create_dir_all(d).ok();
    }

    // Permissions
    run_piped("chown", &["-R", "root:lp", "/share/cups"]);
    run_piped("chmod", &["-R", "775", "/share/cups"]);

    // Start D-Bus system bus (required by Avahi)
    std::process::Command::new("mkdir")
        .args(["-p", "/var/run/dbus"])
        .status()
        .ok();
    std::process::Command::new("dbus-daemon")
        .args(["--system"])
        .spawn()
        .ok();

    // Start Avahi mDNS responder for Apple device discovery
    std::process::Command::new("avahi-daemon")
        .args(["-D"])
        .spawn()
        .ok();

    // cupsd.conf
    let conf = r#"# Listen on all interfaces
Listen 0.0.0.0:631

# mDNS/DNS-SD — allow Apple devices to auto-discover printers
ServerName %H
BrowseLocalProtocols dnssd
BrowseDNSSDSubTypes _cups,_print

<Location />
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Location>

<Location /admin>
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Location>

<Location /jobs>
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Location>

<Limit Send-Document Send-URI Hold-Job Release-Job Restart-Job Purge-Jobs Set-Job-Attributes Create-Job-Subscription Renew-Subscription Cancel-Subscription Get-Notifications Reprocess-Job Cancel-Current-Job Suspend-Current-Job Resume-Job Cancel-My-Jobs Close-Job CUPS-Move-Job CUPS-Get-Document>
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Limit>

WebInterface Yes
LogLevel info
DefaultAuthType None
JobSheets none,none
PreserveJobHistory No
"#;
    std::fs::write("/share/cups/config/cupsd.conf", conf).ok();

    // Migrate legacy /data/cups → /share/cups
    let migrated = std::path::Path::new("/share/cups/config/.migrated");
    if std::path::Path::new("/data/cups/config").exists() && !migrated.exists() {
        eprintln!("migrating /data/cups → /share/cups...");
        for entry in &[
            "printers.conf",
            "printers.conf.O",
        ] {
            let src = format!("/data/cups/config/{}", entry);
            let dst = format!("/share/cups/config/{}", entry);
            if std::path::Path::new(&src).exists() {
                std::fs::copy(&src, &dst).ok();
            }
        }
        for sub in &["ppd", "ssl"] {
            let src = format!("/data/cups/config/{}", sub);
            let dst = format!("/share/cups/config/{}", sub);
            if std::path::Path::new(&src).is_dir() {
                run_piped("cp", &["-r", &format!("{}/.", src), &dst]);
            }
        }
        std::fs::write(migrated, b"").ok();
    }

    // Symlink /etc/cups → /share/cups/config
    let _ = std::fs::remove_dir_all("/etc/cups");
    let _ = std::fs::remove_file("/etc/cups");
    symlink("/share/cups/config", "/etc/cups").ok();

    // Ensure printers.conf exists
    if !std::path::Path::new("/share/cups/config/printers.conf").exists() {
        std::fs::write("/share/cups/config/printers.conf", b"").ok();
    }

    // Copy default config files from package /etc/cups/ that don't exist yet
    // (This runs after the symlink, so we check /share/cups/config directly)
    // Defaults like cups-files.conf are installed by the Alpine package
    for default_file in &["cups-files.conf", "snmp.conf", "subscriptions.conf"] {
        let default_src = format!("/usr/share/cups/default-config/{}", default_file);
        let target = format!("/share/cups/config/{}", default_file);
        if std::path::Path::new(&default_src).exists() && !std::path::Path::new(&target).exists() {
            std::fs::copy(&default_src, &target).ok();
        }
    }

    // Install user-supplied printer driver .deb
    let options_path = "/data/options.json";
    if let Ok(content) = std::fs::read_to_string(options_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(deb) = json["printer_driver_deb"].as_str() {
                if !deb.is_empty() {
                    let deb_path = format!("/share/{}", deb);
                    if std::path::Path::new(&deb_path).exists() {
                        eprintln!("installing driver from {deb_path}...");
                        let extract_dir = format!("/tmp/driver-{}", std::process::id());
                        let _ = std::fs::create_dir_all(&extract_dir);
                        run_piped("dpkg", &["-x", &deb_path, &extract_dir]);

                        let filter_src = format!("{extract_dir}/usr/lib/cups/filter");
                        if std::path::Path::new(&filter_src).is_dir() {
                            run_piped("cp", &["-r", &format!("{filter_src}/."), "/usr/lib/cups/filter"]);
                            run_piped("chmod", &["755", "/usr/lib/cups/filter/*"]);
                        }
                        let lib_src = format!("{extract_dir}/usr/lib");
                        if std::path::Path::new(&lib_src).is_dir() {
                            run_piped("find", &[&lib_src, "-name", "*.so*", "-exec", "cp", "{}", "/usr/lib/", ";"]);
                        }
                        let model_src = format!("{extract_dir}/usr/share/cups/model");
                        if std::path::Path::new(&model_src).is_dir() {
                            run_piped("cp", &["-r", &format!("{model_src}/."), "/usr/share/cups/model"]);
                        }
                        let _ = std::fs::remove_dir_all(&extract_dir);
                    }
                }
            }
        }
    }

    eprintln!("setup complete");
}

fn parse_status() -> StatusResponse {
    let (rout, _) = run(&["-r"]);
    let cups_running = rout.contains("is running");

    // CUPS version
    let version = run_piped("cups-config", &["--version"]).0.trim().to_string();
    let version = if version.is_empty() { "CUPS".to_string() } else { format!("CUPS {}", version) };

    let (pout, _) = run(&["-p"]);
    let re_printer = Regex::new(r"^printer\s+(\S+)\s+is\s+(\S+)").unwrap();

    let mut printers: Vec<PrinterInfo> = Vec::new();

    for cap in re_printer.captures_iter(&pout) {
        let name = cap[1].to_string();
        let state = cap[2].to_string();
        let (dout, _) = run(&["-l", "-p", &name]);

        let re_make = Regex::new(r"Make and Model:\s+(.*)").unwrap();
        let re_dev = Regex::new(r"Device URI:\s+(.*)").unwrap();
        let re_accept = Regex::new(r"Printer is\s+(.*)").unwrap();
        let re_reason = Regex::new(r"Reason\(s\):\s+(.*)").unwrap();
        let re_desc = Regex::new(r"Description:\s+(.*)").unwrap();

        let mut make_and_model = String::new();
        let mut device_uri = String::new();
        let mut accepting_jobs = true;
        let mut state_reasons: Vec<String> = Vec::new();
        let mut description = String::new();

        for line in dout.lines() {
            if let Some(m) = re_make.captures(line) {
                make_and_model = m[1].trim().to_string();
            }
            if let Some(m) = re_dev.captures(line) {
                device_uri = m[1].trim().to_string();
            }
            if let Some(m) = re_accept.captures(line) {
                accepting_jobs = m[1].contains("accepting");
            }
            if let Some(m) = re_reason.captures(line) {
                state_reasons = m[1].split(',').map(|s| s.trim().to_string()).collect();
            }
            if let Some(m) = re_desc.captures(line) {
                description = m[1].trim().to_string();
            }
        }
        // lpstat -l -p doesn't show "Make and Model"; fall back to Description
        if make_and_model.is_empty() {
            make_and_model = description;
        }

        let (jout, _) = run(&["-o"]);
        let jobs_in_queue = jout.lines().filter(|l| l.starts_with(&name)).count();

        printers.push(PrinterInfo {
            name,
            state,
            is_accepting_jobs: accepting_jobs,
            is_enabled: cups_running,
            make_and_model,
            device_uri,
            state_reasons,
            jobs_in_queue,
        });
    }

    let completed = run_piped("grep", &["-c", "Job completed", "/share/cups/logs/error_log"]);
    let jobs_completed_total: u64 = completed.0.trim().parse().unwrap_or(0);

    let avahi_running = std::process::Command::new("pidof")
        .args(["avahi-daemon"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    StatusResponse {
        cups: CupsInfo { is_running: cups_running, version },
        printers,
        jobs_completed_total,
        avahi_running,
    }
}

// ── MQTT auto-discovery ──────────────────────────────────────

fn mqtt_discovery_config() -> String {
    serde_json::json!({
        "name": "CUPS Printer",
        "state_topic": "homeassistant/sensor/cups_printer/state",
        "json_attributes_topic": "homeassistant/sensor/cups_printer/attributes",
        "unique_id": "cups_addon_printer",
        "icon": "mdi:printer",
        "device_class": "enum",
        "value_template": "{{ value_json.state }}",
        "device": {
            "identifiers": ["cups_addon"],
            "name": "CUPS Print Server",
            "manufacturer": "CUPS",
            "model": "CUPS Print Server"
        }
    }).to_string()
}

async fn mqtt_publish(client: &AsyncClient) {
    let status = parse_status();
    let primary = status.printers.first();

    // State topic — concise
    let state_msg = serde_json::json!({
        "state": primary.map_or("unknown", |p| &p.state),
        "printer_name": primary.map_or("", |p| &p.name),
    });
    let _ = client
        .publish(
            "homeassistant/sensor/cups_printer/state",
            QoS::AtMostOnce,
            false,
            state_msg.to_string(),
        )
        .await;

    // Attributes topic — full payload
    let _ = client
        .publish(
            "homeassistant/sensor/cups_printer/attributes",
            QoS::AtMostOnce,
            false,
            serde_json::to_string(&status).unwrap_or_default(),
        )
        .await;
}

async fn mqtt_loop(cfg: MqttConfig) {
    let client_id = format!("cups-api-{}", std::process::id());
    let mut mqttopts = MqttOptions::new(&client_id, &cfg.host, cfg.port);
    mqttopts.set_keep_alive(Duration::from_secs(60));
    mqttopts.set_clean_session(true);

    if !cfg.username.is_empty() {
        mqttopts.set_credentials(&cfg.username, &cfg.password);
    }

    let (client, mut eventloop) = AsyncClient::new(mqttopts, 100);

    // Publish discovery config immediately
    let _ = client
        .publish(
            "homeassistant/sensor/cups_printer/config",
            QoS::AtMostOnce,
            true,
            mqtt_discovery_config(),
        )
        .await;

    // Publish status immediately, then every 30s
    mqtt_publish(&client).await;

    let mut interval = time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                mqtt_publish(&client).await;
            }
            event = eventloop.poll() => {
                match event {
                    Ok(rumqttc::Event::Incoming(_)) => {},
                    Ok(rumqttc::Event::Outgoing(_)) => {},
                    Err(e) => {
                        eprintln!("mqtt error: {e}");
                        // Reconnect after a delay on error
                        time::sleep(Duration::from_secs(10)).await;
                        break;  // restart the loop
                    }
                }
            }
        }
    }
}

// ── REST handlers ────────────────────────────────────────────

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

async fn handle_status() -> Json<StatusResponse> {
    Json(parse_status())
}

async fn handle_health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

// ── Main ─────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // ── Setup ────────────────────────────────────────────
    setup();

    // ── Start CUPS daemon as child process ────────────────
    eprintln!("starting CUPS daemon...");
    let mut cupsd = tokio::process::Command::new("/usr/sbin/cupsd")
        .arg("-f")
        .kill_on_drop(true)
        .spawn()
        .expect("failed to start cupsd");

    // ── Start HTTP server ────────────────────────────────
    let app = Router::new()
        .route("/api/status", get(handle_status))
        .route("/health", get(handle_health));

    let addr = format!("0.0.0.0:{}", *PORT);
    let listener = TcpListener::bind(&addr).await.unwrap();
    eprintln!("cups-api listening on {addr}");

    // ── Start MQTT in background if detected ─────────────
    if let Some(cfg) = detect_mqtt().await {
        eprintln!("mqtt auto-discovery enabled → {}:{}", cfg.host, cfg.port);
        tokio::spawn(async move {
            loop {
                mqtt_loop(cfg.clone()).await;
                eprintln!("mqtt reconnecting in 10s...");
                time::sleep(Duration::from_secs(10)).await;
            }
        });
    } else {
        eprintln!("mqtt not detected — skipping auto-discovery");
    }

    // ── Serve HTTP until cupsd exits ────────────────────
    tokio::select! {
        _ = axum::serve(listener, app) => {}
        status = cupsd.wait() => {
            match status {
                Ok(s) if s.success() => eprintln!("cupsd exited cleanly"),
                Ok(s) => eprintln!("cupsd exited with code {}", s.code().unwrap_or(-1)),
                Err(e) => eprintln!("cupsd error: {e}"),
            }
        }
    }
    eprintln!("shutting down");
}
