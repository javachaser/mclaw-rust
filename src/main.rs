/*!
 * MClaw - OpenClaw Local Manager
 * Rust Edition - Single executable desktop app
 * Backend logic reimplemented in Rust with axum HTTP server
 */

use axum::{
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    process::Command,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};
use tokio::net::TcpListener;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{UserAttentionType, Window, WindowId},
};
use wry::{WebView, WebViewBuilder};

// ── Embedded frontend HTML ─────────────────────────────────────────────────────
const INDEX_HTML: &str = include_str!("../assets/index.html");

// ── OpenClaw config ────────────────────────────────────────────────────────────
const MCLAW_PORT: u16 = 19000;
const OPENCLAW_PORT: u16 = 18789;
const OPENCLAW_STATE_DIR: &str = "C:/soft/openclaw/config";
const OPENCLAW_TOKEN: &str = "899a26b6c9a4ddbc977614471cecf39b81518423ee7e4d2e";
const OPENCLAW_NODE_PATH: &str = r"C:\Program Files\nodejs\node.exe";
const OPENCLAW_INDEX_JS: &str =
    r"C:\Users\85949\AppData\Roaming\npm\node_modules\openclaw\dist\index.js";
const OPENCLAW_NPM_MODULES: &str = r"C:\Users\85949\AppData\Roaming\npm\node_modules";
const OPENCLAW_CONFIG_FILE: &str = r"C:\soft\openclaw\config\openclaw.json";
const OPENCLAW_MODELS_FILE: &str =
    r"C:\soft\openclaw\config\agents\main\agent\models.json";
const OPENCLAW_AUTH_FILE: &str =
    r"C:\soft\openclaw\config\agents\main\agent\auth-profiles.json";
const OPENCLAW_LOG_FILE: &str =
    r"C:\soft\openclaw\config\logs\gateway-native.log";
const OPENCLAW_LOG_ERR_FILE: &str =
    r"C:\soft\openclaw\config\logs\gateway-native-err.log";
const OPENROUTER_API_KEY: &str =
    "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";
const CLAWHUB_URL: &str = "https://cn.clawhub-mirror.com";

fn json_response(val: Value) -> Response {
    let body = val.to_string();
    (
        StatusCode::OK,
        [
            ("Access-Control-Allow-Origin", "*"),
            ("Content-Type", "application/json; charset=utf-8"),
        ],
        body,
    )
        .into_response()
}

fn configure_hidden_process(cmd: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

// ── Helper: check if TCP port is open ─────────────────────────────────────────
fn check_port(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_millis(1500),
    )
    .is_ok()
}

// ── Helper: get PID by port (Windows netstat) ──────────────────────────────────
fn get_pid_by_port(port: u16) -> Option<String> {
    let mut cmd = Command::new("netstat");
    cmd.args(["-ano"]);
    configure_hidden_process(&mut cmd);
    let output = cmd.output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let search = format!(":{}", port);
    for line in stdout.lines() {
        if line.contains(&search) && line.contains("LISTENING") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(pid) = parts.last() {
                if pid.chars().all(|c| c.is_ascii_digit()) {
                    return Some(pid.to_string());
                }
            }
        }
    }
    None
}

// ── Helper: detect installation ───────────────────────────────────────────────
#[derive(Serialize)]
struct InstallInfo {
    #[serde(rename = "nodeExists")]
    node_exists: bool,
    #[serde(rename = "packageExists")]
    package_exists: bool,
    #[serde(rename = "configExists")]
    config_exists: bool,
    #[serde(rename = "modelsFileExists")]
    models_file_exists: bool,
    version: Option<String>,
    #[serde(rename = "configPath")]
    config_path: Option<String>,
}

