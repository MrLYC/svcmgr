# svcmgr 测试文档

本文档描述 svcmgr v2.0 项目的测试策略、测试工具和最佳实践。

## 测试策略

### 测试金字塔

```
        /\
       /  \      E2E 测试 (10%)
      /----\     集成测试 (20%)
     /------\    单元测试 (70%)
    /________\
```

- **单元测试** (目标覆盖率 >90%): 测试单个模块和函数
- **集成测试** (目标覆盖率 >80%): 测试模块间交互
- **E2E 测试**: 测试完整用户场景

### 覆盖率目标

- **Phase 0 (测试基础设施)**: 100% (Mock 实现)
- **Phase 1-2**: >90% 单元测试，>80% 集成测试
- **Phase 3+**: 维持 >85% 总体覆盖率

---

## 测试工具

### 单元测试 Mock

位于 `src/backend/mocks/` 模块，仅在 `#[cfg(test)]` 下编译。

#### 1. MiseMock - mise CLI 模拟器

模拟 mise 命令行工具，用于测试 mise 集成逻辑。

**功能**:
- 工具版本配置 (`with_tool`)
- 环境变量配置 (`with_env`)
- 任务定义和依赖管理 (`with_task`)
- 配置文件生成 (`write_config`)
- 命令执行模拟 (`mock_exec`)
- 依赖解析和循环检测 (`resolve_task_dependencies`)

**示例用法**:

```rust
use svcmgr::mocks::mise::MiseMock;
use tempfile::tempdir;

#[test]
fn test_my_feature() {
    let dir = tempdir().unwrap();
    
    let mock = MiseMock::new(dir.path().to_path_buf())
        .with_tool("node", "20.0.0")
        .with_env("DATABASE_URL", "postgres://localhost")
        .with_task("dev", "npm run dev", vec!["install"]);
    
    // 写入配置文件
    let config_path = mock.write_config().unwrap();
    
    // 模拟命令执行
    let output = mock.mock_exec(&["run", "dev"]).unwrap();
    
    // 验证结果
    assert!(output.contains("Running task: dev"));
}
```

**测试覆盖**:
- ✅ 基础配置
- ✅ 配置文件写入
- ✅ 依赖解析
- ✅ 循环依赖检测
- ✅ 命令执行模拟
- ✅ 环境变量获取

#### 2. FakeProcessManager - 进程管理器模拟

模拟进程管理器行为，用于测试进程生命周期逻辑。

**功能**:
- 进程启动/停止 (`start`, `stop`)
- 进程状态查询 (`get_state`, `get_process`)
- 崩溃模拟 (`simulate_crash`)
- 重启模拟 (`simulate_restart`)
- 健康检查失败模拟 (`simulate_health_check_failure`)
- 事件历史记录 (`get_history`)
- 进程列表 (`list_processes`)

**示例用法**:

```rust
use svcmgr::mocks::process::{FakeProcessManager, ProcessState};

#[tokio::test]
async fn test_process_lifecycle() {
    let manager = FakeProcessManager::new();
    
    // 启动进程
    let pid = manager.start("web-server", "npm start").await.unwrap();
    assert_eq!(manager.get_state("web-server").unwrap(), ProcessState::Running);
    
    // 模拟崩溃
    manager.simulate_crash("web-server", 1).unwrap();
    assert_eq!(manager.get_state("web-server").unwrap(), ProcessState::Exited(1));
    
    // 检查事件历史
    let history = manager.get_history();
    assert_eq!(history.len(), 2); // Start + Crash
}
```

**测试覆盖**:
- ✅ 进程启动
- ✅ 进程停止
- ✅ 重复启动检测
- ✅ 崩溃模拟
- ✅ 重启模拟
- ✅ 事件历史
- ✅ 健康检查失败
- ✅ 进程列表

---

## 测试 Fixtures

位于 `tests/fixtures/` 目录，包含各种 TOML 配置样本。

### 有效配置

1. **simple-service.toml**: 单服务配置（带健康检查）
2. **multi-service.toml**: 多服务配置
3. **service-with-deps.toml**: 带依赖链的服务
4. **cron-tasks.toml**: 定时任务配置
5. **http-routes.toml**: HTTP 路由配置

### 无效配置（用于错误处理测试）

6. **invalid-syntax.toml**: TOML 语法错误
7. **invalid-missing-task.toml**: 引用不存在的任务
8. **invalid-circular-deps.toml**: 循环依赖

**使用方式**:

```rust
use std::fs;

#[test]
fn test_parse_simple_service() {
    let toml_str = fs::read_to_string("tests/fixtures/simple-service.toml").unwrap();
    let config: ServiceConfig = toml::from_str(&toml_str).unwrap();
    
    assert_eq!(config.services.len(), 1);
    assert_eq!(config.services[0].name, "web");
}
```

---

## 运行测试

### 所有测试

```bash
cargo test
```

### 单元测试（仅库模块）

```bash
cargo test --lib
```

### 集成测试

```bash
cargo test --test '*'
```

### 特定测试

```bash
# 运行 MiseMock 测试
cargo test --lib mocks::mise

# 运行 FakeProcessManager 测试
cargo test --lib mocks::process

# 查看测试输出
cargo test -- --nocapture

# 显示测试列表
cargo test -- --list
```

