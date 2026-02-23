# 03 - 子进程管理和资源限制

> **规格编号**: 03  
> **依赖**: [00-架构总览](./00-architecture-overview.md), [01-配置设计](./01-config-design.md), [02-调度引擎](./02-scheduler-engine.md)  
> **相关**: [06-功能特性标志](./06-feature-flags.md), [11-服务管理 API](./11-api-services.md)

---

## 1. 概述

### 1.1 目标

子进程管理模块负责：
- **进程生命周期管理**：启动、停止、重启、监控子进程
- **资源限制**：CPU、内存、文件描述符等资源的限制和监控
- **进程隔离**：使用 cgroups v2 实现资源隔离（可选功能）
- **日志管理**：捕获 stdout/stderr，支持日志轮转
- **健康检查**：进程存活性检查和自动重启策略

### 1.2 设计原则

- **安全第一**：默认资源限制，防止资源耗尽
- **渐进增强**：基础功能不依赖 cgroups，cgroups 作为可选增强
- **透明监控**：所有进程状态和资源使用可观测
- **优雅降级**：cgroups 不可用时自动回退到基础监控

---

## 2. 进程生命周期管理

### 2.1 进程启动

#### 启动流程

```rust
pub struct ProcessManager {
    processes: HashMap<String, ManagedProcess>,
    cgroup_manager: Option<CgroupManager>, // 可选的 cgroups 管理器
}

pub struct ProcessConfig {
    pub command: Vec<String>,           // 命令和参数
    pub working_dir: Option<PathBuf>,   // 工作目录
    pub env: HashMap<String, String>,   // 环境变量
    pub user: Option<String>,           // 运行用户（需要 root 权限）
    pub limits: ResourceLimits,         // 资源限制
    pub restart_policy: RestartPolicy,  // 重启策略
    pub health_check: Option<HealthCheck>, // 健康检查
    pub log_config: LogConfig,          // 日志配置
}

impl ProcessManager {
    pub async fn start_process(
        &mut self,
        name: &str,
        config: ProcessConfig,
    ) -> Result<Pid, ProcessError> {
        // 1. 应用资源限制（如果启用 cgroups）
        let cgroup_path = if let Some(ref manager) = self.cgroup_manager {
            Some(manager.create_cgroup(name, &config.limits).await?)
        } else {
            None
        };

        // 2. 构造命令
        let mut cmd = tokio::process::Command::new(&config.command[0]);
        cmd.args(&config.command[1..])
            .envs(config.env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(dir) = config.working_dir {
            cmd.current_dir(dir);
        }

        // 3. 启动进程
        let mut child = cmd.spawn()?;
        let pid = child.id().ok_or(ProcessError::NoPid)?;

        // 4. 如果启用 cgroups，将进程加入 cgroup
        if let Some(ref cgroup) = cgroup_path {
            self.cgroup_manager.as_ref().unwrap()
                .add_process(cgroup, pid).await?;
        }

        // 5. 启动日志捕获
        let log_handle = LogCapture::new(
            child.stdout.take().unwrap(),
            child.stderr.take().unwrap(),
            config.log_config,
        ).spawn();

        // 6. 注册到管理器
        let managed = ManagedProcess {
            pid,
            child,
            config,
            cgroup_path,
            log_handle,
            start_time: Instant::now(),
            restart_count: 0,
            state: ProcessState::Running,
        };

        self.processes.insert(name.to_string(), managed);

        Ok(pid)
    }
}
```

#### 环境变量继承

```rust
// 从 mise 获取环境变量
let mise_env = self.mise_adapter.get_env().await?;

// 合并用户配置的环境变量
let mut final_env = mise_env;
final_env.extend(config.env);

cmd.envs(final_env);
```

### 2.2 进程停止

#### 优雅停止流程