fn detect_installation() -> InstallInfo {
    let node_exists = Path::new(OPENCLAW_NODE_PATH).exists();
    let package_exists = Path::new(OPENCLAW_INDEX_JS).exists();
    let config_exists = Path::new(OPENCLAW_CONFIG_FILE).exists();
    let models_file_exists = Path::new(OPENCLAW_MODELS_FILE).exists();

    let version = if package_exists {
        let pkg_path = format!("{}\\openclaw\\package.json", OPENCLAW_NPM_MODULES);
        std::fs::read_to_string(&pkg_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v["version"].as_str().map(|s| s.to_string()))
    } else {
        None
    };

    InstallInfo {
        node_exists,
        package_exists,
        config_exists,
        models_file_exists,
        config_path: if config_exists {
            Some(OPENCLAW_CONFIG_FILE.to_string())
        } else {
            None
        },
        version,
    }
}

// ── Route: GET / (serve embedded HTML) ───────────────────────────────────────
async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

// ── Route: GET /api/detect ────────────────────────────────────────────────────
async fn api_detect() -> Response {
    let install = detect_installation();
    json_response(json!({ "success": true, "data": install }))
}

// ── Route: GET /api/status ────────────────────────────────────────────────────
async fn api_status() -> Response {
    let install = detect_installation();
    let port_open = check_port(OPENCLAW_PORT);

    let mut healthy = false;
    let mut health_body: Option<Value> = None;
    let pid = get_pid_by_port(OPENCLAW_PORT);

    if port_open {
        if let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            let url = format!("http://localhost:{}/healthz", OPENCLAW_PORT);
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(body) = resp.json::<Value>().await {
                        healthy = body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        health_body = Some(body);
                    }
                }
            }
        }
    }

    json_response(json!({
        "success": true,
        "data": {
            "installed": install.package_exists && install.node_exists,
            "version": install.version,
            "portOpen": port_open,
            "healthy": healthy,
            "health": health_body,
            "pid": pid,
            "port": OPENCLAW_PORT,
            "configExists": install.config_exists,
            "accessUrl": format!("http://localhost:{}/?token={}", OPENCLAW_PORT, OPENCLAW_TOKEN),
        }
    }))
}

// ── Route: POST /api/start ────────────────────────────────────────────────────
async fn api_start() -> Response {
    if check_port(OPENCLAW_PORT) {
        return json_response(json!({
            "success": true,
            "message": "服务已在运行中",
            "alreadyRunning": true
        }));
    }

    let install = detect_installation();
    if !install.package_exists || !install.node_exists {
        return json_response(json!({
            "success": false,
            "message": "OpenClaw 未安装，无法启动"
        }));
    }

    let log_dir = Path::new(OPENCLAW_LOG_FILE).parent().unwrap();
    let _ = std::fs::create_dir_all(log_dir);

    let log_out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(OPENCLAW_LOG_FILE);
    let log_err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(OPENCLAW_LOG_ERR_FILE);

    let mut cmd = Command::new(OPENCLAW_NODE_PATH);
    cmd.args([
        OPENCLAW_INDEX_JS,
        "gateway",
        "--port",
        &OPENCLAW_PORT.to_string(),
    ])
    .env("OPENCLAW_STATE_DIR", OPENCLAW_STATE_DIR)
    .env("OPENCLAW_GATEWAY_PORT", OPENCLAW_PORT.to_string())
    .env("OPENROUTER_API_KEY", OPENROUTER_API_KEY)
    .env("CLAWHUB_URL", CLAWHUB_URL)
    .env("HOME", r"C:\Users\85949");

    use std::process::Stdio;
    cmd.stdin(Stdio::null());

    if let Ok(f) = log_out {
        cmd.stdout(f);
    } else {
        cmd.stdout(Stdio::null());
    }
    if let Ok(f) = log_err {
        cmd.stderr(f);
    } else {
        cmd.stderr(Stdio::null());
    }

    configure_hidden_process(&mut cmd);

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return json_response(json!({
                "success": false,
                "message": format!("启动进程失败: {}", e)
            }));
        }
    };

    let start_pid = child.id();

    for attempt in 0..10 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if check_port(OPENCLAW_PORT) {
            return json_response(json!({
                "success": true,
                "message": "服务启动成功",
                "pid": start_pid
            }));
        }
        if attempt == 9 {
            return json_response(json!({
                "success": false,
                "message": "服务启动超时（等待了 20s），请检查日志",
                "pid": start_pid
            }));
        }
    }

    json_response(json!({ "success": false, "message": "未知错误" }))
}

