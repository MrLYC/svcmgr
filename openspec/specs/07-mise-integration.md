# 07 - mise 集成层设计（Port-Adapter 模式）

> 版本：2.0.0-draft
> 状态：设计中

## 1. 概述

mise 采用 CalVer 版本号（如 `v2026.2.17`），迭代频繁，且不提供 Rust 库 API。svcmgr 必须通过 **Port-Adapter（端口-适配器）** 模式建立防腐层，以应对 mise 的持续演进。

## 2. 核心设计原则

| 原则 | 说明 |
|------|------|
| **面向接口而非实现** | svcmgr 内部通过 trait 抽象 mise 能力，不直接耦合 CLI 命令或 TOML 段名 |
| **适配器隔离** | 所有 mise 交互集中在独立的适配器模块，业务逻辑层不直接调用 mise |
| **配置分层** | svcmgr 配置与 mise 配置在解析层分离 |
| **版本感知** | 运行时检测 mise 版本，根据版本选择兼容的交互策略 |
| **优雅降级** | 当 mise 行为变化导致特定功能不可用时，降级到备选方案而非崩溃 |

## 3. 架构分层

```
┌─────────────────────────────────────────────────────────────┐
│                    svcmgr 核心业务逻辑                        │
│  （调度引擎、进程管理、Web 服务、配置管理）                      │
├──────────────────────┬──────────────────────────────────────┤
│      Port 层         │  Rust trait 定义（纯接口契约）          │
│  ┌─────────────────┐ │  ┌──────────────────────────────┐    │
│  │ DependencyPort  │ │  │ fn install(tool, ver)         │    │
│  │ TaskPort        │ │  │ fn run_task(name, args)       │    │
│  │ EnvPort         │ │  │ fn get_env() -> HashMap       │    │
│  │ ConfigPort      │ │  │ fn list_config_files()        │    │
│  └─────────────────┘ │  └──────────────────────────────┘    │
├──────────────────────┴──────────────────────────────────────┤
│      Adapter 层（adapters/mise/）                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ MiseV2026    │  │ MiseV2025    │  │ MockAdapter  │       │
│  │ Adapter      │  │ Adapter      │  │ (测试用)      │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
│         │                 │                 │               │
│  ┌──────┴─────────────────┴─────────────────┴──────┐        │
│  │          AdapterFactory（版本检测 + 路由）         │        │
│  └─────────────────────────────────────────────────┘        │
├─────────────────────────────────────────────────────────────┤
│  mise CLI / mise 配置文件 / mise 环境变量                      │
└─────────────────────────────────────────────────────────────┘
```

## 4. Port 接口定义

### 4.1 DependencyPort（依赖管理）

```rust
use async_trait::async_trait;

/// 依赖管理端口
#[async_trait]
pub trait DependencyPort: Send + Sync {
    /// 安装指定工具和版本
    async fn install(&self, tool: &str, version: &str) -> Result<()>;
    
    /// 列出已安装的工具
    async fn list_installed(&self) -> Result<Vec<ToolInfo>>;
    
    /// 设置当前目录使用的工具版本
    async fn use_tool(&self, tool: &str, version: &str) -> Result<()>;
    
    /// 移除工具
    async fn remove(&self, tool: &str, version: &str) -> Result<()>;
    
    /// 获取 mise 版本信息
    fn mise_version(&self) -> &MiseVersion;
}

pub struct ToolInfo {
    pub name: String,
    pub version: String,
    pub source: String,  // e.g. "asdf", "core"
}
```

### 4.2 TaskPort（任务管理）

```rust
/// 任务管理端口
#[async_trait]
pub trait TaskPort: Send + Sync {
    /// 运行指定任务（一次性前台执行）
    async fn run_task(&self, name: &str, args: &[String]) -> Result<TaskOutput>;
    
    /// 获取任务定义（从 mise 配置中读取 run 命令）
    async fn get_task_command(&self, name: &str) -> Result<TaskCommand>;
    
    /// 列出所有任务
    async fn list_tasks(&self) -> Result<Vec<TaskInfo>>;
}

pub struct TaskCommand {
    pub command: String,
    pub env: HashMap<String, String>,
    pub workdir: Option<PathBuf>,
}

pub struct TaskOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct TaskInfo {
    pub name: String,
    pub description: Option<String>,
    pub command: String,
    pub depends: Vec<String>,
}
```

### 4.3 EnvPort（环境变量）

```rust
/// 环境变量端口
#[async_trait]
pub trait EnvPort: Send + Sync {
    /// 获取 mise 解析后的完整环境变量
    async fn get_env(&self) -> Result<HashMap<String, String>>;
    
    /// 获取指定目录下的环境变量
    async fn get_env_for_dir(&self, dir: &Path) -> Result<HashMap<String, String>>;
}
```

