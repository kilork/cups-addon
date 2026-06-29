use axum::{routing::get, Json, Router};
use regex::Regex;
use serde::Serialize;
use std::process::Command;
use std::sync::LazyLock;
use tokio::net::TcpListener;

static PORT: LazyLock<u16> =
    LazyLock::new(|| std::env::var("CUPS_API_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8000));

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
    // --- CUPS alive check ---
    let (rout, _) = run(&["-r"]);
    let cups_running = rout.contains("is running");

    // --- CUPS version ---
    let (vout, _) = run(&["-v"]);
    let version = vout.lines().next().unwrap_or("unknown").to_string();

    // --- Printer list ---
    let (pout, _) = run(&["-p"]);
    let re_printer = Regex::new(r"^printer\s+(\S+)\s+is\s+(\S+)").unwrap();

    let mut printers: Vec<PrinterInfo> = Vec::new();

    for cap in re_printer.captures_iter(&pout) {
        let name = cap[1].to_string();
        let state = cap[2].to_string();

        // Detail via lpstat -l -p <name>
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

        // Jobs in queue for this printer
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

    // --- Completed jobs (from error_log) ---
    let completed = run_piped(
        "grep",
        &["-c", "Job completed", "/share/cups/logs/error_log"],
    );
    let jobs_completed_total: u64 = completed
        .0
        .trim()
        .parse()
        .unwrap_or(0);

    StatusResponse {
        cups: CupsInfo {
            is_running: cups_running,
            version,
        },
        printers,
        jobs_completed_total,
    }
}

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

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/status", get(handle_status))
        .route("/health", get(handle_health));

    let addr = format!("0.0.0.0:{}", *PORT);
    eprintln!("cups-api listening on {addr}");
    let listener = TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
