#![allow(dead_code)]

/// Mise 原子模块 - 依赖管理、全局任务和环境变量
///
/// 本模块提供三个核心 trait：
/// - DependencyAtom: 管理工具版本(通过 mise 安装 node, python, rust 等)
/// - TaskAtom: 管理全局任务（mise 任务系统）
/// - EnvAtom: 管理环境变量（mise 环境变量系统）
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

// ============================================================================
// 数据结构定义
// ============================================================================

/// 任务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    /// 执行命令列表
    pub run: Vec<String>,
    /// 任务描述
    pub description: Option<String>,
    /// 依赖任务列表
    pub depends: Vec<String>,
    /// 环境变量
    pub env: HashMap<String, String>,
    /// 工作目录
    pub dir: Option<PathBuf>,
}

/// 临时 systemd 单元信息
#[derive(Debug, Clone)]
pub struct TransientUnit {
    /// 单元名称
    pub unit_name: String,
    /// 进程 PID
    pub pid: u32,
}

/// 工具信息
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// 工具名称
    pub name: String,
    /// 版本号
    pub version: String,
    /// 安装路径
    pub path: Option<PathBuf>,
    /// 是否为当前激活版本
    pub active: bool,
}

/// 任务信息
#[derive(Debug, Clone)]
pub struct TaskInfo {
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: Option<String>,
    /// 依赖任务
    pub depends: Vec<String>,
}

/// 环境变量来源
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSource {
    /// 来自文件
    File,
    /// 来自配置
    Config,
}

/// 环境变量
#[derive(Debug, Clone)]
pub struct EnvVar {
    /// 变量名
    pub key: String,
    /// 变量值
    pub value: String,
    /// 来源
    pub source: EnvSource,
}

// ============================================================================
// Trait 定义
// ============================================================================

/// 依赖管理原子 - 管理工具版本
pub trait DependencyAtom {
    /// 安装工具
    ///
    /// # 参数
    /// - `tool`: 工具名称（如 node, python）
    /// - `version`: 版本号（如 20.0.0, latest）
    fn install(&self, tool: &str, version: &str) -> Result<()>;

    /// 卸载工具
    fn uninstall(&self, tool: &str) -> Result<()>;

    /// 列出已安装工具
    fn list_tools(&self) -> Result<Vec<ToolInfo>>;

    /// 获取工具可用版本列表
    fn available_versions(&self, tool: &str) -> Result<Vec<String>>;

    /// 切换工具版本
    fn use_version(&self, tool: &str, version: &str) -> Result<()>;
}

/// 全局任务原子 - 管理 mise 任务
pub trait TaskAtom {
    /// 添加任务
    fn add_task(&self, name: &str, config: &TaskConfig) -> Result<()>;

    /// 移除任务
    fn remove_task(&self, name: &str) -> Result<()>;

    /// 列出所有任务
    fn list_tasks(&self) -> Result<Vec<TaskInfo>>;

    /// 前台执行任务
    fn run(&self, name: &str, args: &[String]) -> Result<()>;

    /// 后台执行任务（使用 systemd-run）
    fn run_background(&self, name: &str, args: &[String]) -> Result<TransientUnit>;
}

/// 环境变量原子 - 管理环境变量
pub trait EnvAtom {
    /// 设置环境变量
    fn set(&self, key: &str, value: &str) -> Result<()>;

    /// 删除环境变量
    fn unset(&self, key: &str) -> Result<()>;

    /// 列出所有环境变量
    fn list_env(&self) -> Result<Vec<EnvVar>>;

    /// 从 .env 文件加载环境变量
    fn load_file(&self, path: &Path) -> Result<()>;

    /// 获取当前 mise 管理的环境变量
    fn get_env(&self) -> Result<HashMap<String, String>>;
}

// ============================================================================
// 实现
// ============================================================================

/// Mise 管理器 - 统一实现三个 trait
pub struct MiseManager {
    /// mise 配置文件路径
    config_path: PathBuf,
}

impl MiseManager {
    /// 创建新的 Mise 管理器
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    /// 使用默认配置路径创建管理器
    pub fn default_config() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
            .join("svcmgr")
            .join("managed")
            .join("mise");

        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join(".mise.toml");