### 4.4 ConfigPort（配置文件）

```rust
/// 配置文件端口
#[async_trait]
pub trait ConfigPort: Send + Sync {
    /// 获取 mise 当前加载的配置文件列表（按优先级排序）
    async fn list_config_files(&self) -> Result<Vec<PathBuf>>;
    
    /// 读取指定配置文件的原始 TOML
    async fn read_config(&self, path: &Path) -> Result<toml::Value>;
    
    /// 写入配置文件（仅写 mise 原生段）
    async fn write_config(&self, path: &Path, value: &toml::Value) -> Result<()>;
}
```

## 5. mise 集成策略

### 5.1 配置文件驱动（层级 1 - 主要方式）

```rust
/// 直接解析 mise.toml 获取任务定义
fn parse_mise_config(path: &Path) -> Result<MiseConfig> {
    let content = std::fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&content)?;
    
    Ok(MiseConfig {
        tools: parse_tools_section(&value),
        tasks: parse_tasks_section(&value),
        env: parse_env_section(&value),
    })
}

/// 从配置中获取任务命令（用于直接 spawn，而非通过 mise run）
fn get_task_command(config: &MiseConfig, task_name: &str) -> Option<TaskCommand> {
    config.tasks.get(task_name).map(|t| TaskCommand {
        command: t.run.clone(),
        env: t.env.clone(),
        workdir: t.dir.clone(),
    })
}
```

### 5.2 mise 子进程调用（层级 2 - 仅必要时）

```rust
/// mise CLI 命令构造器
pub struct MiseCommand {
    version: MiseVersion,
}

impl MiseCommand {
    /// 安装工具（必须调用 mise）
    pub fn install(&self, tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("install").arg(format!("{}@{}", tool, version));
        cmd
    }
    
    /// 获取解析后的环境变量（mise 支持模板、_.file 等复杂解析）
    pub fn env_json(&self) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("env").arg("--json");
        cmd
    }
    
    /// 激活工具版本
    pub fn use_tool(&self, tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("use").arg(format!("{}@{}", tool, version));
        cmd
    }
}
```

**关键原则**：
- 能通过配置文件获取的信息，不调用 mise 进程
- 必须调用 mise 进程的场景，通过 Port-Adapter 抽象层隔离

## 6. 版本检测与兼容策略

### 6.1 版本检测

```rust
/// mise 版本信息
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MiseVersion {
    pub year: u16,    // e.g. 2026
    pub minor: u16,   // e.g. 2
    pub patch: u16,   // e.g. 17
}

impl MiseVersion {
    /// 从 `mise --version` 输出解析
    pub fn detect() -> Result<Self> {
        let output = Command::new("mise")
            .arg("--version")
            .output()?;
        
        let version_str = String::from_utf8(output.stdout)?;
        Self::parse(&version_str)
    }
    
    fn parse(s: &str) -> Result<Self> {
        // 解析 "2026.2.17" 或 "mise 2026.2.17" 格式
        let parts: Vec<&str> = s.trim()
            .split_whitespace()
            .last()
            .unwrap()
            .split('.')
            .collect();
        
        Ok(Self {
            year: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
        })
    }
    
    /// 检查是否支持特定特性
    pub fn supports(&self, feature: MiseFeature) -> bool {
        match feature {
            MiseFeature::ConfD       => self >= &Self::new(2024, 12, 0),
            MiseFeature::TaskDepends => self >= &Self::new(2024, 1, 0),
            MiseFeature::Lockfiles   => self >= &Self::new(2026, 2, 0),
            MiseFeature::McpRunTask  => self >= &Self::new(2026, 2, 16),
        }
    }
}

/// mise 特性枚举
pub enum MiseFeature {
    ConfD,          // conf.d 目录支持
    TaskDepends,    // 任务依赖
    Lockfiles,      // 锁文件
    McpRunTask,     // MCP run_task 工具
}
```

### 6.2 兼容策略矩阵

| mise 版本范围 | 策略 | 说明 |
|--------------|------|------|
| < 最低支持版本 | **拒绝启动** | 给出明确错误信息和升级指引 |
| 最低版本 ~ 推荐版本 | **兼容模式** | 关闭依赖新特性的功能，使用备选实现 |
| 推荐版本 ~ 当前最新 | **完整模式** | 所有功能可用 |
| > 已知最新版本 | **乐观模式** | 假设向后兼容，记录警告日志 |

### 6.3 AdapterFactory

