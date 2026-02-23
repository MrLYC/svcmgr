# 08 - 配置校验与 Doctor 机制

> 版本：2.0.0-draft  
> 状态：设计中

## 1. 概述

配置校验与 Doctor 机制为 svcmgr 提供以下能力：

1. **配置校验**：在配置加载、更新时自动验证配置的语法和语义正确性
2. **Doctor 诊断**：检测配置问题、环境问题、依赖问题，并提供修复建议
3. **自动修复**：对常见配置问题提供自动修复能力（交互式或批量）
4. **健康检查**：定期检查系统健康状态，预防潜在问题

### 设计目标

- **早期发现问题**：在配置应用前发现错误，避免运行时故障
- **清晰的错误信息**：提供准确的错误位置和修复建议
- **自动化修复**：减少手动配置调试时间
- **渐进式修复**：支持交互式确认或批量自动修复

## 2. 配置校验架构

### 2.1 校验分层

```
┌─────────────────────────────────────────────────────────┐
│                    配置校验引擎                            │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │  语法校验    │  │  语义校验    │  │  运行时校验  │     │
│  │  (Syntax)   │  │  (Semantic) │  │  (Runtime)  │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
│        │                 │                 │             │
│        ├─ TOML 解析      ├─ 依赖检查       ├─ 文件存在   │
│        ├─ 字段类型       ├─ 循环依赖       ├─ 端口占用   │
│        ├─ 必填项         ├─ 端口冲突       ├─ 权限检查   │
│        └─ 枚举值         ├─ 资源限制       └─ 工具版本   │
│                          └─ 任务引用                     │
│                                                           │
└─────────────────────────────────────────────────────────┘
```

### 2.2 校验时机

| 触发时机 | 校验类型 | 失败行为 |
|---------|---------|---------|
| 配置文件加载时 | 语法 + 语义 | 拒绝启动，输出错误 |
| API 更新配置时 | 语法 + 语义 | 返回 400 错误 |
| 手动验证命令 | 语法 + 语义 + 运行时 | 输出诊断报告 |
| Doctor 诊断时 | 语义 + 运行时 | 输出问题列表 |
| 定期健康检查 | 运行时 | 记录日志 + 事件 |

### 2.3 校验规则分类

#### 语法校验（Syntax Validation）

**目标**：确保配置文件格式正确，可被正确解析

```rust
// 语法校验规则
pub enum SyntaxRule {
    // TOML 格式有效性
    ValidToml,
    
    // 字段类型匹配
    FieldType { field: String, expected: TypeInfo },
    
    // 必填字段存在
    RequiredField { section: String, field: String },
    
    // 枚举值有效性
    EnumValue { field: String, allowed: Vec<String> },
    
    // 字段值范围
    ValueRange { field: String, min: Option<f64>, max: Option<f64> },
    
    // 字符串格式（正则匹配）
    StringPattern { field: String, pattern: String },
}
```

**示例规则**：

```toml
# ✅ 正确
[services.api]
run_mode = "mise"
task = "api-start"
enable = true
restart = "always"
cpu_max_percent = 50

# ❌ 语法错误：run_mode 枚举值无效
[services.api]
run_mode = "invalid"  # 错误：必须是 "mise" 或 "script"

# ❌ 语法错误：cpu_max_percent 超出范围
[services.api]
cpu_max_percent = 150  # 错误：必须 <= 100

# ❌ 语法错误：必填字段缺失（mise 模式缺少 task）
[services.api]
run_mode = "mise"
enable = true  # 错误：mise 模式必须有 task 字段

# ❌ 语法错误：字段互斥冲突
[services.api]
run_mode = "mise"
task = "api-start"
command = "node server.js"  # 错误：mise 模式不能有 command 字段
```

#### 语义校验（Semantic Validation）

**目标**：确保配置在逻辑上合理，各部分正确关联

