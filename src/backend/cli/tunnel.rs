use crate::atoms::systemd::SystemdManager;
use crate::atoms::tunnel::{IngressRule, TunnelAtom};
use crate::cli::TunnelAction;
use crate::error::Result;
use crate::features::TunnelManager;

pub async fn handle_tunnel_command(action: TunnelAction) -> Result<()> {
    let manager = TunnelManager::default_config(SystemdManager::default_config()?)?;

    match action {
        TunnelAction::Login => login(&manager).await,
        TunnelAction::Create { name } => create_tunnel(&manager, &name).await,
        TunnelAction::List => list_tunnels(&manager).await,
        TunnelAction::Delete { tunnel_id } => delete_tunnel(&manager, &tunnel_id).await,
        TunnelAction::Info { tunnel_id } => show_tunnel_info(&manager, &tunnel_id).await,
        TunnelAction::AddIngress {
            tunnel_id,
            hostname,
            service,
            path,
        } => add_ingress_rule(&manager, &tunnel_id, &hostname, &service, path.as_deref()),
        TunnelAction::RemoveIngress {
            tunnel_id,
            hostname,
        } => remove_ingress_rule(&manager, &tunnel_id, &hostname),
        TunnelAction::RouteDns {
            tunnel_id,
            hostname,
        } => route_dns(&manager, &tunnel_id, &hostname).await,
        TunnelAction::Start { tunnel_id } => start_tunnel(&manager, &tunnel_id).await,
        TunnelAction::Stop { tunnel_id } => stop_tunnel(&manager, &tunnel_id).await,
        TunnelAction::Status { tunnel_id } => show_tunnel_status(&manager, &tunnel_id).await,
    }
}

async fn login(manager: &TunnelManager) -> Result<()> {
    if manager.is_authenticated().await? {
        println!("Already authenticated with Cloudflare");
        return Ok(());
    }

    println!("Starting Cloudflare login process...");
    println!("A browser window will open for authentication.");
    manager.login().await?;
    println!("✓ Successfully authenticated with Cloudflare");
    Ok(())
}

async fn create_tunnel(manager: &TunnelManager, name: &str) -> Result<()> {
    println!("Creating tunnel '{}'...", name);
    let tunnel_info = manager.create(name).await?;
    println!("✓ Tunnel created successfully");
    println!("  ID: {}", tunnel_info.id);
    println!("  Name: {}", tunnel_info.name);
    println!("  Created: {}", tunnel_info.created_at);
    Ok(())
}

async fn list_tunnels(manager: &TunnelManager) -> Result<()> {
    let tunnels = manager.list().await?;

    if tunnels.is_empty() {
        println!("No tunnels found");
        return Ok(());
    }

    println!("{:<36} {:<20} {:<25}", "ID", "Name", "Created At");
    println!("{}", "=".repeat(81));
    for tunnel in tunnels {
        println!(
            "{:<36} {:<20} {:<25}",
            tunnel.id, tunnel.name, tunnel.created_at
        );
    }

    Ok(())
}

async fn delete_tunnel(manager: &TunnelManager, tunnel_id: &str) -> Result<()> {
    println!("Deleting tunnel '{}'...", tunnel_id);
    manager.delete(tunnel_id).await?;
    println!("✓ Tunnel deleted successfully");
    Ok(())
}

async fn show_tunnel_info(manager: &TunnelManager, tunnel_id: &str) -> Result<()> {
    let tunnel_info = manager.get(tunnel_id).await?;
    println!("Tunnel Information:");
    println!("  ID: {}", tunnel_info.id);
    println!("  Name: {}", tunnel_info.name);
    println!("  Created: {}", tunnel_info.created_at);

    let ingress = manager.get_ingress(tunnel_id)?;
    if ingress.is_empty() {
        println!("\nNo ingress rules configured");
    } else {
        println!("\nIngress Rules:");
        for (idx, rule) in ingress.iter().enumerate() {
            let hostname = rule.hostname.as_deref().unwrap_or("*");
            println!("  {}. {} -> {}", idx + 1, hostname, rule.service);
            if let Some(path) = &rule.path {
                println!("     Path: {}", path);
            }
        }
    }

    Ok(())
}

fn add_ingress_rule(
    manager: &TunnelManager,
    tunnel_id: &str,
    hostname: &str,
    service: &str,
    path: Option<&str>,
) -> Result<()> {
    println!("Adding ingress rule for '{}'...", hostname);

    let rule = IngressRule {
        hostname: Some(hostname.to_string()),
        path: path.map(|s| s.to_string()),
        service: service.to_string(),
    };

    manager.add_ingress_rule(tunnel_id, &rule)?;
    println!("✓ Ingress rule added successfully");
    println!("  {} -> {}", hostname, service);
    if let Some(p) = path {
        println!("  Path: {}", p);
    }
    Ok(())
}

fn remove_ingress_rule(manager: &TunnelManager, tunnel_id: &str, hostname: &str) -> Result<()> {
    println!("Removing ingress rule for '{}'...", hostname);
    manager.remove_ingress_rule(tunnel_id, hostname)?;
    println!("✓ Ingress rule removed successfully");
    Ok(())
}

async fn route_dns(manager: &TunnelManager, tunnel_id: &str, hostname: &str) -> Result<()> {
    println!("Routing DNS for '{}' to tunnel...", hostname);
    manager.route_dns(tunnel_id, hostname).await?;
    println!("✓ DNS route created successfully");
    println!("  {} -> {}", hostname, tunnel_id);
    Ok(())
}

async fn start_tunnel(manager: &TunnelManager, tunnel_id: &str) -> Result<()> {
    println!("Starting tunnel '{}'...", tunnel_id);
    manager.start(tunnel_id).await?;
    println!("✓ Tunnel started successfully");
    Ok(())
}

async fn stop_tunnel(manager: &TunnelManager, tunnel_id: &str) -> Result<()> {
    println!("Stopping tunnel '{}'...", tunnel_id);
    manager.stop(tunnel_id).await?;
    println!("✓ Tunnel stopped successfully");
    Ok(())
}

async fn show_tunnel_status(manager: &TunnelManager, tunnel_id: &str) -> Result<()> {
    let status = manager.status(tunnel_id).await?;
    println!("Tunnel Status:");
    println!("  Running: {}", if status.running { "Yes" } else { "No" });
    println!("  Connections: {}", status.connections);

    if let Some(latency) = status.latency_ms {
        println!("  Latency: {}ms", latency);
    }

    if !status.errors.is_empty() {
        println!("\nErrors:");
        for error in status.errors {
            println!("  - {}", error);
        }
    }

    Ok(())
}