```rust
/// 适配器工厂
pub struct AdapterFactory {
    version: MiseVersion,
}

impl AdapterFactory {
    pub fn new() -> Result<Self> {
        let version = MiseVersion::detect()?;
        Ok(Self { version })
    }
    
    /// 创建适配器
    pub fn create(&self) -> Box<dyn MiseAdapter> {
        if self.version >= MiseVersion::new(2026, 0, 0) {
            Box::new(MiseV2026Adapter::new(self.version.clone()))
        } else if self.version >= MiseVersion::new(2025, 0, 0) {
            Box::new(MiseV2025Adapter::new(self.version.clone()))
        } else {
            panic!(
                "mise version {} is not supported. Minimum version: 2025.0.0",
                self.version
            );
        }
    }
}

/// 统一的 Adapter trait
pub trait MiseAdapter: DependencyPort + TaskPort + EnvPort + ConfigPort {}
```

---

## 6.4 pitchfork 复用策略

svcmgr 仅复用 pitchfork 的核心进程管理模块：

| pitchfork 模块 | 复用状态 | 说明 |
|---------------|---------|------|
| `supervisor` | ✅ 复用 | 进程监督管理 |
| `daemon` | ✅ 复用 | 守护进程化 |
| `procs` | ✅ 复用 | 进程操作封装 |
| `web` | ❌ 不复用 | 独立实现 axum Web 层（详见 05-web-service.md §1.2） |

**不复用 Web 模块的原因**：
- API 文档覆盖率低（28.61%），稳定性未知
- svcmgr 需要高度定制的功能（反向代理、Git 版本化）
- 避免 Web 层的高耦合风险

**依赖配置示例**：
```toml
# Cargo.toml
[dependencies]
# 仅引入 pitchfork 核心模块
pitchfork-cli = { version = "1.6", default-features = false, features = ["supervisor", "daemon", "procs"] }

# 独立实现 Web 层
axum = { version = "0.7", features = ["ws"] }
hyper = { version = "1.0", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "trace"] }
```

**参考文档**：`docs/DESIGN_WEB_FRAMEWORK.md`

## 7. 优雅降级机制

```rust
async fn get_env_with_fallback(&self) -> Result<HashMap<String, String>> {
    // 优先：结构化 JSON 输出
    match self.get_env_json().await {
        Ok(env) => return Ok(env),
        Err(e) => {
            tracing::warn!("mise env --json failed, falling back: {e}");
        }
    }
    
    // 降级：文本输出解析
    match self.get_env_text().await {
        Ok(env) => return Ok(env),
        Err(e) => {
            tracing::warn!("mise env text parse failed, falling back: {e}");
        }
    }
    
    // 最终降级：直接解析配置文件中的 [env] 段
    self.parse_env_from_config().await
}
```

## 8. 配置格式适配

```rust
/// 版本化配置解析
pub fn parse_tasks(value: &toml::Value, version: &MiseVersion) -> Result<HashMap<String, Task>> {
    if version >= &MiseVersion::new(2026, 2, 0) {
        // 新格式：嵌套结构
        parse_tasks_nested(value)
    } else {
        // 旧格式：平铺结构
        parse_tasks_flat(value)
    }
}
```

## 9. 测试策略

### 9.1 单元测试（MockAdapter）

```rust
pub struct MockMiseAdapter {
    tools: HashMap<String, String>,
    env: HashMap<String, String>,
    tasks: HashMap<String, TaskCommand>,
}

impl DependencyPort for MockMiseAdapter { /* ... */ }
impl TaskPort for MockMiseAdapter { /* ... */ }
impl EnvPort for MockMiseAdapter { /* ... */ }
impl MiseAdapter for MockMiseAdapter {}
```

### 9.2 CI 多版本矩阵

```yaml
strategy:
  matrix:
    mise-version:
      - "latest"
      - "2026.2.0"
      - "2025.12.0"
```

### 9.3 契约测试

```rust
#[test]
fn test_mise_env_json_contract() {
    let output = Command::new("mise")
        .arg("env")
        .arg("--json")
        .output()
        .unwrap();
    
    let env: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(env.is_object(), "mise env --json should return JSON object");
}
```

## 10. 目录结构

```
src/
├── ports/                    # Port 接口定义
│   ├── mod.rs
│   ├── dependency.rs
│   ├── task.rs
│   ├── env.rs
│   └── config.rs
├── adapters/                 # Adapter 实现
│   ├── mod.rs
│   ├── mise/
│   │   ├── mod.rs            # AdapterFactory + MiseVersion
│   │   ├── command.rs        # MiseCommand 构造器
│   │   ├── v2026.rs          # 2026.x 适配器
│   │   ├── v2025.rs          # 2025.x 适配器
│   │   └── parser.rs         # 版本化配置解析
│   └── mock.rs               # Mock 适配器
```

## 参考

- [00-architecture-overview.md](./00-architecture-overview.md) - 整体架构
- [01-config-design.md](./01-config-design.md) - 配置文件设计
- [MISE_REDESIGN_RESEARCH_ZH.md](../../MISE_REDESIGN_RESEARCH_ZH.md) - 完整设计文档（§6 mise 解耦架构）