```rust
// 语义校验规则
pub enum SemanticRule {
    // mise 任务引用有效性
    TaskExists { service: String, task: String },
    
    // 循环依赖检查（服务/任务依赖链）
    NoCyclicDependency { chain: Vec<String> },
    
    // 端口冲突检查
    NoPortConflict { services: Vec<String>, port: u16 },
    
    // Cron 表达式有效性
    ValidCronExpr { service: String, cron: String },
    
    // 环境变量引用有效性
    EnvVarExists { service: String, var: String },
    
    // 工作目录有效性
    ValidWorkdir { service: String, path: String },
    
    // 资源限制合理性
    ResourceLimitsReasonable { service: String },
    
    // 字段组合冲突（mise vs script 模式）
    FieldCompatibility { service: String, conflict: String },
}
```

**示例规则**：

```toml
# ❌ 语义错误：引用不存在的 mise 任务
[services.api]
task = "non-existent-task"  # 错误：mise.toml 中无此任务

# ❌ 语义错误：端口冲突
[services.api]
ports = { http = 8080 }

[services.admin]
ports = { http = 8080 }  # 错误：端口 8080 已被 api 服务占用

# ❌ 语义错误：循环依赖
[services.api]
task = "api-start"
depends = ["worker"]

[services.worker]
task = "worker-run"
depends = ["api"]  # 错误：api → worker → api 形成循环依赖

# ❌ 语义错误：Cron 表达式无效
[services.cleanup]
task = "cleanup"
cron = "invalid cron"  # 错误：无效的 cron 表达式
```

#### 运行时校验（Runtime Validation）

**目标**：确保配置在当前环境下可执行

```rust
// 运行时校验规则
pub enum RuntimeRule {
    // 可执行文件存在
    CommandExists { service: String, command: String },
    
    // 文件路径存在
    PathExists { service: String, path: String },
    
    // 端口可用性
    PortAvailable { service: String, port: u16 },
    
    // 文件权限充足
    SufficientPermissions { service: String, path: String, mode: u32 },
    
    // mise 工具已安装
    MiseToolInstalled { tool: String, version: String },
    
    // 工作目录可写
    WorkdirWritable { service: String, path: String },
    
    // 环境变量已设置
    EnvVarSet { service: String, var: String },
    
    // 系统资源充足（内存、CPU）
    SufficientSystemResources,
}
```

**示例规则**：

```toml
# ❌ 运行时错误：命令不存在（script 模式）
[services.redis]
run_mode = "script"
command = "redis-server-nonexistent"  # 错误：redis-server-nonexistent 命令不存在

# ❌ 运行时错误：工作目录不存在
[services.api]
task = "api-start"
workdir = "/nonexistent/path"  # 错误：目录不存在

# ❌ 运行时错误：端口已被占用
[services.api]
ports = { http = 8080 }  # 错误：端口 8080 已被其他进程占用

# ❌ 运行时错误：mise 工具未安装
[services.api]
task = "api-start"  # 错误：任务需要 node@22，但未安装
```

## 3. 校验错误格式

### 3.1 错误数据结构

```rust
use serde::{Deserialize, Serialize};

/// 校验错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// 错误级别：error | warning | info
    pub level: ErrorLevel,
    
    /// 错误分类：syntax | semantic | runtime
    pub category: ErrorCategory,
    
    /// 错误代码（用于文档查询）
    pub code: String,
    
    /// 错误标题（简短描述）
    pub title: String,
    
    /// 详细错误信息
    pub message: String,
    
    /// 错误位置（文件路径 + 行号）
    pub location: Option<ErrorLocation>,
    
    /// 建议的修复方案
    pub suggestions: Vec<Suggestion>,
    
    /// 相关配置上下文
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorLevel {
    Error,   // 阻塞性错误，必须修复
    Warning, // 警告，建议修复
    Info,    // 提示信息
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCategory {
    Syntax,   // 语法错误
    Semantic, // 语义错误
    Runtime,  // 运行时错误
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLocation {
    /// 文件路径
    pub file: String,
    
    /// 行号（1-based）
    pub line: usize,
    
    /// 列号（1-based，可选）
    pub column: Option<usize>,
    
    /// 配置段路径（如 "services.api.run_mode"）
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// 建议标题
    pub title: String,
    
    /// 详细说明
    pub description: String,
    
    /// 是否可自动修复
    pub auto_fixable: bool,
    
    /// 自动修复操作（如果支持）
    pub fix: Option<AutoFix>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFix {
    /// 修复类型
    pub fix_type: FixType,
    
    /// 修复操作的描述
    pub description: String,
    
    /// 修复操作的具体内容
    pub operation: FixOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixType {
    Replace,    // 替换值
    Add,        // 添加字段
    Remove,     // 删除字段
    Rename,     // 重命名字段
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FixOperation {
    ReplaceValue {
        path: String,
        old_value: String,
        new_value: String,
    },
    AddField {
        section: String,
        field: String,
        value: String,
    },
    RemoveField {
        section: String,
        field: String,
    },
    RenameField {
        section: String,
        old_name: String,
        new_name: String,
    },
}
```

