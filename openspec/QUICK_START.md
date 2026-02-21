# svcmgr 快速开始指南

本文档提供最小可行产品（MVP）的快速开发路径，帮助你在最短时间内验证核心概念。

## 🎯 MVP 目标

在 **3-5 天**内实现核心功能验证：

1. ✅ 基础 CLI 框架
2. ✅ systemd 服务管理（增删改查）
3. ✅ 模板渲染能力
4. ✅ 简单的配置管理

**不包含**: Cloudflare 隧道、Web 界面、完整的 mise 集成

---

## 📋 MVP 实施步骤

### Day 1: 项目搭建 + 模板原子

#### 1.1 创建项目
```bash
cargo new svcmgr --bin
cd svcmgr
```

#### 1.2 添加核心依赖
```bash
cargo add clap --features derive
cargo add anyhow
cargo add serde --features derive
cargo add serde_json
cargo add tera
cargo add tokio --features full
```

编辑 `Cargo.toml`:
```toml
[package]
name = "svcmgr"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tera = "1"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"
```

#### 1.3 创建目录结构
```bash
mkdir -p src/{atoms,features,commands}
mkdir -p templates/{systemd,crontab,mise,nginx}
```

#### 1.4 实现模板原子
创建 `src/atoms/template.rs`:
```rust
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tera::{Tera, Context as TeraContext};

pub struct TemplateManager {
    tera: Tera,
    template_dir: PathBuf,
}

impl TemplateManager {
    pub fn new<P: AsRef<Path>>(template_dir: P) -> Result<Self> {
        let template_dir = template_dir.as_ref().to_path_buf();
        let pattern = format!("{}/**/*", template_dir.display());
        let tera = Tera::new(&pattern).context("Failed to initialize template engine")?;
        
        Ok(Self { tera, template_dir })
    }

    pub fn render(&self, template_name: &str, data: &Value) -> Result<String> {
        let context = TeraContext::from_serialize(data)?;
        self.tera
            .render(template_name, &context)
            .context("Failed to render template")
    }

    pub fn list_templates(&self) -> Vec<String> {
        self.tera.get_template_names().map(String::from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_render_simple_template() {
        let dir = tempdir().unwrap();
        let template_path = dir.path().join("test.txt");
        fs::write(&template_path, "Hello {{ name }}!").unwrap();

        let manager = TemplateManager::new(dir.path()).unwrap();
        let result = manager.render("test.txt", &json!({"name": "World"})).unwrap();
        assert_eq!(result, "Hello World!");
    }
}
```

#### 1.5 更新 `src/main.rs`
```rust
mod atoms;
mod features;
mod commands;

use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "svcmgr")]
#[command(about = "Linux Service Management Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the environment
    Setup,
    /// Start the management service
    Run,
    /// Teardown the environment
    Teardown,
    /// Manage systemd services
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// List all managed services
    List,
    /// Add a new service
    Add { name: String },
    /// Remove a service
    Remove { name: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => {
            println!("Setting up environment...");
            Ok(())
        }
        Commands::Run => {
            println!("Starting management service...");
            Ok(())
        }
        Commands::Teardown => {
            println!("Tearing down environment...");
            Ok(())
        }
        Commands::Service { action } => match action {
            ServiceAction::List => {
                println!("Listing services...");
                Ok(())
            }
            ServiceAction::Add { name } => {
                println!("Adding service: {}", name);
                Ok(())
            }
            ServiceAction::Remove { name } => {
                println!("Removing service: {}", name);
                Ok(())
            }
        },
    }
}
```

#### 1.6 验证
```bash
cargo build
cargo run -- --help
cargo test
```

**验收**: CLI 框架可运行，模板测试通过

---

### Day 2: systemd 原子实现

#### 2.1 添加 systemd 依赖
```bash
cargo add which
```

