# Task API 序列化修复

**状态**: 待实现  
**优先级**: 中  
**来源**: Phase 7 完成后识别的 pre-existing 问题  
**相关提交**: 7c83b08 (引入问题), 8ef2c52 (部分修复)

## 问题描述

task_api_basic 测试套件中有 3 个序列化相关测试失败,导致 GitHub Actions CI 无法通过:

1. `test_create_scheduled_task_request_deserialization` - CreateScheduledTaskRequest 反序列化失败
2. `test_task_execution_record_deserialization` - TaskExecutionRecord 反序列化失败
3. `test_validate_task_name_valid` - 任务名称验证失败

### 当前状态
- **测试结果**: task_api_basic 24/27 通过 (88.9%)
- **影响范围**: 序列化/反序列化逻辑,不影响实际 API 功能
- **性质**: Pre-existing 问题 (commit 7c83b08 引入)

## 根本原因分析

### 1. CreateScheduledTaskRequest 反序列化

**疑似问题**:
- TaskExecution enum 使用 `#[serde(tag = "type")]` tagged union
- 测试可能使用了旧的 JSON 格式 (嵌套对象而非 tagged enum)
- 或者测试的 JSON 结构与 Rust struct 定义不一致

**需要验证**:
```bash
cargo test --test task_api_basic test_create_scheduled_task_request_deserialization -- --nocapture
```

### 2. TaskExecutionRecord 反序列化

**疑似问题**:
- TaskExecutionRecord 可能包含 TaskExecution 字段
- 序列化/反序列化路径不一致

**需要检查**:
- src/backend/web/api/task_models.rs 中 TaskExecutionRecord 定义
- 测试中使用的 JSON payload 格式

### 3. validate_task_name_valid

**疑似问题**:
- 任务名称验证规则可能发生变化
- 测试使用的任务名称可能包含无效字符 (如 `-` 连字符)

**已知约束**:
- Phase 7 修复中发现任务名称不允许 `-`,只允许 `_`
- 可能需要更新测试使用的任务名称

## 修复方案

### 步骤 1: 分析失败原因

1. 运行每个失败测试并捕获详细错误:
   ```bash
   cargo test --test task_api_basic test_create_scheduled_task_request_deserialization -- --nocapture 2>&1 | tee test1.log
   cargo test --test task_api_basic test_task_execution_record_deserialization -- --nocapture 2>&1 | tee test2.log
   cargo test --test task_api_basic test_validate_task_name_valid -- --nocapture 2>&1 | tee test3.log
   ```

2. 检查错误信息中的具体序列化失败点:
   - 字段缺失?
   - 类型不匹配?
   - Enum variant 不正确?

### 步骤 2: 修复序列化测试

**选项 A: 修改测试 JSON payload**
- 如果测试使用了错误的 JSON 格式,对齐到当前数据结构
- 参考 task_api_integration.rs 中已修复的 payload 格式

**选项 B: 修复数据结构定义**
- 如果数据结构定义有问题 (缺少 `#[serde(...)]` 属性),添加必要的序列化指令
- 确保 Rust struct 和 JSON 结构一致

**选项 C: 添加 serde 兼容层**
- 如果需要支持多种 JSON 格式,使用 `#[serde(alias = "...")]`
- 或者实现自定义 Deserialize

### 步骤 3: 修复任务名称验证

1. 检查 validate_task_name 函数的当前规则:
   ```bash
   grep -A20 "fn validate_task_name" src/backend/web/api/tasks.rs
   ```

2. 修改测试使用的任务名称:
   - 将 `-` 替换为 `_`
   - 确保符合验证规则

### 步骤 4: 验证修复

1. 运行 task_api_basic 测试套件:
   ```bash
   cargo test --test task_api_basic --jobs=1
   ```
   期望: 27/27 通过

2. 运行所有集成测试确保无回归:
   ```bash
   cargo test --jobs=1
   ```

3. 推送并验证 GitHub Actions 通过

## 验收标准

- [ ] test_create_scheduled_task_request_deserialization 通过
- [ ] test_task_execution_record_deserialization 通过
- [ ] test_validate_task_name_valid 通过
- [ ] task_api_basic 测试套件 27/27 通过 (100%)
- [ ] GitHub Actions CI Test Suite job 通过 (绿色)
- [ ] 无新的测试回归

## 技术约束

1. **OpenSpec 优先**: 数据结构必须符合 openspec/specs/12-api-tasks.md 定义
2. **Tagged enum**: TaskExecution 必须保持 `#[serde(tag = "type")]` 结构
3. **向后兼容**: 不要破坏已通过的 task_api_integration 测试

## 风险评估

**低风险**:
- 仅影响序列化测试,不影响实际 API 功能
- 已有 96 个核心测试通过,修复范围有限

**潜在风险**:
- 修改序列化逻辑可能影响前端集成 (如果前端已依赖当前格式)
- 建议在修复后同步更新 API 文档

## 相关文件

```
src/backend/web/api/task_models.rs    - 数据结构定义
src/backend/web/api/tasks.rs          - validate_task_name 实现
tests/task_api_basic.rs               - 失败的测试
openspec/specs/12-api-tasks.md        - API 规范定义
```

## 参考资料

- [Serde Tagged Enums](https://serde.rs/enum-representations.html#internally-tagged)
- Phase 7 修复: commit 8ef2c52 (task_api_integration payload 修复)
- OpenSpec 12-api-tasks.md: 权威的 JSON 格式定义
