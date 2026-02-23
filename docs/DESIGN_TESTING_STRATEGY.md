# 测试方案设计

> 版本：1.0.0
> 日期：2026-02-23
> 相关文档：MISE_REDESIGN_RESEARCH_ZH.md

## 背景

### 测试挑战

在基于 mise 重新设计 svcmgr 时，面临以下测试挑战：

1. **外部依赖复杂**：
   - mise CLI（依赖管理、环境变量、任务执行）
   - systemd --user（服务管理）
   - 真实进程（服务生命周期测试）
   - 文件系统（配置文件、Git 仓库）

2. **测试路径长**：
   - 传统方式：配置 → mise 解析 → 任务定义 → 进程启动 → 验证状态
   - 每个环节都可能失败，难以隔离问题

3. **CI 环境限制**：
   - 可能没有 mise 安装
   - systemd 可能不可用
   - Docker 容器环境限制

### 设计目标

1. **简化外部依赖**：使用 Mock 替代真实 mise，减少测试环境要求
2. **缩短测试路径**：基于配置文件驱动，跳过不必要的集成环节
3. **分层测试策略**：单元测试（无依赖）→ 集成测试（Mock）→ E2E 测试（真实环境）

---

## 测试分层策略

### 层级划分

```
┌─────────────────────────────────────────────────────────────┐
│ E2E 测试（真实环境）                                          │
│ - 真实 mise + systemd + 进程                                 │
│ - CI/CD 或本地完整环境                                        │
│ - 覆盖率目标：关键场景（服务启停、配置热更新）                │
└─────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌─────────────────────────────────────────────────────────────┐
│ 集成测试（Mock 环境）                                         │
│ - MiseMock 模拟 mise 行为                                    │
│ - 假进程（不启动真实进程）                                    │
│ - 覆盖率目标：80%+ 业务逻辑                                   │
└─────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌─────────────────────────────────────────────────────────────┐
│ 单元测试（纯逻辑）                                            │
│ - 配置解析、验证、转换                                        │
│ - 无外部依赖                                                 │
│ - 覆盖率目标：90%+ 核心模块                                   │
└─────────────────────────────────────────────────────────────┘
```

### 层级对比

| 测试层级 | 外部依赖 | 执行速度 | 覆盖范围 | 失败定位 | 适用场景 |
|---------|---------|---------|---------|---------|---------|
| **单元测试** | 无 | 快（<1ms） | 单个函数/模块 | 精确 | 配置解析、数据转换、验证逻辑 |
| **集成测试（Mock）** | MiseMock | 中（~10ms） | 多模块交互 | 较精确 | 服务生命周期、调度引擎、配置热更新 |
| **E2E 测试** | mise + systemd | 慢（~1s） | 完整流程 | 粗略 | 关键业务场景、回归测试 |

---

## 方案 1: mise Mock 设计

### 1.1 核心思路

**不依赖真实 mise CLI，通过 Mock 结构模拟 mise 行为**：

