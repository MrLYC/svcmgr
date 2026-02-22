use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "svcmgr")]
#[command(version = "0.1.0")]
#[command(about = "Linux service management tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Initialize base environment (nginx, mise, cloudflare, etc.)")]
    Setup {
        #[arg(short, long, help = "Force re-initialization even if already setup")]
        force: bool,
    },

    #[command(about = "Start svcmgr service")]
    Run,

    #[command(about = "Uninstall base environment")]
    Teardown {
        #[arg(short, long, help = "Force teardown without confirmation")]
        force: bool,
    },

    #[command(about = "Manage systemd services")]
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },

    #[command(about = "Manage crontab tasks")]
    Cron {
        #[command(subcommand)]
        action: CronAction,
    },

    #[command(about = "Manage mise dependencies and tasks")]
    Mise {
        #[command(subcommand)]
        action: MiseAction,
    },

    #[command(about = "Manage nginx proxy configurations")]
    Nginx {
        #[command(subcommand)]
        action: NginxAction,
    },

    #[command(about = "Manage Cloudflare Tunnel connections")]
    Tunnel {
        #[command(subcommand)]
        action: TunnelAction,
    },
}

#[derive(Subcommand)]
pub enum ServiceAction {
    #[command(about = "List all managed services")]
    List,

    #[command(about = "Create a new service from template")]
    Add {
        #[arg(help = "Service name (must end with .service)")]
        name: String,
        #[arg(short, long, help = "Template name")]
        template: String,
        #[arg(short, long, help = "Template variables (key=value)", value_parser = parse_key_val)]
        var: Vec<(String, String)>,
    },

    #[command(about = "Show service status and details")]
    Status {
        #[arg(help = "Service name")]
        name: String,
    },

    #[command(about = "Start a service")]
    Start {
        #[arg(help = "Service name")]
        name: String,
    },

    #[command(about = "Stop a service")]
    Stop {
        #[arg(help = "Service name")]
        name: String,
    },

    #[command(about = "Restart a service")]
    Restart {
        #[arg(help = "Service name")]
        name: String,
    },

    #[command(about = "Enable service (auto-start on boot)")]
    Enable {
        #[arg(help = "Service name")]
        name: String,
    },

    #[command(about = "Disable service (remove auto-start)")]
    Disable {
        #[arg(help = "Service name")]
        name: String,
    },

    #[command(about = "View service logs")]
    Logs {
        #[arg(help = "Service name")]
        name: String,
        #[arg(short, long, default_value = "100", help = "Number of lines to show")]
        lines: usize,
        #[arg(short, long, help = "Follow log output")]
        follow: bool,
    },

    #[command(about = "Delete a service")]
    Remove {
        #[arg(help = "Service name")]
        name: String,
        #[arg(short, long, help = "Force removal without confirmation")]
        force: bool,
    },

    #[command(about = "Run a transient service (temporary task)")]
    Run {
        #[arg(help = "Command to execute")]
        command: Vec<String>,
        #[arg(short, long, help = "Working directory")]
        workdir: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CronAction {
    #[command(about = "List all managed cron tasks")]
    List,

    #[command(about = "Create a new cron task")]
    Add {
        #[arg(help = "Task ID/name")]
        id: String,
        #[arg(short, long, help = "Cron expression (e.g., '0 9 * * *' or '@daily')")]
        expression: String,
        #[arg(short, long, help = "Command to execute")]
        command: String,
        #[arg(short, long, default_value = "", help = "Task description")]
        description: String,
        #[arg(short = 't', long, help = "Template name")]
        template: Option<String>,
        #[arg(short = 'v', long, help = "Template variables (key=value)", value_parser = parse_key_val)]
        var: Vec<(String, String)>,
    },

    #[command(about = "Show task details")]
    Status {
        #[arg(help = "Task ID")]
        id: String,
    },

    #[command(about = "Update an existing cron task")]
    Update {
        #[arg(help = "Task ID")]
        id: String,
        #[arg(short, long, help = "New cron expression")]
        expression: Option<String>,
        #[arg(short, long, help = "New command")]
        command: Option<String>,
        #[arg(short, long, help = "New description")]
        description: Option<String>,
    },

    #[command(about = "Delete a cron task")]
    Remove {
        #[arg(help = "Task ID")]
        id: String,
        #[arg(short, long, help = "Force removal without confirmation")]
        force: bool,
    },

    #[command(about = "Show next N execution times for a task")]
    Next {
        #[arg(help = "Task ID")]
        id: String,
        #[arg(
            short,
            long,
            default_value = "5",
            help = "Number of executions to show"
        )]
        count: usize,
    },

    #[command(about = "Validate a cron expression")]
    Validate {
        #[arg(help = "Cron expression to validate")]
        expression: String,
    },

    #[command(about = "Set environment variable for cron tasks")]
    SetEnv {
        #[arg(help = "Variable name")]
        key: String,
        #[arg(help = "Variable value")]
        value: String,
    },

    #[command(about = "Show all environment variables")]
    GetEnv,
}

#[derive(Subcommand)]
pub enum MiseAction {
    #[command(about = "Install a tool with specific version")]
    Install {
        #[arg(help = "Tool name (e.g., node, python, rust)")]
        tool: String,
        #[arg(help = "Version (e.g., 20.10.0, latest)")]
        version: String,
    },

    #[command(about = "List all installed tools")]
    ListTools,

    #[command(about = "Update a tool to a new version")]
    Update {
        #[arg(help = "Tool name")]
        tool: String,
        #[arg(help = "New version")]
        version: String,
    },

