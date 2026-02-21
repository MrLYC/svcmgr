#![allow(dead_code)]

/// Crontab 周期任务管理原子
///
/// 本模块提供 Crontab 用户级周期任务管理功能：
/// - 任务 CRUD 操作（添加、更新、删除、查询）
/// - 任务列表和状态查询
/// - Cron 表达式验证和下次执行时间预测
/// - 环境变量管理
/// - 仅管理带 [svcmgr:*] 标识的 crontab 条目
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

// ========================================
// 数据结构
// ========================================

/// Cron 任务配置
#[derive(Debug, Clone)]
pub struct CronTask {
    /// 任务 ID（自动生成或指定）
    pub id: Option<String>,
    /// 任务描述
    pub description: String,
    /// Cron 表达式（支持标准格式和预定义格式如 @hourly）
    pub expression: String,
    /// 执行命令
    pub command: String,
    /// 环境变量
    pub env: HashMap<String, String>,
    /// 是否启用
    pub enabled: bool,
}

// ========================================
// CrontabAtom Trait
// ========================================

/// Crontab 周期任务管理 trait
pub trait CrontabAtom {
    /// 添加新的 cron 任务
    ///
    /// # 参数
    /// - `task`: 任务配置
    ///
    /// # 返回
    /// - 生成的任务 ID
    fn add(&self, task: &CronTask) -> Result<String>;

    /// 更新已存在的 cron 任务
    ///
    /// # 参数
    /// - `task_id`: 任务 ID
    /// - `task`: 新的任务配置
    fn update(&self, task_id: &str, task: &CronTask) -> Result<()>;

    /// 删除 cron 任务
    ///
    /// # 参数
    /// - `task_id`: 任务 ID
    fn remove(&self, task_id: &str) -> Result<()>;

    /// 获取指定任务信息
    ///
    /// # 参数
    /// - `task_id`: 任务 ID
    fn get(&self, task_id: &str) -> Result<CronTask>;

    /// 列出所有 svcmgr 管理的任务
    fn list(&self) -> Result<Vec<CronTask>>;

    /// 预测任务的下 N 次执行时间
    ///
    /// # 参数
    /// - `task_id`: 任务 ID
    /// - `count`: 预测次数
    fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>>;

    /// 验证 cron 表达式是否合法
    ///
    /// # 参数
    /// - `expr`: cron 表达式
    fn validate_expression(&self, expr: &str) -> Result<bool>;

    /// 设置全局环境变量
    ///
    /// # 参数
    /// - `key`: 变量名
    /// - `value`: 变量值
    fn set_env(&self, key: &str, value: &str) -> Result<()>;

    /// 获取全局环境变量
    fn get_env(&self) -> Result<HashMap<String, String>>;

    /// 重新加载 crontab
    fn reload(&self) -> Result<()>;
}

// ========================================
// CrontabManager 实现
// ========================================

/// Crontab 管理器，使用用户级 crontab
pub struct CrontabManager {
    /// 任务 ID 前缀
    prefix: String,
}

impl CrontabManager {
    /// 创建新的 Crontab 管理器
    pub fn new() -> Self {
        Self {
            prefix: "svcmgr".to_string(),
        }
    }

