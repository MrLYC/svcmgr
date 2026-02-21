# Feature Spec: Configuration Management

**版本**: 1.0.0  
**状态**: Draft  
**创建日期**: 2026-02-21

## ADDED Requirements

### Requirement: Configuration Directory Management
系统 MUST 支持将特定目录设置为配置目录并进行版本管理。

#### Scenario: Initialize Configuration Directory
- **WHEN** 用户初始化配置目录
- **THEN** 系统应在指定目录执行 `git init`
- **AND** 系统应创建 `.gitignore` 文件排除敏感文件
- **AND** 系统应创建 `.gitattributes` 配置 LFS(如需要)
- **AND** 系统应创建初始 commit

#### Scenario: Set Configuration Directory
- **WHEN** 用户设置配置目录路径
- **THEN** 系统应验证目录存在且可写
- **AND** 系统应将路径保存到 svcmgr 配置
- **AND** 如果目录不是 git 仓库,系统应询问是否初始化

#### Scenario: Built-in Gitignore Templates
- **WHEN** 系统初始化配置目录
- **THEN** 系统应生成 `.gitignore` 包含:
  - `*.credentials` - Cloudflare 隧道凭证
  - `*.key`, `*.pem` - 私钥文件
  - `.env.local` - 本地环境变量
  - `*.log` - 日志文件
  - `tmp/`, `cache/` - 临时目录

### Requirement: Automatic Version Tracking
系统 MUST 自动追踪配置文件变更。

#### Scenario: Auto-commit Configuration Changes
- **WHEN** 用户通过 svcmgr 修改配置
- **THEN** 系统应自动提交变更到 git
- **AND** commit message 应遵循约定格式
- **AND** commit message 应包含:模块名、操作类型、资源名称

#### Scenario: Commit Message Format
- **WHEN** 系统生成 commit message
- **THEN** 系统应使用以下格式:
  ```
  <module>(<resource>): <action> <name>
  
  <详细描述>
  ```
- **AND** 示例:
  - `systemd(service): add web-server`
  - `nginx(proxy): update /api route to localhost:3000`
  - `mise(task): remove deprecated build task`

#### Scenario: Batch Commit Related Changes
- **WHEN** 单个操作修改多个配置文件
- **THEN** 系统应在一个 commit 中提交所有变更
- **AND** commit message 应列出所有受影响的文件

### Requirement: Configuration History Query
系统 MUST 支持查询配置历史。

#### Scenario: List Configuration History
- **WHEN** 用户查询配置历史
- **THEN** 系统应执行 `git log` 并解析输出
- **AND** 系统应返回结构化列表,包含:
  - Commit hash(短格式)
  - 时间戳
  - 提交信息
  - 作者
  - 变更文件列表

#### Scenario: Show Configuration Diff
- **WHEN** 用户查看特定 commit 的变更
- **THEN** 系统应执行 `git show <commit>`
- **AND** 系统应返回格式化的 diff 内容
- **AND** 系统应高亮显示关键变更

#### Scenario: Compare Configurations
- **WHEN** 用户比较两个版本的配置
- **THEN** 系统应执行 `git diff <commit1> <commit2>`
- **AND** 系统应按文件分组显示差异
- **AND** 系统应支持比较特定文件或目录

### Requirement: Configuration Rollback
系统 MUST 支持回滚配置到历史版本。

#### Scenario: Rollback to Previous Version
- **WHEN** 用户回滚配置到指定 commit
- **THEN** 系统应先备份当前配置(创建 stash)
- **AND** 系统应执行 `git checkout <commit> -- <path>`
- **AND** 系统应重新加载受影响的服务/配置
- **AND** 系统应验证回滚后的配置可用

#### Scenario: Undo Last Change
- **WHEN** 用户撤销最近一次变更
- **THEN** 系统应执行 `git revert HEAD`
- **AND** 系统应自动生成 revert commit message
- **AND** 系统应重新加载配置

#### Scenario: Rollback with Service Restart
- **WHEN** 回滚涉及运行中的服务
- **THEN** 系统应询问是否重启服务
- **AND** 如果确认,系统应:
  1. 回滚配置文件
  2. 验证配置语法
  3. 重启服务
  4. 验证服务状态

### Requirement: Configuration Backup and Restore
系统 MUST 支持配置备份和恢复。

#### Scenario: Create Configuration Backup
- **WHEN** 用户创建配置备份
- **THEN** 系统应创建 git tag 标记当前状态
- **AND** tag 名称应包含时间戳(如 `backup-2026-02-21-1234`)
- **AND** 系统应支持添加备份描述

#### Scenario: List Available Backups
- **WHEN** 用户查询备份列表
- **THEN** 系统应执行 `git tag -l "backup-*"`
- **AND** 系统应返回按时间排序的备份列表
- **AND** 系统应显示每个备份的描述和创建时间