```rust
pub struct ShutdownConfig {
    pub signal: Signal,           // 初始信号（默认 SIGTERM）
    pub timeout: Duration,        // 等待超时（默认 30s）
    pub force_signal: Signal,     // 强制信号（默认 SIGKILL）
}

impl ProcessManager {
    pub async fn stop_process(
        &mut self,
        name: &str,
        config: ShutdownConfig,
    ) -> Result<(), ProcessError> {
        let process = self.processes.get_mut(name)
            .ok_or(ProcessError::NotFound)?;

        // 1. 发送初始信号
        process.send_signal(config.signal)?;
        process.state = ProcessState::Stopping;

        // 2. 等待进程退出
        tokio::select! {
            result = process.child.wait() => {
                // 进程正常退出
                let exit_status = result?;
                self.cleanup_process(name, exit_status).await?;
                Ok(())
            }
            _ = tokio::time::sleep(config.timeout) => {
                // 超时，发送强制信号
                tracing::warn!(
                    "Process {} did not stop within timeout, sending {}",
                    name, config.force_signal
                );
                process.send_signal(config.force_signal)?;
                
                // 再等待 5 秒
                tokio::time::timeout(
                    Duration::from_secs(5),
                    process.child.wait()
                ).await??;
                
                self.cleanup_process(name, ExitStatus::from_raw(137)).await?;
                Ok(())
            }
        }
    }

    async fn cleanup_process(
        &mut self,
        name: &str,
        exit_status: ExitStatus,
    ) -> Result<(), ProcessError> {
        if let Some(mut process) = self.processes.remove(name) {
            // 1. 清理 cgroup
            if let Some(ref cgroup) = process.cgroup_path {
                if let Some(ref manager) = self.cgroup_manager {
                    manager.remove_cgroup(cgroup).await?;
                }
            }

            // 2. 停止日志捕获
            process.log_handle.stop().await?;

            // 3. 记录退出状态
            tracing::info!(
                "Process {} exited with status {}",
                name, exit_status
            );
        }

        Ok(())
    }
}
```

### 2.3 进程重启

#### 重启策略

```rust
#[derive(Debug, Clone)]
pub enum RestartPolicy {
    No,                          // 不重启
    Always,                      // 总是重启
    OnFailure {                  // 失败时重启
        max_retries: u32,        // 最大重试次数
        backoff: BackoffStrategy, // 退避策略
    },
    UnlessStopped,               // 除非被手动停止，否则重启
}

#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Linear { interval: Duration },           // 线性退避
    Exponential { base: Duration, max: Duration }, // 指数退避
}

impl ProcessManager {
    pub async fn handle_process_exit(
        &mut self,
        name: &str,
        exit_status: ExitStatus,
    ) -> Result<(), ProcessError> {
        let process = self.processes.get(name)
            .ok_or(ProcessError::NotFound)?;

        let should_restart = match &process.config.restart_policy {
            RestartPolicy::No => false,
            RestartPolicy::Always => true,
            RestartPolicy::OnFailure { max_retries, .. } => {
                !exit_status.success() && process.restart_count < *max_retries
            }
            RestartPolicy::UnlessStopped => {
                process.state != ProcessState::Stopped
            }
        };

        if should_restart {
            // 计算退避时间
            let delay = self.calculate_backoff(process)?;
            
            tracing::info!(
                "Restarting process {} in {:?} (attempt {}/{})",
                name, delay, process.restart_count + 1,
                match &process.config.restart_policy {
                    RestartPolicy::OnFailure { max_retries, .. } => max_retries.to_string(),
                    _ => "∞".to_string(),
                }
            );

            tokio::time::sleep(delay).await;
            
            // 重启进程
            let config = process.config.clone();
            self.cleanup_process(name, exit_status).await?;
            self.start_process(name, config).await?;
            
            // 增加重启计数
            if let Some(process) = self.processes.get_mut(name) {
                process.restart_count += 1;
            }
        } else {
            tracing::info!("Process {} will not be restarted", name);
            self.cleanup_process(name, exit_status).await?;
        }

        Ok(())
    }

    fn calculate_backoff(&self, process: &ManagedProcess) -> Result<Duration, ProcessError> {
        match &process.config.restart_policy {
            RestartPolicy::OnFailure { backoff, .. } => {
                match backoff {
                    BackoffStrategy::Linear { interval } => {
                        Ok(*interval * (process.restart_count + 1))
                    }
                    BackoffStrategy::Exponential { base, max } => {
                        let delay = *base * 2_u32.pow(process.restart_count);
                        Ok(delay.min(*max))
                    }
                }
            }
            _ => Ok(Duration::from_secs(1)), // 默认延迟
        }
    }
}
```

