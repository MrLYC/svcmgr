//! mise Adapter factory and version detection
//!
//! Implements AdapterFactory that detects mise version and routes to appropriate adapter.

use crate::ports::{ConfigPort, DependencyPort, EnvPort, MiseVersion, TaskPort};
use anyhow::{Context, Result};
use std::process::Command;

pub mod command;
pub mod parser;
pub mod v2026;

pub use command::MiseCommand;
pub use v2026::MiseV2026Adapter;

pub trait MiseAdapter: DependencyPort + TaskPort + EnvPort + ConfigPort {}

pub struct AdapterFactory {
    version: MiseVersion,
}

impl AdapterFactory {
    pub fn new() -> Result<Self> {
        let version = Self::detect_mise_version()?;
        Ok(Self { version })
    }

    fn detect_mise_version() -> Result<MiseVersion> {
        let output = Command::new("mise")
            .arg("--version")
            .output()
            .context("Failed to execute 'mise --version'. Is mise installed?")?;

        if !output.status.success() {
            anyhow::bail!(
                "mise --version failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            );
        }

        let version_str =
            String::from_utf8(output.stdout).context("mise --version output is not valid UTF-8")?;

        MiseVersion::parse(&version_str)
            .with_context(|| format!("Failed to parse mise version from: {}", version_str))
    }

    pub fn create(&self) -> Box<dyn MiseAdapter> {
        if self.version >= MiseVersion::new(2026, 0, 0) {
            Box::new(MiseV2026Adapter::new(self.version.clone()))
        } else if self.version >= MiseVersion::new(2025, 0, 0) {
            panic!(
                "mise version {} is supported but v2025 adapter not yet implemented. Please use mise >= 2026.0.0",
                self.version
            );
        } else {
            panic!(
                "mise version {} is not supported. Minimum version: 2025.0.0. Please upgrade mise.",
                self.version
            );
        }
    }

    pub fn mise_version(&self) -> &MiseVersion {
        &self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_factory_version_routing() {
        let factory = AdapterFactory {
            version: MiseVersion::new(2026, 2, 17),
        };
        assert_eq!(factory.mise_version(), &MiseVersion::new(2026, 2, 17));
    }

    #[test]
    #[should_panic(expected = "not supported")]
    fn test_adapter_factory_rejects_old_version() {
        let factory = AdapterFactory {
            version: MiseVersion::new(2024, 1, 0),
        };
        let _ = factory.create();
    }
}
