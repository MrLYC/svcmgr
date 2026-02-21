# Feature: System Settings (F09)

**特性代号**: F09  
**原子依赖**: 无（系统级配置管理）  
**功能目标**: 提供系统级设置管理，包括全局配置、工具状态检测和系统重置

---

## 需求说明

### 概述

系统 **必须** 提供系统设置功能，允许用户通过 Web 界面和 REST API 管理 svcmgr 的全局配置，检测依赖工具的安装状态，并支持系统重置操作。

### 核心能力

1. **全局配置查询**: 获取系统全局配置（Nginx 端口、配置目录、自动提交、日志级别）
2. **全局配置更新**: 修改系统全局配置
3. **工具状态检测**: 检测依赖工具的安装状态和版本（nginx, systemd, crontab, cloudflared, mise, ttyd）
4. **系统重置**: 重置系统到初始状态（清除所有配置和数据）

### 技术约束

- **API 基础路径**: `/svcmgr/api/settings`
- **配置存储**: `~/.local/share/svcmgr/config.toml`
- **版本管理**: 配置变更应当通过 Git 原子提交
- **安全性**: 系统重置操作需要二次确认
- **工具检测**: 使用 `which` 命令和版本命令检测工具状态

---

## ADDED Requirements

### Requirement: 全局配置查询
系统 **必须** 提供 REST API 端点用于查询系统全局配置。

#### Scenario: 查询全局配置
- **WHEN** 客户端发送 `GET /svcmgr/api/settings` 请求
- **THEN** 系统 **应当** 从配置文件读取全局配置
- **AND** 系统 **应当** 返回包含 `nginx_port, config_dir, auto_commit, log_level` 的对象
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 配置文件不存在
- **WHEN** 配置文件不存在或读取失败
- **THEN** 系统 **应当** 返回默认配置
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

### Requirement: 全局配置更新
系统 **必须** 提供 REST API 端点用于更新系统全局配置。

#### Scenario: 更新全局配置
- **WHEN** 客户端发送 `PUT /svcmgr/api/settings` 请求
- **AND** 请求体包含需要更新的配置字段（支持部分更新）
- **THEN** 系统 **应当** 验证配置值的合法性
- **AND** 系统 **应当** 更新配置文件
- **AND** 系统 **应当** 通过 Git 原子提交配置变更（如果启用版本管理）
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

#### Scenario: 配置验证失败
- **WHEN** 客户端提供非法配置值（如 `nginx_port` 超出范围 1024-65535）
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `VALIDATION_ERROR`
- **AND** HTTP 响应状态码 **应当** 为 `422 Unprocessable Entity`

#### Scenario: 端口被占用
- **WHEN** 客户端尝试更新 `nginx_port` 为已被占用的端口
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `CONFLICT`
- **AND** 错误消息 **应当** 说明端口被占用
- **AND** HTTP 响应状态码 **应当** 为 `409 Conflict`

---

### Requirement: 工具状态检测
系统 **必须** 提供 REST API 端点用于检测依赖工具的安装状态。

#### Scenario: 查询工具状态
- **WHEN** 客户端发送 `GET /svcmgr/api/settings/tools` 请求
- **THEN** 系统 **应当** 检测以下工具的安装状态：
  - `nginx`: Web 服务器和反向代理
  - `systemd`: 系统服务管理器
  - `crontab`: 定时任务管理器
  - `cloudflared`: Cloudflare 隧道客户端
  - `mise`: 开发工具版本管理器
  - `ttyd`: Web 终端工具
- **AND** 对于已安装的工具，系统 **应当** 检测其版本号和可执行文件路径
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 工具检测逻辑
- **WHEN** 系统检测工具安装状态时
- **THEN** 系统 **应当** 使用 `which {tool}` 命令检测工具是否安装
- **AND** 如果工具已安装，系统 **应当** 执行版本命令获取版本号：
  - `nginx -v` → 解析 `nginx version: nginx/1.24.0`
  - `systemctl --version` → 解析 `systemd 255`
  - `crontab -l` → 检查命令是否可用（crontab 无版本命令）
  - `cloudflared --version` → 解析 `cloudflared version 2024.2.1`
  - `mise --version` → 解析 `2024.11.0`
  - `ttyd --version` → 解析 `ttyd version 1.7.4`

---

### Requirement: 系统重置
系统 **必须** 提供 REST API 端点用于重置系统到初始状态。

