//! # cgroups v2 资源限制管理
//!
//! 本模块提供 cgroups v2 资源限制功能，用于控制服务的 CPU、内存等资源使用。
//!
//! ## 特性开关
//!
//! 此模块通过 Cargo feature `cgroups` 控制编译：
//! - **未启用 feature**: 编译时完全排除代码，零开销
//! - **启用 feature**: 编译相关代码，运行时仍需检测系统支持
//!
//! ## 平台兼容性
//!
//! - **Linux**: 支持 cgroups v2（内核 4.5+ 且挂载 unified hierarchy）
//! - **非 Linux**: 编译时排除，运行时自动禁用，不影响服务启动
//!
//! ## 优雅降级
//!
//! 即使在 Linux 平台，如果 cgroups v2 不可用（未挂载、权限不足等），
//! 模块会自动禁用并记录警告，不阻止服务正常运行。

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

/// 资源限制配置
#[derive(Debug, Clone, Default)]
pub struct ResourceLimits {
    /// CPU 配额（核心数，例如 0.5 = 50% 单核，2.0 = 2 核）
    pub cpu_quota: Option<f64>,

    /// 内存限制（字节数）
    pub memory_max: Option<u64>,
}

impl ResourceLimits {
    /// 检查是否有任何限制配置
    pub fn has_limits(&self) -> bool {
        self.cpu_quota.is_some() || self.memory_max.is_some()
    }
}

/// cgroups v2 管理器
pub struct CgroupManager {
    /// 是否实际启用（检测后的结果）
    enabled: bool,

    /// cgroup 根路径（通常是 /sys/fs/cgroup）
    cgroup_root: PathBuf,
}

impl CgroupManager {
    /// 创建 cgroup 管理器
    ///
    /// 会自动检测系统是否支持 cgroups v2：
    /// - 检查 /sys/fs/cgroup/cgroup.controllers 文件（v2 标识）
    /// - 检查创建权限（尝试创建测试 cgroup）
    ///
    /// 如果不支持，会记录警告并返回禁用状态的管理器。
    pub fn new() -> Result<Self> {
        let enabled = Self::detect_availability();

        if !enabled {
            tracing::warn!("cgroups v2 not available or disabled, resource limits will be ignored");
        } else {
            tracing::info!("cgroups v2 enabled and available");
        }

        Ok(Self {
            enabled,
            cgroup_root: PathBuf::from("/sys/fs/cgroup"),
        })
    }

    /// 检测 cgroups v2 是否可用
    ///
    /// 检测条件：
    /// 1. Linux 平台（非 Linux 平台返回 false）
    /// 2. cgroups v2 已挂载（/sys/fs/cgroup/cgroup.controllers 存在）
    /// 3. 有创建 cgroup 的权限（尝试创建测试目录）
    fn detect_availability() -> bool {
        #[cfg(not(target_os = "linux"))]
        {
            return false;
        }

        #[cfg(target_os = "linux")]
        {
            // 检查 cgroups v2 控制器文件（v2 标识）
            let controllers_path = Path::new("/sys/fs/cgroup/cgroup.controllers");
            if !controllers_path.exists() {
                tracing::debug!("cgroups v2 not available: cgroup.controllers file not found");
                return false;
            }

            // 检查是否有创建 cgroup 的权限
            match Self::can_create_cgroup() {
                Ok(true) => true,
                Ok(false) => {
                    tracing::debug!(
                        "cgroups v2 detected but cannot create cgroup (permission denied)"
                    );
                    false
                }
                Err(e) => {
                    tracing::debug!("Failed to check cgroup creation permission: {}", e);
                    false
                }
            }
        }
    }

    /// 检查是否能创建 cgroup（权限测试）
    ///
    /// 尝试创建测试 cgroup 目录并立即删除
    #[cfg(target_os = "linux")]
    fn can_create_cgroup() -> Result<bool> {
        let test_path = Path::new("/sys/fs/cgroup/svcmgr-test");

        // 尝试创建测试目录
        match fs::create_dir(test_path) {
            Ok(_) => {
                // 立即删除测试目录
                let _ = fs::remove_dir(test_path);
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // 测试目录已存在，说明之前创建过，有权限
                let _ = fs::remove_dir(test_path);
                Ok(true)
            }
            Err(e) => Err(anyhow!("Failed to test cgroup creation: {}", e)),
        }
    }

    /// 应用资源限制到进程
    ///
    /// # 参数
    ///
    /// - `name`: 服务名称（用于 cgroup 路径）
    /// - `limits`: 资源限制配置
    /// - `pid`: 进程 ID
    ///
    /// # 行为
    ///
    /// - **cgroups 启用**: 创建 cgroup 并设置限制，将进程加入 cgroup
    /// - **cgroups 禁用**: 记录调试日志，直接返回成功（优雅降级）
    ///
    /// # 错误
    ///
    /// 仅在 cgroups 启用且操作失败时返回错误。
    pub fn apply_limits(&self, name: &str, limits: &ResourceLimits, pid: u32) -> Result<()> {
        if !self.enabled {
            tracing::debug!("Skipping cgroup limits for '{}' (cgroups disabled)", name);
            return Ok(());
        }

        if !limits.has_limits() {
            tracing::debug!("No resource limits specified for '{}'", name);
            return Ok(());
        }

        tracing::info!(
            "Applying cgroup limits to '{}' (pid {}): {:?}",
            name,
            pid,
            limits
        );

        // 创建 cgroup 并设置限制
        self.create_service_cgroup(name, limits)?;

        // 将进程加入 cgroup
        self.attach_process(name, pid)?;

        Ok(())
    }

