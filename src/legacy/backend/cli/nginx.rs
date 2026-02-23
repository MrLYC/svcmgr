use crate::atoms::SupervisorManager;
use crate::atoms::proxy::ProxyAtom;
use crate::cli::NginxAction;
use crate::error::Result;
use crate::features::NginxManager;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

pub async fn handle_nginx_command(action: NginxAction) -> Result<()> {
    let home = env::var("HOME").expect("HOME environment variable not set");
    let config_dir = PathBuf::from(&home).join(".config/svcmgr/nginx");
    let data_dir = PathBuf::from(&home).join(".local/share/svcmgr/nginx");

    let supervisor = SupervisorManager::default_config()?;
    let manager = NginxManager::new(config_dir, data_dir, supervisor);

    match action {
        NginxAction::Start => start_nginx(&manager).await,
        NginxAction::Stop => stop_nginx(&manager).await,
        NginxAction::Reload => reload_nginx(&manager).await,
        NginxAction::Status => show_status(&manager).await,
        NginxAction::Test => test_config(&manager).await,
        NginxAction::AddProxy {
            location,
            upstream,
            websocket,
        } => add_http_proxy(&manager, &location, &upstream, websocket),
        NginxAction::AddStatic {
            location,
            root,
            autoindex,
            index,
        } => add_static_site(&manager, &location, &root, autoindex, index.as_deref()),
        NginxAction::AddTcp { port, upstream } => add_tcp_proxy(&manager, port, &upstream),
        NginxAction::AddTty { name, port } => add_tty_route(&manager, &name, port),
        NginxAction::List { type_filter } => list_configs(&manager, type_filter.as_deref()),
        NginxAction::RemoveProxy { location } => remove_http_proxy(&manager, &location),
        NginxAction::RemoveStatic { location } => remove_static_site(&manager, &location),
        NginxAction::RemoveTcp { port } => remove_tcp_proxy(&manager, port),
        NginxAction::RemoveTty { name } => remove_tty_route(&manager, &name),
        NginxAction::Logs { error, lines: _ } => show_logs(&manager, error),
    }
}

async fn start_nginx(manager: &NginxManager) -> Result<()> {
    manager.start().await?;
    println!("Nginx started successfully");
    Ok(())
}

async fn stop_nginx(manager: &NginxManager) -> Result<()> {
    manager.stop().await?;
    println!("Nginx stopped successfully");
    Ok(())
}

async fn reload_nginx(manager: &NginxManager) -> Result<()> {
    if !manager.test_config().await? {
        println!("Configuration test failed! Not reloading.");
        return Ok(());
    }
    manager.reload().await?;
    println!("Nginx configuration reloaded successfully");
    Ok(())
}

async fn show_status(manager: &NginxManager) -> Result<()> {
    let status = manager.status().await?;
    println!("Nginx Status:");
    println!("  Running: {}", status.running);
    if let Some(pid) = status.pid {
        println!("  PID: {}", pid);
    }
    println!("  Worker Processes: {}", status.worker_processes);
    println!("  Connections: {}", status.connections);
    Ok(())
}

async fn test_config(manager: &NginxManager) -> Result<()> {
    let valid = manager.test_config().await?;
    if valid {
        println!("✓ Configuration syntax is valid");
    } else {
        println!("✗ Configuration syntax is invalid");
    }
    Ok(())
}

fn add_http_proxy(
    manager: &NginxManager,
    location: &str,
    upstream: &str,
    websocket: bool,
) -> Result<()> {
    use crate::atoms::proxy::HttpProxyConfig;
    let config = HttpProxyConfig {
        location: location.to_string(),
        upstream: upstream.to_string(),
        websocket,
        proxy_headers: HashMap::new(),
    };
    manager.add_http_proxy(&config)?;
    println!("Added HTTP proxy: {} -> {}", location, upstream);
    if websocket {
        println!("  WebSocket: enabled");
    }
    Ok(())
}

fn add_static_site(
    manager: &NginxManager,
    location: &str,
    root: &str,
    autoindex: bool,
    index: Option<&str>,
) -> Result<()> {
    use crate::atoms::proxy::StaticSiteConfig;
    let index_files = if let Some(idx) = index {
        idx.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        vec!["index.html".to_string(), "index.htm".to_string()]
    };

    let config = StaticSiteConfig {
        location: location.to_string(),
        root: PathBuf::from(root),
        autoindex,
        index: index_files,
    };
    manager.add_static_site(&config)?;
    println!("Added static site: {} -> {}", location, root);
    if autoindex {
        println!("  Directory listing: enabled");
    }
    Ok(())
}