---

## 3. 资源限制

### 3.1 资源限制配置

```rust
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub cpu: Option<CpuLimit>,         // CPU 限制
    pub memory: Option<MemoryLimit>,   // 内存限制
    pub io: Option<IoLimit>,           // I/O 限制
    pub processes: Option<u64>,        // 进程数限制
    pub open_files: Option<u64>,       // 打开文件数限制
}

#[derive(Debug, Clone)]
pub struct CpuLimit {
    pub shares: u64,          // CPU 权重（100-10000）
    pub quota: Option<f64>,   // CPU 配额（核心数，如 0.5 = 50%）
}

#[derive(Debug, Clone)]
pub struct MemoryLimit {
    pub limit: u64,           // 内存限制（字节）
    pub swap: Option<u64>,    // swap 限制（字节）
    pub oom_kill: bool,       // 是否允许 OOM killer 杀死进程
}

#[derive(Debug, Clone)]
pub struct IoLimit {
    pub read_bps: Option<u64>,   // 读取速率限制（字节/秒）
    pub write_bps: Option<u64>,  // 写入速率限制（字节/秒）
    pub read_iops: Option<u64>,  // 读取 IOPS 限制
    pub write_iops: Option<u64>, // 写入 IOPS 限制
}
```

#### 配置示例

```toml
[services.database]
task = "start-database"

# 资源限制
[services.database.limits]
cpu_shares = 2048        # CPU 权重（默认 1024）
cpu_quota = 1.5          # 最多使用 1.5 个 CPU 核心
memory_limit = "2G"      # 内存限制 2GB
memory_swap = "4G"       # swap 限制 4GB
max_processes = 100      # 最多 100 个子进程
max_open_files = 1024    # 最多 1024 个打开文件

# I/O 限制（需要 feature = "cgroups"）
io_read_bps = "100M"     # 读取速率限制 100MB/s
io_write_bps = "50M"     # 写入速率限制 50MB/s
```

### 3.2 cgroups v2 集成

#### cgroups 管理器

