use crate::atoms::{LogOptions, TemplateContext, TransientOptions};
use crate::cli::ServiceAction;
use crate::error::Result;
use crate::features::{ServiceConfig, SystemdServiceManager};

pub async fn handle_service_command(action: ServiceAction) -> Result<()> {
    let manager = SystemdServiceManager::default_config()?;

    match action {
        ServiceAction::List => list_services(&manager).await,
        ServiceAction::Add {
            name,
            template,
            var,
        } => add_service(&manager, name, template, var).await,
        ServiceAction::Status { name } => show_status(&manager, name).await,
        ServiceAction::Start { name } => start_service(&manager, name).await,
        ServiceAction::Stop { name } => stop_service(&manager, name).await,
        ServiceAction::Restart { name } => restart_service(&manager, name).await,
        ServiceAction::Enable { name } => enable_service(&manager, name).await,
        ServiceAction::Disable { name } => disable_service(&manager, name).await,
        ServiceAction::Logs {
            name,
            lines,
            follow,
        } => show_logs(&manager, name, lines, follow).await,
        ServiceAction::Remove { name, force } => remove_service(&manager, name, force).await,
        ServiceAction::Run { command, workdir } => run_transient(&manager, command, workdir).await,
    }
}

async fn list_services(manager: &SystemdServiceManager) -> Result<()> {
    let services = manager.list_services().await?;

    if services.is_empty() {
        println!("No managed services found.");
        return Ok(());
    }

    println!(
        "{:<30} {:<15} {:<10} DESCRIPTION",
        "NAME", "STATUS", "ENABLED"
    );
    println!("{}", "-".repeat(80));

    for service in services {
        let status = if service.active { "active" } else { "inactive" };
        let enabled = if service.enabled {
            "enabled"
        } else {
            "disabled"
        };
        println!(
            "{:<30} {:<15} {:<10} {}",
            service.name, status, enabled, service.description
        );
    }

    Ok(())
}

async fn add_service(
    manager: &SystemdServiceManager,
    name: String,
    template: String,
    variables: Vec<(String, String)>,
) -> Result<()> {
    let mut context = TemplateContext::new();
    for (key, value) in variables {
        context.insert(&key, value);
    }

    let config = ServiceConfig {
        name: name.clone(),
        template,
        variables: context,
    };

    manager.create_service(&config).await?;
    println!("Service {} created successfully.", name);
    println!("Use 'svcmgr service start {}' to start the service.", name);

    Ok(())
}

async fn show_status(manager: &SystemdServiceManager, name: String) -> Result<()> {
    let status = manager.get_status(&name).await?;

    println!("\n=== Service Status: {} ===", name);
    println!("Active: {:?}", status.active_state);
    println!("Running: {}", status.sub_state);
    println!(
        "Main PID: {}",
        status
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "N/A".to_string())
    );

    if let Some(memory) = status.memory {
        println!("Memory: {} bytes", memory);
    }

    if let Some(started) = status.started_at {
        println!("Started at: {}", started.to_rfc3339());
    }

    if !status.recent_logs.is_empty() {
        println!("\nRecent logs:");
        for log in &status.recent_logs {
            println!("  {}", log);
        }
    }

    Ok(())
}

async fn start_service(manager: &SystemdServiceManager, name: String) -> Result<()> {
    manager.start_service(&name).await?;
    println!("Service {} started.", name);
    Ok(())
}

async fn stop_service(manager: &SystemdServiceManager, name: String) -> Result<()> {
    manager.stop_service(&name).await?;
    println!("Service {} stopped.", name);
    Ok(())
}

async fn restart_service(manager: &SystemdServiceManager, name: String) -> Result<()> {
    manager.restart_service(&name).await?;
    println!("Service {} restarted.", name);
    Ok(())
}

async fn enable_service(manager: &SystemdServiceManager, name: String) -> Result<()> {
    manager.enable_service(&name).await?;
    println!("Service {} enabled (will start on boot).", name);
    Ok(())
}

async fn disable_service(manager: &SystemdServiceManager, name: String) -> Result<()> {
    manager.disable_service(&name).await?;
    println!("Service {} disabled (will not start on boot).", name);
    Ok(())
}

async fn show_logs(
    manager: &SystemdServiceManager,
    name: String,
    lines: usize,
    follow: bool,
) -> Result<()> {
    if follow {
        println!("Following logs for {} (Ctrl+C to stop)...", name);
        return Err(crate::error::Error::NotSupported(
            "Log following not yet implemented".to_string(),
        ));
    }

    let options = LogOptions {
        since: None,
        until: None,
        lines: Some(lines),
        priority: None,
    };

    let logs: Vec<crate::atoms::LogEntry> = manager.get_logs(&name, &options).await?;

    if logs.is_empty() {
        println!("No logs found for service {}", name);
    } else {
        for log in logs {
            println!("{} [{:?}] {}", log.timestamp, log.priority, log.message);
        }
    }

    Ok(())
}

async fn remove_service(manager: &SystemdServiceManager, name: String, force: bool) -> Result<()> {
    if !force {
        print!("Are you sure you want to remove service {}? [y/N] ", name);
        use std::io::{self, Write};
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    manager.stop_service(&name).await.ok();

    manager.delete_service(&name).await?;
    println!("Service {} removed.", name);

    Ok(())
}

async fn run_transient(
    manager: &SystemdServiceManager,
    command: Vec<String>,
    workdir: Option<String>,
) -> Result<()> {
    if command.is_empty() {
        return Err(crate::error::Error::InvalidArgument(
            "Command cannot be empty".to_string(),
        ));
    }

    let options = TransientOptions {
        name: format!("svcmgr-run-{}", std::process::id()),
        command,
        scope: true,
        remain_after_exit: false,
        collect: true,
        env: std::collections::HashMap::new(),
        working_directory: workdir.map(std::path::PathBuf::from),
    };

    let unit = manager.run_transient(&options).await?;
    println!("Transient service started:");
    println!("  Unit: {}", unit.name);
    if let Some(pid) = unit.pid {
        println!("  PID: {}", pid);
    }
    println!("\nUse 'systemctl --user status {}' to monitor.", unit.name);

    Ok(())
}
