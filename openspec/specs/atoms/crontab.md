# Crontab 周期任务原子

## Overview

基于 crontab 提供用户级周期任务的管理能力，支持任务的增删改查和通用任务模板。

---

## ADDED Requirements

### Requirement: Crontab 管理

系统 **MUST** 管理用户 crontab 条目。

#### Scenario: 添加定时任务

- **GIVEN** cron 表达式和执行命令
- **WHEN** 请求添加定时任务
- **THEN** 系统必须将任务添加到用户 crontab
- **AND** 任务条目包含 svcmgr 标识注释

#### Scenario: 使用托管文件方式

- **WHEN** 添加定时任务
- **THEN** 系统应将任务写入托管目录的配置文件
- **AND** 使用 `crontab` 命令加载合并后的配置

#### Scenario: 任务标识

- **WHEN** 创建定时任务
- **THEN** 每个任务必须有唯一标识符
- **AND** 标识符以注释形式存储在 crontab 中

---

### Requirement: 任务查询

系统 **MUST** 提供定时任务查询。

#### Scenario: 列出所有托管任务

- **WHEN** 请求任务列表
- **THEN** 系统必须返回所有由 svcmgr 管理的 cron 任务
- **AND** 每项包含：任务ID、cron表达式、命令、描述

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
- **THEN** 系统必须更新 crontab 中的对应条目
- **AND** 通过 git 原子记录变更

#### Scenario: 修改执行命令

- **GIVEN** 任务 ID 和新命令
- **WHEN** 请求修改命令
- **THEN** 系统必须更新 crontab 中的对应条目

---

### Requirement: 任务删除

系统 **MUST** 支持删除定时任务。

#### Scenario: 删除单个任务

- **GIVEN** 任务 ID
- **WHEN** 请求删除任务
- **THEN** 系统必须从 crontab 中移除对应条目
- **AND** 更新托管配置文件

#### Scenario: 删除所有托管任务

- **WHEN** 请求清除所有任务
- **THEN** 系统必须移除所有 svcmgr 标识的 cron 条目
- **AND** 保留非 svcmgr 管理的任务

---

### Requirement: 任务状态和日志

系统 **SHOULD** 提供任务执行状态跟踪。

#### Scenario: 查询任务最近执行时间

- **GIVEN** 任务 ID
- **WHEN** 请求执行历史
- **THEN** 系统应返回任务的最近执行时间（基于系统日志）

#### Scenario: 查询任务执行日志

- **GIVEN** 任务 ID
- **WHEN** 请求任务日志
- **THEN** 系统应返回相关的 syslog/journal 条目

---

### Requirement: 通用任务模板

系统 **MUST** 提供常用周期任务模板。

#### Scenario: 每日任务模板

- **WHEN** 使用 `daily` 模板创建任务
- **THEN** 系统必须生成 `0 0 * * *` 的 cron 表达式
- **AND** 支持自定义执行时间（小时、分钟）

#### Scenario: 每周任务模板

- **WHEN** 使用 `weekly` 模板创建任务
- **THEN** 系统必须生成 `0 0 * * 0` 的 cron 表达式
- **AND** 支持自定义星期几和执行时间

#### Scenario: 每月任务模板

- **WHEN** 使用 `monthly` 模板创建任务
- **THEN** 系统必须生成 `0 0 1 * *` 的 cron 表达式
- **AND** 支持自定义日期和执行时间

#### Scenario: 间隔执行模板

- **WHEN** 使用 `interval` 模板创建任务
- **GIVEN** 间隔分钟数
- **THEN** 系统必须生成 `*/{minutes} * * * *` 的 cron 表达式

---

### Requirement: Cron 表达式验证

系统 **MUST** 验证 cron 表达式有效性。

#### Scenario: 验证标准 cron 表达式

- **GIVEN** 一个 cron 表达式
- **WHEN** 请求验证
- **THEN** 系统必须检查表达式格式是否有效
- **AND** 返回验证结果和下次执行时间

#### Scenario: 无效表达式处理

- **GIVEN** 无效的 cron 表达式
- **WHEN** 尝试创建或修改任务
- **THEN** 系统必须拒绝操作
- **AND** 返回详细的错误说明
