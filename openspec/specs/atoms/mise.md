# Mise 原子（依赖/任务/环境）

## Overview

基于 mise 提供用户级的依赖管理、全局任务定义和环境变量管理能力。本原子包含三个子能力：mise-dep（依赖）、mise-task（任务）、mise-env（环境变量）。

---

## ADDED Requirements

### Requirement: Mise 安装检测

系统 **MUST** 确保 mise 可用。

#### Scenario: 检测 mise 安装状态

- **WHEN** 系统启动或执行 setup 命令
- **THEN** 系统必须检测 mise 是否已安装
- **AND** 返回 mise 版本信息

#### Scenario: mise 未安装

- **WHEN** 检测到 mise 未安装
- **THEN** 系统必须提供安装指引
- **AND** 可选择自动安装（需用户确认）

---

## 子能力: mise-dep 依赖管理

### Requirement: 依赖安装

系统 **MUST** 通过 mise 管理用户级依赖。

#### Scenario: 安装指定版本的工具

- **GIVEN** 工具名称和版本号
- **WHEN** 请求安装依赖
- **THEN** 系统必须调用 `mise install {tool}@{version}`
- **AND** 将依赖配置持久化到 mise 配置文件

#### Scenario: 安装最新版本

- **GIVEN** 工具名称，未指定版本
- **WHEN** 请求安装依赖
- **THEN** 系统必须安装该工具的最新稳定版本

#### Scenario: 批量安装依赖

- **GIVEN** 多个工具的名称和版本
- **WHEN** 请求批量安装
- **THEN** 系统必须依次安装所有依赖
- **AND** 返回安装结果摘要

---

### Requirement: 依赖查询

系统 **MUST** 提供依赖状态查询。

#### Scenario: 列出已安装依赖

- **WHEN** 请求依赖列表
- **THEN** 系统必须返回所有 mise 管理的工具
- **AND** 每项包含：工具名、当前版本、安装路径

#### Scenario: 查询可用版本

- **GIVEN** 工具名称
- **WHEN** 请求可用版本列表
- **THEN** 系统必须返回该工具的所有可安装版本

---

### Requirement: 依赖卸载

系统 **MUST** 支持依赖卸载。

#### Scenario: 卸载指定工具

- **GIVEN** 工具名称
- **WHEN** 请求卸载
- **THEN** 系统必须移除该工具的所有版本
- **AND** 更新 mise 配置文件

---

## 子能力: mise-task 全局任务

### Requirement: 任务定义

系统 **MUST** 支持通过 mise 定义全局任务。

#### Scenario: 创建新任务

- **GIVEN** 任务名称、执行命令、描述
- **WHEN** 请求创建任务
- **THEN** 系统必须在 mise 配置中添加任务定义
- **AND** 任务可通过 `mise run {task_name}` 执行

#### Scenario: 创建带依赖的任务

- **GIVEN** 任务名称和依赖任务列表
- **WHEN** 请求创建任务
- **THEN** 任务定义必须包含 `depends` 字段
- **AND** 执行时按依赖顺序执行

#### Scenario: 使用模板创建任务

- **GIVEN** mise 任务模板名称和变量
- **WHEN** 请求通过模板创建任务
- **THEN** 系统必须调用 template 原子渲染任务配置
- **AND** 将渲染结果写入 mise 配置

---

### Requirement: 任务执行

系统 **MUST** 支持任务执行和状态查询。

#### Scenario: 执行任务

- **GIVEN** 任务名称
- **WHEN** 请求执行任务
- **THEN** 系统必须调用 `mise run {task_name}`
- **AND** 返回执行输出和退出码

#### Scenario: 列出所有任务

- **WHEN** 请求任务列表
- **THEN** 系统必须返回所有已定义任务
- **AND** 每项包含：任务名、描述、依赖关系

---

### Requirement: 任务删除

系统 **MUST** 支持任务删除。

#### Scenario: 删除任务

- **GIVEN** 任务名称
- **WHEN** 请求删除任务
- **THEN** 系统必须从 mise 配置中移除该任务
- **AND** 不影响其他任务

---

## 子能力: mise-env 环境变量

### Requirement: 环境变量定义

系统 **MUST** 支持通过 mise 管理环境变量。

#### Scenario: 设置环境变量

- **GIVEN** 变量名和变量值
- **WHEN** 请求设置环境变量
- **THEN** 系统必须将变量添加到 mise 配置的 `[env]` 段
- **AND** 变量在 mise shell 激活后生效

#### Scenario: 设置目录作用域的环境变量

- **GIVEN** 变量名、变量值和目录路径
- **WHEN** 请求设置作用域环境变量
- **THEN** 系统必须在指定目录的 `.mise.toml` 中添加变量
- **AND** 仅在该目录及子目录生效

---

### Requirement: 环境变量查询

系统 **MUST** 提供环境变量查询。

#### Scenario: 列出所有托管环境变量

- **WHEN** 请求环境变量列表
- **THEN** 系统必须返回所有 mise 管理的环境变量
- **AND** 标识变量的作用域（全局/目录）

---

### Requirement: 环境变量删除

系统 **MUST** 支持删除环境变量。

#### Scenario: 删除环境变量

- **GIVEN** 变量名
- **WHEN** 请求删除环境变量
- **THEN** 系统必须从 mise 配置中移除该变量
