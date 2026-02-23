# 06 - 功能开关（Feature Flags）

> 版本：2.0.0-draft
> 状态：设计中

## 1. 设计目标

### 1.1 为什么需要功能开关

**核心问题**：
- **平台差异**：cgroups v2 仅在 Linux 可用，macOS/Windows 需要优雅降级
- **可选依赖**：Git 配置版本化、内置代理等功能可能不是所有场景都需要
- **渐进式迁移**：从旧架构迁移到新架构时，允许逐步启用新功能
- **性能权衡**：某些功能（如详细日志、metrics 收集）有性能开销，应可选

**设计原则**：
1. **默认安全**：功能开关默认值应为最保守、最兼容的选项
2. **运行时检测**：自动检测系统能力，避免手动配置
3. **优雅降级**：功能不可用时回退到备选方案，而非崩溃
4. **显式配置优先**：用户显式配置优先于自动检测
5. **最小侵入**：功能开关不应污染核心业务逻辑

---

## 2. 功能开关清单

### 2.1 核心功能开关

| 功能 | 配置键 | 默认值 | 说明 |
|------|--------|--------|------|
| **cgroups 资源限制** | `features.cgroups` | `auto` | 自动检测 cgroups v2 可用性 |
| **Git 配置版本化** | `features.git_versioning` | `true` | 配置文件自动提交到 Git |
| **内置 HTTP 代理** | `features.builtin_proxy` | `true` | 使用内置代理或外部 nginx |
| **事件总线** | `features.event_bus` | `true` | 启用事件驱动通知机制 |
| **健康检查** | `features.health_checks` | `true` | 启用进程健康检查 |
| **Metrics 收集** | `features.metrics` | `false` | 启用 Prometheus metrics |
| **详细日志** | `features.verbose_logging` | `false` | 启用 TRACE 级别日志 |

### 2.2 配置格式

```toml
# .config/mise/svcmgr/config.toml

[features]
# cgroups 资源限制：auto（自动检测）、true（强制启用）、false（禁用）
cgroups = "auto"

# Git 配置版本化
git_versioning = true

# 内置 HTTP 代理
builtin_proxy = true

# 事件总线
event_bus = true

# 健康检查
health_checks = true

# Prometheus metrics（默认禁用，启用后会占用一个端口）
metrics = false
metrics_port = 9090

# 详细日志（默认禁用，性能开销较大）
verbose_logging = false
```

### 2.3 环境变量覆盖

所有功能开关都可以通过环境变量覆盖配置文件：

```bash
# 强制启用 cgroups
SVCMGR_FEATURE_CGROUPS=true svcmgr run

# 禁用 Git 版本化（快速测试场景）
SVCMGR_FEATURE_GIT_VERSIONING=false svcmgr run

# 启用详细日志
SVCMGR_FEATURE_VERBOSE_LOGGING=true svcmgr run
```

**优先级**：环境变量 > 配置文件 > 默认值

---

## 3. 实现设计

### 3.1 功能开关管理器