#### 2.2 实现 systemd 原子
创建 `src/atoms/systemd.rs`:
```rust
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub description: String,
    pub exec_start: String,
    pub restart: Option<String>,
    pub environment: Option<Vec<String>>,
}

pub struct SystemdManager {
    service_dir: PathBuf,
}

impl SystemdManager {
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let service_dir = PathBuf::from(home)
            .join(".config/systemd/user");
        
        fs::create_dir_all(&service_dir)
            .context("Failed to create systemd user directory")?;
        
        Ok(Self { service_dir })
    }

    pub fn create_service(&self, config: &ServiceConfig) -> Result<()> {
        let service_content = self.generate_service_file(config)?;
        let service_path = self.service_dir.join(format!("{}.service", config.name));
        
        fs::write(&service_path, service_content)
            .context("Failed to write service file")?;
        
        self.daemon_reload()?;
        Ok(())
    }

    pub fn delete_service(&self, name: &str) -> Result<()> {
        self.stop_service(name)?;
        self.disable_service(name)?;
        
        let service_path = self.service_dir.join(format!("{}.service", name));
        fs::remove_file(&service_path)
            .context("Failed to remove service file")?;
        
        self.daemon_reload()?;
        Ok(())
    }

    pub fn start_service(&self, name: &str) -> Result<()> {
        self.systemctl(&["start", name])
    }

    pub fn stop_service(&self, name: &str) -> Result<()> {
        self.systemctl(&["stop", name])
    }

    pub fn enable_service(&self, name: &str) -> Result<()> {
        self.systemctl(&["enable", name])
    }

    pub fn disable_service(&self, name: &str) -> Result<()> {
        self.systemctl(&["disable", name])
    }

    pub fn get_status(&self, name: &str) -> Result<String> {
        let output = Command::new("systemctl")
            .args(&["--user", "status", name])
            .output()
            .context("Failed to get service status")?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn list_services(&self) -> Result<Vec<String>> {
        let entries = fs::read_dir(&self.service_dir)
            .context("Failed to read service directory")?;
        
        let services: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "service")
                    .unwrap_or(false)
            })
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            })
            .collect();
        
        Ok(services)
    }

    fn generate_service_file(&self, config: &ServiceConfig) -> Result<String> {
        let mut content = format!(
            "[Unit]\nDescription={}\n\n[Service]\nExecStart={}\n",
            config.description, config.exec_start
        );

        if let Some(restart) = &config.restart {
            content.push_str(&format!("Restart={}\n", restart));
        }

        if let Some(env_vars) = &config.environment {
            for var in env_vars {
                content.push_str(&format!("Environment=\"{}\"\n", var));
            }
        }

        content.push_str("\n[Install]\nWantedBy=default.target\n");
        Ok(content)
    }

    fn systemctl(&self, args: &[&str]) -> Result<()> {
        let status = Command::new("systemctl")
            .arg("--user")
            .args(args)
            .status()
            .context("Failed to execute systemctl")?;
        
        if !status.success() {
            bail!("systemctl command failed");
        }
        Ok(())
    }

    fn daemon_reload(&self) -> Result<()> {
        self.systemctl(&["daemon-reload"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_service_file() {
        let manager = SystemdManager::new().unwrap();
        let config = ServiceConfig {
            name: "test".to_string(),
            description: "Test Service".to_string(),
            exec_start: "/bin/true".to_string(),
            restart: Some("on-failure".to_string()),
            environment: Some(vec!["FOO=bar".to_string()]),
        };

        let content = manager.generate_service_file(&config).unwrap();
        assert!(content.contains("Description=Test Service"));
        assert!(content.contains("ExecStart=/bin/true"));
        assert!(content.contains("Restart=on-failure"));
        assert!(content.contains("Environment=\"FOO=bar\""));
    }
}
```

#### 2.3 更新 `src/atoms/mod.rs`
```rust
pub mod template;
pub mod systemd;
```

#### 2.4 验证
```bash
cargo test
```

**验收**: systemd 单元测试通过

---

### Day 3: 功能组合 - systemd 服务管理

#### 3.1 实现 systemd 功能模块
创建 `src/features/systemd_service.rs`:
```rust
use crate::atoms::{systemd::*, template::TemplateManager};
use anyhow::{Context, Result};
use serde_json::json;

pub struct SystemdServiceFeature {
    systemd: SystemdManager,
    templates: TemplateManager,
}

impl SystemdServiceFeature {
    pub fn new(template_dir: &str) -> Result<Self> {
        Ok(Self {
            systemd: SystemdManager::new()?,
            templates: TemplateManager::new(template_dir)?,
        })
    }

    pub fn add_service_from_template(
        &self,
        name: &str,
        template: &str,
        vars: serde_json::Value,
    ) -> Result<()> {
        let rendered = self.templates.render(template, &vars)?;
        
        // Parse rendered content as ServiceConfig
        // (Simplified - you'd parse the INI-like format properly)
        let config = ServiceConfig {
            name: name.to_string(),
            description: vars["description"].as_str().unwrap_or("").to_string(),
            exec_start: vars["exec_start"].as_str().unwrap_or("").to_string(),
            restart: vars["restart"].as_str().map(String::from),
            environment: None,
        };

        self.systemd.create_service(&config)?;
        self.systemd.enable_service(name)?;
        Ok(())
    }

    pub fn list_services(&self) -> Result<Vec<String>> {
        self.systemd.list_services()
    }

    pub fn remove_service(&self, name: &str) -> Result<()> {
        self.systemd.delete_service(name)
    }

    pub fn get_status(&self, name: &str) -> Result<String> {
        self.systemd.get_status(name)
    }

    pub fn start(&self, name: &str) -> Result<()> {
        self.systemd.start_service(name)
    }

    pub fn stop(&self, name: &str) -> Result<()> {
        self.systemd.stop_service(name)
    }
}
```

#### 3.2 创建 systemd 服务模板
创建 `templates/systemd/simple.service`:
```ini
[Unit]
Description={{ description }}

[Service]
ExecStart={{ exec_start }}
{% if restart %}Restart={{ restart }}{% endif %}
{% if working_directory %}WorkingDirectory={{ working_directory }}{% endif %}

[Install]
WantedBy=default.target
```

