#![allow(dead_code)]

/// Service Management Feature (F01)
///
/// This module combines SupervisorAtom and TemplateAtom to provide
/// high-level service management functionality.
///
/// Features:
/// - Service CRUD with template rendering
/// - Service lifecycle management
/// - Status monitoring and logging
/// - Transient service execution
/// - Git-backed configuration management
use crate::atoms::{
    LogEntry, LogOptions, ProcessTree, SupervisorAtom, SupervisorManager, TemplateAtom,
    TemplateContext, TemplateEngine, TransientOptions, TransientUnit, UnitInfo, UnitStatus,
};
use crate::error::{Error, Result};
use std::path::PathBuf;

// ========================================
// Data Structures
// ========================================

/// Service configuration for creation
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub template: String,
    pub variables: TemplateContext,
}

/// Service information with extended metadata
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub active: bool,
    pub template: Option<String>,
}

impl From<UnitInfo> for ServiceInfo {
    fn from(unit: UnitInfo) -> Self {
        Self {
            name: unit.name,
            description: unit.description,
            enabled: unit.enabled,
            active: matches!(unit.active_state, crate::atoms::ActiveState::Active),
            template: None,
        }
    }
}

// ========================================
// SystemdServiceManager
// ========================================

/// High-level service manager
///
/// Combines SupervisorAtom (low-level process management) and
/// TemplateAtom (configuration generation) to provide a complete
/// service management solution.
pub struct SystemdServiceManager {
    supervisor: SupervisorManager,
    template: TemplateEngine,
    config_dir: PathBuf,
}

impl SystemdServiceManager {
    /// Create a new service manager
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        let service_dir = config_dir.join("managed").join("supervisor");
        let template_dir = config_dir.join("managed").join("templates");

        let supervisor = SupervisorManager::new(service_dir, true);
        let template = TemplateEngine::new(template_dir)?;

        Ok(Self {
            supervisor,
            template,
            config_dir,
        })
    }

    /// Create with default configuration (~/.config/svcmgr)
    pub fn default_config() -> Result<Self> {
        let home = std::env::var("HOME")
            .map_err(|_| Error::Config("HOME environment variable not set".to_string()))?;
        let config_dir = PathBuf::from(home).join(".config/svcmgr");
        Self::new(config_dir)
    }

    // ========================================
    // Service CRUD Operations
    // ========================================

    /// Create a new service from template
    pub async fn create_service(&self, config: &ServiceConfig) -> Result<()> {
        // Validate service name
        if !config.name.ends_with(".service") {
            return Err(Error::InvalidArgument(
                "Service name must end with .service".to_string(),
            ));
        }

        // Check if service already exists
        if self.supervisor.get_unit(&config.name).await.is_ok() {
            return Err(Error::InvalidArgument(format!(
                "Service {} already exists",
                config.name
            )));
        }

        // Render template
        let mut variables = config.variables.clone();
        variables.insert("name", &config.name);
        let content = self.template.render(&config.template, &variables)?;

        // Create unit file
        self.supervisor.create_unit(&config.name, &content).await?;

        // Reload daemon
        self.supervisor.daemon_reload().await?;

        Ok(())
    }

    /// List all managed services
    pub async fn list_services(&self) -> Result<Vec<ServiceInfo>> {
        let units = self.supervisor.list_units().await?;
        Ok(units.into_iter().map(ServiceInfo::from).collect())
    }

    /// Get service details
    pub async fn get_service(&self, name: &str) -> Result<ServiceInfo> {
        let unit = self.supervisor.get_unit(name).await?;
        let units = self.supervisor.list_units().await?;

        let unit_info = units
            .into_iter()
            .find(|u| u.name == name)
            .ok_or_else(|| Error::InvalidArgument(format!("Service {} not found", name)))?;

        let mut info = ServiceInfo::from(unit_info);

        // Try to extract template name from unit file content
        if let Ok(template_line) = extract_template_name(&unit.content) {
            info.template = Some(template_line);
        }

        Ok(info)
    }

    /// Update service configuration
    pub async fn update_service(&self, config: &ServiceConfig) -> Result<()> {
        // Verify service exists
        self.supervisor.get_unit(&config.name).await?;

        // Render new template
        let mut variables = config.variables.clone();
        variables.insert("name", &config.name);
        let content = self.template.render(&config.template, &variables)?;

        // Update unit file
        self.supervisor.update_unit(&config.name, &content).await?;

        // Reload daemon
        self.supervisor.daemon_reload().await?;

        Ok(())
    }

    /// Delete a service
    pub async fn delete_service(&self, name: &str) -> Result<()> {
        self.supervisor.delete_unit(name).await?;
        self.supervisor.daemon_reload().await?;
        Ok(())
    }

    // ========================================
    // Service Lifecycle Management
    // ========================================

    /// Start a service
    pub async fn start_service(&self, name: &str) -> Result<()> {
        self.supervisor.start(name).await
    }

    /// Stop a service
    pub async fn stop_service(&self, name: &str) -> Result<()> {
        self.supervisor.stop(name).await
    }

    /// Restart a service
    pub async fn restart_service(&self, name: &str) -> Result<()> {
        self.supervisor.restart(name).await
    }

    /// Reload service configuration (without restart)
    pub async fn reload_service(&self, name: &str) -> Result<()> {
        self.supervisor.reload(name).await
    }

    /// Enable service (auto-start on boot)
    pub async fn enable_service(&self, name: &str) -> Result<()> {
        self.supervisor.enable(name).await
    }

    /// Disable service (remove auto-start)
    pub async fn disable_service(&self, name: &str) -> Result<()> {
        self.supervisor.disable(name).await
    }

    // ========================================
    // Service Status & Monitoring
    // ========================================

    /// Get service status
    pub async fn get_status(&self, name: &str) -> Result<UnitStatus> {
        self.supervisor.status(name).await
    }

    /// Get process tree for a service
    pub async fn get_process_tree(&self, name: &str) -> Result<ProcessTree> {
        self.supervisor.process_tree(name).await
    }

    /// Query service logs
    pub async fn get_logs(&self, name: &str, options: &LogOptions) -> Result<Vec<LogEntry>> {
        self.supervisor.logs(name, options).await
    }

    // ========================================
    // Transient Service (Temporary Tasks)
    // ========================================

    /// Run a transient service (temporary task)
    pub async fn run_transient(&self, options: &TransientOptions) -> Result<TransientUnit> {
        self.supervisor.run_transient(options).await
    }

    /// List active transient units
    pub async fn list_transient(&self) -> Result<Vec<TransientUnit>> {
        self.supervisor.list_transient().await
    }

    /// Stop a transient unit
    pub async fn stop_transient(&self, name: &str) -> Result<()> {
        self.supervisor.stop_transient(name).await
    }

    // ========================================
    // Template Management
    // ========================================

    /// List available service templates
    pub fn list_templates(&self) -> Result<Vec<String>> {
        let templates = self.template.list_templates(Some("systemd"))?;
        Ok(templates.into_iter().map(|t| t.name).collect())
    }

    /// Get template content
    pub fn get_template(&self, name: &str) -> Result<String> {
        Ok(self.template.get_template(name)?)
    }

    /// Validate template
    pub fn validate_template(&self, template: &str) -> Result<()> {
        let result = self.template.validate(template)?;
        if !result.valid {
            return Err(Error::InvalidArgument(format!(
                "Template validation failed: {}",
                result.errors.join(", ")
            )));
        }
        Ok(())
    }
}