        Ok(Self::new(config_path))
    }

    /// 执行 mise 命令
    fn run_mise(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("mise")
            .args(args)
            .env("MISE_CONFIG_FILE", &self.config_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: "mise".to_string(),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 解析 mise ls 输出
    fn parse_tool_list(&self, output: &str) -> Vec<ToolInfo> {
        output
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some(ToolInfo {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        path: None,
                        active: true,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// 解析 mise tasks 输出
    fn parse_task_list(&self, output: &str) -> Vec<TaskInfo> {
        output
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if !parts.is_empty() {
                    Some(TaskInfo {
                        name: parts[0].to_string(),
                        description: if parts.len() > 1 {
                            Some(parts[1..].join(" "))
                        } else {
                            None
                        },
                        depends: vec![],
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

// ============================================================================
// DependencyAtom 实现
// ============================================================================

impl DependencyAtom for MiseManager {
    fn install(&self, tool: &str, version: &str) -> Result<()> {
        let tool_spec = format!("{}@{}", tool, version);
        self.run_mise(&["install", &tool_spec])?;
        Ok(())
    }

    fn uninstall(&self, tool: &str) -> Result<()> {
        self.run_mise(&["uninstall", tool])?;
        Ok(())
    }

    fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let output = self.run_mise(&["ls"])?;
        Ok(self.parse_tool_list(&output))
    }

    fn available_versions(&self, tool: &str) -> Result<Vec<String>> {
        let output = self.run_mise(&["ls-remote", tool])?;
        let versions: Vec<String> = output
            .lines()
            .map(|line: &str| line.trim().to_string())
            .filter(|line: &String| !line.is_empty())
            .collect();
        Ok(versions)
    }

    fn use_version(&self, tool: &str, version: &str) -> Result<()> {
        let tool_spec = format!("{}@{}", tool, version);
        self.run_mise(&["use", &tool_spec])?;
        Ok(())
    }
}

// ============================================================================
// TaskAtom 实现
// ============================================================================

impl TaskAtom for MiseManager {
    fn add_task(&self, name: &str, config: &TaskConfig) -> Result<()> {
        // 读取现有配置
        let mut toml_content = if self.config_path.exists() {
            std::fs::read_to_string(&self.config_path)?
        } else {
            String::new()
        };

        // 构建任务配置
        let mut task_toml = format!("[tasks.{}]\n", name);
        task_toml.push_str(&format!("run = {:?}\n", config.run));

        if let Some(desc) = &config.description {
            task_toml.push_str(&format!("description = \"{}\"\n", desc));
        }

        if !config.depends.is_empty() {
            task_toml.push_str(&format!("depends = {:?}\n", config.depends));
        }

        if !config.env.is_empty() {
            task_toml.push_str("env = { ");
            let env_pairs: Vec<String> = config
                .env
                .iter()
                .map(|(k, v)| format!("{} = \"{}\"", k, v))
                .collect();
            task_toml.push_str(&env_pairs.join(", "));
            task_toml.push_str(" }\n");
        }

        if let Some(dir) = &config.dir {
            task_toml.push_str(&format!("dir = \"{}\"\n", dir.display()));
        }

        // 追加到配置文件
        toml_content.push('\n');
        toml_content.push_str(&task_toml);

        std::fs::write(&self.config_path, toml_content)?;
        Ok(())
    }

    fn remove_task(&self, name: &str) -> Result<()> {
        if !self.config_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.config_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let mut result = Vec::new();
        let mut skip = false;
        let task_header = format!("[tasks.{}]", name);

        for line in lines {
            if line.trim() == task_header {
                skip = true;
                continue;
            }

            if skip && line.trim().starts_with("[tasks.") {
                skip = false;
            }

            if !skip {
                result.push(line);
            }
        }

        std::fs::write(&self.config_path, result.join("\n"))?;
        Ok(())
    }

    fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        let output = self.run_mise(&["tasks"])?;
        Ok(self.parse_task_list(&output))
    }

    fn run(&self, name: &str, args: &[String]) -> Result<()> {
        let mut cmd_args = vec!["run", name];
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        cmd_args.extend(arg_refs);

        self.run_mise(&cmd_args)?;
        Ok(())
    }

    fn run_background(&self, name: &str, args: &[String]) -> Result<TransientUnit> {
        // 构建 systemd-run 命令
        let unit_name = format!("svcmgr-mise-{}", name);

        let mut cmd = Command::new("systemd-run");
        cmd.arg("--user")
            .arg("--unit")
            .arg(&unit_name)
            .arg("--")
            .arg("mise")
            .arg("run")
            .arg(name);

        for arg in args {
            cmd.arg(arg);
        }

        cmd.env("MISE_CONFIG_FILE", &self.config_path);

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("systemd-run 失败: {}", stderr).into());
        }

        // 解析输出获取 PID（简化实现，实际需要更复杂的解析）
        let pid = 0; // TODO: 解析 systemd-run 输出获取真实 PID

        Ok(TransientUnit { unit_name, pid })
    }
}

// ============================================================================
// EnvAtom 实现
// ============================================================================

impl EnvAtom for MiseManager {
    fn set(&self, key: &str, value: &str) -> Result<()> {
        // 读取现有配置
        let mut toml_content = if self.config_path.exists() {
            std::fs::read_to_string(&self.config_path)?
        } else {
            String::new()
        };

        // 添加环境变量配置
        if !toml_content.contains("[env]") {
            toml_content.push_str("\n[env]\n");
        }

        let env_line = format!("{} = \"{}\"\n", key, value);

        // 简单追加（实际应该检查重复）
        toml_content.push_str(&env_line);

        std::fs::write(&self.config_path, toml_content)?;
        Ok(())
    }

    fn unset(&self, key: &str) -> Result<()> {
        if !self.config_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.config_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let mut result = Vec::new();
        let key_prefix = format!("{} =", key);

        for line in lines {
            if !line.trim().starts_with(&key_prefix) {
                result.push(line);
            }
        }

        std::fs::write(&self.config_path, result.join("\n"))?;
        Ok(())
    }

    fn list_env(&self) -> Result<Vec<EnvVar>> {
        let output = self.run_mise(&["env"])?;

        let vars: Vec<EnvVar> = output
            .lines()
            .filter_map(|line: &str| {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some(EnvVar {
                        key: parts[0].to_string(),
                        value: parts[1].to_string(),
                        source: EnvSource::Config,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(vars)
    }

    fn load_file(&self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                self.set(key.trim(), value.trim())?;
            }
        }

        Ok(())
    }

    fn get_env(&self) -> Result<HashMap<String, String>> {
        let vars = self.list_env()?;
        let mut env = HashMap::new();

        for var in vars {
            env.insert(var.key, var.value);
        }

        Ok(env)
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (MiseManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".mise.toml");
        (MiseManager::new(config_path), temp_dir)
    }

    #[test]
    fn test_add_task() {
        let (manager, _temp_dir) = create_test_manager();

        let config = TaskConfig {
            run: vec!["echo hello".to_string()],
            description: Some("测试任务".to_string()),
            depends: vec![],
            env: HashMap::new(),
            dir: None,
        };

        manager.add_task("test", &config).unwrap();

        // 验证配置文件存在
        assert!(manager.config_path.exists());

        // 读取并验证内容
        let content = std::fs::read_to_string(&manager.config_path).unwrap();
        assert!(content.contains("[tasks.test]"));
        assert!(content.contains("测试任务"));
    }

    #[test]
    fn test_remove_task() {
        let (manager, _temp_dir) = create_test_manager();

        // 先添加任务
        let config = TaskConfig {
            run: vec!["echo test".to_string()],
            description: None,
            depends: vec![],
            env: HashMap::new(),
            dir: None,
        };
        manager.add_task("test", &config).unwrap();

        // 再删除
        manager.remove_task("test").unwrap();

        // 验证已删除
        let content = std::fs::read_to_string(&manager.config_path).unwrap();
        assert!(!content.contains("[tasks.test]"));
    }

    #[test]
    fn test_set_env() {
        let (manager, _temp_dir) = create_test_manager();

        manager.set("TEST_VAR", "test_value").unwrap();

        let content = std::fs::read_to_string(&manager.config_path).unwrap();
        assert!(content.contains("[env]"));
        assert!(content.contains("TEST_VAR = \"test_value\""));
    }

    #[test]
    fn test_unset_env() {
        let (manager, _temp_dir) = create_test_manager();

        manager.set("TEST_VAR", "test_value").unwrap();
        manager.unset("TEST_VAR").unwrap();

        let content = std::fs::read_to_string(&manager.config_path).unwrap();
        assert!(!content.contains("TEST_VAR"));
    }

    #[test]
    fn test_load_env_file() {
        let (manager, temp_dir) = create_test_manager();

        // 创建 .env 文件
        let env_file = temp_dir.path().join(".env");
        std::fs::write(
            &env_file,
            "KEY1=value1\nKEY2=value2\n# comment\nKEY3=value3",
        )
        .unwrap();

        manager.load_file(&env_file).unwrap();

        let content = std::fs::read_to_string(&manager.config_path).unwrap();
        assert!(content.contains("KEY1 = \"value1\""));
        assert!(content.contains("KEY2 = \"value2\""));
        assert!(content.contains("KEY3 = \"value3\""));
        assert!(!content.contains("# comment"));
    }
}
