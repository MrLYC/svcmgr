//! 环境变量管理模块
//!
//! 根据 OpenSpec 15-api-env.md 定义的环境变量管理功能。
//! 支持分层作用域、变量展开、循环引用检测。

pub mod expander;

use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// 数据结构定义
// ============================================================================

/// 环境变量作用域
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EnvScope {
    /// 全局作用域
    Global,
    /// 服务作用域
    Service { name: String },
    /// 任务作用域
    Task { name: String },
}

// ============================================================================
// 错误类型定义
// ============================================================================

/// 环境变量操作错误
#[derive(Debug)]
pub enum EnvError {
    /// 循环引用错误
    CircularReference { key: String, chain: Vec<String> },
    /// 最大递归深度超限
    MaxDepthExceeded { key: String, depth: usize },
    /// 配置错误
    ConfigError(String),
    /// IO 错误
    IoError(std::io::Error),
}

impl From<std::io::Error> for EnvError {
    fn from(e: std::io::Error) -> Self {
        EnvError::IoError(e)
    }
}

impl fmt::Display for EnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvError::CircularReference { key, chain } => {
                write!(
                    f,
                    "Circular reference detected for variable '{}': {}",
                    key,
                    chain.join(" -> ")
                )
            }
            EnvError::MaxDepthExceeded { key, depth } => {
                write!(
                    f,
                    "Variable expansion depth exceeded for '{}': {} > 10",
                    key, depth
                )
            }
            EnvError::ConfigError(msg) => write!(f, "Config error: {}", msg),
            EnvError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for EnvError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EnvError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

// Re-export commonly used types
pub use expander::VariableExpander;