```rust
pub struct CgroupManager {
    root_path: PathBuf,  // cgroup 根路径（通常是 /sys/fs/cgroup/svcmgr）
    enabled: bool,       // 是否启用
}

impl CgroupManager {
    pub fn new() -> Result<Self, CgroupError> {
        // 检查 cgroups v2 是否可用
        let cgroup_version = Self::detect_cgroup_version()?;
        if cgroup_version != 2 {
            return Err(CgroupError::UnsupportedVersion(cgroup_version));
        }

        // 检查是否有权限创建 cgroup
        let root_path = PathBuf::from("/sys/fs/cgroup/svcmgr");
        if !Self::can_create_cgroup(&root_path)? {
            tracing::warn!("Cannot create cgroups, running without resource limits");
            return Ok(Self {
                root_path,
                enabled: false,
            });
        }

        Ok(Self {
            root_path,
            enabled: true,
        })
    }

    pub async fn create_cgroup(
        &self,
        name: &str,
        limits: &ResourceLimits,
    ) -> Result<PathBuf, CgroupError> {
        if !self.enabled {
            return Err(CgroupError::Disabled);
        }

        let cgroup_path = self.root_path.join(name);
        tokio::fs::create_dir_all(&cgroup_path).await?;

        // 启用所需的控制器
        self.enable_controllers(&cgroup_path, limits).await?;

        // 应用 CPU 限制
        if let Some(ref cpu) = limits.cpu {
            self.apply_cpu_limits(&cgroup_path, cpu).await?;
        }

        // 应用内存限制
        if let Some(ref memory) = limits.memory {
            self.apply_memory_limits(&cgroup_path, memory).await?;
        }

        // 应用 I/O 限制
        if let Some(ref io) = limits.io {
            self.apply_io_limits(&cgroup_path, io).await?;
        }

        Ok(cgroup_path)
    }

    async fn apply_cpu_limits(
        &self,
        cgroup_path: &Path,
        cpu: &CpuLimit,
    ) -> Result<(), CgroupError> {
        // 设置 CPU 权重
        tokio::fs::write(
            cgroup_path.join("cpu.weight"),
            cpu.shares.to_string(),
        ).await?;

        // 设置 CPU 配额（如果指定）
        if let Some(quota) = cpu.quota {
            // cgroups v2 使用 cpu.max: "配额 周期"
            // 例如："50000 100000" = 50% CPU
            let period = 100_000; // 100ms
            let quota_us = (quota * period as f64) as u64;
            tokio::fs::write(
                cgroup_path.join("cpu.max"),
                format!("{} {}", quota_us, period),
            ).await?;
        }

        Ok(())
    }

    async fn apply_memory_limits(
        &self,
        cgroup_path: &Path,
        memory: &MemoryLimit,
    ) -> Result<(), CgroupError> {
        // 设置内存限制
        tokio::fs::write(
            cgroup_path.join("memory.max"),
            memory.limit.to_string(),
        ).await?;

        // 设置 swap 限制
        if let Some(swap) = memory.swap {
            tokio::fs::write(
                cgroup_path.join("memory.swap.max"),
                swap.to_string(),
            ).await?;
        }

        // 设置 OOM 行为
        if !memory.oom_kill {
            tokio::fs::write(
                cgroup_path.join("memory.oom.group"),
                "1",
            ).await?;
        }

        Ok(())
    }

    pub async fn add_process(
        &self,
        cgroup_path: &Path,
        pid: u32,
    ) -> Result<(), CgroupError> {
        tokio::fs::write(
            cgroup_path.join("cgroup.procs"),
            pid.to_string(),
        ).await?;
        Ok(())
    }

    pub async fn remove_cgroup(&self, cgroup_path: &Path) -> Result<(), CgroupError> {
        // cgroup 只有在没有进程时才能删除
        // 如果进程已经退出，这将成功
        tokio::fs::remove_dir(cgroup_path).await.ok();
        Ok(())
    }
}
```

### 3.3 资源监控

#### 监控数据结构

```rust
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu: CpuUsage,
    pub memory: MemoryUsage,
    pub io: IoUsage,
    pub processes: ProcessUsage,
}

#[derive(Debug, Clone)]
pub struct CpuUsage {
    pub usage_percent: f64,      // CPU 使用率（%）
    pub user_time: Duration,     // 用户态时间
    pub system_time: Duration,   // 内核态时间
}

#[derive(Debug, Clone)]
pub struct MemoryUsage {
    pub rss: u64,                // 常驻内存（RSS）
    pub vms: u64,                // 虚拟内存（VMS）
    pub swap: u64,               // swap 使用量
    pub cache: u64,              // 缓存使用量
}

#[derive(Debug, Clone)]
pub struct IoUsage {
    pub read_bytes: u64,         // 读取字节数
    pub write_bytes: u64,        // 写入字节数
    pub read_ops: u64,           // 读取操作数
    pub write_ops: u64,          // 写入操作数
}

#[derive(Debug, Clone)]
pub struct ProcessUsage {
    pub count: usize,            // 进程数
    pub threads: usize,          // 线程数
    pub open_files: usize,       // 打开文件数
}
```

#### 监控实现