### 3.2 错误输出示例

#### JSON 格式（API 响应）

```json
{
  "valid": false,
  "errors": [
    {
      "level": "error",
      "category": "syntax",
      "code": "E001",
      "title": "Invalid enum value for run_mode",
      "message": "Field 'run_mode' must be either 'mise' or 'script', got 'invalid'",
      "location": {
        "file": ".config/mise/svcmgr/config.toml",
        "line": 3,
        "column": 12,
        "path": "services.api.run_mode"
      },
      "suggestions": [
        {
          "title": "Use 'mise' mode (recommended)",
          "description": "Change run_mode to 'mise' for mise-managed services",
          "auto_fixable": true,
          "fix": {
            "fix_type": "Replace",
            "description": "Replace 'invalid' with 'mise'",
            "operation": {
              "type": "ReplaceValue",
              "path": "services.api.run_mode",
              "old_value": "invalid",
              "new_value": "mise"
            }
          }
        },
        {
          "title": "Use 'script' mode",
          "description": "Change run_mode to 'script' for direct command execution",
          "auto_fixable": true,
          "fix": {
            "fix_type": "Replace",
            "description": "Replace 'invalid' with 'script'",
            "operation": {
              "type": "ReplaceValue",
              "path": "services.api.run_mode",
              "old_value": "invalid",
              "new_value": "script"
            }
          }
        }
      ],
      "context": "[services.api]\nrun_mode = \"invalid\"\ntask = \"api-start\""
    }
  ],
  "warnings": [],
  "info": []
}
```

#### 命令行格式（Doctor 输出）

```
🔍 Validating configuration...

❌ ERROR [E001] Invalid enum value for run_mode
   ╭─ .config/mise/svcmgr/config.toml:3:12
   │
 3 │ run_mode = "invalid"
   │            ^^^^^^^^^ must be "mise" or "script"
   │
   ├─ Context:
   │  [services.api]
   │  run_mode = "invalid"
   │  task = "api-start"
   │
   └─ Suggestions:
      1. [Auto-fixable] Use 'mise' mode (recommended)
         Replace 'invalid' with 'mise'
      
      2. [Auto-fixable] Use 'script' mode
         Replace 'invalid' with 'script'

⚠️  WARNING [W001] Missing restart configuration
   ╭─ .config/mise/svcmgr/config.toml:5
   │
 5 │ [services.api]
   │ ^^^^^^^^^^^^^^ no restart policy defined
   │
   └─ Suggestion:
      [Auto-fixable] Add restart policy
      Add field 'restart = "always"' to services.api

ℹ️  INFO: 1 error, 1 warning found
    Run 'svcmgr doctor --fix' to apply auto-fixes
```

## 4. Doctor 机制

### 4.1 Doctor 命令设计

Doctor 机制提供诊断和修复功能，命令格式：

