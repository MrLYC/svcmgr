//! # 健康检查机制
//!
//! 本模块提供服务健康检查功能，支持 HTTP、TCP 和命令执行三种探针类型。
//!
//! ## 探针类型
//!
//! - **HTTP**: GET 请求指定 URL，检查响应状态码
//! - **TCP**: 尝试连接指定端口，连接成功即健康
//! - **Command**: 执行命令，退出码为 0 即健康
//!
//! ## 配置示例
//!
//! ```toml
//! [services.api.health_check]
//! type = "http"
//! url = "http://localhost:3000/health"
//! interval = "10s"
//! timeout = "2s"
//! retries = 3
//! ```

use anyhow::{Context, Result};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::timeout;

/// 健康检查探针类型
#[derive(Debug, Clone)]
pub enum HealthCheck {
    /// HTTP 健康检查
    ///
    /// 发送 GET 请求到指定 URL，检查响应状态码
    Http {
        /// 健康检查 URL（例如: `http://localhost:3000/health`）
        url: String,

        /// 期望的 HTTP 状态码（默认: 200）
        expected_status: u16,

        /// 超时时间（默认: 2s）
        timeout: Duration,
    },

    /// TCP 端口检查
    ///
    /// 尝试连接指定主机和端口，连接成功即健康
    Tcp {
        /// 主机地址（例如: `localhost`）
        host: String,

        /// 端口号（例如: `3000`）
        port: u16,

        /// 超时时间（默认: 2s）
        timeout: Duration,
    },

    /// 命令执行检查
    ///
    /// 执行指定命令，退出码为期望值即健康
    Command {
        /// 命令（例如: `/usr/local/bin/check.sh`）
        command: String,

        /// 命令参数
        args: Vec<String>,

        /// 期望的退出码（默认: 0）
        expected_exit_code: i32,

        /// 超时时间（默认: 5s，命令执行通常较慢）
        timeout: Duration,
    },
}

impl HealthCheck {
    /// 创建 HTTP 健康检查
    pub fn http(url: String, timeout: Duration) -> Self {
        Self::Http {
            url,
            expected_status: 200,
            timeout,
        }
    }

    /// 创建 TCP 端口检查
    pub fn tcp(host: String, port: u16, timeout: Duration) -> Self {
        Self::Tcp {
            host,
            port,
            timeout,
        }
    }

    /// 创建命令执行检查
    pub fn command(command: String, args: Vec<String>, timeout: Duration) -> Self {
        Self::Command {
            command,
            args,
            expected_exit_code: 0,
            timeout,
        }
    }
}

/// 健康检查执行器
pub struct HealthChecker {
    /// HTTP 客户端（懒加载）
    http_client: Option<reqwest::Client>,
}

impl HealthChecker {
    /// 创建健康检查执行器
    pub fn new() -> Self {
        Self { http_client: None }
    }

    /// 执行健康检查
    ///
    /// 返回 `Ok(true)` 表示健康，`Ok(false)` 表示不健康，`Err` 表示检查失败（网络错误等）
    pub async fn check(&mut self, health_check: &HealthCheck) -> Result<bool> {
        match health_check {
            HealthCheck::Http {
                url,
                expected_status,
                timeout: check_timeout,
            } => self.check_http(url, *expected_status, *check_timeout).await,

            HealthCheck::Tcp {
                host,
                port,
                timeout: check_timeout,
            } => self.check_tcp(host, *port, *check_timeout).await,

            HealthCheck::Command {
                command,
                args,
                expected_exit_code,
                timeout: check_timeout,
            } => {
                self.check_command(command, args, *expected_exit_code, *check_timeout)
                    .await
            }
        }
    }

