# T02: 模板管理原子

> 版本：1.0.0
> 技术基础：Jinja2 (minijinja Rust 实现)

## 概述

提供配置模板的管理和渲染能力，支持 Jinja2 语法，用于生成各类配置文件。

---

## ADDED Requirements

### Requirement: Jinja2 语法支持
系统 **MUST** 支持标准 Jinja2 模板语法。

#### Scenario: 变量替换
- **WHEN** 模板包含 `{{ variable }}` 语法
- **THEN** 系统 **SHALL** 用上下文中的值替换
- **AND** 变量不存在时报错或使用默认值

#### Scenario: 条件判断
- **WHEN** 模板包含 `{% if condition %}...{% endif %}` 语法
- **THEN** 系统 **SHALL** 根据条件渲染对应内容

#### Scenario: 循环
- **WHEN** 模板包含 `{% for item in list %}...{% endfor %}` 语法
- **THEN** 系统 **SHALL** 迭代渲染列表内容

#### Scenario: 过滤器
- **WHEN** 模板使用过滤器如 `{{ value | upper }}`
- **THEN** 系统 **SHALL** 应用过滤器转换值
- **AND** 支持常用过滤器：upper, lower, trim, default, join, split

---

### Requirement: 内置模板库
系统 **MUST** 提供预定义的配置模板。

#### Scenario: Systemd 服务模板
- **WHEN** 用户需要创建 systemd 服务
- **THEN** 系统 **SHALL** 提供以下模板：
  - `simple-service`: 简单服务（ExecStart）
  - `forking-service`: fork 类型服务
  - `oneshot-service`: 一次性任务
  - `timer-service`: 定时器服务

#### Scenario: Crontab 任务模板
- **WHEN** 用户需要创建定时任务
- **THEN** 系统 **SHALL** 提供以下模板：
  - `daily-task`: 每日任务
  - `weekly-task`: 每周任务
  - `monthly-task`: 每月任务
  - `custom-cron`: 自定义 cron 表达式

#### Scenario: Nginx 代理模板
- **WHEN** 用户需要配置代理
- **THEN** 系统 **SHALL** 提供以下模板：
  - `http-proxy`: HTTP 反向代理
  - `tcp-proxy`: TCP 流代理
  - `static-files`: 静态文件服务
  - `websocket-proxy`: WebSocket 代理

#### Scenario: Mise 任务模板
- **WHEN** 用户需要定义 mise 任务
- **THEN** 系统 **SHALL** 提供以下模板：
  - `shell-task`: Shell 命令任务
  - `script-task`: 脚本文件任务
  - `multi-step-task`: 多步骤任务

---

### Requirement: 用户自定义模板
系统 **MUST** 支持用户创建和管理自定义模板。

#### Scenario: 添加模板
- **WHEN** 用户提供模板文件
- **THEN** 系统 **SHALL** 保存到用户模板目录
- **AND** 验证模板语法正确性

#### Scenario: 列出模板
- **WHEN** 用户请求列出可用模板
- **THEN** 系统 **SHALL** 返回内置模板和用户模板
- **AND** 标识模板来源（built-in / user）

#### Scenario: 模板继承
- **WHEN** 用户模板使用 `{% extends "base" %}` 语法
- **THEN** 系统 **SHALL** 支持模板继承
- **AND** 查找顺序：用户目录 → 内置目录

---

### Requirement: 模板验证
系统 **MUST** 在渲染前验证模板。

#### Scenario: 语法验证
- **WHEN** 加载模板时
- **THEN** 系统 **SHALL** 验证 Jinja2 语法
- **AND** 报告语法错误位置

#### Scenario: 变量检查
- **WHEN** 渲染模板时
- **THEN** 系统 **SHALL** 检查必需变量是否提供
- **AND** 对于缺失的必需变量报错

---

### Requirement: 预览功能
系统 **SHOULD** 支持模板渲染预览。

#### Scenario: 干运行
- **WHEN** 用户请求预览渲染结果
- **THEN** 系统 **SHALL** 返回渲染后的内容
- **AND** 不实际写入文件

---

## 接口定义

```rust
pub trait TemplateAtom {
    /// 列出所有可用模板
    fn list_templates(&self, category: Option<&str>) -> Result<Vec<TemplateInfo>>;
    
    /// 获取模板内容
    fn get_template(&self, name: &str) -> Result<String>;
    
    /// 渲染模板
    fn render(&self, template: &str, context: &Context) -> Result<String>;
    
    /// 渲染模板到文件
    fn render_to_file(&self, template: &str, context: &Context, output: &Path) -> Result<()>;
    
    /// 验证模板语法
    fn validate(&self, template: &str) -> Result<ValidationResult>;
    
    /// 添加用户模板
    fn add_user_template(&self, name: &str, content: &str) -> Result<()>;
    
    /// 删除用户模板
    fn remove_user_template(&self, name: &str) -> Result<()>;
}

pub struct TemplateInfo {
    pub name: String,
    pub category: String,
    pub source: TemplateSource,  // BuiltIn | User
    pub description: String,
    pub required_vars: Vec<String>,
}

pub struct Context {
    pub vars: HashMap<String, Value>,
}
```

---

## 内置模板示例

### simple-service.service.j2
```jinja2
[Unit]
Description={{ description | default(name ~ " service") }}
After=network.target
{% if requires %}
Requires={{ requires | join(" ") }}
{% endif %}

[Service]
Type=simple
ExecStart={{ exec_start }}
{% if working_directory %}
WorkingDirectory={{ working_directory }}
{% endif %}
{% if environment %}
{% for key, value in environment.items() %}
Environment="{{ key }}={{ value }}"
{% endfor %}
{% endif %}
Restart={{ restart | default("on-failure") }}
RestartSec={{ restart_sec | default(5) }}

[Install]
WantedBy=default.target
```

### daily-task.cron.j2
```jinja2
# {{ description | default(name ~ " daily task") }}
# Runs at {{ hour | default("03") }}:{{ minute | default("00") }} every day
{{ minute | default("0") }} {{ hour | default("3") }} * * * {{ command }}
```

---

## 配置项

```toml
[template]
# 用户模板目录
user_dir = "~/.config/svcmgr/templates"

# 未定义变量的行为：error | warning | ignore
undefined_behavior = "error"

# 是否启用自动转义
auto_escape = false
```