```bash
# 基本诊断（仅检查，不修复）
svcmgr doctor

# 详细诊断（显示更多上下文）
svcmgr doctor --verbose

# 交互式修复（逐个确认）
svcmgr doctor --fix

# 自动修复（无需确认）
svcmgr doctor --fix --auto

# 诊断特定服务
svcmgr doctor --service api

# 诊断特定类别
svcmgr doctor --category syntax    # 仅语法错误
svcmgr doctor --category semantic  # 仅语义错误
svcmgr doctor --category runtime   # 仅运行时错误

# 输出为 JSON 格式
svcmgr doctor --format json

# 仅显示错误（不显示警告和提示）
svcmgr doctor --errors-only
```

### 4.2 诊断流程

```
┌──────────────────────────────────────────────────────────┐
│                      Doctor 诊断流程                       │
└──────────────────────────────────────────────────────────┘

1. 配置加载
   ├─ 加载 mise.toml
   ├─ 加载 svcmgr/config.toml
   └─ 解析合并配置
         ↓
2. 语法校验
   ├─ TOML 格式
   ├─ 字段类型
   ├─ 必填字段
   └─ 枚举值
         ↓
3. 语义校验
   ├─ 任务引用
   ├─ 循环依赖
   ├─ 端口冲突
   └─ 资源限制
         ↓
4. 运行时校验
   ├─ 命令存在
   ├─ 文件路径
   ├─ 端口可用
   └─ 工具版本
         ↓
5. 生成诊断报告
   ├─ 错误列表
   ├─ 警告列表
   └─ 修复建议
         ↓
6. 交互式修复（可选）
   ├─ 显示修复选项
   ├─ 用户确认
   ├─ 应用修复
   └─ 重新验证
         ↓
7. 输出结果
   ├─ 显示修复结果
   ├─ 保存修复记录
   └─ 触发配置重载
```

### 4.3 诊断类别

Doctor 机制检测以下类别的问题：

#### 配置问题（Config Issues）

- **语法错误**：TOML 格式、字段类型、必填项、枚举值
- **语义错误**：任务引用、循环依赖、端口冲突、字段冲突
- **最佳实践偏差**：缺少 restart 策略、缺少资源限制、缺少健康检查

#### 环境问题（Environment Issues）

- **mise 工具缺失**：任务依赖的工具未安装
- **命令不存在**：script 模式的命令不存在
- **路径问题**：工作目录不存在、配置路径无效
- **权限问题**：文件不可读、目录不可写

#### 依赖问题（Dependency Issues）

- **mise 版本不兼容**：当前 mise 版本不支持某些特性
- **工具版本冲突**：多个服务依赖不同版本的工具
- **循环依赖**：服务依赖链形成循环

#### 运行时问题（Runtime Issues）

- **端口占用**：端口已被其他进程占用
- **资源不足**：系统内存、CPU 不足
- **进程僵死**：服务进程状态异常
- **日志问题**：日志文件过大、日志目录不可写

### 4.4 自动修复策略

自动修复分为三个级别：

#### Level 1: 安全修复（Safe Fixes）

**特点**：修复不会改变配置语义，仅修正格式或补充默认值

**示例**：
- 补充缺失的默认值（如 `restart = "always"`）
- 修正字段名拼写错误（`run_mod` → `run_mode`）
- 删除重复的配置项
- 格式化 TOML 文件（缩进、对齐）

```bash
# 安全修复，无需确认
svcmgr doctor --fix --safe
```

#### Level 2: 建议修复（Suggested Fixes）

**特点**：修复可能改变配置行为，需要用户确认

**示例**：
- 解决端口冲突（自动分配新端口）
- 解决循环依赖（删除某个依赖）
- 调整资源限制（根据系统资源自动设置）
- 安装缺失的 mise 工具

```bash
# 交互式确认每个修复
svcmgr doctor --fix

# 示例交互输出
⚠️  Port conflict detected: services.api and services.admin both use port 8080

  Suggested fix:
  1. Change services.admin.ports.http from 8080 to 8081
  
  Apply this fix? [Y/n] █
```

#### Level 3: 高风险修复（Risky Fixes）

**特点**：修复会显著改变配置，需要明确确认

**示例**：
- 删除无效的服务定义
- 修改服务的运行模式（mise ↔ script）
- 修改任务依赖链
- 重构配置结构

