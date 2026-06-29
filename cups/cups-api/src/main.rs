use axum::{routing::get, Json, Router};
use regex::Regex;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time;

// ── Config (from env) ────────────────────────────────────────

static PORT: LazyLock<u16> =
    LazyLock::new(|| std::env::var("CUPS_API_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8000));

static MQTT_HOST: LazyLock<String> =
    LazyLock::new(|| std::env::var("MQTT_HOST").unwrap_or_default());

static MQTT_PORT: LazyLock<u16> =
    LazyLock::new(|| std::env::var("MQTT_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(1883));

static MQTT_USER: LazyLock<String> =
    LazyLock::new(|| std::env::var("MQTT_USERNAME").unwrap_or_default());

static MQTT_PASS: LazyLock<String> =
    LazyLock::new(|| std::env::var("MQTT_PASSWORD").unwrap_or_default());

// ── Data types ────────────────────────────────────────────────

#[derive(Serialize)]
struct StatusResponse {
    cups: CupsInfo,
    printers: Vec<PrinterInfo>,
    jobs_completed_total: u64,
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

fn parse_status() -> StatusResponse {
    let (rout, _) = run(&["-r"]);
    let cups_running = rout.contains("is running");

    let (vout, _) = run(&["-v"]);
    let version = vout.lines().next().unwrap_or("unknown").to_string();

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

        let mut make_and_model = String::new();
        let mut device_uri = String::new();
        let mut accepting_jobs = true;
        let mut state_reasons: Vec<String> = Vec::new();

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

    StatusResponse {
        cups: CupsInfo { is_running: cups_running, version },
        printers,
        jobs_completed_total,
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

async fn mqtt_loop() {
    let client_id = format!("cups-api-{}", std::process::id());
    let mut mqttopts = MqttOptions::new(&client_id, MQTT_HOST.as_str(), *MQTT_PORT);
    mqttopts.set_keep_alive(Duration::from_secs(60));
    mqttopts.set_clean_session(true);

    if !MQTT_USER.is_empty() {
        mqttopts.set_credentials(&MQTT_USER, &MQTT_PASS);
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
    let app = Router::new()
        .route("/api/status", get(handle_status))
        .route("/health", get(handle_health));

    let addr = format!("0.0.0.0:{}", *PORT);
    eprintln!("cups-api listening on {addr}");
    let listener = TcpListener::bind(&addr).await.unwrap();

    // Start MQTT in background if host is configured
    if !MQTT_HOST.is_empty() {
        eprintln!("mqtt auto-discovery enabled → {}:{}", *MQTT_HOST, *MQTT_PORT);
        tokio::spawn(async {
            // Keep restarting on disconnect
            loop {
                mqtt_loop().await;
                eprintln!("mqtt reconnecting in 10s...");
                time::sleep(Duration::from_secs(10)).await;
            }
        });
    } else {
        eprintln!("mqtt not configured — skipping auto-discovery");
    }

    axum::serve(listener, app).await.unwrap();
}