```rust
impl ProcessManager {
    pub async fn get_resource_usage(
        &self,
        name: &str,
    ) -> Result<ResourceUsage, ProcessError> {
        let process = self.processes.get(name)
            .ok_or(ProcessError::NotFound)?;

        // 优先从 cgroups 读取（如果可用）
        if let Some(ref cgroup) = process.cgroup_path {
            if let Some(ref manager) = self.cgroup_manager {
                return manager.get_cgroup_usage(cgroup).await;
            }
        }

        // 回退到 /proc 读取
        self.get_proc_usage(process.pid).await
    }

    async fn get_proc_usage(&self, pid: u32) -> Result<ResourceUsage, ProcessError> {
        // 从 /proc/{pid}/stat 读取 CPU 使用
        let stat = tokio::fs::read_to_string(format!("/proc/{}/stat", pid)).await?;
        let cpu = Self::parse_cpu_usage(&stat)?;

        // 从 /proc/{pid}/status 读取内存使用
        let status = tokio::fs::read_to_string(format!("/proc/{}/status", pid)).await?;
        let memory = Self::parse_memory_usage(&status)?;

        // 从 /proc/{pid}/io 读取 I/O 使用
        let io_stat = tokio::fs::read_to_string(format!("/proc/{}/io", pid)).await?;
        let io = Self::parse_io_usage(&io_stat)?;

        // 统计进程和线程数
        let processes = Self::count_processes(pid).await?;

        Ok(ResourceUsage {
            cpu,
            memory,
            io,
            processes,
        })
    }
}
```

---

## 4. 健康检查

### 4.1 健康检查配置

```rust
#[derive(Debug, Clone)]
pub enum HealthCheck {
    Exec {
        command: Vec<String>,
        interval: Duration,
        timeout: Duration,
        retries: u32,
    },
    Http {
        url: String,
        interval: Duration,
        timeout: Duration,
        retries: u32,
        expected_status: u16,
    },
    Tcp {
        host: String,
        port: u16,
        interval: Duration,
        timeout: Duration,
        retries: u32,
    },
}
```

#### 配置示例

```toml
[services.web]
task = "start-web"

# 健康检查
[services.web.health_check]
type = "http"
url = "http://localhost:8080/health"
interval = "10s"        # 每 10 秒检查一次
timeout = "5s"          # 超时时间 5 秒
retries = 3             # 失败 3 次后认为不健康
expected_status = 200   # 期望的 HTTP 状态码
```

### 4.2 健康检查实现

```rust
pub struct HealthChecker {
    checks: HashMap<String, HealthCheckState>,
}

struct HealthCheckState {
    config: HealthCheck,
    last_check: Instant,
    consecutive_failures: u32,
    is_healthy: bool,
}

impl HealthChecker {
    pub async fn check_health(
        &mut self,
        name: &str,
    ) -> Result<bool, HealthCheckError> {
        let state = self.checks.get_mut(name)
            .ok_or(HealthCheckError::NotConfigured)?;

        // 检查是否到了检查时间
        let interval = match &state.config {
            HealthCheck::Exec { interval, .. } => *interval,
            HealthCheck::Http { interval, .. } => *interval,
            HealthCheck::Tcp { interval, .. } => *interval,
        };

        if state.last_check.elapsed() < interval {
            return Ok(state.is_healthy);
        }

        // 执行健康检查
        let result = match &state.config {
            HealthCheck::Exec { command, timeout, .. } => {
                self.check_exec(command, *timeout).await
            }
            HealthCheck::Http { url, timeout, expected_status, .. } => {
                self.check_http(url, *timeout, *expected_status).await
            }
            HealthCheck::Tcp { host, port, timeout, .. } => {
                self.check_tcp(host, *port, *timeout).await
            }
        };

        // 更新状态
        state.last_check = Instant::now();
        match result {
            Ok(true) => {
                state.consecutive_failures = 0;
                state.is_healthy = true;
            }
            Ok(false) | Err(_) => {
                state.consecutive_failures += 1;
                let max_retries = match &state.config {
                    HealthCheck::Exec { retries, .. } => *retries,
                    HealthCheck::Http { retries, .. } => *retries,
                    HealthCheck::Tcp { retries, .. } => *retries,
                };
                if state.consecutive_failures >= max_retries {
                    state.is_healthy = false;
                }
            }
        }

        Ok(state.is_healthy)
    }

    async fn check_http(
        &self,
        url: &str,
        timeout: Duration,
        expected_status: u16,
    ) -> Result<bool, HealthCheckError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()?;

        let response = client.get(url).send().await?;
        Ok(response.status().as_u16() == expected_status)
    }

    async fn check_tcp(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<bool, HealthCheckError> {
        let addr = format!("{}:{}", host, port);
        tokio::time::timeout(
            timeout,
            tokio::net::TcpStream::connect(addr)
        ).await.is_ok()
    }
}
```