```rust
// tests/mocks/mise.rs
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use anyhow::Result;

/// mise 模拟器
pub struct MiseMock {
    /// 工具版本映射
    tools: HashMap<String, String>,
    
    /// 环境变量
    env: HashMap<String, String>,
    
    /// 任务定义（任务名 → 命令）
    tasks: HashMap<String, TaskDef>,
    
    /// 模拟的工作目录
    workdir: PathBuf,
}

#[derive(Clone)]
pub struct TaskDef {
    pub run: String,
    pub env: HashMap<String, String>,
    pub dir: Option<String>,
}

impl MiseMock {
    /// 创建空的 mise mock
    pub fn new(workdir: PathBuf) -> Self {
        Self {
            tools: HashMap::new(),
            env: HashMap::new(),
            tasks: HashMap::new(),
            workdir,
        }
    }
    
    /// 添加工具定义
    pub fn with_tool(mut self, name: &str, version: &str) -> Self {
        self.tools.insert(name.to_string(), version.to_string());
        self
    }
    
    /// 添加环境变量
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }
    
    /// 添加任务定义
    pub fn with_task(mut self, name: &str, run: &str) -> Self {
        self.tasks.insert(name.to_string(), TaskDef {
            run: run.to_string(),
            env: HashMap::new(),
            dir: None,
        });
        self
    }
    
    /// 生成 mise.toml 配置文件
    pub fn write_config(&self) -> Result<()> {
        let config_path = self.workdir.join(".config/mise/config.toml");
        std::fs::create_dir_all(config_path.parent().unwrap())?;
        
        let mut content = String::new();
        
        // [tools]
        if !self.tools.is_empty() {
            content.push_str("[tools]\n");
            for (name, version) in &self.tools {
                content.push_str(&format!("{} = \"{}\"\n", name, version));
            }
            content.push_str("\n");
        }
        
        // [env]
        if !self.env.is_empty() {
            content.push_str("[env]\n");
            for (key, value) in &self.env {
                content.push_str(&format!("{} = \"{}\"\n", key, value));
            }
            content.push_str("\n");
        }
        
        // [tasks.*]
        for (name, task) in &self.tasks {
            content.push_str(&format!("[tasks.{}]\n", name));
            content.push_str(&format!("run = \"{}\"\n", task.run));
            if let Some(dir) = &task.dir {
                content.push_str(&format!("dir = \"{}\"\n", dir));
            }
            if !task.env.is_empty() {
                content.push_str("env = { ");
                let env_pairs: Vec<String> = task.env
                    .iter()
                    .map(|(k, v)| format!("{} = \"{}\"", k, v))
                    .collect();
                content.push_str(&env_pairs.join(", "));
                content.push_str(" }\n");
            }
            content.push_str("\n");
        }
        
        std::fs::write(&config_path, content)?;
        Ok(())
    }
    
    /// 模拟 mise exec 行为（不调用真实 mise）
    pub fn mock_exec(&self, task_name: &str) -> Result<Output> {
        let task = self.tasks.get(task_name)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_name))?;
        
        // 合并环境变量
        let mut env = self.env.clone();
        env.extend(task.env.clone());
        
        // 执行命令（直接运行，不通过 mise）
        let output = Command::new("sh")
            .arg("-c")
            .arg(&task.run)
            .envs(&env)
            .current_dir(&self.workdir)
            .output()?;
        
        Ok(output)
    }
    
    /// 模拟 mise ls（列出工具）
    pub fn mock_ls(&self) -> Vec<(String, String)> {
        self.tools.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    
    /// 模拟 mise env（获取环境变量）
    pub fn mock_env(&self) -> HashMap<String, String> {
        self.env.clone()
    }
}
```

### 1.2 测试示例

```rust
// tests/integration/service_lifecycle.rs
use tempfile::TempDir;
use svcmgr::config::SvcmgrConfig;
use svcmgr::scheduler::Scheduler;

#[tokio::test]
async fn test_service_start_stop_with_mock() {
    // 1. 创建临时目录
    let temp = TempDir::new().unwrap();
    
    // 2. 配置 mise mock
    let mise_mock = MiseMock::new(temp.path().to_path_buf())
        .with_tool("node", "22.0.0")
        .with_env("PORT", "3000")
        .with_task("api:start", "echo 'API started' && sleep 30");
    
    // 3. 写入 mise 配置
    mise_mock.write_config().unwrap();
    
    // 4. 写入 svcmgr 配置
    let svcmgr_config = r#"
[services.api]
task = "api:start"
enable = true
restart = "always"
ports = { web = 3000 }
"#;
    let config_path = temp.path().join(".config/mise/svcmgr/config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, svcmgr_config).unwrap();
    
    // 5. 加载配置（测试配置解析）
    let config = SvcmgrConfig::load(&config_path).unwrap();
    assert_eq!(config.services.len(), 1);
    assert_eq!(config.services["api"].task, "api:start");
    
    // 6. 创建调度器（使用 mock）
    let scheduler = Scheduler::new_with_mock(config, mise_mock).await.unwrap();
    
    // 7. 启动服务
    scheduler.start_service("api").await.unwrap();
    
    // 8. 验证服务状态
    let status = scheduler.service_status("api").await.unwrap();
    assert_eq!(status, ServiceStatus::Running);
    
    // 9. 停止服务
    scheduler.stop_service("api").await.unwrap();
    
    // 10. 验证服务已停止
    let status = scheduler.service_status("api").await.unwrap();
    assert_eq!(status, ServiceStatus::Stopped);
}
```

### 1.3 优势

- ✅ **无外部依赖**：不需要安装 mise
- ✅ **快速执行**：跳过 mise CLI 调用
- ✅ **可控性高**：Mock 行为完全可预测
- ✅ **CI 友好**：任何环境都能运行

---

## 方案 2: 配置驱动测试

### 2.1 核心思路

**直接从配置文件加载，跳过 mise 集成环节**：

```
传统流程（长）:
配置文件 → mise 解析 → mise exec → 进程启动 → 验证
     ↓
配置驱动（短）:
配置文件 → svcmgr 解析 → 模拟进程 → 验证
```

### 2.2 测试 Fixture

```toml
# tests/fixtures/simple-service.toml
[services.echo]
task = "echo:start"
enable = true
restart = "on-failure"
ports = { http = 8080 }

[scheduled_tasks.backup]
schedule = "0 2 * * *"
task = "backup:run"
enable = true
```

### 2.3 测试代码

