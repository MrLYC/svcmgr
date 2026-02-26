//! MiseMock - 模拟 mise CLI 行为的测试工具
//!
//! 用于单元测试和集成测试中模拟 mise 的配置和命令执行，
//! 避免测试依赖真实的 mise 安装。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Output;

/// mise 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDef {
    /// 任务执行的命令
    pub run: String,
    /// 任务特定的环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// 任务依赖的其他任务
    #[serde(default)]
    pub depends: Vec<String>,
    /// 任务描述
    #[serde(default)]
    pub description: Option<String>,
}

/// MiseMock - 模拟 mise CLI 行为
pub struct MiseMock {
    /// 工具名称 -> 版本映射 (例如: "node" -> "20.11.0")
    pub tools: HashMap<String, String>,
    /// 环境变量映射
    pub env: HashMap<String, String>,
    /// 任务名称 -> 任务定义映射
    pub tasks: HashMap<String, TaskDef>,
    /// 工作目录（用于生成 .mise.toml）
    pub workdir: PathBuf,
    /// 服务名称 -> 服务环境变量映射
    pub service_envs: HashMap<String, HashMap<String, String>>,
}

impl MiseMock {
    /// 创建新的 MiseMock 实例
    pub fn new(workdir: PathBuf) -> Self {
        Self {
            tools: HashMap::new(),
            env: HashMap::new(),
            tasks: HashMap::new(),
            workdir,
            service_envs: HashMap::new(),
        }
    }

    /// 添加工具 (builder pattern)
    pub fn with_tool(mut self, name: &str, version: &str) -> Self {
        self.tools.insert(name.to_string(), version.to_string());
        self
    }