    /// 生成任务 ID
    fn generate_task_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        format!("{}", timestamp)
    }

    /// 读取当前 crontab
    fn read_crontab(&self) -> Result<String> {
        let output = Command::new("crontab").arg("-l").output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    // crontab -l 在没有 crontab 时返回非零，这是正常的
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if stderr.contains("no crontab") {
                        Ok(String::new())
                    } else {
                        Err(Error::CommandFailed {
                            command: "crontab -l".to_string(),
                            exit_code: output.status.code(),
                            stderr: stderr.to_string(),
                        })
                    }
                }
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    /// 写入 crontab
    fn write_crontab(&self, content: &str) -> Result<()> {
        let output = Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(content.as_bytes())?;
                }
                child.wait()
            });

        match output {
            Ok(status) => {
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::CommandFailed {
                        command: "crontab -".to_string(),
                        exit_code: status.code(),
                        stderr: "Failed to write crontab".to_string(),
                    })
                }
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    /// 解析 crontab 内容
    fn parse_crontab(
        &self,
        content: &str,
    ) -> (Vec<CronTask>, Vec<String>, HashMap<String, String>) {
        let mut tasks = Vec::new();
        let mut other_lines = Vec::new();
        let mut env_vars = HashMap::new();
        let mut current_task: Option<(String, String)> = None; // (id, description)

        for line in content.lines() {
            let trimmed = line.trim();

            // 解析 svcmgr 任务注释
            if trimmed.starts_with(&format!("# [{}:", self.prefix)) {
                if let Some(id_end) = trimmed.find(']') {
                    let id_start = format!("# [{}:", self.prefix).len();
                    let id = trimmed[id_start..id_end].to_string();
                    let description = trimmed[id_end + 1..].trim().to_string();
                    current_task = Some((id, description));
                }
                continue;
            }

            // 解析环境变量
            if trimmed.contains('=')
                && !trimmed.starts_with('#')
                && !trimmed.contains(' ')
                && let Some((key, value)) = trimmed.split_once('=')
            {
                env_vars.insert(key.to_string(), value.to_string());
                other_lines.push(line.to_string());
                continue;
            }

            // 解析 cron 任务行
            if let Some((id, description)) = current_task.take() {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if !parts.is_empty() {
                    let (expression, command) = if parts[0].starts_with('@') {
                        (parts[0].to_string(), parts[1..].join(" "))
                    } else if parts.len() >= 6 {
                        (parts[0..5].join(" "), parts[5..].join(" "))
                    } else {
                        other_lines.push(line.to_string());
                        continue;
                    };

                    tasks.push(CronTask {
                        id: Some(id),
                        description,
                        expression,
                        command,
                        env: HashMap::new(),
                        enabled: !line.trim_start().starts_with('#'),
                    });
                    continue;
                }
            }

            // 其他行保持原样
            if !trimmed.is_empty() {
                other_lines.push(line.to_string());
            }
        }

        (tasks, other_lines, env_vars)
    }

    /// 构建 crontab 内容
    fn build_crontab(
        &self,
        tasks: &[CronTask],
        other_lines: &[String],
        env_vars: &HashMap<String, String>,
    ) -> String {
        let mut lines = Vec::new();

        // 添加环境变量
        for (key, value) in env_vars {
            lines.push(format!("{}={}", key, value));
        }

        if !env_vars.is_empty() && (!tasks.is_empty() || !other_lines.is_empty()) {
            lines.push(String::new()); // 空行分隔
        }

        // 添加其他行
        for line in other_lines {
            if !line.trim().is_empty() {
                lines.push(line.clone());
            }
        }

        if !other_lines.is_empty() && !tasks.is_empty() {
            lines.push(String::new()); // 空行分隔
        }

        // 添加 svcmgr 管理的任务
        for task in tasks {
            let id = task.id.as_ref().unwrap();
            lines.push(format!("# [{}:{}] {}", self.prefix, id, task.description));

            let prefix = if task.enabled { "" } else { "# " };
            lines.push(format!("{}{} {}", prefix, task.expression, task.command));
        }

        lines.join("\n") + "\n"
    }

    /// 规范化 cron 表达式（将预定义格式转换为标准格式）
    fn normalize_expression(&self, expr: &str) -> String {
        match expr.trim() {
            "@yearly" | "@annually" => "0 0 1 1 *".to_string(),
            "@monthly" => "0 0 1 * *".to_string(),
            "@weekly" => "0 0 * * 1".to_string(), // 1 = Monday in cron library
            "@daily" | "@midnight" => "0 0 * * *".to_string(),
            "@hourly" => "0 * * * *".to_string(),
            other => other.to_string(),
        }
    }

    /// 将标准 5 字段 cron 表达式转换为带秒的 6 字段格式（cron 库需要）
    fn to_schedule_format(&self, expr: &str) -> String {
        let normalized = self.normalize_expression(expr);
        format!("0 {}", normalized) // 添加秒字段
    }
}

