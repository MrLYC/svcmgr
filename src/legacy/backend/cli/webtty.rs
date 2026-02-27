use crate::atoms::SupervisorManager;
use crate::atoms::proxy::NginxManager;
use crate::cli::TtyAction;
use crate::error::Result;
use crate::features::{TtyConfig, TtyStatus, WebTtyManager};

pub async fn handle_tty_command(action: TtyAction) -> Result<()> {
    let manager = create_default_manager()?;

    match action {
        TtyAction::Create {
            name,
            command,
            port,
            readonly,
            credential,
        } => create_tty(&manager, name, command, port, readonly, credential).await,
        TtyAction::List => list_ttys(&manager).await,
        TtyAction::Remove { name } => remove_tty(&manager, name).await,
        TtyAction::Persist { name } => persist_tty(&manager, name).await,
    }
}

async fn create_tty(
    manager: &WebTtyManager<SupervisorManager, NginxManager>,
    name: String,
    command: Option<String>,
    port: Option<u16>,
    readonly: bool,
    credential: Option<String>,
) -> Result<()> {
    let config = TtyConfig {
        name: name.clone(),
        command,
        port,
        readonly,
        credential,
    };

    let instance = manager.create_transient(&config).await?;

    println!("✓ 成功创建 TTY 实例: {}", instance.name);
    println!("  端口: {}", instance.port);
    println!("  URL: {}", instance.url);
    println!("  单元: {}", instance.unit_name);
    println!("  状态: {:?}", instance.status);

    Ok(())
}

async fn list_ttys(manager: &WebTtyManager<SupervisorManager, NginxManager>) -> Result<()> {
    let instances = manager.list().await?;

    if instances.is_empty() {
        println!("没有找到 TTY 实例。");
        return Ok(());
    }

    println!(
        "{:<15} {:<10} {:<20} {:<12} {:<10}",
        "名称", "端口", "URL", "持久化", "状态"
    );
    println!("{}", "-".repeat(80));

    for instance in instances {
        let persistent_str = if instance.persistent { "是" } else { "否" };
        let status_str = match instance.status {
            TtyStatus::Running => "运行中",
            TtyStatus::Stopped => "已停止",
            TtyStatus::Failed => "失败",
        };

        println!(
            "{:<15} {:<10} {:<20} {:<12} {:<10}",
            instance.name, instance.port, instance.url, persistent_str, status_str
        );
    }

    Ok(())
}

async fn remove_tty(
    manager: &WebTtyManager<SupervisorManager, NginxManager>,
    name: String,
) -> Result<()> {
    manager.remove(&name).await?;
    println!("✓ 成功移除 TTY 实例: {}", name);
    Ok(())
}

async fn persist_tty(
    manager: &WebTtyManager<SupervisorManager, NginxManager>,
    name: String,
) -> Result<()> {
    let instance = manager.make_persistent(&name).await?;
    println!("✓ 成功持久化 TTY 实例: {}", instance.name);
    println!("  单元: {}", instance.unit_name);
    Ok(())
}

fn create_default_manager() -> Result<WebTtyManager<SupervisorManager, NginxManager>> {
    let supervisor = SupervisorManager::default_config()?;
    let proxy = NginxManager::default_config(supervisor.clone())?;

    Ok(WebTtyManager::new(supervisor, proxy))
}