// ========================================
// Helper Functions
// ========================================

/// Extract template name from unit file content
/// Looks for a comment line: # Template: <name>
fn extract_template_name(content: &str) -> Result<String> {
    for line in content.lines() {
        if let Some(stripped) = line.strip_prefix("# Template:") {
            return Ok(stripped.trim().to_string());
        }
    }
    Err(Error::Other(
        "Template name not found in unit file".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (SystemdServiceManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();

        // Create required directories
        std::fs::create_dir_all(config_dir.join("managed/supervisor")).unwrap();
        std::fs::create_dir_all(config_dir.join("managed/templates/systemd")).unwrap();

        // Create a simple test template
        let template_path = config_dir.join("managed/templates/systemd/simple.service.j2");
        std::fs::write(
            &template_path,
            "name = \"{{ name }}\"\ndescription = \"{{ description }}\"\ncommand = \"{{ command }}\"\nargs = []\nenv = {}\nrestart_policy = \"No\"\nenabled = true\nrestart_sec = 1\nstop_timeout_sec = 10\n",
        )
        .unwrap();

        let manager = SystemdServiceManager::new(config_dir).unwrap();
        (manager, temp_dir)
    }

    #[test]
    fn test_create_manager() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();

        std::fs::create_dir_all(config_dir.join("managed/supervisor")).unwrap();
        std::fs::create_dir_all(config_dir.join("managed/templates")).unwrap();

        let result = SystemdServiceManager::new(config_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_service_name() {
        let (manager, _temp) = create_test_manager();

        let mut context = TemplateContext::new();
        context.insert("description", "Test service");
        context.insert("command", "/bin/true");

        let config = ServiceConfig {
            name: "invalid-name".to_string(), // Missing .service suffix
            template: "simple".to_string(),
            variables: context,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(manager.create_service(&config));

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must end with .service")
        );
    }

    #[test]
    fn test_extract_template_name() {
        let content = "# Template: simple-service\n[Unit]\nDescription=Test\n";
        let result = extract_template_name(content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "simple-service");
    }

    #[test]
    fn test_extract_template_name_missing() {
        let content = "[Unit]\nDescription=Test\n";
        let result = extract_template_name(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_service_info_from_unit_info() {
        use crate::atoms::{ActiveState, LoadState};

        let unit_info = UnitInfo {
            name: "test.service".to_string(),
            description: "Test Service".to_string(),
            load_state: LoadState::Loaded,
            active_state: ActiveState::Active,
            sub_state: "running".to_string(),
            enabled: true,
        };

        let service_info = ServiceInfo::from(unit_info);

        assert_eq!(service_info.name, "test.service");
        assert_eq!(service_info.description, "Test Service");
        assert!(service_info.enabled);
        assert!(service_info.active);
        assert!(service_info.template.is_none());
    }
}
