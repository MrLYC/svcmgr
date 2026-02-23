use crate::atoms::mise::{DependencyAtom, EnvAtom, TaskAtom, TaskConfig};
use crate::cli::MiseAction;
use crate::error::Result;
use crate::features::MiseManager;
use std::collections::HashMap;
use std::env;

pub async fn handle_mise_command(action: MiseAction) -> Result<()> {
    let config_path = env::current_dir()?.join(".mise.toml");
    let manager = MiseManager::new(config_path);

    match action {
        MiseAction::Install { tool, version } => install_tool(&manager, &tool, &version),
        MiseAction::ListTools => list_tools(&manager),
        MiseAction::Update { tool, version } => update_tool(&manager, &tool, &version),
        MiseAction::Remove {
            tool,
            version: _,
            force: _,
        } => remove_tool(&manager, &tool),
        MiseAction::AddTask {
            name,
            run,
            description,
            depends,
            template: _,
            var: _,
        } => add_task(&manager, &name, &run, description, &depends),
        MiseAction::ListTasks => list_tasks(&manager),
        MiseAction::RunTask { name, args } => run_task(&manager, &name, &args),
        MiseAction::DeleteTask { name, force: _ } => delete_task(&manager, &name),
        MiseAction::SetEnv { key, value } => set_env(&manager, &key, &value),
        MiseAction::GetEnv => get_env(&manager),
        MiseAction::DeleteEnv { key } => delete_env(&manager, &key),
    }
}

fn list_tools(manager: &MiseManager) -> Result<()> {
    let tools = manager.list_tools()?;
    println!("{:<20} {:<15}", "Tool", "Version");
    println!("{}", "-".repeat(40));
    for tool in tools {
        println!("{:<20} {:<15}", tool.name, tool.version);
    }
    Ok(())
}

fn install_tool(manager: &MiseManager, tool: &str, version: &str) -> Result<()> {
    manager.install(tool, version)?;
    println!("Installed {} {}", tool, version);
    Ok(())
}

fn update_tool(manager: &MiseManager, tool: &str, version: &str) -> Result<()> {
    manager.install(tool, version)?;
    println!("Updated {} to {}", tool, version);
    Ok(())
}

fn remove_tool(manager: &MiseManager, tool: &str) -> Result<()> {
    manager.uninstall(tool)?;
    println!("Removed {}", tool);
    Ok(())
}

fn add_task(
    manager: &MiseManager,
    name: &str,
    run: &[String],
    description: Option<String>,
    depends: &[String],
) -> Result<()> {
    let config = TaskConfig {
        run: run.to_vec(),
        description,
        depends: depends.to_vec(),
        env: HashMap::new(),
        dir: None,
    };
    manager.add_task(name, &config)?;
    println!("Added task: {}", name);
    Ok(())
}

fn list_tasks(manager: &MiseManager) -> Result<()> {
    let tasks = manager.list_tasks()?;
    println!("{:<20} Description", "Task");
    println!("{}", "-".repeat(60));
    for task in tasks {
        println!("{:<20} {}", task.name, task.description.unwrap_or_default());
    }
    Ok(())
}

fn run_task(manager: &MiseManager, name: &str, args: &[String]) -> Result<()> {
    println!("Running task: {}", name);
    manager.run(name, args)?;
    println!("Task completed");
    Ok(())
}

fn delete_task(manager: &MiseManager, name: &str) -> Result<()> {
    manager.remove_task(name)?;
    println!("Deleted task: {}", name);
    Ok(())
}

fn set_env(manager: &MiseManager, key: &str, value: &str) -> Result<()> {
    manager.set(key, value)?;
    println!("Set {} = {}", key, value);
    Ok(())
}

fn get_env(manager: &MiseManager) -> Result<()> {
    let env_vars = manager.list_env()?;
    println!("{:<30} Value", "Key");
    println!("{}", "-".repeat(80));
    for env in env_vars {
        println!("{:<30} {}", env.key, env.value);
    }
    Ok(())
}

fn delete_env(manager: &MiseManager, key: &str) -> Result<()> {
    manager.unset(key)?;
    println!("Deleted: {}", key);
    Ok(())
}