#### Scenario: 执行系统重置
- **WHEN** 客户端发送 `POST /svcmgr/api/settings/reset` 请求
- **THEN** 系统 **应当** 执行以下操作：
  1. 停止所有 svcmgr 管理的 systemd 服务
  2. 删除所有配置文件（`~/.local/share/svcmgr/`）
  3. 删除所有 Git 仓库
  4. 恢复默认配置
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

#### Scenario: 重置前确认
- **WHEN** 系统执行重置操作时
- **THEN** 客户端 **应当** 在前端提供二次确认对话框
- **AND** 对话框 **应当** 明确说明重置操作将清除所有数据且不可恢复

---

## REST API 接口规范

### 1. 获取全局配置

#### `GET /svcmgr/api/settings`

**描述**: 获取系统全局配置

**请求参数**: 无

**响应** (200):
```json
{
  "nginx_port": 8080,
  "config_dir": "/home/user/.local/share/svcmgr",
  "auto_commit": true,
  "log_level": "info"
}
```

**字段说明**:
- `nginx_port` (int): Nginx 监听端口（默认 8080，范围 1024-65535）
- `config_dir` (string): 配置目录路径（默认 `~/.local/share/svcmgr`）
- `auto_commit` (boolean): 是否自动提交配置变更到 Git（默认 `true`）
- `log_level` (string): 日志级别（`"debug" | "info" | "warn" | "error"`，默认 `"info"`）

**错误响应**:
- `500 INTERNAL_ERROR`: 配置文件读取失败

---

### 2. 更新全局配置

#### `PUT /svcmgr/api/settings`

**描述**: 更新系统全局配置（支持部分更新）

**请求体** (支持部分字段):
```json
{
  "nginx_port": 9090,
  "auto_commit": false,
  "log_level": "debug"
}
```

**响应** (204):
无响应体

**错误响应**:
- `400 INVALID_REQUEST`: 请求格式错误
- `409 CONFLICT`: 端口被占用
- `422 VALIDATION_ERROR`: 配置值验证失败
- `500 INTERNAL_ERROR`: 配置更新失败

---

### 3. 查询工具状态

#### `GET /svcmgr/api/settings/tools`

**描述**: 检测依赖工具的安装状态和版本

**请求参数**: 无

**响应** (200):
```json
[
  {
    "name": "nginx",
    "installed": true,
    "version": "1.24.0",
    "path": "/usr/bin/nginx"
  },
  {
    "name": "systemd",
    "installed": true,
    "version": "255",
    "path": "/usr/bin/systemctl"
  },
  {
    "name": "crontab",
    "installed": true,
    "path": "/usr/bin/crontab"
  },
  {
    "name": "cloudflared",
    "installed": true,
    "version": "2024.2.1",
    "path": "/usr/local/bin/cloudflared"
  },
  {
    "name": "mise",
    "installed": true,
    "version": "2024.11.0",
    "path": "/usr/local/bin/mise"
  },
  {
    "name": "ttyd",
    "installed": false
  }
]
```

**字段说明**:
- `name` (string): 工具名称
- `installed` (boolean): 是否已安装
- `version` (string, 可选): 工具版本号（如果已安装且可检测）
- `path` (string, 可选): 可执行文件路径（如果已安装）

**错误响应**:
- `500 INTERNAL_ERROR`: 工具检测失败

---

### 4. 重置系统

#### `POST /svcmgr/api/settings/reset`

**描述**: 重置系统到初始状态（清除所有配置和数据）

**请求体**: 无

**响应** (204):
无响应体

**错误响应**:
- `500 INTERNAL_ERROR`: 重置失败

---

## Rust 数据类型定义

### 全局配置对象

```rust
use serde::{Deserialize, Serialize};

/// 系统全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsConfig {
    /// Nginx 监听端口
    pub nginx_port: u16,
    
    /// 配置目录路径
    pub config_dir: String,
    
    /// 是否自动提交配置变更到 Git
    pub auto_commit: bool,
    
    /// 日志级别
    pub log_level: LogLevel,
}

/// 日志级别
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// 调试级别（最详细）
    Debug,
    
    /// 信息级别（默认）
    Info,
    
    /// 警告级别
    Warn,
    
    /// 错误级别（最简洁）
    Error,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            nginx_port: 8080,
            config_dir: format!("{}/.local/share/svcmgr", std::env::var("HOME").unwrap_or_default()),
            auto_commit: true,
            log_level: LogLevel::Info,
        }
    }
}
```

