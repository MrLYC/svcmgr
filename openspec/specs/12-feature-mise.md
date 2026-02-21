# Feature Spec: Mise-based Dependency & Task Management

**版本**: 1.0.0  
**状态**: Draft  
**创建日期**: 2026-02-21

## ADDED Requirements

### Requirement: Dependency Management
系统 MUST 通过 mise 管理项目依赖和工具版本。

#### Scenario: Install Tool Dependency
- **WHEN** 用户添加工具依赖,指定工具名称和版本
- **THEN** 系统应更新 `.mise.toml` 配置文件的 `[tools]` 部分
- **AND** 系统应执行 `mise install` 安装工具
- **AND** 系统应验证安装是否成功
- **AND** 系统应返回工具的实际安装路径

#### Scenario: List Installed Tools
- **WHEN** 用户查询已安装工具列表
- **THEN** 系统应执行 `mise list`
- **AND** 系统应返回结构化列表,包含:工具名、版本、安装路径、是否激活

#### Scenario: Update Tool Version
- **WHEN** 用户更新工具版本
- **THEN** 系统应更新 `.mise.toml` 中的版本号
- **AND** 系统应执行 `mise install` 安装新版本
- **AND** 系统应询问是否卸载旧版本

#### Scenario: Remove Tool Dependency
- **WHEN** 用户移除工具依赖
- **THEN** 系统应从 `.mise.toml` 中删除对应配置
- **AND** 系统应询问是否同时卸载工具二进制
- **AND** 如果确认卸载,系统应执行 `mise uninstall`

### Requirement: Global Task Management
系统 MUST 支持定义和执行全局任务。

#### Scenario: Define Global Task
- **WHEN** 用户创建全局任务,指定任务名称和命令
- **THEN** 系统应更新 `.mise.toml` 的 `[tasks]` 部分
- **AND** 系统应支持定义任务依赖(`depends`)
- **AND** 系统应支持定义任务描述(`description`)
- **AND** 系统应自动提交配置到 git

#### Scenario: Execute Global Task
- **WHEN** 用户执行全局任务
- **THEN** 系统应执行 `mise run <task>`
- **AND** 系统应捕获标准输出和标准错误
- **AND** 系统应返回执行结果和退出码
- **AND** 如果任务有依赖,系统应先执行依赖任务

#### Scenario: List Available Tasks
- **WHEN** 用户查询任务列表
- **THEN** 系统应执行 `mise tasks`
- **AND** 系统应返回结构化列表,包含:任务名、描述、依赖关系

#### Scenario: Delete Global Task
- **WHEN** 用户删除全局任务
- **THEN** 系统应从 `.mise.toml` 中移除任务定义
- **AND** 系统应检查是否有其他任务依赖该任务
- **AND** 如果存在依赖,系统应警告用户并要求确认

### Requirement: Environment Variable Management
系统 MUST 支持通过 mise 管理环境变量。

#### Scenario: Set Environment Variable
- **WHEN** 用户设置环境变量
- **THEN** 系统应更新 `.mise.toml` 的 `[env]` 部分
- **AND** 系统应支持模板语法引用其他变量
- **AND** 系统应自动提交配置到 git

#### Scenario: Get Environment Variables
- **WHEN** 用户查询环境变量
- **THEN** 系统应执行 `mise env` 获取所有环境变量
- **AND** 系统应返回结构化键值对列表
- **AND** 系统应区分 mise 定义的变量和系统变量

#### Scenario: Delete Environment Variable
- **WHEN** 用户删除环境变量
- **THEN** 系统应从 `.mise.toml` 中移除变量定义
- **AND** 系统应自动提交配置到 git

### Requirement: Task Template Management
系统 MUST 提供全局任务模板。

#### Scenario: List Task Templates
- **WHEN** 用户请求任务模板列表
- **THEN** 系统应返回所有内置模板
- **AND** 每个模板应包含:名称、描述、参数列表、示例