---

## 5. 日志管理

### 5.1 日志捕获

```rust
pub struct LogConfig {
    pub stdout_path: Option<PathBuf>,   // stdout 日志路径
    pub stderr_path: Option<PathBuf>,   // stderr 日志路径
    pub rotation: LogRotation,          // 日志轮转配置
    pub level_filter: Option<String>,   // 日志级别过滤
}

#[derive(Debug, Clone)]
pub enum LogRotation {
    None,                               // 不轮转
    Size { max_size: u64 },             // 按大小轮转
    Time { interval: Duration },        // 按时间轮转
    Both { max_size: u64, interval: Duration }, // 两者都用
}

pub struct LogCapture {
    stdout_writer: Option<LogWriter>,
    stderr_writer: Option<LogWriter>,
}

impl LogCapture {
    pub fn new(
        stdout: impl AsyncRead + Unpin + Send + 'static,
        stderr: impl AsyncRead + Unpin + Send + 'static,
        config: LogConfig,
    ) -> Self {
        let stdout_writer = config.stdout_path.map(|path| {
            LogWriter::new(path, config.rotation.clone())
        });

        let stderr_writer = config.stderr_path.map(|path| {
            LogWriter::new(path, config.rotation.clone())
        });

        Self {
            stdout_writer,
            stderr_writer,
        }
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            // 捕获并写入日志
            // ...
        })
    }
}
```

#### 配置示例

```toml
[services.web]
task = "start-web"

# 日志配置
[services.web.logs]
stdout = ".svcmgr/logs/web.stdout.log"
stderr = ".svcmgr/logs/web.stderr.log"

# 日志轮转
[services.web.logs.rotation]
max_size = "100M"       # 最大 100MB
max_age = "7d"          # 保留 7 天
max_backups = 5         # 最多 5 个备份文件
compress = true         # 压缩旧日志
```

---

## 6. 与调度引擎的集成

### 6.1 任务执行接口

```rust
impl ProcessManager {
    pub async fn execute_task(
        &mut self,
        task: &ScheduledTask,
    ) -> Result<TaskExecution, ProcessError> {
        match &task.trigger {
            Trigger::OneShot { .. } | Trigger::Delayed { .. } | Trigger::Cron { .. } => {
                // 一次性任务：启动进程，等待完成
                let config = self.build_process_config(task)?;
                let pid = self.start_process(&task.name, config).await?;
                
                // 等待进程完成
                let exit_status = self.wait_for_completion(&task.name).await?;
                
                Ok(TaskExecution {
                    pid,
                    exit_status,
                    duration: Instant::now() - task.start_time,
                })
            }
            Trigger::Event { .. } => {
                // 长期运行的服务：启动进程，返回 PID
                let config = self.build_process_config(task)?;
                let pid = self.start_process(&task.name, config).await?;
                
                Ok(TaskExecution {
                    pid,
                    exit_status: None, // 服务不会立即退出
                    duration: Duration::ZERO,
                })
            }
        }
    }

    fn build_process_config(
        &self,
        task: &ScheduledTask,
    ) -> Result<ProcessConfig, ProcessError> {
        // 从任务定义构造进程配置
        // ...
    }
}
```

---

## 7. 错误处理