```rust
// src/features/mod.rs

use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// 功能开关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// cgroups 资源限制：Auto（自动检测）、Enabled、Disabled
    #[serde(default = "default_cgroups")]
    pub cgroups: CgroupsMode,

    /// Git 配置版本化
    #[serde(default = "default_true")]
    pub git_versioning: bool,

    /// 内置 HTTP 代理
    #[serde(default = "default_true")]
    pub builtin_proxy: bool,

    /// 事件总线
    #[serde(default = "default_true")]
    pub event_bus: bool,

    /// 健康检查
    #[serde(default = "default_true")]
    pub health_checks: bool,

    /// Prometheus metrics
    #[serde(default)]
    pub metrics: bool,

    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    /// 详细日志
    #[serde(default)]
    pub verbose_logging: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CgroupsMode {
    Auto,    // 自动检测
    Enabled, // 强制启用（检测失败则报错）
    Disabled, // 完全禁用
}

fn default_cgroups() -> CgroupsMode { CgroupsMode::Auto }
fn default_true() -> bool { true }
fn default_metrics_port() -> u16 { 9090 }

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            cgroups: CgroupsMode::Auto,
            git_versioning: true,
            builtin_proxy: true,
            event_bus: true,
            health_checks: true,
            metrics: false,
            metrics_port: 9090,
            verbose_logging: false,
        }
    }
}

impl FeatureFlags {
    /// 从配置文件和环境变量加载功能开关
    pub fn load(config: Option<FeatureFlags>) -> Result<Self> {
        let mut flags = config.unwrap_or_default();

        // 环境变量覆盖配置文件
        if let Ok(val) = std::env::var("SVCMGR_FEATURE_CGROUPS") {
            flags.cgroups = match val.to_lowercase().as_str() {
                "true" | "enabled" => CgroupsMode::Enabled,
                "false" | "disabled" => CgroupsMode::Disabled,
                "auto" => CgroupsMode::Auto,
                _ => return Err(Error::InvalidFeatureFlag("cgroups", val)),
            };
        }

        if let Ok(val) = std::env::var("SVCMGR_FEATURE_GIT_VERSIONING") {
            flags.git_versioning = parse_bool(&val)?;
        }

        if let Ok(val) = std::env::var("SVCMGR_FEATURE_BUILTIN_PROXY") {
            flags.builtin_proxy = parse_bool(&val)?;
        }

        if let Ok(val) = std::env::var("SVCMGR_FEATURE_METRICS") {
            flags.metrics = parse_bool(&val)?;
        }

        if let Ok(val) = std::env::var("SVCMGR_FEATURE_VERBOSE_LOGGING") {
            flags.verbose_logging = parse_bool(&val)?;
        }

        Ok(flags)
    }

    /// 自动检测系统能力，解析 Auto 模式
    pub async fn resolve_auto_detection(mut self) -> Self {
        // 自动检测 cgroups v2 可用性
        if self.cgroups == CgroupsMode::Auto {
            self.cgroups = if Self::detect_cgroups_v2() {
                tracing::info!("检测到 cgroups v2，已启用资源限制功能");
                CgroupsMode::Enabled
            } else {
                tracing::warn!("未检测到 cgroups v2，资源限制功能已禁用");
                CgroupsMode::Disabled
            };
        }

        self
    }

    /// 检测 cgroups v2 是否可用
    fn detect_cgroups_v2() -> bool {
        #[cfg(target_os = "linux")]
        {
            // 检查 /sys/fs/cgroup/cgroup.controllers 是否存在（cgroups v2 标志）
            std::fs::metadata("/sys/fs/cgroup/cgroup.controllers").is_ok()
        }

        #[cfg(not(target_os = "linux"))]
        {
            false // 非 Linux 系统不支持 cgroups
        }
    }

    /// 验证功能开关一致性
    pub fn validate(&self) -> Result<()> {
        // 强制启用 cgroups 但系统不支持
        if self.cgroups == CgroupsMode::Enabled && !Self::detect_cgroups_v2() {
            return Err(Error::CgroupsNotAvailable);
        }

        // metrics 启用时检查端口冲突
        if self.metrics && self.metrics_port == 0 {
            return Err(Error::InvalidMetricsPort);
        }

        Ok(())
    }
}

fn parse_bool(s: &str) -> Result<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(Error::InvalidBoolValue(s.to_string())),
    }
}
```

### 3.2 运行时特性检测

```rust
// src/features/detection.rs

use std::sync::OnceLock;

/// 全局特性检测结果（单例，延迟初始化）
static SYSTEM_CAPABILITIES: OnceLock<SystemCapabilities> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct SystemCapabilities {
    /// cgroups v2 可用性
    pub cgroups_v2: bool,
    /// Git 可执行文件是否存在
    pub git_available: bool,
    /// 当前用户是否有 sudo 权限
    pub has_sudo: bool,
}

impl SystemCapabilities {
    /// 初始化系统能力检测（启动时调用一次）
    pub fn init() -> &'static Self {
        SYSTEM_CAPABILITIES.get_or_init(|| {
            Self {
                cgroups_v2: Self::check_cgroups_v2(),
                git_available: Self::check_git(),
                has_sudo: Self::check_sudo(),
            }
        })
    }

    /// 获取全局单例
    pub fn get() -> &'static Self {
        SYSTEM_CAPABILITIES.get().expect("SystemCapabilities not initialized")
    }

    fn check_cgroups_v2() -> bool {
        #[cfg(target_os = "linux")]
        {
            std::fs::metadata("/sys/fs/cgroup/cgroup.controllers").is_ok()
        }

        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    fn check_git() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_sudo() -> bool {
        std::process::Command::new("sudo")
            .arg("-n")
            .arg("true")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
```

