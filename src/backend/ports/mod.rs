//! Port layer - trait definitions for external dependencies
//!
//! Port-Adapter pattern implementation according to OpenSpec 07-mise-integration.md

pub mod mise_port;

// Re-export commonly used types
pub use mise_port::{
    ConfigPort, DependencyPort, EnvPort, MiseFeature, MiseVersion, TaskCommand, TaskInfo,
    TaskOutput, TaskPort, ToolInfo,
};