    /// 创建服务专属 cgroup 并设置资源限制
    ///
    /// cgroup 路径: `/sys/fs/cgroup/svcmgr/<service_name>`
    fn create_service_cgroup(&self, name: &str, limits: &ResourceLimits) -> Result<()> {
        let cgroup_path = self.get_service_cgroup_path(name);

        // 创建 cgroup 目录（如果已存在则忽略）
        fs::create_dir_all(&cgroup_path).with_context(|| {
            format!(
                "Failed to create cgroup directory: {}",
                cgroup_path.display()
            )
        })?;

        // 设置 CPU 限制
        if let Some(cpu_quota) = limits.cpu_quota {
            self.apply_cpu_limit(&cgroup_path, cpu_quota)?;
        }

        // 设置内存限制
        if let Some(memory_max) = limits.memory_max {
            self.apply_memory_limit(&cgroup_path, memory_max)?;
        }

        Ok(())
    }

    /// 设置 CPU 限制
    ///
    /// cgroups v2 格式: `cpu.max = "<配额微秒> <周期微秒>"`
    ///
    /// 例如: `50000 100000` 表示 50% CPU（100ms 周期内最多使用 50ms）
    fn apply_cpu_limit(&self, cgroup_path: &Path, cpu_quota: f64) -> Result<()> {
        const PERIOD_US: u64 = 100_000; // 100ms 周期（标准值）
        let quota_us = (cpu_quota * PERIOD_US as f64) as u64;

        let cpu_max_path = cgroup_path.join("cpu.max");
        let cpu_max_value = format!("{} {}", quota_us, PERIOD_US);

        fs::write(&cpu_max_path, cpu_max_value)
            .with_context(|| format!("Failed to write cpu.max: {}", cpu_max_path.display()))?;

        tracing::debug!("Set CPU limit: {} cores (quota={} us)", cpu_quota, quota_us);
        Ok(())
    }

    /// 设置内存限制
    ///
    /// cgroups v2 格式: `memory.max = <字节数>`
    fn apply_memory_limit(&self, cgroup_path: &Path, memory_max: u64) -> Result<()> {
        let memory_max_path = cgroup_path.join("memory.max");

        fs::write(&memory_max_path, memory_max.to_string()).with_context(|| {
            format!("Failed to write memory.max: {}", memory_max_path.display())
        })?;

        tracing::debug!(
            "Set memory limit: {} bytes ({} MB)",
            memory_max,
            memory_max / 1024 / 1024
        );
        Ok(())
    }

    /// 将进程加入 cgroup
    ///
    /// 写入 PID 到 `cgroup.procs` 文件
    fn attach_process(&self, name: &str, pid: u32) -> Result<()> {
        let cgroup_path = self.get_service_cgroup_path(name);
        let procs_path = cgroup_path.join("cgroup.procs");

        fs::write(&procs_path, pid.to_string())
            .with_context(|| format!("Failed to attach process {} to cgroup", pid))?;

        tracing::debug!(
            "Process {} attached to cgroup: {}",
            pid,
            cgroup_path.display()
        );
        Ok(())
    }

    /// 清理服务 cgroup
    ///
    /// 注意：只有当 cgroup 中没有进程时才能删除，因此这里使用 `.ok()` 忽略失败。
    pub fn cleanup_cgroup(&self, name: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let cgroup_path = self.get_service_cgroup_path(name);

        // 尝试删除 cgroup 目录（可能失败，因为进程可能还未完全退出）
        if let Err(e) = fs::remove_dir(&cgroup_path) {
            tracing::debug!(
                "Failed to remove cgroup '{}': {} (this is normal if process is still exiting)",
                name,
                e
            );
        } else {
            tracing::debug!("Cleaned up cgroup for service '{}'", name);
        }

        Ok(())
    }

    /// 获取服务 cgroup 路径
    ///
    /// 路径格式: `/sys/fs/cgroup/svcmgr/<service_name>`
    fn get_service_cgroup_path(&self, name: &str) -> PathBuf {
        self.cgroup_root.join("svcmgr").join(name)
    }

    /// 检查 cgroups 是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert!(!limits.has_limits());
    }

    #[test]
    fn test_resource_limits_with_cpu() {
        let limits = ResourceLimits {
            cpu_quota: Some(1.5),
            memory_max: None,
        };
        assert!(limits.has_limits());
    }

    #[test]
    fn test_resource_limits_with_memory() {
        let limits = ResourceLimits {
            cpu_quota: None,
            memory_max: Some(512 * 1024 * 1024),
        };
        assert!(limits.has_limits());
    }

    #[test]
    fn test_cgroup_manager_creation() {
        // 在测试环境中，CgroupManager::new() 应该总能成功创建（即使禁用）
        let result = CgroupManager::new();
        assert!(result.is_ok());

        let manager = result.unwrap();
        // 在 CI 环境中可能没有 cgroups v2，但不应该导致创建失败
        tracing::info!("CgroupManager enabled: {}", manager.is_enabled());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_cgroup_path_generation() {
        let manager = CgroupManager::new().unwrap();
        let path = manager.get_service_cgroup_path("test-service");

        assert_eq!(path, PathBuf::from("/sys/fs/cgroup/svcmgr/test-service"));
    }

    #[test]
    fn test_apply_limits_when_disabled() {
        // 创建禁用的管理器（通过强制设置 enabled = false）
        let manager = CgroupManager {
            enabled: false,
            cgroup_root: PathBuf::from("/sys/fs/cgroup"),
        };

        let limits = ResourceLimits {
            cpu_quota: Some(1.0),
            memory_max: Some(512 * 1024 * 1024),
        };

        // 禁用时应该直接返回成功（优雅降级）
        let result = manager.apply_limits("test-service", &limits, 12345);
        assert!(result.is_ok());
    }
}