    #[command(about = "Remove an installed tool")]
    Remove {
        #[arg(help = "Tool name")]
        tool: String,
        #[arg(help = "Version to remove")]
        version: String,
        #[arg(short, long, help = "Force removal without confirmation")]
        force: bool,
    },

    #[command(about = "Add a new task")]
    AddTask {
        #[arg(help = "Task name")]
        name: String,
        #[arg(short, long, help = "Commands to run")]
        run: Vec<String>,
        #[arg(short = 'd', long, help = "Task description")]
        description: Option<String>,
        #[arg(long, help = "Task dependencies")]
        depends: Vec<String>,
        #[arg(short = 't', long, help = "Template name")]
        template: Option<String>,
        #[arg(short = 'v', long, help = "Template variables (key=value)", value_parser = parse_key_val)]
        var: Vec<(String, String)>,
    },

    #[command(about = "List all tasks")]
    ListTasks,

    #[command(about = "Execute a task")]
    RunTask {
        #[arg(help = "Task name")]
        name: String,
        #[arg(help = "Task arguments")]
        args: Vec<String>,
    },

    #[command(about = "Delete a task")]
    DeleteTask {
        #[arg(help = "Task name")]
        name: String,
        #[arg(short, long, help = "Force deletion without confirmation")]
        force: bool,
    },

    #[command(about = "Set environment variable")]
    SetEnv {
        #[arg(help = "Variable name")]
        key: String,
        #[arg(help = "Variable value")]
        value: String,
    },

    #[command(about = "Show all environment variables")]
    GetEnv,

    #[command(about = "Delete an environment variable")]
    DeleteEnv {
        #[arg(help = "Variable name")]
        key: String,
    },
}

#[derive(Subcommand)]
pub enum NginxAction {
    #[command(about = "Start nginx service")]
    Start,

    #[command(about = "Stop nginx service")]
    Stop,

    #[command(about = "Reload nginx configuration")]
    Reload,

    #[command(about = "Show nginx status")]
    Status,

    #[command(about = "Test nginx configuration")]
    Test,

    #[command(about = "Add HTTP proxy configuration")]
    AddProxy {
        #[arg(help = "Location path (e.g., /api)")]
        location: String,
        #[arg(help = "Upstream target (e.g., http://localhost:3000)")]
        upstream: String,
        #[arg(short, long, help = "Enable WebSocket support")]
        websocket: bool,
    },

    #[command(about = "Add static file site")]
    AddStatic {
        #[arg(help = "Location path (e.g., /static)")]
        location: String,
        #[arg(help = "Root directory path")]
        root: String,
        #[arg(short, long, help = "Enable directory listing")]
        autoindex: bool,
        #[arg(short, long, help = "Index files (comma-separated)")]
        index: Option<String>,
    },

    #[command(about = "Add TCP proxy configuration")]
    AddTcp {
        #[arg(help = "Listen port")]
        port: u16,
        #[arg(help = "Upstream target (e.g., postgres.internal:5432)")]
        upstream: String,
    },

    #[command(about = "Add TTY route")]
    AddTty {
        #[arg(help = "TTY name")]
        name: String,
        #[arg(help = "TTY service port")]
        port: u16,
    },

    #[command(about = "List all proxy configurations")]
    List {
        #[arg(short, long, help = "Type filter: http, tcp, static, tty")]
        type_filter: Option<String>,
    },

    #[command(about = "Remove HTTP proxy")]
    RemoveProxy {
        #[arg(help = "Location path")]
        location: String,
    },

    #[command(about = "Remove static site")]
    RemoveStatic {
        #[arg(help = "Location path")]
        location: String,
    },

    #[command(about = "Remove TCP proxy")]
    RemoveTcp {
        #[arg(help = "Listen port")]
        port: u16,
    },

    #[command(about = "Remove TTY route")]
    RemoveTty {
        #[arg(help = "TTY name")]
        name: String,
    },

    #[command(about = "View nginx logs")]
    Logs {
        #[arg(short, long, help = "Show error log instead of access log")]
        error: bool,
        #[arg(short, long, default_value = "50", help = "Number of lines to show")]
        lines: usize,
    },
}

#[derive(Subcommand)]
pub enum TunnelAction {
    #[command(about = "Authenticate with Cloudflare")]
    Login,

    #[command(about = "Create a new tunnel")]
    Create {
        #[arg(help = "Tunnel name")]
        name: String,
    },

    #[command(about = "List all tunnels")]
    List,

    #[command(about = "Delete a tunnel")]
    Delete {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
    },

    #[command(about = "Get tunnel information")]
    Info {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
    },

    #[command(about = "Add ingress rule")]
    AddIngress {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
        #[arg(help = "Hostname (e.g., app.example.com)")]
        hostname: String,
        #[arg(help = "Service URL (e.g., http://localhost:3000)")]
        service: String,
        #[arg(short, long, help = "Path prefix")]
        path: Option<String>,
    },

    #[command(about = "Remove ingress rule")]
    RemoveIngress {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
        #[arg(help = "Hostname to remove")]
        hostname: String,
    },

    #[command(about = "Route DNS to tunnel")]
    RouteDns {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
        #[arg(help = "Hostname to route (e.g., app.example.com)")]
        hostname: String,
    },

    #[command(about = "Start tunnel service")]
    Start {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
    },

    #[command(about = "Stop tunnel service")]
    Stop {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
    },

    #[command(about = "Show tunnel status")]
    Status {
        #[arg(help = "Tunnel ID or name")]
        tunnel_id: String,
    },
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

pub mod cron;
pub mod mise;
pub mod nginx;
pub mod run;
pub mod service;
pub mod setup;
pub mod teardown;
pub mod tunnel;