```rust
// tests/integration/config_driven.rs
use std::path::PathBuf;
use svcmgr::config::SvcmgrConfig;
use svcmgr::scheduler::Scheduler;

#[tokio::test]
async fn test_config_parsing_and_validation() {
    // 1. 加载 fixture
    let fixture = PathBuf::from("tests/fixtures/simple-service.toml");
    let config = SvcmgrConfig::load(&fixture).unwrap();
    
    // 2. 验证服务配置
    assert_eq!(config.services.len(), 1);
    let echo_service = &config.services["echo"];
    assert_eq!(echo_service.task, "echo:start");
    assert_eq!(echo_service.enable, true);
    assert_eq!(echo_service.restart, RestartPolicy::OnFailure);
    assert_eq!(echo_service.ports.get("http"), Some(&8080));
    
    // 3. 验证定时任务配置
    assert_eq!(config.scheduled_tasks.len(), 1);
    let backup_task = &config.scheduled_tasks["backup"];
    assert_eq!(backup_task.schedule, "0 2 * * *");
    assert_eq!(backup_task.task, "backup:run");
}

#[tokio::test]
async fn test_scheduler_from_config() {
    // 1. 加载配置
    let fixture = PathBuf::from("tests/fixtures/simple-service.toml");
    let config = SvcmgrConfig::load(&fixture).unwrap();
    
    // 2. 创建调度器（不启动真实进程）
    let scheduler = Scheduler::from_config(config).await.unwrap();
    
    // 3. 验证调度器状态
    assert_eq!(scheduler.services().len(), 1);
    assert_eq!(scheduler.scheduled_tasks().len(), 1);
    
    // 4. 验证服务初始状态
    let status = scheduler.service_status("echo").await.unwrap();
    assert_eq!(status, ServiceStatus::Stopped);
}

#[tokio::test]
async fn test_config_hot_reload() {
    // 1. 创建临时配置文件
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");
    
    let initial_config = r#"
[services.api]
task = "api:start"
enable = true
"#;
    std::fs::write(&config_path, initial_config).unwrap();
    
    // 2. 加载初始配置
    let config = SvcmgrConfig::load(&config_path).unwrap();
    let scheduler = Scheduler::from_config(config).await.unwrap();
    assert_eq!(scheduler.services().len(), 1);
    
    // 3. 修改配置文件
    let updated_config = r#"
[services.api]
task = "api:start"
enable = true

[services.worker]
task = "worker:start"
enable = true
"#;
    std::fs::write(&config_path, updated_config).unwrap();
    
    // 4. 热重载配置
    scheduler.reload_config(&config_path).await.unwrap();
    
    // 5. 验证新配置生效
    assert_eq!(scheduler.services().len(), 2);
    assert!(scheduler.has_service("worker"));
}
```

### 2.4 优势

- ✅ **路径最短**：直接测试配置解析和业务逻辑
- ✅ **易于维护**：Fixture 文件清晰易读
- ✅ **覆盖面广**：可测试配置验证、热重载、错误处理

---

## 方案 3: 假进程测试

### 3.1 核心思路

**使用虚拟进程（FakeProcess）替代真实进程**：

```rust
// tests/mocks/process.rs
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// 假进程状态
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FakeProcessState {
    Running,
    Stopped,
    Failed,
}

/// 假进程
pub struct FakeProcess {
    pub pid: u32,
    pub command: String,
    pub state: Arc<Mutex<FakeProcessState>>,
}

impl FakeProcess {
    pub fn new(pid: u32, command: String) -> Self {
        Self {
            pid,
            command,
            state: Arc::new(Mutex::new(FakeProcessState::Running)),
        }
    }
    
    pub fn stop(&self) {
        *self.state.lock().unwrap() = FakeProcessState::Stopped;
    }
    
    pub fn fail(&self) {
        *self.state.lock().unwrap() = FakeProcessState::Failed;
    }
    
    pub fn is_running(&self) -> bool {
        *self.state.lock().unwrap() == FakeProcessState::Running
    }
}

/// 假进程管理器
pub struct FakeProcessManager {
    processes: Arc<Mutex<HashMap<String, FakeProcess>>>,
}

impl FakeProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub async fn spawn(&self, name: &str, command: &str) -> Result<u32> {
        let pid = rand::random::<u32>();
        let process = FakeProcess::new(pid, command.to_string());
        self.processes.lock().unwrap().insert(name.to_string(), process);
        Ok(pid)
    }
    
    pub async fn stop(&self, name: &str) -> Result<()> {
        if let Some(process) = self.processes.lock().unwrap().get(name) {
            process.stop();
        }
        Ok(())
    }
    
    pub async fn is_running(&self, name: &str) -> bool {
        self.processes.lock().unwrap()
            .get(name)
            .map(|p| p.is_running())
            .unwrap_or(false)
    }
}
```

