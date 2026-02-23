//! Runtime components for process management
//!
//! Phase 1.4: Basic process management
//! Phase 2.3: cgroups v2 resource limits (optional feature)

pub mod health_check;
pub mod process;

// cgroups v2 资源限制（可选特性）
#[cfg(feature = "cgroups")]
pub mod cgroups;

#[cfg(feature = "cgroups")]
pub use cgroups::{CgroupManager, ResourceLimits};
pub use health_check::{HealthCheck, HealthChecker};

pub use process::ProcessHandle;

// 如果 cgroups feature 未启用，提供占位类型（避免条件编译传播）
#[cfg(not(feature = "cgroups"))]
pub use cgroups_disabled::{CgroupManager, ResourceLimits};

#[cfg(not(feature = "cgroups"))]
mod cgroups_disabled {
    use anyhow::Result;

    /// 资源限制配置（禁用版本）
    #[derive(Debug, Clone, Default)]
    pub struct ResourceLimits {
        pub cpu_quota: Option<f64>,
        pub memory_max: Option<u64>,
    }

    impl ResourceLimits {
        pub fn has_limits(&self) -> bool {
            false
        }
    }

    /// cgroups 管理器（禁用版本）
    pub struct CgroupManager {
        _private: (),
    }

    impl CgroupManager {
        pub fn new() -> Result<Self> {
            tracing::debug!("cgroups feature disabled at compile time");
            Ok(Self { _private: () })
        }

        pub fn apply_limits(&self, _name: &str, _limits: &ResourceLimits, _pid: u32) -> Result<()> {
            Ok(())
        }

        pub fn cleanup_cgroup(&self, _name: &str) -> Result<()> {
            Ok(())
        }

        pub fn is_enabled(&self) -> bool {
            false
        }
    }
}