fn add_tcp_proxy(manager: &NginxManager, port: u16, upstream: &str) -> Result<()> {
    use crate::atoms::proxy::TcpProxyConfig;
    let config = TcpProxyConfig {
        listen_port: port,
        upstream: upstream.to_string(),
    };
    manager.add_tcp_proxy(&config)?;
    println!("Added TCP proxy: port {} -> {}", port, upstream);
    Ok(())
}

fn add_tty_route(manager: &NginxManager, name: &str, port: u16) -> Result<()> {
    manager.add_tty_route(name, port)?;
    println!("Added TTY route: /tty/{} -> localhost:{}", name, port);
    Ok(())
}

fn list_configs(manager: &NginxManager, type_filter: Option<&str>) -> Result<()> {
    match type_filter {
        Some("http") | None => {
            let proxies = manager.list_http_proxies()?;
            if !proxies.is_empty() {
                println!("HTTP Proxies:");
                println!("{:<30} {:<40} WebSocket", "Location", "Upstream");
                println!("{}", "-".repeat(80));
                for proxy in proxies {
                    let ws = if proxy.websocket { "Yes" } else { "No" };
                    println!("{:<30} {:<40} {}", proxy.location, proxy.upstream, ws);
                }
                println!();
            }
        }
        _ => {}
    }

    match type_filter {
        Some("static") | None => {
            let sites = manager.list_static_sites()?;
            if !sites.is_empty() {
                println!("Static Sites:");
                println!("{:<30} {:<40} AutoIndex", "Location", "Root");
                println!("{}", "-".repeat(80));
                for site in sites {
                    let ai = if site.autoindex { "Yes" } else { "No" };
                    println!("{:<30} {:<40} {}", site.location, site.root.display(), ai);
                }
                println!();
            }
        }
        _ => {}
    }

    match type_filter {
        Some("tcp") | None => {
            let tcp_proxies = manager.list_tcp_proxies()?;
            if !tcp_proxies.is_empty() {
                println!("TCP Proxies:");
                println!("{:<10} Upstream", "Port");
                println!("{}", "-".repeat(50));
                for tcp in tcp_proxies {
                    println!("{:<10} {}", tcp.listen_port, tcp.upstream);
                }
                println!();
            }
        }
        _ => {}
    }

    match type_filter {
        Some("tty") | None => {
            let routes = manager.list_tty_routes()?;
            if !routes.is_empty() {
                println!("TTY Routes:");
                println!("{:<30} Port", "Name");
                println!("{}", "-".repeat(40));
                for route in routes {
                    println!("{:<30} {}", route.name, route.port);
                }
                println!();
            }
        }
        _ => {}
    }

    Ok(())
}

fn remove_http_proxy(manager: &NginxManager, location: &str) -> Result<()> {
    manager.remove_http_proxy(location)?;
    println!("Removed HTTP proxy: {}", location);
    Ok(())
}

fn remove_static_site(manager: &NginxManager, location: &str) -> Result<()> {
    manager.remove_static_site(location)?;
    println!("Removed static site: {}", location);
    Ok(())
}

fn remove_tcp_proxy(manager: &NginxManager, port: u16) -> Result<()> {
    manager.remove_tcp_proxy(port)?;
    println!("Removed TCP proxy on port: {}", port);
    Ok(())
}

fn remove_tty_route(manager: &NginxManager, name: &str) -> Result<()> {
    manager.remove_tty_route(name)?;
    println!("Removed TTY route: {}", name);
    Ok(())
}

fn show_logs(_manager: &NginxManager, error: bool) -> Result<()> {
    let home = env::var("HOME").expect("HOME not set");
    let log_file = if error {
        PathBuf::from(&home).join(".local/share/svcmgr/nginx/logs/error.log")
    } else {
        PathBuf::from(&home).join(".local/share/svcmgr/nginx/logs/access.log")
    };

    if !log_file.exists() {
        println!("Log file not found: {}", log_file.display());
        return Ok(());
    }

    let content = std::fs::read_to_string(&log_file)?;
    println!("{}", content);
    Ok(())
}