impl Default for CrontabManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CrontabAtom for CrontabManager {
    fn add(&self, task: &CronTask) -> Result<String> {
        // 验证 cron 表达式
        self.validate_expression(&task.expression)?;

        // 读取现有 crontab
        let content = self.read_crontab()?;
        let (mut tasks, other_lines, env_vars) = self.parse_crontab(&content);

        // 生成任务 ID
        let task_id = task.id.clone().unwrap_or_else(|| self.generate_task_id());

        // 检查任务 ID 是否已存在
        if tasks.iter().any(|t| t.id.as_ref() == Some(&task_id)) {
            return Err(Error::InvalidArgument(format!(
                "Task ID {} already exists",
                task_id
            )));
        }

        // 添加新任务
        let mut new_task = task.clone();
        new_task.id = Some(task_id.clone());
        tasks.push(new_task);

        // 构建并写入新的 crontab
        let new_content = self.build_crontab(&tasks, &other_lines, &env_vars);
        self.write_crontab(&new_content)?;

        Ok(task_id)
    }

    fn update(&self, task_id: &str, task: &CronTask) -> Result<()> {
        // 验证 cron 表达式
        self.validate_expression(&task.expression)?;

        // 读取现有 crontab
        let content = self.read_crontab()?;
        let (mut tasks, other_lines, env_vars) = self.parse_crontab(&content);

        // 查找并更新任务
        let task_index = tasks
            .iter()
            .position(|t| t.id.as_ref() == Some(&task_id.to_string()))
            .ok_or_else(|| Error::NotSupported(format!("Task {} not found", task_id)))?;

        let mut updated_task = task.clone();
        updated_task.id = Some(task_id.to_string());
        tasks[task_index] = updated_task;

        // 构建并写入新的 crontab
        let new_content = self.build_crontab(&tasks, &other_lines, &env_vars);
        self.write_crontab(&new_content)?;

        Ok(())
    }

    fn remove(&self, task_id: &str) -> Result<()> {
        // 读取现有 crontab
        let content = self.read_crontab()?;
        let (mut tasks, other_lines, env_vars) = self.parse_crontab(&content);

        // 查找并删除任务
        let original_len = tasks.len();
        tasks.retain(|t| t.id.as_ref() != Some(&task_id.to_string()));

        if tasks.len() == original_len {
            return Err(Error::NotSupported(format!("Task {} not found", task_id)));
        }

        // 构建并写入新的 crontab
        let new_content = self.build_crontab(&tasks, &other_lines, &env_vars);
        self.write_crontab(&new_content)?;

        Ok(())
    }

    fn get(&self, task_id: &str) -> Result<CronTask> {
        let content = self.read_crontab()?;
        let (tasks, _, _) = self.parse_crontab(&content);

        tasks
            .into_iter()
            .find(|t| t.id.as_ref() == Some(&task_id.to_string()))
            .ok_or_else(|| Error::NotSupported(format!("Task {} not found", task_id)))
    }

    fn list(&self) -> Result<Vec<CronTask>> {
        let content = self.read_crontab()?;
        let (tasks, _, _) = self.parse_crontab(&content);
        Ok(tasks)
    }

    fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>> {
        let task = self.get(task_id)?;

        // 转换为带秒的格式
        let schedule_expr = self.to_schedule_format(&task.expression);

        // 解析 cron 表达式
        let schedule = Schedule::from_str(&schedule_expr)
            .map_err(|e| Error::InvalidArgument(format!("Invalid cron expression: {}", e)))?;

        // 计算下 N 次执行时间
        let now = Utc::now();
        let upcoming: Vec<DateTime<Utc>> = schedule.after(&now).take(count).collect();

        Ok(upcoming)
    }

    fn validate_expression(&self, expr: &str) -> Result<bool> {
        // 转换为带秒的格式
        let schedule_expr = self.to_schedule_format(expr);

        // 尝试解析
        Schedule::from_str(&schedule_expr)
            .map(|_| true)
            .map_err(|e| Error::InvalidArgument(format!("Invalid cron expression: {}", e)))
    }

    fn set_env(&self, key: &str, value: &str) -> Result<()> {
        // 读取现有 crontab
        let content = self.read_crontab()?;
        let (tasks, other_lines, mut env_vars) = self.parse_crontab(&content);

        // 设置环境变量
        env_vars.insert(key.to_string(), value.to_string());

        // 构建并写入新的 crontab
        let new_content = self.build_crontab(&tasks, &other_lines, &env_vars);
        self.write_crontab(&new_content)?;

        Ok(())
    }