#### 3.3 更新 CLI 命令
更新 `src/main.rs` 的 service 命令处理：
```rust
use crate::features::systemd_service::SystemdServiceFeature;
use serde_json::json;

// In match Commands::Service
Commands::Service { action } => {
    let feature = SystemdServiceFeature::new("./templates")?;
    
    match action {
        ServiceAction::List => {
            let services = feature.list_services()?;
            for service in services {
                println!("{}", service);
            }
            Ok(())
        }
        ServiceAction::Add { name } => {
            // Simple interactive prompt (simplified)
            println!("Enter description:");
            let mut description = String::new();
            std::io::stdin().read_line(&mut description)?;
            
            println!("Enter exec command:");
            let mut exec_start = String::new();
            std::io::stdin().read_line(&mut exec_start)?;
            
            feature.add_service_from_template(
                &name,
                "systemd/simple.service",
                json!({
                    "description": description.trim(),
                    "exec_start": exec_start.trim(),
                    "restart": "on-failure"
                }),
            )?;
            
            println!("Service {} added successfully", name);
            Ok(())
        }
        ServiceAction::Remove { name } => {
            feature.remove_service(&name)?;
            println!("Service {} removed", name);
            Ok(())
        }
    }
}
```

#### 3.4 验证
```bash
cargo build
cargo run -- service list
cargo run -- service add test-service
cargo run -- service list
systemctl --user status test-service
cargo run -- service remove test-service
```

**验收**: 可以通过 CLI 管理 systemd 服务

---

### Day 4-5: 完善和测试

#### 4.1 添加更多 CLI 子命令
```rust
#[derive(Subcommand)]
enum ServiceAction {
    List,
    Add { name: String },
    Remove { name: String },
    Status { name: String },
    Start { name: String },
    Stop { name: String },
    Restart { name: String },
    Logs { 
        name: String,
        #[arg(short, long)]
        lines: Option<usize>,
    },
}
```

#### 4.2 实现日志查询
在 `SystemdManager` 添加:
```rust
pub fn get_logs(&self, name: &str, lines: Option<usize>) -> Result<String> {
    let mut args = vec!["--user", "-u", name];
    if let Some(n) = lines {
        args.push("-n");
        args.push(&n.to_string());
    }
    
    let output = Command::new("journalctl")
        .args(&args)
        .output()
        .context("Failed to get logs")?;
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

#### 4.3 添加集成测试
创建 `tests/integration_test.rs`:
```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("svcmgr").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Linux Service Management Tool"));
}

#[test]
fn test_service_list() {
    let mut cmd = Command::cargo_bin("svcmgr").unwrap();
    cmd.arg("service")
        .arg("list")
        .assert()
        .success();
}
```

#### 4.4 编写 README
创建 `README.md`:
```markdown
# svcmgr - Linux Service Management Tool

A modern CLI tool for managing Linux services with systemd.

## Quick Start

### Installation
\`\`\`bash
cargo install --path .
\`\`\`

### Usage
\`\`\`bash
# List services
svcmgr service list

# Add a service
svcmgr service add my-service

# Check status
svcmgr service status my-service

# View logs
svcmgr service logs my-service --lines 50

# Remove service
svcmgr service remove my-service
\`\`\`

## Features
- ✅ systemd service management
- ✅ Template-based configuration
- ⏳ More features coming soon...
```

#### 4.5 最终验证
```bash
# 运行所有测试
cargo test

# 构建 release 版本
cargo build --release

# 端到端测试
./target/release/svcmgr service add test-svc
./target/release/svcmgr service status test-svc
./target/release/svcmgr service logs test-svc
./target/release/svcmgr service remove test-svc
```

---

## ✅ MVP 完成检查清单

- [ ] CLI 框架搭建完成，帮助信息正确
- [ ] 模板引擎集成，可渲染 Jinja2 模板
- [ ] systemd 原子实现，单元测试通过
- [ ] systemd 服务管理功能可用（增删改查）
- [ ] 集成测试覆盖主要场景
- [ ] README 文档完整
- [ ] 代码可编译为 release 版本

---

## 🚀 下一步

MVP 完成后，参考 [IMPLEMENTATION_GUIDE.md](IMPLEMENTATION_GUIDE.md) 继续实现：

1. **Phase 2**: Git 配置管理、crontab 集成
2. **Phase 3**: mise 集成、nginx 代理
3. **Phase 4**: Cloudflare 隧道、Web TTY
4. **Phase 5**: Web 界面

---

## 💡 常见问题

### Q: 为什么 systemd 服务文件不生效？
A: 确保:
1. 文件位于 `~/.config/systemd/user/`
2. 执行了 `systemctl --user daemon-reload`
3. 使用 `systemctl --user status <name>` 检查状态

### Q: 模板渲染失败？
A: 检查:
1. 模板目录路径正确
2. 模板文件语法正确（Jinja2）
3. 传入的变量名匹配模板中的占位符

### Q: 如何调试？
A: 使用环境变量启用日志:
```bash
RUST_LOG=debug cargo run -- service list
```

---

**提示**: 这是一个最小化的实现路径，专注于核心功能验证。生产环境需要更完善的错误处理、日志记录和测试覆盖。