// ── Route: POST /api/stop ─────────────────────────────────────────────────────
async fn api_stop() -> Response {
    if !check_port(OPENCLAW_PORT) {
        return json_response(json!({
            "success": true,
            "message": "服务未在运行",
            "alreadyStopped": true
        }));
    }

    let pid = match get_pid_by_port(OPENCLAW_PORT) {
        Some(p) => p,
        None => {
            return json_response(json!({
                "success": false,
                "message": "未能找到占用端口的进程 PID"
            }));
        }
    };

    let mut cmd = Command::new("taskkill");
    cmd.args(["/PID", &pid, "/F"]);
    configure_hidden_process(&mut cmd);
    let result = cmd.output();

    match result {
        Err(e) => json_response(json!({
            "success": false,
            "message": format!("终止进程失败: {}", e)
        })),
        Ok(_) => {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            if check_port(OPENCLAW_PORT) {
                json_response(json!({
                    "success": false,
                    "message": "已发送终止信号，但端口仍被占用，请稍后重试"
                }))
            } else {
                json_response(json!({
                    "success": true,
                    "message": format!("服务已停止 (PID: {})", pid),
                    "pid": pid
                }))
            }
        }
    }
}

// ── Route: GET /api/models ────────────────────────────────────────────────────
async fn api_models() -> Response {
    let port_open = check_port(OPENCLAW_PORT);

    let models_data: Option<Value> = std::fs::read_to_string(OPENCLAW_MODELS_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let auth_data: Option<Value> = std::fs::read_to_string(OPENCLAW_AUTH_FILE)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .map(|mut raw| {
            if let Some(profiles) = raw.get_mut("profiles").and_then(|p| p.as_object_mut()) {
                for (_k, v) in profiles.iter_mut() {
                    if let Some(key) = v
                        .get_mut("credentials")
                        .and_then(|c| c.get_mut("apiKey"))
                        .and_then(|k| k.as_str())
                    {
                        let masked = format!("{}****", &key[..key.len().min(12)]);
                        *v.get_mut("credentials").unwrap().get_mut("apiKey").unwrap() =
                            Value::String(masked);
                    }
                }
            }
            raw
        });

    match models_data {
        None => json_response(json!({
            "success": false,
            "serviceRunning": port_open,
            "message": "未找到模型配置文件"
        })),
        Some(data) => json_response(json!({
            "success": true,
            "serviceRunning": port_open,
            "source": "config-file",
            "modelsFile": OPENCLAW_MODELS_FILE,
            "data": data,
            "auth": auth_data
        })),
    }
}

// ── Route: GET /api/config ────────────────────────────────────────────────────
async fn api_config() -> Response {
    match std::fs::read_to_string(OPENCLAW_CONFIG_FILE)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
    {
        None => json_response(json!({ "success": false, "message": "配置文件读取失败" })),
        Some(mut cfg) => {
            let masked_token = cfg
                .get("gateway")
                .and_then(|g| g.get("auth"))
                .and_then(|a| a.get("token"))
                .and_then(|t| t.as_str())
                .map(|t| format!("{}****", &t[..t.len().min(8)]));
            if let Some(masked) = masked_token {
                if let Some(auth) = cfg
                    .get_mut("gateway")
                    .and_then(|g| g.get_mut("auth"))
                {
                    auth["token"] = Value::String(masked);
                }
            }
            json_response(json!({ "success": true, "data": cfg }))
        }
    }
}

// ── Route: GET /api/logs ──────────────────────────────────────────────────────
#[derive(Deserialize)]
struct LogsQuery {
    lines: Option<usize>,
}

async fn api_logs(Query(params): Query<LogsQuery>) -> Response {
    let lines_count = params.lines.unwrap_or(50);

    if !Path::new(OPENCLAW_LOG_FILE).exists() {
        return json_response(json!({
            "success": true,
            "lines": [],
            "message": "日志文件不存在"
        }));
    }

    match std::fs::read_to_string(OPENCLAW_LOG_FILE) {
        Err(e) => json_response(json!({ "success": false, "message": e.to_string() })),
        Ok(content) => {
            let all_lines: Vec<&str> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect();
            let total = all_lines.len();
            let last: Vec<&str> = all_lines
                .iter()
                .rev()
                .take(lines_count)
                .rev()
                .copied()
                .collect();
            json_response(json!({
                "success": true,
                "lines": last,
                "total": total
            }))
        }
    }
}

async fn api_activate(
    axum::extract::State(proxy): axum::extract::State<EventLoopProxy<AppUserEvent>>,
) -> Response {
    match proxy.send_event(AppUserEvent::FocusMainWindow) {
        Ok(_) => json_response(json!({
            "success": true,
            "app": "mclaw",
            "message": "已通知现有窗口前台显示"
        })),
        Err(err) => json_response(json!({
            "success": false,
            "app": "mclaw",
            "message": format!("通知现有窗口失败: {}", err)
        })),
    }
}

fn build_router(proxy: EventLoopProxy<AppUserEvent>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/status", get(api_status))
        .route("/api/detect", get(api_detect))
        .route("/api/start", post(api_start))
        .route("/api/stop", post(api_stop))
        .route("/api/models", get(api_models))
        .route("/api/config", get(api_config))
        .route("/api/logs", get(api_logs))
        .route("/api/activate", post(api_activate))
        .with_state(proxy)
}

enum MClawServerStartup {
    Ready,
    Failed { title: String, message: String },
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn build_startup_error_html(title: &str, message: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{title}</title>
  <style>
    :root {{
      color-scheme: dark;
      --bg: #111827;
      --panel: #1f2937;
      --text: #f9fafb;
      --muted: #cbd5e1;
      --accent: #f59e0b;
      --danger: #ef4444;
      font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
    }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      background: linear-gradient(135deg, #0f172a, #111827 55%, #1e293b);
      color: var(--text);
      padding: 24px;
      box-sizing: border-box;
    }}
    .card {{
      width: min(760px, 100%);
      background: rgba(31, 41, 55, 0.92);
      border: 1px solid rgba(245, 158, 11, 0.25);
      border-radius: 20px;
      padding: 32px;
      box-shadow: 0 24px 60px rgba(0, 0, 0, 0.35);
    }}
    .badge {{
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 12px;
      border-radius: 999px;
      background: rgba(239, 68, 68, 0.16);
      color: #fecaca;
      font-size: 13px;
      margin-bottom: 16px;
    }}
    h1 {{ margin: 0 0 12px; font-size: 28px; }}
    p {{ margin: 0 0 12px; color: var(--muted); line-height: 1.7; }}
    .detail {{
      margin-top: 20px;
      padding: 16px;
      border-radius: 14px;
      background: rgba(15, 23, 42, 0.85);
      border: 1px solid rgba(148, 163, 184, 0.2);
      color: #e2e8f0;
      white-space: pre-wrap;
      line-height: 1.7;
      font-family: "Cascadia Mono", "Consolas", monospace;
      font-size: 13px;
    }}
    ul {{ margin: 18px 0 0; padding-left: 20px; color: var(--muted); line-height: 1.8; }}
    strong {{ color: var(--text); }}
    .tip {{ color: #fde68a; }}
  </style>
</head>
<body>
  <main class="card">
    <div class="badge">⚠️ 已阻止接入旧服务</div>
    <h1>{title}</h1>
    <p>MClaw 为了避免误连到旧版或其它程序占用的本地端口，已停止加载 <strong>127.0.0.1:{port}</strong> 上现有的页面。</p>
    <p class="tip">这通常表示旧版 Node / bat 版 MClaw 还在后台运行，或者有别的程序占用了相同端口。</p>
    <div class="detail">{message}</div>
    <ul>
      <li>先关闭占用 <strong>{port}</strong> 端口的程序，再重新启动 MClaw。</li>
      <li>如果你之前装过旧版 MClaw，优先结束它的后台进程后再试。</li>
      <li>处理完成后，关闭本窗口并重新打开应用。</li>
    </ul>
  </main>
</body>
</html>"#,
        title = escape_html(title),
        message = escape_html(message),
        port = MCLAW_PORT,
    )
}

fn spawn_http_server(proxy: EventLoopProxy<AppUserEvent>) -> Receiver<MClawServerStartup> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                let _ = tx.send(MClawServerStartup::Failed {
                    title: "MClaw 启动失败".to_string(),
                    message: format!("创建内部运行时失败：{}", err),
                });
                return;
            }
        };

        runtime.block_on(async move {
            let app = build_router(proxy);
            let addr = format!("127.0.0.1:{}", MCLAW_PORT);

            match TcpListener::bind(&addr).await {
                Ok(listener) => {
                    let _ = tx.send(MClawServerStartup::Ready);
                    if let Err(err) = axum::serve(listener, app).await {
                        eprintln!("MClaw HTTP server error: {}", err);
                    }
                }
                Err(err) => {
                    let pid = get_pid_by_port(MCLAW_PORT);
                    let pid_hint = pid
                        .as_deref()
                        .map(|pid| format!("（PID: {}）", pid))
                        .unwrap_or_default();
                    let _ = tx.send(MClawServerStartup::Failed {
                        title: "MClaw 端口冲突".to_string(),
                        message: format!(
                            "本地端口 {} 已被其他程序占用{}。\n为避免误接入旧服务，MClaw 本次不会复用该端口上的页面。\n请先关闭占用端口的程序后再重新打开。\n\n原始错误：{}",
                            MCLAW_PORT, pid_hint, err
                        ),
                    });
                }
            }
        });
    });

    rx
}