#### Scenario: Restore from Backup
- **WHEN** 用户从备份恢复
- **THEN** 系统应先创建当前状态的自动备份
- **AND** 系统应执行 `git checkout <tag>`
- **AND** 系统应重新加载所有配置
- **AND** 系统应验证恢复后的配置完整性

### Requirement: Remote Repository Integration
系统 SHOULD 支持与远程 git 仓库同步。

#### Scenario: Add Remote Repository
- **WHEN** 用户添加远程仓库
- **THEN** 系统应执行 `git remote add <name> <url>`
- **AND** 系统应验证远程仓库可访问
- **AND** 系统应保存远程仓库配置

#### Scenario: Push Configuration to Remote
- **WHEN** 用户推送配置到远程仓库
- **THEN** 系统应执行 `git push <remote> <branch>`
- **AND** 系统应处理认证(SSH key/HTTP token)
- **AND** 如果推送失败,系统应返回详细错误信息

#### Scenario: Pull Configuration from Remote
- **WHEN** 用户从远程仓库拉取配置
- **THEN** 系统应执行 `git pull <remote> <branch>`
- **AND** 如果有冲突,系统应进入冲突解决模式
- **AND** 系统应在合并后重新加载配置

#### Scenario: Auto-push on Change
- **WHEN** 用户启用自动推送功能
- **THEN** 系统应在每次 commit 后自动推送到远程
- **AND** 如果推送失败,系统应记录错误但不阻塞操作

### Requirement: Configuration Conflict Resolution
系统 MUST 提供配置冲突解决机制。

#### Scenario: Detect Configuration Conflict
- **WHEN** 拉取远程配置时发生冲突
- **THEN** 系统应标识冲突文件
- **AND** 系统应返回冲突详情(本地版本 vs 远程版本)

#### Scenario: Resolve Conflict - Keep Local
- **WHEN** 用户选择保留本地配置
- **THEN** 系统应执行 `git checkout --ours <file>`
- **AND** 系统应创建合并 commit

#### Scenario: Resolve Conflict - Use Remote
- **WHEN** 用户选择使用远程配置
- **THEN** 系统应执行 `git checkout --theirs <file>`
- **AND** 系统应创建合并 commit
- **AND** 系统应重新加载配置

### Requirement: Configuration Validation
系统 MUST 在提交前验证配置完整性。

#### Scenario: Validate Before Commit
- **WHEN** 系统准备提交配置变更
- **THEN** 系统应验证所有配置文件语法
- **AND** 对于 systemd service: 执行 `systemd-analyze verify`
- **AND** 对于 nginx config: 执行 `nginx -t`
- **AND** 对于 mise config: 解析 TOML 格式
- **AND** 如果验证失败,系统应拒绝提交并返回错误

#### Scenario: Dry-run Rollback
- **WHEN** 用户预览回滚操作
- **THEN** 系统应显示将要恢复的文件列表
- **AND** 系统应显示配置差异
- **AND** 系统应列出受影响的服务
- **AND** 用户确认后才执行实际回滚

### Requirement: Integration with All Modules
系统 MUST 确保所有模块的配置变更都被追踪。

#### Scenario: Unified Configuration Directory
- **WHEN** 系统初始化
- **THEN** 系统应将所有配置文件组织在统一目录:
  ```
  {config_dir}/
  ├── systemd/          # systemd services
  ├── crontab/          # cron jobs
  ├── nginx/            # nginx configs
  ├── cloudflared/      # tunnel configs
  ├── mise/             # mise configs (.mise.toml)
  └── templates/        # user templates
  ```

#### Scenario: Module-specific Config Hooks
- **WHEN** 模块修改配置
- **THEN** 系统应触发配置管理钩子
- **AND** 钩子应自动提交变更到 git
- **AND** 钩子应生成标准化的 commit message

## Technical Notes

### Implementation Dependencies
- 技术原子: Git Repository Management (01)
- 集成: 所有功能模块(10-16)

### Git Configuration
```bash
# 用户信息
git config user.name "svcmgr"
git config user.email "svcmgr@localhost"

# 默认分支
git config init.defaultBranch main

# 自动 CRLF 转换
git config core.autocrlf input
```

### Commit Message Convention
```
<module>(<resource>): <action> <name>

<details>

Files changed:
- <file1>
- <file2>
```

### Sensitive File Patterns
```gitignore
# Cloudflare credentials
*.json
!config.json

# SSL certificates
*.key
*.pem
*.crt

# Environment variables
.env.local
.env.*.local

# Logs
*.log
logs/

# Temporary files
tmp/
cache/
*.tmp
```

### Error Handling
- Git 命令失败: 返回详细的 git 错误信息
- 配置验证失败: 阻止提交,返回验证错误
- 远程推送失败: 记录错误,不影响本地操作
- 冲突无法自动解决: 提供手动解决指引