```bash
# 不自动应用高风险修复，仅显示建议
svcmgr doctor --fix --include-risky

# 示例交互输出
⚠️  Service 'legacy' uses deprecated configuration format

  ⚠️  RISKY FIX: This will change service behavior
  
  Suggested fix:
  1. Convert services.legacy from script mode to mise mode
     - Create new mise task 'legacy-start'
     - Remove 'command' field
     - Add 'task = "legacy-start"' field
  
  ⚠️  This is a risky operation. Review the changes carefully.
  
  Apply this fix? [y/N] █
```

### 4.5 修复操作实现

```rust
use std::collections::HashMap;
use anyhow::Result;

/// Doctor 修复引擎
pub struct DoctorEngine {
    /// 配置文件路径
    config_paths: ConfigPaths,
    
    /// 修复历史记录
    fix_history: Vec<FixRecord>,
    
    /// 交互模式
    interactive: bool,
}

impl DoctorEngine {
    /// 执行完整诊断
    pub async fn diagnose(&self) -> Result<DiagnosticReport> {
        let mut report = DiagnosticReport::default();
        
        // 1. 语法校验
        let syntax_errors = self.validate_syntax().await?;
        report.errors.extend(syntax_errors);
        
        // 2. 语义校验
        let semantic_errors = self.validate_semantics().await?;
        report.errors.extend(semantic_errors);
        
        // 3. 运行时校验
        let runtime_errors = self.validate_runtime().await?;
        report.errors.extend(runtime_errors);
        
        // 4. 生成修复建议
        for error in &report.errors {
            let suggestions = self.generate_suggestions(error)?;
            report.suggestions.extend(suggestions);
        }
        
        Ok(report)
    }
    
    /// 应用修复
    pub async fn apply_fixes(
        &mut self,
        fixes: Vec<AutoFix>,
        auto_approve: bool,
    ) -> Result<FixResult> {
        let mut result = FixResult::default();
        
        for fix in fixes {
            // 交互式确认（如果需要）
            if !auto_approve && self.interactive {
                if !self.confirm_fix(&fix)? {
                    result.skipped.push(fix);
                    continue;
                }
            }
            
            // 应用修复
            match self.apply_single_fix(&fix).await {
                Ok(_) => {
                    result.applied.push(fix.clone());
                    self.fix_history.push(FixRecord {
                        timestamp: chrono::Utc::now(),
                        fix: fix.clone(),
                        success: true,
                    });
                }
                Err(e) => {
                    result.failed.push((fix.clone(), e.to_string()));
                    self.fix_history.push(FixRecord {
                        timestamp: chrono::Utc::now(),
                        fix: fix.clone(),
                        success: false,
                    });
                }
            }
        }
        
        // 重新验证配置
        if !result.applied.is_empty() {
            result.validation = Some(self.diagnose().await?);
        }
        
        Ok(result)
    }
    
    /// 应用单个修复
    async fn apply_single_fix(&self, fix: &AutoFix) -> Result<()> {
        match &fix.operation {
            FixOperation::ReplaceValue { path, old_value, new_value } => {
                self.replace_config_value(path, old_value, new_value)?;
            }
            FixOperation::AddField { section, field, value } => {
                self.add_config_field(section, field, value)?;
            }
            FixOperation::RemoveField { section, field } => {
                self.remove_config_field(section, field)?;
            }
            FixOperation::RenameField { section, old_name, new_name } => {
                self.rename_config_field(section, old_name, new_name)?;
            }
        }
        
        Ok(())
    }
    
    /// 交互式确认修复
    fn confirm_fix(&self, fix: &AutoFix) -> Result<bool> {
        println!("\n{}", fix.description);
        
        if matches!(fix.fix_type, FixType::Replace) {
            println!("  Apply this fix? [Y/n] ");
        } else {
            println!("  ⚠️  This is a structural change.");
            println!("  Apply this fix? [y/N] ");
        }
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        let input = input.trim().to_lowercase();
        Ok(input == "y" || input == "yes")
    }
}

/// 诊断报告
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
    pub info: Vec<ValidationError>,
    pub suggestions: Vec<Suggestion>,
    pub summary: ReportSummary,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_errors: usize,
    pub total_warnings: usize,
    pub total_info: usize,
    pub auto_fixable: usize,
    pub requires_manual_fix: usize,
}

/// 修复结果
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FixResult {
    pub applied: Vec<AutoFix>,
    pub skipped: Vec<AutoFix>,
    pub failed: Vec<(AutoFix, String)>,
    pub validation: Option<DiagnosticReport>,
}

/// 修复记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRecord {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub fix: AutoFix,
    pub success: bool,
}
```