#### Scenario: Create Task from Template
- **WHEN** 用户使用模板创建任务
- **THEN** 系统应渲染模板并生成任务定义
- **AND** 系统应验证所有必需参数已提供
- **AND** 系统应将任务添加到 `.mise.toml`

#### Scenario: Built-in Task Templates
- **WHEN** 系统初始化
- **THEN** 系统应提供以下内置模板:
  - `build`: 构建项目模板
  - `test`: 测试运行模板
  - `dev`: 开发服务器模板
  - `deploy`: 部署任务模板
  - `backup`: 备份任务模板
  - `health-check`: 健康检查模板

### Requirement: Version Query
系统 MUST 支持查询工具版本信息。

#### Scenario: Query Installed Version
- **WHEN** 用户查询工具的已安装版本
- **THEN** 系统应执行 `mise current <tool>`
- **AND** 系统应返回当前激活的版本号

#### Scenario: Query Available Versions
- **WHEN** 用户查询工具的可用版本列表
- **THEN** 系统应执行 `mise ls-remote <tool>`
- **AND** 系统应返回按时间排序的版本列表
- **AND** 系统应标识当前已安装的版本

#### Scenario: Check for Updates
- **WHEN** 用户检查工具更新
- **THEN** 系统应比较当前版本和最新版本
- **AND** 如果有新版本,系统应返回更新日志(changelog)链接
- **AND** 系统应提供一键更新功能

### Requirement: Integration with Systemd Services
系统 MUST 支持在 systemd 服务中使用 mise 环境。

#### Scenario: Generate Service with Mise Environment
- **WHEN** 用户创建基于 mise 的服务
- **THEN** 系统应在 service 文件中设置正确的环境变量
- **AND** 系统应使用 `mise env` 导出环境变量
- **AND** 系统应确保 PATH 包含 mise shims 目录

#### Scenario: Service Template for Mise Projects
- **WHEN** 用户使用 mise-project 模板创建服务
- **THEN** 系统应自动配置:
  - `WorkingDirectory` 指向项目目录
  - `Environment` 包含 mise 环境变量
  - `ExecStart` 使用 `mise run` 执行任务

### Requirement: Integration with Config Management
系统 MUST 将 mise 配置纳入 git 版本管理。

#### Scenario: Auto-commit Mise Configuration
- **WHEN** 用户修改 mise 配置
- **THEN** 系统应自动提交 `.mise.toml` 到 git
- **AND** commit message 应描述具体变更(工具/任务/环境变量)

#### Scenario: Restore Mise Configuration
- **WHEN** 用户回滚 mise 配置
- **THEN** 系统应从 git 历史恢复 `.mise.toml`
- **AND** 系统应执行 `mise install` 同步工具版本
- **AND** 系统应报告环境差异

## Technical Notes

### Implementation Dependencies
- 技术原子: Template Management (02)
- 技术原子: Mise Integration (03)
- 技术原子: Git Repository Management (01)
- 集成: Systemd Service Management (10)

### Mise Configuration Path
- Global config: `~/.config/mise/config.toml`
- Project config: `.mise.toml` (in project root)
- Environment file: `.mise.local.toml` (git-ignored)

### Mise Command Reference
```bash
mise install <tool>[@version]    # Install tool
mise use <tool>[@version]        # Set tool version
mise run <task>                  # Run task
mise env                         # Show environment
mise tasks                       # List tasks
mise ls                          # List installed tools
mise ls-remote <tool>            # List available versions
```

### Task Definition Format
```toml
[tasks.build]
description = "Build project"
run = "cargo build --release"
depends = ["test"]

[tasks.test]
run = "cargo test"
```

### Environment Variable Format
```toml
[env]
DATABASE_URL = "postgresql://localhost/mydb"
API_KEY = { file = ".env.api_key" }
PATH = ["./bin", "$PATH"]
```

### Error Handling
- Tool installation failure: 返回 mise 错误信息
- 任务执行失败: 捕获标准错误和退出码
- 版本冲突: 提示用户选择解决方案(升级/降级/保持)