### 3.3 功能开关集成到核心组件

#### 3.3.1 进程管理器（cgroups 可选）

```rust
// src/process/manager.rs

use crate::features::FeatureFlags;

pub struct ProcessManager {
    flags: Arc<FeatureFlags>,
    // ... 其他字段
}

impl ProcessManager {
    pub async fn spawn(&self, service: &ServiceConfig) -> Result<ProcessHandle> {
        let mut cmd = self.build_command(service)?;

        // 仅在 cgroups 启用时应用资源限制
        if self.flags.cgroups == CgroupsMode::Enabled {
            self.apply_cgroups_limits(&mut cmd, &service.resources)?;
        } else {
            tracing::debug!("cgroups 已禁用，跳过资源限制");
        }

        let child = cmd.spawn()?;
        Ok(ProcessHandle::new(child, service.clone()))
    }

    #[cfg(target_os = "linux")]
    fn apply_cgroups_limits(&self, cmd: &mut Command, resources: &ResourceLimits) -> Result<()> {
        // cgroups v2 资源限制实现
        // ...
    }

    #[cfg(not(target_os = "linux"))]
    fn apply_cgroups_limits(&self, _cmd: &mut Command, _resources: &ResourceLimits) -> Result<()> {
        // 非 Linux 系统，直接返回
        Ok(())
    }
}
```

#### 3.3.2 配置管理器（Git 版本化可选）

```rust
// src/config/manager.rs

use crate::features::FeatureFlags;

pub struct ConfigManager {
    flags: Arc<FeatureFlags>,
    git_repo: Option<GitRepository>,
    // ... 其他字段
}

impl ConfigManager {
    pub fn new(flags: Arc<FeatureFlags>, config_dir: PathBuf) -> Result<Self> {
        let git_repo = if flags.git_versioning {
            // 初始化或打开 Git 仓库
            Some(GitRepository::open_or_init(&config_dir)?)
        } else {
            tracing::info!("Git 版本化已禁用");
            None
        };

        Ok(Self { flags, git_repo, /* ... */ })
    }

    pub async fn save_config(&mut self, config: &Config) -> Result<()> {
        // 保存配置文件到磁盘
        self.write_to_disk(config).await?;

        // 如果启用了 Git 版本化，自动提交
        if let Some(repo) = &self.git_repo {
            repo.stage_and_commit("Auto-commit: config updated")?;
        }

        Ok(())
    }

    pub async fn rollback(&mut self, commit_id: &str) -> Result<()> {
        if let Some(repo) = &self.git_repo {
            repo.rollback(commit_id)?;
            self.reload_from_disk().await?;
            Ok(())
        } else {
            Err(Error::GitVersioningDisabled)
        }
    }
}
```

#### 3.3.3 Web 服务（内置代理可选）

```rust
// src/web/mod.rs

use crate::features::FeatureFlags;

pub struct WebService {
    flags: Arc<FeatureFlags>,
    proxy_server: Option<ProxyServer>,
    // ... 其他字段
}

impl WebService {
    pub async fn start(flags: Arc<FeatureFlags>, config: WebConfig) -> Result<Self> {
        let proxy_server = if flags.builtin_proxy {
            tracing::info!("启用内置 HTTP 代理");
            Some(ProxyServer::start(config.proxy_config).await?)
        } else {
            tracing::info!("内置代理已禁用，需手动配置外部 nginx");
            None
        };

        Ok(Self { flags, proxy_server, /* ... */ })
    }

    pub async fn update_backend(&self, service: &str, port: &str, addr: Option<SocketAddr>) {
        if let Some(proxy) = &self.proxy_server {
            proxy.update_backend(service, port, addr).await;
        } else {
            tracing::debug!("内置代理未启用，跳过后端更新");
        }
    }
}
```

#### 3.3.4 健康检查（可选）

```rust
// src/process/health_check.rs

use crate::features::FeatureFlags;

pub struct HealthChecker {
    flags: Arc<FeatureFlags>,
    // ... 其他字段
}

impl HealthChecker {
    pub fn new(flags: Arc<FeatureFlags>) -> Self {
        Self { flags, /* ... */ }
    }

    pub async fn start_checking(&self, service: &ServiceConfig) {
        if !self.flags.health_checks {
            tracing::debug!("健康检查已禁用");
            return;
        }

        if let Some(health_config) = &service.health_check {
            self.spawn_check_loop(service.name.clone(), health_config.clone()).await;
        }
    }
}
```