    fn get_env(&self) -> Result<HashMap<String, String>> {
        let content = self.read_crontab()?;
        let (_, _, env_vars) = self.parse_crontab(&content);
        Ok(env_vars)
    }

    fn reload(&self) -> Result<()> {
        // crontab 自动重新加载，无需额外操作
        // 这个方法主要用于一致性和未来扩展
        Ok(())
    }
}

// ========================================
// 单元测试
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task() -> CronTask {
        CronTask {
            id: None,
            description: "Test task".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo 'test'".to_string(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    #[test]
    fn test_normalize_expression() {
        let manager = CrontabManager::new();

        assert_eq!(manager.normalize_expression("@hourly"), "0 * * * *");
        assert_eq!(manager.normalize_expression("@daily"), "0 0 * * *");
        assert_eq!(manager.normalize_expression("@weekly"), "0 0 * * 1");
        assert_eq!(manager.normalize_expression("@monthly"), "0 0 1 * *");
        assert_eq!(manager.normalize_expression("@yearly"), "0 0 1 1 *");
        assert_eq!(manager.normalize_expression("0 2 * * *"), "0 2 * * *");
    }

    #[test]
    fn test_to_schedule_format() {
        let manager = CrontabManager::new();

        assert_eq!(manager.to_schedule_format("0 2 * * *"), "0 0 2 * * *");
        assert_eq!(manager.to_schedule_format("@hourly"), "0 0 * * * *");
        assert_eq!(manager.to_schedule_format("@daily"), "0 0 0 * * *");
    }

    #[test]
    fn test_validate_expression() {
        let manager = CrontabManager::new();

        // 有效的表达式
        assert!(manager.validate_expression("0 2 * * *").is_ok());
        assert!(manager.validate_expression("*/5 * * * *").is_ok());
        assert!(manager.validate_expression("@hourly").is_ok());
        assert!(manager.validate_expression("@daily").is_ok());

        // 无效的表达式
        assert!(manager.validate_expression("invalid").is_err());
        assert!(manager.validate_expression("0 25 * * *").is_err()); // 无效小时
    }

    #[test]
    fn test_parse_crontab() {
        let manager = CrontabManager::new();

        let content = r#"SHELL=/bin/bash
PATH=/usr/bin:/bin

# [svcmgr:123456] Backup database
0 2 * * * /usr/local/bin/backup.sh

# [svcmgr:789012] Cleanup logs
0 3 * * * /usr/local/bin/cleanup.sh
"#;

        let (tasks, other_lines, env_vars) = manager.parse_crontab(content);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, Some("123456".to_string()));
        assert_eq!(tasks[0].description, "Backup database");
        assert_eq!(tasks[0].expression, "0 2 * * *");
        assert_eq!(tasks[0].command, "/usr/local/bin/backup.sh");

        assert_eq!(env_vars.get("SHELL"), Some(&"/bin/bash".to_string()));
        assert_eq!(env_vars.get("PATH"), Some(&"/usr/bin:/bin".to_string()));

        assert_eq!(other_lines.len(), 2); // 环境变量行
    }

    #[test]
    fn test_build_crontab() {
        let manager = CrontabManager::new();

        let tasks = vec![CronTask {
            id: Some("123456".to_string()),
            description: "Test task".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "/usr/local/bin/test.sh".to_string(),
            env: HashMap::new(),
            enabled: true,
        }];

        let mut env_vars = HashMap::new();
        env_vars.insert("SHELL".to_string(), "/bin/bash".to_string());

        let content = manager.build_crontab(&tasks, &[], &env_vars);

        assert!(content.contains("SHELL=/bin/bash"));
        assert!(content.contains("# [svcmgr:123456] Test task"));
        assert!(content.contains("0 2 * * * /usr/local/bin/test.sh"));
    }

    #[test]
    fn test_build_crontab_with_disabled_task() {
        let manager = CrontabManager::new();

        let tasks = vec![CronTask {
            id: Some("123456".to_string()),
            description: "Disabled task".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "/usr/local/bin/test.sh".to_string(),
            env: HashMap::new(),
            enabled: false,
        }];

        let content = manager.build_crontab(&tasks, &[], &HashMap::new());

        assert!(content.contains("# [svcmgr:123456] Disabled task"));
        assert!(content.contains("# 0 2 * * * /usr/local/bin/test.sh"));
    }

    #[test]
    fn test_generate_task_id() {
        let manager = CrontabManager::new();

        let id1 = manager.generate_task_id();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let id2 = manager.generate_task_id();

        assert_ne!(id1, id2);
        assert!(id1.parse::<u64>().is_ok());
    }

    #[test]
    fn test_cron_task_creation() {
        let task = create_test_task();

        assert_eq!(task.description, "Test task");
        assert_eq!(task.expression, "0 2 * * *");
        assert_eq!(task.command, "echo 'test'");
        assert!(task.enabled);
        assert!(task.id.is_none());
    }

    #[test]
    fn test_parse_crontab_with_predefined_expressions() {
        let manager = CrontabManager::new();

        let content = r#"# [svcmgr:111] Hourly task
@hourly /usr/local/bin/hourly.sh

# [svcmgr:222] Daily task
@daily /usr/local/bin/daily.sh
"#;

        let (tasks, _, _) = manager.parse_crontab(content);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].expression, "@hourly");
        assert_eq!(tasks[1].expression, "@daily");
    }

    #[test]
    fn test_parse_crontab_preserves_other_entries() {
        let manager = CrontabManager::new();

        let content = r#"# User's personal crontab entry
0 1 * * * /home/user/backup.sh

# [svcmgr:123] Managed task
0 2 * * * /usr/local/bin/managed.sh
"#;

        let (tasks, other_lines, _) = manager.parse_crontab(content);

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, Some("123".to_string()));

        // 检查其他行是否被保留
        let other_content = other_lines.join("\n");
        assert!(other_content.contains("# User's personal crontab entry"));
        assert!(other_content.contains("0 1 * * * /home/user/backup.sh"));
    }

    #[test]
    fn test_validate_predefined_expressions() {
        let manager = CrontabManager::new();

        assert!(manager.validate_expression("@hourly").is_ok());
        assert!(manager.validate_expression("@daily").is_ok());
        assert!(manager.validate_expression("@weekly").is_ok());
        assert!(manager.validate_expression("@monthly").is_ok());
        assert!(manager.validate_expression("@yearly").is_ok());
        assert!(manager.validate_expression("@annually").is_ok());
    }

    /// 测试无效 cron 表达式
    #[test]
    fn test_validate_invalid_expressions() {
        let manager = CrontabManager::new();

        assert!(manager.validate_expression("").is_err());
        assert!(manager.validate_expression("not a cron").is_err());
        assert!(manager.validate_expression("60 * * * *").is_err());
    }

    /// 测试 parse_crontab 空输入
    #[test]
    fn test_parse_crontab_empty() {
        let manager = CrontabManager::new();

        let (tasks, other_lines, env_vars) = manager.parse_crontab("");
        assert_eq!(tasks.len(), 0);
        assert_eq!(other_lines.len(), 0);
        assert_eq!(env_vars.len(), 0);
    }

    /// 测试 normalize_expression 边界情况
    #[test]
    fn test_normalize_expression_edge_cases() {
        let manager = CrontabManager::new();

        assert_eq!(manager.normalize_expression("@reboot"), "@reboot");
        assert_eq!(manager.normalize_expression(""), "");
    }

    /// 测试 to_schedule_format 月度表达式
    #[test]
    fn test_to_schedule_format_monthly() {
        let manager = CrontabManager::new();

        assert_eq!(manager.to_schedule_format("@monthly"), "0 0 0 1 * *");
    }

    /// 测试 build_crontab 带环境变量
    #[test]
    fn test_build_crontab_with_env() {
        let manager = CrontabManager::new();

        let mut env_vars = HashMap::new();
        env_vars.insert("PATH".to_string(), "/usr/bin".to_string());

        let content = manager.build_crontab(&[], &[], &env_vars);
        assert!(content.contains("PATH=/usr/bin"));
    }
}