### 代码覆盖率

使用 tarpaulin 生成覆盖率报告：

```bash
# 安装 tarpaulin
cargo install cargo-tarpaulin

# 生成覆盖率报告
cargo tarpaulin --out Html --output-dir ./coverage

# 生成 JSON 报告（用于 CI）
cargo tarpaulin --out Json --output-dir ./coverage
```

---

## CI/CD 集成

### GitHub Actions

项目使用 `.github/workflows/test.yml` 进行自动化测试：

**触发条件**:
- Push 到 `main` 或 `dev` 分支
- Pull Request 到 `main` 分支

**测试步骤**:
1. 安装 Rust 工具链
2. 安装 mise（依赖工具）
3. 代码格式检查 (`cargo fmt`)
4. Clippy lints 检查 (`cargo clippy`)
5. 运行单元测试
6. 运行集成测试
7. 生成代码覆盖率报告
8. 上传覆盖率到 Codecov
9. 验证覆盖率阈值 (>80%)

**覆盖率门槛**:
- 总体覆盖率必须 ≥80%
- 未达标时 CI 失败

---

## 测试最佳实践

### 1. 测试命名规范

```rust
#[test]
fn test_<function_name>_<scenario>_<expected_result>() {
    // 测试实现
}
```

**示例**:
```rust
#[test]
fn test_start_process_already_running_returns_error() {
    // ...
}
```

### 2. AAA 模式（Arrange-Act-Assert）

```rust
#[test]
fn test_example() {
    // Arrange: 准备测试数据
    let manager = FakeProcessManager::new();
    
    // Act: 执行操作
    let result = manager.start("test", "echo hello").await;
    
    // Assert: 验证结果
    assert!(result.is_ok());
}
```

### 3. 使用 pretty_assertions

```rust
use pretty_assertions::assert_eq;

#[test]
fn test_with_better_diffs() {
    let expected = vec![1, 2, 3];
    let actual = vec![1, 2, 4];
    
    // 失败时会显示彩色 diff
    assert_eq!(expected, actual);
}
```

### 4. 异步测试

```rust
#[tokio::test]
async fn test_async_operation() {
    let manager = FakeProcessManager::new();
    let pid = manager.start("test", "sleep 1").await.unwrap();
    
    // 可以使用 tokio 的时间控制
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    manager.stop("test").await.unwrap();
}
```

### 5. 临时文件/目录

```rust
use tempfile::tempdir;

#[test]
fn test_with_temp_dir() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.toml");
    
    // 写入测试数据
    std::fs::write(&file_path, "key = 'value'").unwrap();
    
    // 测试完成后自动清理
}
```

### 6. 错误场景测试

```rust
#[test]
#[should_panic(expected = "circular dependency detected")]
fn test_circular_dependency_panics() {
    let mock = MiseMock::new(PathBuf::from("/tmp"))
        .with_task("a", "echo a", vec!["b"])
        .with_task("b", "echo b", vec!["a"]);
    
    // 应该触发 panic
    mock.resolve_task_dependencies("a").unwrap();
}
```

---

## Phase 0 测试清单

### ✅ 已完成

- [x] MiseMock 实现（377 行）
- [x] FakeProcessManager 实现（343 行）
- [x] 8 个测试 fixtures（有效 + 无效配置）
- [x] 14 个单元测试（全部通过）
- [x] CI 配置（GitHub Actions）
- [x] 测试文档（本文档）

### 测试统计

```
Total Tests: 14
├─ MiseMock:           6 tests ✅
├─ FakeProcessManager: 8 tests ✅
└─ Coverage:           100% (Phase 0)
```

---

## 下一步（Phase 1）

1. **配置解析器测试**:
   - 解析 `.mise.toml` 和 `svcmgr/config.toml`
   - 配置合并和验证
   - 热重载测试

2. **Mise Port-Adapter 测试**:
   - MisePort trait mock 实现
   - MiseCliAdapter 集成测试（使用 MiseMock）
   - 环境变量注入测试

3. **CLI 命令测试**:
   - 使用 `assert_cmd` 测试 CLI 行为
   - 子命令参数解析
   - 错误消息验证

4. **进程管理测试**:
   - ProcessHandle 生命周期
   - 日志文件创建
   - PID 文件管理
   - 信号处理（SIGTERM/SIGKILL）

---

## 附录

### 相关依赖

```toml
[dev-dependencies]
assert_cmd = "2.0"        # CLI 测试
mockall = "0.14.0"        # Mock 框架
predicates = "3.1"        # 断言辅助
pretty_assertions = "1.4" # 更好的 diff 显示
tempfile = "3.25.0"       # 临时文件/目录
tokio-test = "0.4"        # 异步测试工具
```

### 参考资源

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [tokio Testing Guide](https://tokio.rs/tokio/topics/testing)
- [tarpaulin Documentation](https://github.com/xd009642/tarpaulin)
- [OpenSpec 测试规范](./openspec/specs/20-implementation-phases.md#phase-0-testing-infrastructure)

---

**最后更新**: 2026-02-23  
**Phase 0 状态**: ✅ 完成