#### 3.3.5 Metrics 收集（可选）

```rust
// src/metrics/mod.rs

use crate::features::FeatureFlags;
use prometheus::{Registry, Encoder, TextEncoder};

pub struct MetricsServer {
    registry: Registry,
    port: u16,
}

impl MetricsServer {
    pub async fn start_if_enabled(flags: &FeatureFlags) -> Result<Option<Self>> {
        if !flags.metrics {
            tracing::info!("Metrics 收集已禁用");
            return Ok(None);
        }

        let registry = Registry::new();
        let port = flags.metrics_port;

        // 启动 HTTP 服务器暴露 /metrics 端点
        let server = Self { registry, port };
        tokio::spawn(server.clone().serve());

        tracing::info!("Metrics 服务器已启动: http://0.0.0.0:{}/metrics", port);
        Ok(Some(server))
    }

    async fn serve(self) {
        let app = axum::Router::new()
            .route("/metrics", get(|| async move {
                let encoder = TextEncoder::new();
                let metric_families = self.registry.gather();
                let mut buffer = Vec::new();
                encoder.encode(&metric_families, &mut buffer).unwrap();
                String::from_utf8(buffer).unwrap()
            }));

        axum::Server::bind(&format!("0.0.0.0:{}", self.port).parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    }
}
```

---

## 4. 优雅降级策略

### 4.1 cgroups 不可用时的降级

```rust
// src/process/resources.rs

impl ProcessManager {
    fn apply_resource_limits(&self, cmd: &mut Command, limits: &ResourceLimits) -> Result<()> {
        match self.flags.cgroups {
            CgroupsMode::Enabled => {
                // 尝试应用 cgroups 限制
                self.apply_cgroups_limits(cmd, limits)?;
            },
            CgroupsMode::Disabled => {
                // 使用 /proc 监控（只读，无法限制）
                tracing::warn!("资源限制已禁用，仅通过 /proc 监控资源使用");
                // 记录配置的限制值，但不强制执行
                tracing::info!("配置的资源限制: {:?}", limits);
            },
            CgroupsMode::Auto => unreachable!("Auto 应该在启动时已解析"),
        }

        Ok(())
    }
}
```

### 4.2 Git 不可用时的降级

```rust
impl ConfigManager {
    pub fn new(flags: Arc<FeatureFlags>, config_dir: PathBuf) -> Result<Self> {
        let git_repo = if flags.git_versioning {
            match GitRepository::open_or_init(&config_dir) {
                Ok(repo) => Some(repo),
                Err(e) => {
                    tracing::error!("Git 初始化失败: {}, 配置版本化已禁用", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self { flags, git_repo, /* ... */ })
    }

    pub async fn save_config(&mut self, config: &Config) -> Result<()> {
        self.write_to_disk(config).await?;

        // Git 提交失败不影响配置保存
        if let Some(repo) = &self.git_repo {
            if let Err(e) = repo.stage_and_commit("Auto-commit: config updated") {
                tracing::error!("Git 提交失败: {}, 配置已保存但未版本化", e);
            }
        }

        Ok(())
    }
}
```

### 4.3 内置代理不可用时的提示

```rust
impl WebService {
    pub async fn start(flags: Arc<FeatureFlags>, config: WebConfig) -> Result<Self> {
        let proxy_server = if flags.builtin_proxy {
            match ProxyServer::start(config.proxy_config).await {
                Ok(server) => Some(server),
                Err(e) => {
                    tracing::error!("内置代理启动失败: {}", e);
                    tracing::warn!("请手动配置外部 nginx 作为备选方案");
                    None
                }
            }
        } else {
            tracing::info!("内置代理已禁用");
            tracing::info!("请确保外部 nginx 已正确配置");
            None
        };

        Ok(Self { flags, proxy_server, /* ... */ })
    }
}
```

---

## 5. 配置示例

### 5.1 生产环境（完整功能）

```toml
# .config/mise/svcmgr/config.toml

[features]
cgroups = "auto"           # 自动检测 cgroups v2
git_versioning = true      # 启用配置版本化
builtin_proxy = true       # 使用内置代理
event_bus = true           # 启用事件总线
health_checks = true       # 启用健康检查
metrics = true             # 启用 Prometheus metrics
metrics_port = 9090
verbose_logging = false    # 不启用详细日志（性能考虑）
```