fn wait_for_mclaw_server(
    startup_rx: Receiver<MClawServerStartup>,
    timeout: Duration,
) -> MClawServerStartup {
    match startup_rx.recv_timeout(timeout) {
        Ok(status) => status,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => MClawServerStartup::Failed {
            title: "MClaw 启动超时".to_string(),
            message: format!(
                "等待内置服务在 {} 秒内完成启动时超时。为避免误连到旧服务，本次未继续加载本地页面。",
                timeout.as_secs()
            ),
        },
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => MClawServerStartup::Failed {
            title: "MClaw 启动失败".to_string(),
            message: "内部服务线程提前退出，未能完成启动。".to_string(),
        },
    }
}



#[derive(Debug, Deserialize)]
struct IpcCommand {
    kind: String,
    url: Option<String>,
}

#[derive(Debug, Clone)]
enum AppUserEvent {
    OpenConsole { url: String },
    FocusMainWindow,
}

struct DesktopApp {
    main_window_id: Option<WindowId>,
    windows: HashMap<WindowId, Window>,
    webviews: HashMap<WindowId, WebView>,
    proxy: EventLoopProxy<AppUserEvent>,
}

impl DesktopApp {
    fn new(proxy: EventLoopProxy<AppUserEvent>) -> Self {
        Self {
            main_window_id: None,
            windows: HashMap::new(),
            webviews: HashMap::new(),
            proxy,
        }
    }