    /// 添加环境变量 (builder pattern)
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }

    /// 添加任务定义 (builder pattern)
    pub fn with_task(mut self, name: &str, task: TaskDef) -> Self {
        self.tasks.insert(name.to_string(), task);
        self
    }

    /// 写入 mise 配置文件到 workdir/.mise.toml
    pub fn write_config(&self) -> Result<PathBuf> {
        let config_path = self.workdir.join(".mise.toml");
        let mut content = String::new();

        // 写入 tools 配置
        if !self.tools.is_empty() {
            content.push_str("[tools]\n");
            for (name, version) in &self.tools {
                content.push_str(&format!("{} = \"{}\"\n", name, version));
            }
            content.push('\n');
        }

        // 写入 env 配置
        if !self.env.is_empty() {
            content.push_str("[env]\n");
            for (key, value) in &self.env {
                content.push_str(&format!("{} = \"{}\"\n", key, value));
            }
            content.push('\n');
        }

        // 写入 tasks 配置
        if !self.tasks.is_empty() {
            for (name, task) in &self.tasks {
                content.push_str(&format!("[tasks.{}]\n", name));
                content.push_str(&format!("run = \"{}\"\n", task.run));

                if let Some(desc) = &task.description {
                    content.push_str(&format!("description = \"{}\"\n", desc));
                }

                if !task.env.is_empty() {
                    content.push_str("env = { ");
                    let env_pairs: Vec<String> = task
                        .env
                        .iter()
                        .map(|(k, v)| format!("{} = \"{}\"", k, v))
                        .collect();
                    content.push_str(&env_pairs.join(", "));
                    content.push_str(" }\n");
                }

                if !task.depends.is_empty() {
                    content.push_str(&format!("depends = {:?}\n", task.depends));
                }

                content.push('\n');
            }
        }

        fs::write(&config_path, content).context("Failed to write mise config")?;

        Ok(config_path)
    }

    /// 模拟执行 mise 任务
    ///
    /// 注意: 这只是返回一个模拟的 Output，不会真正执行命令
    pub fn mock_exec(&self, task_name: &str) -> Result<Output> {
        let task = self
            .tasks
            .get(task_name)
            .context(format!("Task '{}' not found in MiseMock", task_name))?;

        // 返回一个模拟的成功输出
        Ok(Output {
            status: std::process::ExitStatus::default(),
            stdout: format!(
                "Mock execution of task: {}\nCommand: {}\n",
                task_name, task.run
            )
            .into_bytes(),
            stderr: Vec::new(),
        })
    }

    /// 获取所有环境变量 (包括全局和任务特定的)
    pub fn get_env_vars(&self, task_name: Option<&str>) -> HashMap<String, String> {
        let mut env = self.env.clone();

        // 如果指定了任务，添加任务特定的环境变量
        if let Some(name) = task_name {
            if let Some(task) = self.tasks.get(name) {
                env.extend(task.env.clone());
            }
        }

        env
    }

    /// 列出所有任务名称
    pub fn list_tasks(&self) -> Vec<String> {
        self.tasks.keys().cloned().collect()
    }

    /// 检查任务是否存在
    pub fn has_task(&self, task_name: &str) -> bool {
        self.tasks.contains_key(task_name)
    }

    /// 获取任务定义
    pub fn get_task(&self, task_name: &str) -> Option<&TaskDef> {
        self.tasks.get(task_name)
    }

    /// 解析任务的依赖链 (深度优先遍历)
    /// 返回按执行顺序排列的任务列表
    pub fn resolve_task_dependencies(&self, task_name: &str) -> Result<Vec<String>> {
        let mut resolved = Vec::new();
        let mut visited = std::collections::HashSet::new();

        self.visit_task(task_name, &mut resolved, &mut visited)?;

        Ok(resolved)
    }

    fn visit_task(
        &self,
        task_name: &str,
        resolved: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        // 检测循环依赖
        if visited.contains(task_name) {
            anyhow::bail!("Circular dependency detected: {}", task_name);
        }

        visited.insert(task_name.to_string());

        let task = self
            .tasks
            .get(task_name)
            .context(format!("Task '{}' not found", task_name))?;

        // 先解析依赖
        for dep in &task.depends {
            if !resolved.contains(dep) {
                self.visit_task(dep, resolved, visited)?;
            }
        }

        // 再添加当前任务
        if !resolved.contains(&task_name.to_string()) {
            resolved.push(task_name.to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mise_mock_basic() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_tool("node", "20.11.0")
            .with_env("NODE_ENV", "development");

        assert_eq!(mock.tools.get("node"), Some(&"20.11.0".to_string()));
        assert_eq!(mock.env.get("NODE_ENV"), Some(&"development".to_string()));
    }

    #[test]
    fn test_write_config() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_tool("node", "20.11.0")
            .with_env("NODE_ENV", "development")
            .with_task(
                "dev",
                TaskDef {
                    run: "npm run dev".to_string(),
                    env: HashMap::new(),
                    depends: Vec::new(),
                    description: Some("Start development server".to_string()),
                },
            );

        let config_path = mock.write_config().unwrap();
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[tools]"));
        assert!(content.contains("node = \"20.11.0\""));
        assert!(content.contains("[env]"));
        assert!(content.contains("NODE_ENV = \"development\""));
        assert!(content.contains("[tasks.dev]"));
        assert!(content.contains("run = \"npm run dev\""));
    }

    #[test]
    fn test_task_dependencies() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_task(
                "build",
                TaskDef {
                    run: "cargo build".to_string(),
                    env: HashMap::new(),
                    depends: vec!["test".to_string()],
                    description: None,
                },
            )
            .with_task(
                "test",
                TaskDef {
                    run: "cargo test".to_string(),
                    env: HashMap::new(),
                    depends: Vec::new(),
                    description: None,
                },
            );

        let deps = mock.resolve_task_dependencies("build").unwrap();
        assert_eq!(deps, vec!["test", "build"]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_task(
                "a",
                TaskDef {
                    run: "echo a".to_string(),
                    env: HashMap::new(),
                    depends: vec!["b".to_string()],
                    description: None,
                },
            )
            .with_task(
                "b",
                TaskDef {
                    run: "echo b".to_string(),
                    env: HashMap::new(),
                    depends: vec!["a".to_string()],
                    description: None,
                },
            );

        let result = mock.resolve_task_dependencies("a");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }

    #[test]
    fn test_mock_exec() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf()).with_task(
            "hello",
            TaskDef {
                run: "echo hello".to_string(),
                env: HashMap::new(),
                depends: Vec::new(),
                description: None,
            },
        );

        let output = mock.mock_exec("hello").unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Mock execution of task: hello"));
        assert!(stdout.contains("Command: echo hello"));
    }

    #[test]
    fn test_get_env_vars() {
        let temp = TempDir::new().unwrap();
        let mut task_env = HashMap::new();
        task_env.insert("TASK_VAR".to_string(), "task_value".to_string());

        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_env("GLOBAL_VAR", "global_value")
            .with_task(
                "dev",
                TaskDef {
                    run: "npm run dev".to_string(),
                    env: task_env,
                    depends: Vec::new(),
                    description: None,
                },
            );

        // 全局环境变量
        let global_env = mock.get_env_vars(None);
        assert_eq!(
            global_env.get("GLOBAL_VAR"),
            Some(&"global_value".to_string())
        );
        assert!(!global_env.contains_key("TASK_VAR"));

        // 任务特定的环境变量
        let task_env = mock.get_env_vars(Some("dev"));
        assert_eq!(
            task_env.get("GLOBAL_VAR"),
            Some(&"global_value".to_string())
        );
        assert_eq!(task_env.get("TASK_VAR"), Some(&"task_value".to_string()));
    }
}