### 5.2 开发环境（快速迭代）

```toml
[features]
cgroups = false            # 禁用 cgroups（开发机可能不支持）
git_versioning = false     # 禁用 Git 版本化（避免频繁提交）
builtin_proxy = true       # 保持内置代理
health_checks = false      # 禁用健康检查（减少干扰）
metrics = false            # 禁用 metrics
verbose_logging = true     # 启用详细日志（调试）
```

### 5.3 Docker 容器（最小化）

```toml
[features]
cgroups = false            # Docker 容器内通常由外部 cgroups 管理
git_versioning = false     # 容器内不需要 Git
builtin_proxy = true       # 使用内置代理
health_checks = true       # 保留健康检查
metrics = false            # metrics 由外部收集
verbose_logging = false
```

### 5.4 CI/CD 测试环境

```toml
[features]
cgroups = false            # CI 环境通常不支持 cgroups
git_versioning = false     # 测试环境不需要版本化
builtin_proxy = true
health_checks = true
metrics = false
verbose_logging = true     # 测试失败时需要详细日志
```

---

## 6. 启动流程集成

### 6.1 启动时的功能开关初始化

```rust
// src/main.rs

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 加载配置文件
    let config = Config::load_from_file(".config/mise/svcmgr/config.toml")?;

    // 2. 加载功能开关（配置文件 + 环境变量）
    let mut flags = FeatureFlags::load(config.features)?;

    // 3. 运行时自动检测（解析 Auto 模式）
    flags = flags.resolve_auto_detection().await;

    // 4. 验证功能开关一致性
    flags.validate()?;

    // 5. 初始化日志级别（根据 verbose_logging）
    init_logging(&flags);

    // 6. 打印启用的功能
    print_enabled_features(&flags);

    // 7. 初始化系统能力检测
    SystemCapabilities::init();

    // 8. 启动核心服务（传递功能开关）
    let scheduler = SchedulerEngine::new(Arc::new(flags), config).await?;
    scheduler.start().await?;

    Ok(())
}

fn init_logging(flags: &FeatureFlags) {
    let level = if flags.verbose_logging {
        tracing::Level::TRACE
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(level)
        .init();
}

fn print_enabled_features(flags: &FeatureFlags) {
    tracing::info!("功能开关状态:");
    tracing::info!("  - cgroups: {:?}", flags.cgroups);
    tracing::info!("  - git_versioning: {}", flags.git_versioning);
    tracing::info!("  - builtin_proxy: {}", flags.builtin_proxy);
    tracing::info!("  - health_checks: {}", flags.health_checks);
    tracing::info!("  - metrics: {}", flags.metrics);
    tracing::info!("  - verbose_logging: {}", flags.verbose_logging);
}
```

---

## 7. 测试策略

### 7.1 功能开关矩阵测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_all_features_enabled() {
        let flags = FeatureFlags {
            cgroups: CgroupsMode::Enabled,
            git_versioning: true,
            builtin_proxy: true,
            event_bus: true,
            health_checks: true,
            metrics: true,
            metrics_port: 9090,
            verbose_logging: true,
        };

        // 验证所有功能都能正常初始化
        // ...
    }

    #[tokio::test]
    async fn test_all_features_disabled() {
        let flags = FeatureFlags {
            cgroups: CgroupsMode::Disabled,
            git_versioning: false,
            builtin_proxy: false,
            event_bus: false,
            health_checks: false,
            metrics: false,
            metrics_port: 9090,
            verbose_logging: false,
        };

        // 验证最小化配置能正常运行
        // ...
    }

    #[test]
    fn test_env_override() {
        std::env::set_var("SVCMGR_FEATURE_CGROUPS", "false");
        
        let config_flags = FeatureFlags {
            cgroups: CgroupsMode::Enabled,
            ..Default::default()
        };

        let flags = FeatureFlags::load(Some(config_flags)).unwrap();
        assert_eq!(flags.cgroups, CgroupsMode::Disabled); // 环境变量覆盖配置文件
    }

    #[test]
    fn test_auto_detection_linux() {
        #[cfg(target_os = "linux")]
        {
            let flags = FeatureFlags {
                cgroups: CgroupsMode::Auto,
                ..Default::default()
            };

            let resolved = flags.resolve_auto_detection().await;
            // 在 Linux 上应该检测到 cgroups v2 或降级为 Disabled
            assert!(resolved.cgroups != CgroupsMode::Auto);
        }
    }

    #[test]
    fn test_auto_detection_non_linux() {
        #[cfg(not(target_os = "linux"))]
        {
            let flags = FeatureFlags {
                cgroups: CgroupsMode::Auto,
                ..Default::default()
            };

            let resolved = flags.resolve_auto_detection().await;
            assert_eq!(resolved.cgroups, CgroupsMode::Disabled); // 非 Linux 应该禁用
        }
    }
}
```

### 7.2 CI 测试矩阵

```yaml
# .github/workflows/test.yml