    fn build_main_webview(&self, window: &Window, app_url: &str) -> Result<WebView, wry::Error> {
        let proxy = self.proxy.clone();
        WebViewBuilder::new()
            .with_url(app_url)
            .with_ipc_handler(move |request| {
                if let Ok(command) = serde_json::from_str::<IpcCommand>(request.body()) {
                    if command.kind == "open_console" {
                        if let Some(url) = command.url {
                            let _ = proxy.send_event(AppUserEvent::OpenConsole { url });
                        }
                    }
                }
            })
            .build(window)
    }

    fn create_console_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        url: &str,
    ) -> Result<(), String> {
        let window_attributes = Window::default_attributes()
            .with_title("OpenClaw 控制台")
            .with_inner_size(LogicalSize::new(1440.0, 920.0))
            .with_min_inner_size(LogicalSize::new(1024.0, 720.0));

        let window = event_loop
            .create_window(window_attributes)
            .map_err(|err| format!("创建控制台窗口失败：{}", err))?;
        let webview = WebViewBuilder::new()
            .with_url(url)
            .build(&window)
            .map_err(|err| format!("加载控制台页面失败：{}", err))?;

        let window_id = window.id();
        self.webviews.insert(window_id, webview);
        self.windows.insert(window_id, window);
        Ok(())
    }

    fn focus_main_window(&self) {
        if let Some(window_id) = self.main_window_id {
            if let Some(window) = self.windows.get(&window_id) {
                window.set_minimized(false);
                window.set_visible(true);
                window.focus_window();
                window.request_user_attention(Some(UserAttentionType::Informational));
            }
        }
    }
}

