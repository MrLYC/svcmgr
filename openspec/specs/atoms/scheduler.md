# Scheduler 周期任务原子

## Overview

内置 Rust cron 调度器，提供用户级周期任务的管理能力，支持任务的增删改查和 cron 表达式解析。
替代 crontab CLI，适用于 Docker 容器等受限环境。

---

## ADDED Requirements

### Requirement: 任务管理

系统 **MUST** 管理周期任务定义。

#### Scenario: 添加定时任务

- **GIVEN** cron 表达式和执行命令
- **WHEN** 请求添加定时任务
- **THEN** 系统必须将任务添加到 TOML 格式的任务存储文件
- **AND** 为任务生成唯一 ID

#### Scenario: 任务存储方式

- **WHEN** 添加定时任务
- **THEN** 系统将任务写入 TOML 任务存储文件
- **AND** 包含所有任务定义和全局环境变量

#### Scenario: 任务标识

- **WHEN** 创建定时任务
- **THEN** 每个任务必须有唯一标识符
- **AND** 如未提供，系统自动生成 UUID

---

### Requirement: 任务查询

系统 **MUST** 提供定时任务查询。

#### Scenario: 列出所有托管任务

- **WHEN** 请求任务列表
- **THEN** 系统必须返回所有管理的 cron 任务
- **AND** 每项包含：任务ID、cron表达式、命令、描述、启用状态

#### Scenario: 查询任务详情

- **GIVEN** 任务 ID
- **WHEN** 请求任务详情
- **THEN** 系统必须返回完整的任务配置

---

### Requirement: 任务修改

系统 **MUST** 支持修改定时任务。

#### Scenario: 修改执行计划

- **GIVEN** 任务 ID 和新的 cron 表达式
- **WHEN** 请求修改计划
- **THEN** 系统必须更新任务存储文件中的对应条目

#### Scenario: 修改执行命令

- **GIVEN** 任务 ID 和新命令
- **WHEN** 请求修改命令
- **THEN** 系统必须更新任务存储文件中的对应条目

---

### Requirement: 任务删除

系统 **MUST** 支持删除定时任务。

#### Scenario: 删除单个任务

- **GIVEN** 任务 ID
- **WHEN** 请求删除任务
- **THEN** 系统必须从任务存储中移除对应条目

---

### Requirement: Cron 表达式验证

系统 **MUST** 验证 cron 表达式有效性。

#### Scenario: 验证标准 cron 表达式

- **GIVEN** 一个 cron 表达式
- **WHEN** 请求验证
- **THEN** 系统必须检查表达式格式是否有效
- **AND** 返回验证结果

#### Scenario: 表达式规范化

- **GIVEN** 5 字段 cron 表达式
- **WHEN** 系统处理时
- **THEN** 自动补充秒字段（"0"）和年字段（"*"）转为 7 字段格式

#### Scenario: 预定义表达式

- **WHEN** 用户使用预定义名称
- **THEN** 系统必须转换为标准 cron 表达式：
  - `@hourly` → `0 0 * * * * *`
  - `@daily` / `@midnight` → `0 0 0 * * * *`
  - `@weekly` → `0 0 0 * * 0 *`
  - `@monthly` → `0 0 0 1 * * *`
  - `@yearly` / `@annually` → `0 0 0 1 1 * *`

#### Scenario: 无效表达式处理

- **GIVEN** 无效的 cron 表达式
- **WHEN** 尝试创建或修改任务
- **THEN** 系统必须拒绝操作
- **AND** 返回详细的错误说明

---

### Requirement: 执行时间预测

系统 **SHOULD** 支持预测任务执行时间。

#### Scenario: 计算下次执行时间

- **GIVEN** 任务 ID 和所需次数
- **WHEN** 请求下次执行时间
- **THEN** 系统必须计算并返回指定次数的未来执行时间点

---

### Requirement: 环境变量管理

系统 **MUST** 支持环境变量设置。

#### Scenario: 设置全局环境变量

- **GIVEN** 键值对
- **WHEN** 请求设置环境变量
- **THEN** 系统必须将环境变量存储到任务存储文件的 `[env]` 段

#### Scenario: 获取全局环境变量

- **WHEN** 请求获取环境变量
- **THEN** 系统必须返回所有全局环境变量

---

### Requirement: 任务重载

系统 **MUST** 支持重载任务定义。

#### Scenario: 重载任务存储

- **WHEN** 请求重载
- **THEN** 系统必须重新读取任务存储文件
- **AND** 更新内存中的任务列表