## 5. API 端点

### 5.1 配置校验 API

```http
POST /api/v1/config/validate
Content-Type: application/json

{
  "config": {
    "services": {
      "api": {
        "run_mode": "mise",
        "task": "api-start",
        "enable": true
      }
    }
  },
  "strict": true,
  "categories": ["syntax", "semantic", "runtime"]
}
```

**响应**：

```json
{
  "valid": true,
  "errors": [],
  "warnings": [
    {
      "level": "warning",
      "category": "semantic",
      "code": "W001",
      "title": "Missing restart policy",
      "message": "Service 'api' does not have a restart policy defined",
      "suggestions": [
        {
          "title": "Add restart policy",
          "description": "Set restart = 'always' for long-running services",
          "auto_fixable": true
        }
      ]
    }
  ],
  "info": [],
  "summary": {
    "total_errors": 0,
    "total_warnings": 1,
    "total_info": 0,
    "auto_fixable": 1,
    "requires_manual_fix": 0
  }
}
```

### 5.2 Doctor 诊断 API

```http
POST /api/v1/doctor/diagnose
Content-Type: application/json

{
  "categories": ["syntax", "semantic", "runtime"],
  "services": ["api", "worker"],
  "include_suggestions": true
}
```

### 5.3 Doctor 修复 API

```http
POST /api/v1/doctor/fix
Content-Type: application/json

{
  "fixes": [
    {
      "fix_type": "Replace",
      "description": "Fix invalid run_mode value",
      "operation": {
        "type": "ReplaceValue",
        "path": "services.api.run_mode",
        "old_value": "invalid",
        "new_value": "mise"
      }
    }
  ],
  "auto_approve": false,
  "dry_run": true
}
```

**响应**：

```json
{
  "applied": 1,
  "skipped": 0,
  "failed": 0,
  "changes": [
    {
      "file": ".config/mise/svcmgr/config.toml",
      "diff": "- run_mode = \"invalid\"\n+ run_mode = \"mise\""
    }
  ],
  "validation": {
    "valid": true,
    "errors": []
  }
}
```

## 6. 配置校验规则清单

### 6.1 服务配置（services.*）

| 规则 ID | 类别 | 描述 | 自动修复 |
|---------|------|------|---------|
| E001 | syntax | run_mode 必须是 "mise" 或 "script" | ✅ |
| E002 | syntax | mise 模式必须有 task 字段 | ❌ |
| E003 | syntax | script 模式必须有 command 字段 | ❌ |
| E004 | syntax | mise 模式不能有 command 字段 | ✅ |
| E005 | syntax | script 模式不能有 task 字段 | ✅ |
| E006 | syntax | cpu_max_percent 必须 <= 100 | ✅ |
| E007 | syntax | restart 必须是 no/always/on-failure | ✅ |
| E008 | semantic | task 引用的 mise 任务不存在 | ❌ |
| E009 | semantic | 端口冲突：多个服务使用同一端口 | ✅ |
| E010 | semantic | 循环依赖：服务依赖链形成循环 | ⚠️  |
| E011 | semantic | cron 表达式无效 | ❌ |
| E012 | runtime | command 命令不存在（script 模式） | ❌ |
| E013 | runtime | workdir 目录不存在 | ⚠️  |
| E014 | runtime | 端口已被占用 | ⚠️  |
| E015 | runtime | mise 工具未安装 | ⚠️  |
| W001 | semantic | 缺少 restart 策略 | ✅ |
| W002 | semantic | 缺少资源限制 | ✅ |
| W003 | semantic | 缺少健康检查配置 | ✅ |
| W004 | runtime | 工作目录不可写 | ❌ |