    /// HTTP 健康检查实现
    async fn check_http(
        &mut self,
        url: &str,
        expected_status: u16,
        check_timeout: Duration,
    ) -> Result<bool> {
        // 懒加载 HTTP 客户端
        if self.http_client.is_none() {
            self.http_client = Some(
                reqwest::Client::builder()
                    .timeout(check_timeout)
                    .build()
                    .context("Failed to create HTTP client")?,
            );
        }

        let client = self.http_client.as_ref().unwrap();

        match timeout(check_timeout, client.get(url).send()).await {
            Ok(Ok(response)) => {
                let status = response.status().as_u16();
                if status == expected_status {
                    tracing::debug!("HTTP health check passed: {} (status {})", url, status);
                    Ok(true)
                } else {
                    tracing::debug!(
                        "HTTP health check failed: {} (expected {}, got {})",
                        url,
                        expected_status,
                        status
                    );
                    Ok(false)
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("HTTP health check error: {} - {}", url, e);
                Err(e.into())
            }
            Err(_) => {
                tracing::warn!("HTTP health check timeout: {}", url);
                Ok(false)
            }
        }
    }

    /// TCP 端口检查实现
    async fn check_tcp(&self, host: &str, port: u16, check_timeout: Duration) -> Result<bool> {
        let addr = format!("{}:{}", host, port);

        match timeout(check_timeout, TcpStream::connect(&addr)).await {
            Ok(Ok(_stream)) => {
                tracing::debug!("TCP health check passed: {}", addr);
                Ok(true)
            }
            Ok(Err(e)) => {
                tracing::debug!("TCP health check failed: {} - {}", addr, e);
                Ok(false)
            }
            Err(_) => {
                tracing::warn!("TCP health check timeout: {}", addr);
                Ok(false)
            }
        }
    }

    /// 命令执行检查实现
    async fn check_command(
        &self,
        command: &str,
        args: &[String],
        expected_exit_code: i32,
        check_timeout: Duration,
    ) -> Result<bool> {
        let mut cmd = Command::new(command);
        cmd.args(args);

        match timeout(check_timeout, cmd.status()).await {
            Ok(Ok(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                if exit_code == expected_exit_code {
                    tracing::debug!(
                        "Command health check passed: {} (exit code {})",
                        command,
                        exit_code
                    );
                    Ok(true)
                } else {
                    tracing::debug!(
                        "Command health check failed: {} (expected {}, got {})",
                        command,
                        expected_exit_code,
                        exit_code
                    );
                    Ok(false)
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Command health check error: {} - {}", command, e);
                Err(e.into())
            }
            Err(_) => {
                tracing::warn!("Command health check timeout: {}", command);
                Ok(false)
            }
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_health_check_creation() {
        let check = HealthCheck::http(
            "http://localhost:3000/health".to_string(),
            Duration::from_secs(2),
        );

        if let HealthCheck::Http {
            url,
            expected_status,
            timeout,
        } = check
        {
            assert_eq!(url, "http://localhost:3000/health");
            assert_eq!(expected_status, 200);
            assert_eq!(timeout, Duration::from_secs(2));
        } else {
            panic!("Expected Http variant");
        }
    }

    #[tokio::test]
    async fn test_tcp_health_check_creation() {
        let check = HealthCheck::tcp("localhost".to_string(), 3000, Duration::from_secs(2));

        if let HealthCheck::Tcp {
            host,
            port,
            timeout,
        } = check
        {
            assert_eq!(host, "localhost");
            assert_eq!(port, 3000);
            assert_eq!(timeout, Duration::from_secs(2));
        } else {
            panic!("Expected Tcp variant");
        }
    }

    #[tokio::test]
    async fn test_command_health_check_creation() {
        let check = HealthCheck::command("/bin/true".to_string(), vec![], Duration::from_secs(5));

        if let HealthCheck::Command {
            command,
            args,
            expected_exit_code,
            timeout,
        } = check
        {
            assert_eq!(command, "/bin/true");
            assert_eq!(args.len(), 0);
            assert_eq!(expected_exit_code, 0);
            assert_eq!(timeout, Duration::from_secs(5));
        } else {
            panic!("Expected Command variant");
        }
    }

    #[tokio::test]
    async fn test_health_checker_creation() {
        let checker = HealthChecker::new();
        assert!(checker.http_client.is_none());
    }

    #[tokio::test]
    async fn test_command_health_check_success() {
        let mut checker = HealthChecker::new();
        let check = HealthCheck::command("/bin/true".to_string(), vec![], Duration::from_secs(5));

        let result = checker.check(&check).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_command_health_check_failure() {
        let mut checker = HealthChecker::new();
        let check = HealthCheck::command("/bin/false".to_string(), vec![], Duration::from_secs(5));

        let result = checker.check(&check).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_tcp_health_check_localhost() {
        // 测试连接到 localhost:1（假设未开放端口）
        let mut checker = HealthChecker::new();
        let check = HealthCheck::tcp("localhost".to_string(), 1, Duration::from_millis(100));

        let result = checker.check(&check).await;
        // 端口未开放，应该返回 Ok(false)
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
