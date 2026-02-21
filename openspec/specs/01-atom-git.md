# T01: Git 版本管理原子

> 版本：1.0.0
> 技术基础：git CLI

## 概述

提供配置文件的版本控制能力，支持变更追踪、回滚和同步。

---

## ADDED Requirements

### Requirement: 仓库初始化
系统 **MUST** 能够将指定目录初始化为 Git 仓库。

#### Scenario: 初始化新仓库
- **WHEN** 指定目录不是 Git 仓库
- **THEN** 系统 **SHALL** 执行 `git init`
- **AND** 创建 `.gitignore` 文件排除运行时文件

#### Scenario: 已存在仓库
- **WHEN** 指定目录已是 Git 仓库
- **THEN** 系统 **SHOULD** 跳过初始化
- **AND** 返回仓库状态信息

---

### Requirement: 自动提交
系统 **MUST** 在配置变更时自动创建提交。

#### Scenario: 配置文件变更
- **WHEN** 通过 svcmgr 修改配置文件
- **THEN** 系统 **SHALL** 自动 stage 变更的文件
- **AND** 创建带有描述性消息的提交

#### Scenario: 提交消息格式
- **WHEN** 创建自动提交
- **THEN** 提交消息 **MUST** 遵循格式：`[{module}] {action}: {target}`
- **AND** 示例：`[systemd] add: my-service.service`

---

### Requirement: 版本查询
系统 **MUST** 提供配置历史查询能力。

#### Scenario: 列出历史
- **WHEN** 用户请求查看配置历史
- **THEN** 系统 **SHALL** 返回提交列表
- **AND** 包含：提交 ID、时间、消息、变更文件

#### Scenario: 查看差异
- **WHEN** 用户请求查看两个版本差异
- **THEN** 系统 **SHALL** 返回 diff 内容
- **AND** 支持指定文件过滤

---

### Requirement: 版本回滚
系统 **MUST** 支持配置回滚到指定版本。

#### Scenario: 回滚单文件
- **WHEN** 用户请求回滚特定文件到指定版本
- **THEN** 系统 **SHALL** 执行 `git checkout {commit} -- {file}`
- **AND** 创建新提交记录此回滚操作

#### Scenario: 回滚全部
- **WHEN** 用户请求回滚整个配置到指定版本
- **THEN** 系统 **SHALL** 执行 `git revert` 或 `git reset`
- **AND** 保留回滚记录（推荐 revert）

---

### Requirement: 远程同步
系统 **SHOULD** 支持与远程仓库同步。

#### Scenario: 推送变更
- **WHEN** 用户配置了远程仓库且执行推送
- **THEN** 系统 **SHALL** 执行 `git push`
- **AND** 处理认证（SSH key 或 token）

#### Scenario: 拉取变更
- **WHEN** 用户请求从远程同步
- **THEN** 系统 **SHALL** 执行 `git pull --rebase`
- **AND** 处理可能的冲突

---

## 接口定义

```rust
pub trait GitAtom {
    /// 初始化或验证 Git 仓库
    async fn init_repo(&self, path: &Path) -> Result<RepoStatus>;
    
    /// 提交变更
    async fn commit(&self, message: &str, files: &[PathBuf]) -> Result<CommitId>;
    
    /// 获取提交历史
    async fn log(&self, limit: usize, path: Option<&Path>) -> Result<Vec<CommitInfo>>;
    
    /// 获取差异
    async fn diff(&self, from: &str, to: &str, path: Option<&Path>) -> Result<String>;
    
    /// 回滚文件
    async fn checkout_file(&self, commit: &str, file: &Path) -> Result<()>;
    
    /// 回滚提交
    async fn revert(&self, commit: &str) -> Result<CommitId>;
    
    /// 推送到远程
    async fn push(&self, remote: &str, branch: &str) -> Result<()>;
    
    /// 从远程拉取
    async fn pull(&self, remote: &str, branch: &str) -> Result<()>;
}
```

---

## 配置项

```toml
[git]
# 是否启用自动提交
auto_commit = true

# 远程仓库配置（可选）
[git.remote]
url = "git@github.com:user/svcmgr-config.git"
branch = "main"
auto_push = false
```
