use crate::config::models::{MiseConfig, MiseTask};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Parse mise.toml configuration file into MiseConfig structure
///
/// Supports parsing:
/// - [tools] section: tool name -> version mapping
/// - [env] section: environment variable definitions
/// - [tasks] section: task definitions (flat or nested format)
///
/// # Example
/// ```toml
/// [tools]
/// node = "20"
/// python = "3.12"
///
/// [env]
/// NODE_ENV = "production"
///
/// [tasks.build]
/// run = "npm run build"
/// depends = ["install"]
/// ```
pub fn parse_mise_config(path: &Path) -> Result<MiseConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read mise config: {}", path.display()))?;

    let value: toml::Value =
        toml::from_str(&content).with_context(|| format!("Invalid TOML in {}", path.display()))?;

    Ok(MiseConfig {
        tools: parse_tools_section(&value),
        env: parse_env_section(&value),
        tasks: parse_tasks_section(&value),
    })
}

/// Parse [tools] section: { "node": "20", "python": "3.12" }
fn parse_tools_section(value: &toml::Value) -> HashMap<String, String> {
    value
        .get("tools")
        .and_then(|t| t.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| {
                    // Support both string and inline table formats:
                    // node = "20"
                    // python = { version = "3.12" }
                    let version = match v {
                        toml::Value::String(s) => Some(s.clone()),
                        toml::Value::Table(t) => {
                            t.get("version").and_then(|v| v.as_str()).map(String::from)
                        }
                        _ => None,
                    };
                    version.map(|ver| (k.clone(), ver))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse [env] section: { "NODE_ENV": "production" }
fn parse_env_section(value: &toml::Value) -> HashMap<String, String> {
    value
        .get("env")
        .and_then(|e| e.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| {
                    // Support string values only (mise templates like _.file are ignored)
                    v.as_str().map(|s| (k.clone(), s.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse [tasks] section: supports both flat and nested formats
///
/// Flat format:
/// ```toml
/// [tasks]
/// build = "npm run build"
/// test = "npm test"
/// ```
///
/// Nested format:
/// ```toml
/// [tasks.build]
/// run = "npm run build"
/// depends = ["install"]
/// env = { NODE_ENV = "production" }
/// ```
fn parse_tasks_section(value: &toml::Value) -> HashMap<String, MiseTask> {
    value
        .get("tasks")
        .and_then(|t| t.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(name, task_value)| {
                    parse_task(name, task_value).map(|task| (name.clone(), task))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a single task definition
fn parse_task(name: &str, value: &toml::Value) -> Option<MiseTask> {
    match value {
        // Flat format: build = "npm run build"
        toml::Value::String(cmd) => Some(MiseTask {
            description: None,
            run: cmd.clone(),
            depends: vec![],
            env: HashMap::new(),
            sources: vec![],
            outputs: vec![],
        }),

        // Nested format: [tasks.build] with run/depends/env/etc
        toml::Value::Table(table) => {
            let run = table.get("run")?.as_str()?.to_string();

            Some(MiseTask {
                description: table
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from),
                run,
                depends: parse_string_array(table.get("depends")),
                env: table
                    .get("env")
                    .and_then(|e| e.as_table())
                    .map(|t| {
                        t.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
                sources: parse_string_array(table.get("sources")),
                outputs: parse_string_array(table.get("outputs")),
            })
        }

        _ => {
            eprintln!("Warning: Unsupported task format for '{}'", name);
            None
        }
    }
}

/// Helper: parse TOML array of strings
fn parse_string_array(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_tools_section() {
        let toml = r#"
            [tools]
            node = "20"
            python = "3.12"
        "#;
        let value: toml::Value = toml::from_str(toml).unwrap();
        let tools = parse_tools_section(&value);

        assert_eq!(tools.len(), 2);
        assert_eq!(tools.get("node"), Some(&"20".to_string()));
        assert_eq!(tools.get("python"), Some(&"3.12".to_string()));
    }

    #[test]
    fn test_parse_env_section() {
        let toml = r#"
            [env]
            NODE_ENV = "production"
            LOG_LEVEL = "debug"
        "#;
        let value: toml::Value = toml::from_str(toml).unwrap();
        let env = parse_env_section(&value);

        assert_eq!(env.len(), 2);
        assert_eq!(env.get("NODE_ENV"), Some(&"production".to_string()));
        assert_eq!(env.get("LOG_LEVEL"), Some(&"debug".to_string()));
    }

    #[test]
    fn test_parse_tasks_flat_format() {
        let toml = r#"
            [tasks]
            build = "npm run build"
            test = "npm test"
        "#;
        let value: toml::Value = toml::from_str(toml).unwrap();
        let tasks = parse_tasks_section(&value);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks.get("build").unwrap().run, "npm run build");
        assert_eq!(tasks.get("test").unwrap().run, "npm test");
    }

    #[test]
    fn test_parse_tasks_nested_format() {
        let toml = r#"
            [tasks.build]
            run = "npm run build"
            depends = ["install"]
            
            [tasks.build.env]
            NODE_ENV = "production"
        "#;
        let value: toml::Value = toml::from_str(toml).unwrap();
        let tasks = parse_tasks_section(&value);

        assert_eq!(tasks.len(), 1);
        let build = tasks.get("build").unwrap();
        assert_eq!(build.run, "npm run build");
        assert_eq!(build.depends, vec!["install"]);
        assert_eq!(build.env.get("NODE_ENV"), Some(&"production".to_string()));
    }

    #[test]
    fn test_parse_task_with_sources_outputs() {
        let toml = r#"
            [tasks.build]
            run = "cargo build"
            sources = ["src/**/*.rs"]
            outputs = ["target/debug/app"]
        "#;
        let value: toml::Value = toml::from_str(toml).unwrap();
        let tasks = parse_tasks_section(&value);

        let build = tasks.get("build").unwrap();
        assert_eq!(build.sources, vec!["src/**/*.rs"]);
        assert_eq!(build.outputs, vec!["target/debug/app"]);
    }

    #[test]
    fn test_parse_mise_config_full() -> Result<()> {
        let toml_content = r#"
            [tools]
            node = "20"
            rust = "1.75"
            
            [env]
            APP_ENV = "test"
            
            [tasks.serve]
            run = "npm start"
            depends = ["build"]
        "#;

        let mut temp = NamedTempFile::new()?;
        temp.write_all(toml_content.as_bytes())?;
        temp.flush()?;

        let config = parse_mise_config(temp.path())?;

        assert_eq!(config.tools.len(), 2);
        assert_eq!(config.env.len(), 1);
        assert_eq!(config.tasks.len(), 1);
        assert_eq!(config.tasks.get("serve").unwrap().run, "npm start");

        Ok(())
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let result = parse_mise_config(Path::new("/nonexistent/path.toml"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to read"));
    }

    #[test]
    fn test_parse_invalid_toml() -> Result<()> {
        let mut temp = NamedTempFile::new()?;
        temp.write_all(b"invalid toml content [[")?;
        temp.flush()?;

        let result = parse_mise_config(temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid TOML"));

        Ok(())
    }
}