name: Test Feature Flags

on: [push, pull_request]

jobs:
  test-feature-combinations:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
        feature_profile:
          - name: "full"
            cgroups: "auto"
            git: "true"
            proxy: "true"
          - name: "minimal"
            cgroups: "false"
            git: "false"
            proxy: "false"
          - name: "no-cgroups"
            cgroups: "false"
            git: "true"
            proxy: "true"
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Run tests
        env:
          SVCMGR_FEATURE_CGROUPS: ${{ matrix.feature_profile.cgroups }}
          SVCMGR_FEATURE_GIT_VERSIONING: ${{ matrix.feature_profile.git }}
          SVCMGR_FEATURE_BUILTIN_PROXY: ${{ matrix.feature_profile.proxy }}
        run: cargo test --verbose
```

---

## 8. 安全考虑

### 8.1 强制启用验证

```rust
impl FeatureFlags {
    pub fn validate(&self) -> Result<()> {
        // 用户强制启用 cgroups，但系统不支持 → 拒绝启动
        if self.cgroups == CgroupsMode::Enabled {
            if !SystemCapabilities::get().cgroups_v2 {
                return Err(Error::CgroupsForceEnabledButUnavailable);
            }
        }

        // Git 版本化启用，但 Git 不可用 → 警告但不阻止启动
        if self.git_versioning && !SystemCapabilities::get().git_available {
            tracing::warn!("Git 版本化已启用但 Git 不可用，将自动禁用");
            // 运行时自动降级
        }

        Ok(())
    }
}
```

### 8.2 敏感功能默认禁用

```rust
impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            // 性能开销大的功能默认禁用
            metrics: false,
            verbose_logging: false,
            
            // 可选但推荐的功能默认启用
            git_versioning: true,
            health_checks: true,
            
            // 自动检测的功能使用 Auto
            cgroups: CgroupsMode::Auto,
            
            // ... 其他默认值
        }
    }
}
```

---

## 9. 文档与用户指南

### 9.1 功能开关决策树

```
用户想使用资源限制（CPU/内存）？
├─ 是 → cgroups = "auto"（推荐）或 "true"（强制）
└─ 否 → cgroups = false

用户需要配置回滚能力？
├─ 是 → git_versioning = true（需要 Git 可执行文件）
└─ 否 → git_versioning = false

用户需要 HTTP 反向代理？
├─ 是，且希望零配置 → builtin_proxy = true
├─ 是，但想用自己的 nginx → builtin_proxy = false
└─ 否（纯后台任务） → builtin_proxy = false

用户需要监控 metrics？
├─ 是 → metrics = true, metrics_port = 9090
└─ 否 → metrics = false
```

### 9.2 故障排查指南

| 问题 | 可能原因 | 解决方案 |
|------|----------|----------|
| cgroups 限制不生效 | 系统不支持 cgroups v2 | 检查 `dmesg \| grep cgroup`，升级内核或禁用 cgroups |
| 配置无法回滚 | Git 版本化被禁用 | 设置 `git_versioning = true` 并重启 |
| HTTP 代理无响应 | 内置代理启动失败 | 检查日志，考虑改用外部 nginx |
| Metrics 端点 404 | Metrics 功能未启用 | 设置 `metrics = true` 并重启 |

---

## 10. 相关规范

- **03-process-manager.md** - cgroups 资源限制实现
- **04-git-versioning.md** - Git 配置版本化实现
- **05-web-service.md** - 内置 HTTP 代理实现
- **07-mise-integration.md** - mise 集成层的优雅降级
- **20-implementation-phases.md** - 功能开关在实施阶段的使用
