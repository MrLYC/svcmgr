# Template 模板渲染原子

## Overview

基于 Jinja2 语法提供配置模板的管理和渲染能力，支持模板继承、变量替换和条件逻辑。

---

## ADDED Requirements

### Requirement: 模板存储管理

系统 **MUST** 管理内置和用户自定义模板。

#### Scenario: 加载内置模板

- **WHEN** 系统启动
- **THEN** 系统必须加载所有内置模板
- **AND** 内置模板存储于二进制文件中（embed）

#### Scenario: 加载用户自定义模板

- **WHEN** 用户在 `~/.config/svcmgr/templates/` 放置模板文件
- **THEN** 系统必须加载这些模板
- **AND** 用户模板可覆盖同名内置模板

#### Scenario: 模板命名规范

- **WHEN** 定义模板
- **THEN** 模板名称必须遵循格式：`{category}/{name}.{ext}.j2`
- **AND** category 对应技术原子（systemd, nginx, crontab 等）

---

### Requirement: Jinja2 语法支持

系统 **MUST** 完整支持 Jinja2 核心语法。

#### Scenario: 变量替换

- **GIVEN** 模板包含 `{{ variable }}` 占位符
- **WHEN** 渲染模板时提供变量值
- **THEN** 系统必须正确替换变量

#### Scenario: 条件语句

- **GIVEN** 模板包含 `{% if condition %}...{% endif %}` 块
- **WHEN** 渲染模板
- **THEN** 系统必须根据条件值正确输出内容

#### Scenario: 循环语句

- **GIVEN** 模板包含 `{% for item in items %}...{% endfor %}` 块
- **WHEN** 渲染模板时提供列表变量
- **THEN** 系统必须正确迭代生成内容

#### Scenario: 模板继承

- **GIVEN** 模板使用 `{% extends "base.j2" %}` 继承基础模板
- **WHEN** 渲染该模板
- **THEN** 系统必须正确应用继承关系

---

### Requirement: 模板渲染 API

系统 **MUST** 提供统一的模板渲染接口。

#### Scenario: 渲染模板到字符串

- **GIVEN** 模板名称和变量字典
- **WHEN** 调用渲染 API
- **THEN** 系统必须返回渲染后的字符串

#### Scenario: 渲染模板到文件

- **GIVEN** 模板名称、变量字典和目标路径
- **WHEN** 调用渲染到文件 API
- **THEN** 系统必须将渲染结果写入目标文件
- **AND** 触发 git 原子的自动提交（如果目标在托管目录）

#### Scenario: 渲染失败处理

- **WHEN** 模板渲染过程中发生错误（变量缺失、语法错误）
- **THEN** 系统必须返回详细的错误信息
- **AND** 包含错误位置（行号）

---

### Requirement: 模板变量校验

系统 **SHOULD** 支持模板变量的预校验。

#### Scenario: 获取模板所需变量

- **GIVEN** 一个模板文件
- **WHEN** 请求该模板的变量列表
- **THEN** 系统必须返回模板中使用的所有变量名
- **AND** 标识哪些是必需的、哪些有默认值

#### Scenario: 校验变量完整性

- **GIVEN** 模板名称和待用变量字典
- **WHEN** 请求校验变量
- **THEN** 系统必须检查是否所有必需变量都已提供
- **AND** 返回缺失变量列表（如有）

---

### Requirement: 内置模板集

系统 **MUST** 提供以下内置模板。

#### Scenario: systemd 服务单元模板

- **WHEN** 请求 `systemd/simple-service.service.j2` 模板
- **THEN** 系统必须提供基础服务单元模板
- **AND** 支持变量：name, description, exec_start, restart_policy, environment

#### Scenario: nginx 反向代理模板

- **WHEN** 请求 `nginx/reverse-proxy.conf.j2` 模板
- **THEN** 系统必须提供反向代理配置模板
- **AND** 支持变量：upstream_name, listen_path, backend_host, backend_port

#### Scenario: crontab 任务模板

- **WHEN** 请求 `crontab/scheduled-task.j2` 模板
- **THEN** 系统必须提供周期任务模板
- **AND** 支持变量：schedule, command, log_file

#### Scenario: mise 任务模板

- **WHEN** 请求 `mise/task.toml.j2` 模板
- **THEN** 系统必须提供 mise 任务定义模板
- **AND** 支持变量：task_name, run_command, description, depends
