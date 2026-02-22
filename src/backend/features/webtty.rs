//! F07: Web TTY 管理功能
//!
//! 提供 Web 终端（TTY）实例的生命周期管理功能：
//! - 创建临时/持久化 TTY 实例
//! - 端口自动分配（9000-9100）
//! - Systemd 服务管理集成
//! - Nginx 反向代理集成
//! - TTY 实例状态查询

use crate::atoms::proxy::{ProxyAtom, TtyRoute};
use crate::atoms::systemd::{ActiveState, SystemdAtom, TransientOptions, TransientUnit};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ========================================
// 数据结构
// ========================================

/// TTY 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtyConfig {
    /// TTY 实例名称
    pub name: String,
    /// 执行的命令（默认 bash）
    pub command: Option<String>,
    /// 监听端口（自动分配 9000-9100）
    pub port: Option<u16>,
    /// 只读模式
    pub readonly: bool,
    /// 凭据（用户名:密码）
    pub credential: Option<String>,
}

/// TTY 实例信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtyInstance {
    /// TTY 名称
    pub name: String,
    /// 执行命令
    pub command: String,
    /// 监听端口
    pub port: u16,
    /// 访问 URL
    pub url: String,
    /// Systemd 单元名称
    pub unit_name: String,
    /// 是否持久化
    pub persistent: bool,
    /// 运行状态
    pub status: TtyStatus,
}

/// TTY 运行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TtyStatus {
    Running,
    Stopped,
    Failed,
}

// ========================================
// WebTtyManager
// ========================================

pub struct WebTtyManager<S: SystemdAtom, P: ProxyAtom> {
    systemd: S,
    proxy: P,
    port_range: (u16, u16),
}

impl<S: SystemdAtom, P: ProxyAtom> WebTtyManager<S, P> {
    /// 创建新的 WebTtyManager
    pub fn new(systemd: S, proxy: P) -> Self {
        Self {
            systemd,
            proxy,
            port_range: (9000, 9100),
        }
    }

    /// 创建临时 TTY 实例
    ///
    /// 使用 systemd-run 创建临时服务并添加 nginx 代理
    pub async fn create_transient(&self, config: &TtyConfig) -> Result<TtyInstance> {
        // 1. 分配端口
        let port = if let Some(p) = config.port {
            p
        } else {
            self.allocate_port().await?
        };

        // 2. 构建 ttyd 命令
        let mut cmd_parts = vec!["ttyd".to_string()];

        if config.readonly {
            cmd_parts.push("-R".to_string());
        }

        if let Some(ref cred) = config.credential {
            cmd_parts.push("-c".to_string());
            cmd_parts.push(cred.clone());
        }

        cmd_parts.push("-p".to_string());
        cmd_parts.push(port.to_string());

        let command = config.command.clone().unwrap_or_else(|| "bash".to_string());
        cmd_parts.push(command.clone());

        // 3. 创建临时 systemd 服务
        let unit_name = format!("svcmgr-tty-{}", config.name);
        let opts = TransientOptions {
            name: unit_name.clone(),
            command: cmd_parts,
            scope: false,
            remain_after_exit: false,
            collect: true,
            env: HashMap::new(),
            working_directory: None,
        };

        let transient_unit = self.systemd.run_transient(&opts).await?;

        // 4. 添加 nginx 代理路由
        self.proxy.add_tty_route(&config.name, port)?;

        // 5. 构建返回信息
        let url = format!("/tty/{}/", config.name);
        Ok(TtyInstance {
            name: config.name.clone(),
            command,
            port,
            url,
            unit_name: transient_unit.name,
            persistent: false,
            status: TtyStatus::Running,
        })
    }

    /// 将临时 TTY 转为持久化服务
    pub async fn make_persistent(&self, name: &str) -> Result<TtyInstance> {
        // 1. 查询当前临时实例
        let instance = self.get_instance(name).await?;
        if instance.persistent {
            return Err(Error::Config(format!(
                "TTY instance '{}' is already persistent",
                name
            )));
        }

        // 2. 停止临时服务
        self.systemd.stop_transient(&instance.unit_name).await?;

        // 3. 创建持久化配置（这里简化实现，实际应使用 template 渲染）
        let _persistent_unit = format!("svcmgr-tty-{}.service", name);

        // TODO: 实际实现应使用 TemplateAtom 渲染 systemd service 文件
        // 这里返回错误提示需要手动创建持久化配置
        Err(Error::NotSupported(
            "Persistent TTY creation requires systemd service file template".to_string(),
        ))
    }

    /// 移除 TTY 实例
    pub async fn remove(&self, name: &str) -> Result<()> {
        // 1. 查询实例
        let instance = self.get_instance(name).await?;

        // 2. 停止服务
        if instance.persistent {
            self.systemd.stop(&instance.unit_name).await?;
        } else {
            self.systemd.stop_transient(&instance.unit_name).await?;
        }

        // 3. 移除 nginx 代理路由
        self.proxy.remove_tty_route(name)?;

        Ok(())
    }

    /// 列出所有 TTY 实例
    pub async fn list(&self) -> Result<Vec<TtyInstance>> {
        // 从 nginx 配置获取所有 TTY 路由
        let routes = self.proxy.list_tty_routes()?;

        let mut instances = Vec::new();
        for route in routes {
            match self.get_instance(&route.name).await {
                Ok(instance) => instances.push(instance),
                Err(_) => {
                    // 忽略查询失败的实例（可能已停止）
                    continue;
                }
            }
        }

        Ok(instances)
    }

    /// 查询单个 TTY 实例
    async fn get_instance(&self, name: &str) -> Result<TtyInstance> {
        // 1. 从 nginx 配置获取端口信息
        let routes = self.proxy.list_tty_routes()?;
        let route = routes
            .iter()
            .find(|r| r.name == name)
            .ok_or_else(|| Error::Config(format!("TTY instance '{}' not found", name)))?;

        // 2. 查询 systemd 服务状态
        let unit_name = format!("svcmgr-tty-{}", name);
        let unit_status = self.systemd.status(&unit_name).await.ok();

        // 3. 判断运行状态
        let status = if let Some(ref s) = unit_status {
            match s.active_state {
                ActiveState::Active => TtyStatus::Running,
                ActiveState::Failed => TtyStatus::Failed,
                _ => TtyStatus::Stopped,
            }
        } else {
            TtyStatus::Stopped
        };

        // 4. 判断是否持久化（简化判断：临时服务通常不在 systemd list-units 中持久存在）
        let persistent = unit_status.is_some();

        Ok(TtyInstance {
            name: name.to_string(),
            command: "bash".to_string(), // 简化：无法从 systemd 反查命令
            port: route.port,
            url: format!("/tty/{}/", name),
            unit_name,
            persistent,
            status,
        })
    }

    /// 自动分配可用端口
    async fn allocate_port(&self) -> Result<u16> {
        let routes = self.proxy.list_tty_routes()?;
        let used_ports: Vec<u16> = routes.iter().map(|r| r.port).collect();

        for port in self.port_range.0..=self.port_range.1 {
            if !used_ports.contains(&port) {
                return Ok(port);
            }
        }

        Err(Error::Config(
            "No available ports in range 9000-9100".to_string(),
        ))
    }
}