### 工具状态对象

```rust
/// 依赖工具状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    /// 工具名称
    pub name: String,
    
    /// 是否已安装
    pub installed: bool,
    
    /// 工具版本号（如果已安装且可检测）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    
    /// 可执行文件路径（如果已安装）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}
```

### 请求类型

```rust
/// 更新全局配置请求（所有字段可选）
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSettingsRequest {
    /// 新的 Nginx 端口
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nginx_port: Option<u16>,
    
    /// 新的配置目录路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    
    /// 新的自动提交设置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_commit: Option<bool>,
    
    /// 新的日志级别
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<LogLevel>,
}
```

### 验证逻辑

```rust
impl UpdateSettingsRequest {
    /// 验证请求数据
    pub fn validate(&self) -> Result<(), ValidationError> {
        // 验证 nginx_port 范围（1024-65535）
        if let Some(port) = self.nginx_port {
            if port < 1024 || port > 65535 {
                return Err(ValidationError::InvalidPort(port));
            }
        }
        
        // 验证 config_dir 路径格式（必须为绝对路径）
        if let Some(dir) = &self.config_dir {
            if !dir.starts_with('/') && !dir.starts_with('~') {
                return Err(ValidationError::InvalidPath(dir.clone()));
            }
        }
        
        Ok(())
    }
}
```

---

## Rust Trait 接口定义

```rust
use async_trait::async_trait;
use crate::error::ApiError;

/// 系统设置功能特性
#[async_trait]
pub trait SettingsFeature {
    /// 获取全局配置
    async fn get_settings(&self) -> Result<SettingsConfig, ApiError>;
    
    /// 更新全局配置
    async fn update_settings(&self, request: UpdateSettingsRequest) -> Result<(), ApiError>;
    
    /// 获取工具状态列表
    async fn get_tool_statuses(&self) -> Result<Vec<ToolStatus>, ApiError>;
    
    /// 重置系统
    async fn reset_system(&self) -> Result<(), ApiError>;
}
```

### 实现说明

```rust
use std::path::PathBuf;
use tokio::process::Command;

/// 系统设置功能实现
pub struct SettingsManager {
    config_file: PathBuf,
    git_atom: Arc<dyn GitAtom>,
}

impl SettingsManager {
    /// 检测工具安装状态
    async fn detect_tool(&self, name: &str) -> Result<ToolStatus, ApiError> {
        // 1. 使用 which 命令检测工具是否安装
        let which_output = Command::new("which")
            .arg(name)
            .output()
            .await?;
        
        if !which_output.status.success() {
            return Ok(ToolStatus {
                name: name.to_string(),
                installed: false,
                version: None,
                path: None,
            });
        }
        
        let path = String::from_utf8_lossy(&which_output.stdout).trim().to_string();
        
        // 2. 检测工具版本
        let version = self.detect_tool_version(name).await.ok();
        
        Ok(ToolStatus {
            name: name.to_string(),
            installed: true,
            version,
            path: Some(path),
        })
    }
    
    /// 检测工具版本
    async fn detect_tool_version(&self, name: &str) -> Result<String, ApiError> {
        let (cmd, args, pattern) = match name {
            "nginx" => ("nginx", vec!["-v"], r"nginx/(.+)"),
            "systemd" => ("systemctl", vec!["--version"], r"systemd (\d+)"),
            "cloudflared" => ("cloudflared", vec!["--version"], r"cloudflared version (.+)"),
            "mise" => ("mise", vec!["--version"], r"(.+)"),
            "ttyd" => ("ttyd", vec!["--version"], r"ttyd version (.+)"),
            "crontab" => return Err(ApiError::VersionNotAvailable), // crontab 无版本命令
            _ => return Err(ApiError::UnknownTool(name.to_string())),
        };
        
        let output = Command::new(cmd)
            .args(args)
            .output()
            .await?;
        
        let output_str = String::from_utf8_lossy(&output.stderr); // 某些工具版本信息在 stderr
        let output_str = if output_str.is_empty() {
            String::from_utf8_lossy(&output.stdout)
        } else {
            output_str
        };
        
        // 使用正则表达式解析版本号
        let re = regex::Regex::new(pattern)?;
        if let Some(captures) = re.captures(&output_str) {
            Ok(captures.get(1).unwrap().as_str().to_string())
        } else {
            Err(ApiError::VersionParseError)
        }
    }
}

#[async_trait]
impl SettingsFeature for SettingsManager {
    async fn get_settings(&self) -> Result<SettingsConfig, ApiError> {
        // 1. 尝试从配置文件读取
        // 2. 如果文件不存在，返回默认配置
        // 3. 解析 TOML 格式配置
        
        if !self.config_file.exists() {
            return Ok(SettingsConfig::default());
        }
        
        let content = tokio::fs::read_to_string(&self.config_file).await?;
        let config: SettingsConfig = toml::from_str(&content)?;
        Ok(config)
    }
    
    async fn update_settings(&self, request: UpdateSettingsRequest) -> Result<(), ApiError> {
        // 1. 验证请求数据
        request.validate()?;
        
        // 2. 读取当前配置
        let mut config = self.get_settings().await?;
        
        // 3. 应用更新（只更新提供的字段）
        if let Some(port) = request.nginx_port {
            // 检查端口是否被占用
            self.check_port_available(port).await?;
            config.nginx_port = port;
        }
        if let Some(dir) = request.config_dir {
            config.config_dir = dir;
        }
        if let Some(auto_commit) = request.auto_commit {
            config.auto_commit = auto_commit;
        }
        if let Some(log_level) = request.log_level {
            config.log_level = log_level;
        }
        
        // 4. 序列化为 TOML 并写入文件
        let toml_content = toml::to_string_pretty(&config)?;
        tokio::fs::write(&self.config_file, toml_content).await?;
        
        // 5. 如果启用版本管理，通过 Git 原子提交
        if config.auto_commit {
            self.git_atom.commit("Update settings configuration").await?;
        }
        
        Ok(())
    }
    
    async fn get_tool_statuses(&self) -> Result<Vec<ToolStatus>, ApiError> {
        // 1. 并发检测所有工具
        let tools = vec!["nginx", "systemd", "crontab", "cloudflared", "mise", "ttyd"];
        
        let mut statuses = Vec::new();
        for tool in tools {
            statuses.push(self.detect_tool(tool).await?);
        }
        
        Ok(statuses)
    }
    
    async fn reset_system(&self) -> Result<(), ApiError> {
        // 1. 停止所有 svcmgr 管理的 systemd 服务
        // 2. 删除配置目录（~/.local/share/svcmgr/）
        // 3. 删除 systemd 单元文件
        // 4. 重新初始化默认配置
        
        // TODO: 实现完整的重置逻辑
        todo!()
    }
}
```