### 3.2 测试示例

```rust
#[tokio::test]
async fn test_service_restart_on_failure() {
    let temp = TempDir::new().unwrap();
    let fake_pm = FakeProcessManager::new();
    
    // 配置服务（restart = "on-failure"）
    let config = SvcmgrConfig::from_str(r#"
[services.flaky]
task = "flaky:start"
restart = "on-failure"
"#).unwrap();
    
    let scheduler = Scheduler::new_with_fake_pm(config, fake_pm.clone()).await.unwrap();
    
    // 启动服务
    scheduler.start_service("flaky").await.unwrap();
    assert!(fake_pm.is_running("flaky").await);
    
    // 模拟进程失败
    fake_pm.get_process("flaky").unwrap().fail();
    
    // 等待自动重启
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // 验证自动重启成功
    assert!(fake_pm.is_running("flaky").await);
}
```

---

## 方案 4: E2E 测试（真实环境）

### 4.1 环境要求

```yaml
# .github/workflows/e2e.yml
name: E2E Tests

on: [push, pull_request]

jobs:
  e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install mise
        run: |
          curl https://mise.run | sh
          echo "$HOME/.local/bin" >> $GITHUB_PATH
      
      - name: Install dependencies
        run: |
          mise install node@22
          mise install python@3.12
      
      - name: Run E2E tests
        run: cargo test --test e2e -- --nocapture
```

### 4.2 测试示例

```rust
// tests/e2e/real_mise_integration.rs
#[tokio::test]
#[ignore]  // 仅在有 mise 的环境运行
async fn test_real_mise_task_execution() {
    // 1. 创建真实配置文件
    let temp = TempDir::new().unwrap();
    std::fs::write(temp.path().join("mise.toml"), r#"
[tools]
node = "22"

[tasks.hello]
run = "node -e 'console.log(\"Hello from mise\")'"
"#).unwrap();
    
    // 2. 使用真实 MiseAdapter
    let adapter = RealMiseAdapter::new(temp.path());
    let output = adapter.run_task("hello").await.unwrap();
    
    // 3. 验证输出
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "Hello from mise");
}
```

---

## 测试覆盖率目标

| 模块 | 单元测试 | 集成测试（Mock） | E2E 测试 | 总覆盖率目标 |
|------|---------|-----------------|---------|------------|
| 配置解析 | 95% | - | - | 95% |
| 配置验证 | 90% | 10% | - | 100% |
| mise 适配器 | 50% | 40% | 10% | 100% |
| 调度引擎 | 30% | 60% | 10% | 100% |
| 进程管理 | 20% | 70% | 10% | 100% |
| HTTP 代理 | 40% | 50% | 10% | 100% |
| Git 版本化 | 60% | 30% | 10% | 100% |

---

## 实施计划

### Phase 1: 基础设施（1-2天）

- [ ] 实现 `MiseMock` 结构
- [ ] 实现 `FakeProcessManager`
- [ ] 创建测试 fixtures（5-10个配置文件）
- [ ] 配置 CI 环境（GitHub Actions）

### Phase 2: 单元测试（2-3天）

- [ ] 配置解析测试（20+ 测试用例）
- [ ] 配置验证测试（15+ 测试用例）
- [ ] 数据转换测试（10+ 测试用例）

### Phase 3: 集成测试（Mock）（3-5天）

- [ ] 服务生命周期测试（启停、重启、重试）
- [ ] 定时任务调度测试
- [ ] 配置热重载测试
- [ ] HTTP 代理测试

### Phase 4: E2E 测试（1-2天）

- [ ] 关键场景测试（3-5个）
- [ ] CI 集成
- [ ] 性能基准测试

---

## 总结

### 推荐策略

| 阶段 | 策略 | 工具 | 覆盖目标 |
|------|------|------|---------|
| **开发阶段** | 单元测试 + 集成测试（Mock） | MiseMock + FakeProcess | 80%+ |
| **CI/CD** | 单元测试 + 集成测试（Mock） + 少量 E2E | 全自动 | 85%+ |
| **发布前** | 完整 E2E 测试 | 真实环境 | 95%+ |

### 关键优势

1. ✅ **快速反馈**：Mock 测试秒级完成
2. ✅ **环境独立**：不依赖 mise/systemd 安装
3. ✅ **高覆盖率**：配置驱动测试覆盖核心逻辑
4. ✅ **CI 友好**：任何环境都能运行
5. ✅ **易于调试**：Mock 行为可预测，问题定位快

### 下一步行动

1. 实现 `tests/mocks/mise.rs`
2. 实现 `tests/mocks/process.rs`
3. 创建 `tests/fixtures/` 配置文件集
4. 编写第一个集成测试（服务启停）