### 7.1 错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("Process not found: {0}")]
    NotFound(String),

    #[error("Failed to spawn process: {0}")]
    SpawnError(#[from] std::io::Error),

    #[error("No PID available")]
    NoPid,

    #[error("Cgroup error: {0}")]
    CgroupError(#[from] CgroupError),

    #[error("Process is already running")]
    AlreadyRunning,

    #[error("Process is not running")]
    NotRunning,

    #[error("Timeout waiting for process to stop")]
    StopTimeout,
}

#[derive(Debug, thiserror::Error)]
pub enum CgroupError {
    #[error("Cgroups disabled")]
    Disabled,

    #[error("Unsupported cgroup version: {0}")]
    UnsupportedVersion(u8),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## 8. 功能特性标志

### 8.1 cgroups 功能

```toml
[features]
cgroups = true      # 启用 cgroups v2 资源限制
```

#### 功能检测

```rust
pub fn check_cgroups_support() -> bool {
    // 检查 cgroups v2 是否挂载
    std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists()
}

pub fn init_process_manager(config: &Config) -> Result<ProcessManager, ProcessError> {
    let cgroup_manager = if config.features.cgroups {
        match CgroupManager::new() {
            Ok(manager) => {
                tracing::info!("cgroups v2 enabled");
                Some(manager)
            }
            Err(e) => {
                tracing::warn!("cgroups not available: {}, running without resource limits", e);
                None
            }
        }
    } else {
        tracing::info!("cgroups disabled by configuration");
        None
    };

    Ok(ProcessManager {
        processes: HashMap::new(),
        cgroup_manager,
    })
}
```

---

## 9. 安全考虑

### 9.1 权限管理

- **用户切换**：支持以指定用户运行进程（需要 root 权限）
- **cgroups 权限**：需要对 `/sys/fs/cgroup/svcmgr` 有写权限
- **日志文件权限**：日志文件应该设置适当的权限，防止敏感信息泄露

### 9.2 资源限制默认值

```rust
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu: Some(CpuLimit {
                shares: 1024,   // 默认权重
                quota: None,    // 无配额限制
            }),
            memory: Some(MemoryLimit {
                limit: 1024 * 1024 * 1024, // 默认 1GB
                swap: None,
                oom_kill: true,
            }),
            io: None,           // 默认无 I/O 限制
            processes: Some(100), // 默认最多 100 个进程
            open_files: Some(1024), // 默认最多 1024 个打开文件
        }
    }
}
```

---

## 10. 性能优化

### 10.1 监控开销

- 监控数据懒加载：只在请求时读取
- 批量读取：一次性读取所有需要的 `/proc` 文件
- 缓存：监控数据缓存 1 秒，避免频繁读取

### 10.2 日志处理

- 异步写入：日志写入不阻塞主线程
- 缓冲：使用 `BufWriter` 减少系统调用
- 轮转优化：日志轮转在后台线程进行

---

## 11. 测试策略

### 11.1 单元测试

- 进程生命周期测试
- 资源限制解析测试
- 重启策略测试
- 健康检查测试

### 11.2 集成测试

- 真实进程启动/停止测试
- cgroups 集成测试（需要权限）
- 资源使用监控测试
- 日志捕获测试

### 11.3 压力测试

- 大量并发进程测试
- 资源限制边界测试
- 进程崩溃恢复测试

---

## 12. 实施优先级

### Phase 1: 基础功能（P0）
- ✅ 进程启动/停止/重启
- ✅ 基础资源监控（/proc 读取）
- ✅ 简单的重启策略

### Phase 2: 增强功能（P1）
- ✅ cgroups v2 集成（可选）
- ✅ 健康检查
- ✅ 日志捕获和轮转

### Phase 3: 高级功能（P2）
- ⏳ 高级重启策略（指数退避）
- ⏳ 资源使用告警
- ⏳ 性能分析和优化

---

## 13. 相关规格

- [00-架构总览](./00-architecture-overview.md) - 整体架构设计
- [01-配置设计](./01-config-design.md) - 服务配置格式
- [02-调度引擎](./02-scheduler-engine.md) - 任务调度集成
- [06-功能特性标志](./06-feature-flags.md) - cgroups 功能标志
- [11-服务管理 API](./11-api-services.md) - 服务管理接口