---

## 配置文件示例

### 全局配置 (TOML)

**路径**: `~/.local/share/svcmgr/config.toml`

```toml
# svcmgr Global Configuration
# Generated at 2026-02-21T10:00:00Z

nginx_port = 8080
config_dir = "/home/user/.local/share/svcmgr"
auto_commit = true
log_level = "info"
```

---

## 错误码定义

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SettingsError {
    /// 端口被占用
    Conflict { message: String, port: u16 },
    
    /// 验证错误
    ValidationError { message: String, field: Option<String> },
    
    /// 无效请求
    InvalidRequest { message: String },
    
    /// 内部错误
    InternalError { message: String },
}
```

---

## 实施检查清单

### Phase 1: 基础设置功能
- [ ] 实现 `SettingsFeature` trait
- [ ] 实现全局配置读取（TOML 解析）
- [ ] 实现全局配置更新（TOML 序列化）
- [ ] 集成 Git 原子提交配置变更

### Phase 2: 工具检测
- [ ] 实现工具安装检测（使用 `which` 命令）
- [ ] 实现工具版本检测（解析版本命令输出）
- [ ] 支持所有依赖工具的检测（nginx, systemd, crontab, cloudflared, mise, ttyd）

### Phase 3: 系统重置
- [ ] 实现系统重置逻辑（停止服务、删除配置、恢复默认值）
- [ ] 添加安全检查（防止误操作）

### Phase 4: 高级功能
- [ ] 支持端口占用检测
- [ ] 支持配置值验证
- [ ] 支持日志级别动态切换

### Phase 5: 测试
- [ ] 单元测试：配置读取和更新
- [ ] 单元测试：工具检测逻辑
- [ ] 单元测试：配置验证逻辑
- [ ] 集成测试：Git 版本管理集成
- [ ] 端到端测试：完整设置管理流程

---

## 相关文档

- [API 设计规范](./20-api-design.md)
- [Git 配置版本原子 (A01)](./01-atom-git.md)
- [前端 UI 设计](./30-frontend-ui.md)