impl ApplicationHandler<AppUserEvent> for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main_window_id.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("MClaw")
            .with_inner_size(LogicalSize::new(1280.0, 860.0))
            .with_min_inner_size(LogicalSize::new(960.0, 680.0));

        let window = event_loop
            .create_window(window_attributes)
            .expect("failed to create window");

        let app_url = format!("http://127.0.0.1:{}", MCLAW_PORT);
        let startup = wait_for_mclaw_server(spawn_http_server(self.proxy.clone()), Duration::from_secs(8));

        let webview = match startup {
            MClawServerStartup::Ready => self
                .build_main_webview(&window, &app_url)
                .expect("failed to build webview"),
            MClawServerStartup::Failed { title, message } => {
                window.set_title(&title);
                let html = build_startup_error_html(&title, &message);
                WebViewBuilder::new()
                    .with_html(html)
                    .build(&window)
                    .expect("failed to build startup error view")
            }
        };

        let window_id = window.id();
        self.main_window_id = Some(window_id);
        self.webviews.insert(window_id, webview);
        self.windows.insert(window_id, window);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppUserEvent) {
        match event {
            AppUserEvent::OpenConsole { url } => {
                if let Err(err) = self.create_console_window(event_loop, &url) {
                    eprintln!("{}", err);
                }
            }
            AppUserEvent::FocusMainWindow => self.focus_main_window(),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::CloseRequested = event {
            self.webviews.remove(&window_id);
            self.windows.remove(&window_id);

            if Some(window_id) == self.main_window_id {
                event_loop.exit();
            }
        }
    }
}

fn request_existing_instance_activation() -> bool {
    let addr = match format!("127.0.0.1:{}", MCLAW_PORT).parse() {
        Ok(addr) => addr,
        Err(_) => return false,
    };

    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(900)) {
        Ok(stream) => stream,
        Err(_) => return false,
    };

    let _ = stream.set_read_timeout(Some(Duration::from_millis(1200)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1200)));

    let request = concat!(
        "POST /api/activate HTTP/1.1\r\n",
        "Host: 127.0.0.1\r\n",
        "Connection: close\r\n",
        "Content-Length: 0\r\n\r\n"
    );

    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }

    let http_ok = response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200");
    http_ok && response.contains("\"success\":true") && response.contains("\"app\":\"mclaw\"")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if request_existing_instance_activation() {
        return Ok(());
    }

    let event_loop = EventLoop::<AppUserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut app = DesktopApp::new(proxy);
    event_loop.run_app(&mut app)?;
    Ok(())
}