### 6.2 定时任务配置（scheduled_tasks.*）

| 规则 ID | 类别 | 描述 | 自动修复 |
|---------|------|------|---------|
| E020 | syntax | cron 表达式必须有效 | ❌ |
| E021 | semantic | task 引用的 mise 任务不存在 | ❌ |
| E022 | semantic | timeout 必须 > 0 | ✅ |
| W020 | semantic | 定时任务缺少超时配置 | ✅ |

### 6.3 功能开关配置（features.*）

| 规则 ID | 类别 | 描述 | 自动修复 |
|---------|------|------|---------|
| E030 | syntax | 功能开关必须是 boolean 类型 | ✅ |
| W030 | semantic | 某些功能开关组合不推荐 | ❌ |

### 6.4 HTTP 代理配置（http.*）

| 规则 ID | 类别 | 描述 | 自动修复 |
|---------|------|------|---------|
| E040 | syntax | 端口号必须在 1-65535 范围 | ✅ |
| E041 | semantic | upstream 服务不存在 | ❌ |
| E042 | runtime | bind 地址无效 | ❌ |
| E043 | runtime | 端口已被占用 | ⚠️  |

## 7. 最佳实践

### 7.1 开发工作流

```bash
# 1. 修改配置文件
vim .config/mise/svcmgr/config.toml

# 2. 运行 Doctor 诊断
svcmgr doctor

# 3. 应用自动修复（交互式）
svcmgr doctor --fix

# 4. 重新加载配置
svcmgr reload

# 5. 验证服务状态
svcmgr status
```

### 7.2 CI/CD 集成

```yaml
# .github/workflows/config-validation.yml
name: Validate Config

on:
  pull_request:
    paths:
      - '.config/mise/**'

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install svcmgr
        run: cargo install svcmgr
      
      - name: Validate configuration
        run: |
          svcmgr doctor --errors-only --format json > validation.json
          
          # 检查是否有错误
          if jq -e '.summary.total_errors > 0' validation.json; then
            echo "❌ Configuration validation failed"
            jq '.errors' validation.json
            exit 1
          fi
          
          echo "✅ Configuration validation passed"
```

### 7.3 定期健康检查

```toml
# .config/mise/svcmgr/config.toml

[health_check]
# 定期运行 Doctor 诊断
enabled = true
interval = "1h"           # 每小时检查一次
categories = ["runtime"]  # 仅检查运行时问题
notify_on_warning = true  # 发现警告时通知
auto_fix_safe = true      # 自动修复安全级别的问题
```

## 8. 实施计划

### Phase 1: 基础校验（2周）

- [x] 语法校验引擎
- [x] 基本错误格式
- [x] CLI 命令框架
- [ ] 配置校验 API

### Phase 2: 语义校验（3周）

- [ ] 任务引用检查
- [ ] 端口冲突检查
- [ ] 循环依赖检查
- [ ] Cron 表达式校验

### Phase 3: 运行时校验（3周）

- [ ] 命令存在性检查
- [ ] 文件路径检查
- [ ] 端口可用性检查
- [ ] mise 工具检查

### Phase 4: Doctor 机制（4周）

- [ ] 诊断引擎
- [ ] 自动修复引擎
- [ ] 交互式界面
- [ ] 修复历史记录

### Phase 5: 高级功能（2周）

- [ ] 定期健康检查
- [ ] CI/CD 集成
- [ ] 修复建议优化
- [ ] 文档完善

## 9. 参考资料

- [01-config-design.md](./01-config-design.md) - 配置文件设计
- [14-api-config.md](./14-api-config.md) - 配置管理 API
- [mise 官方文档](https://mise.jdx.dev)
- [Rust 错误处理最佳实践](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
