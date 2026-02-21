#![allow(dead_code)]

use anyhow::{Context as AnyhowContext, Result, anyhow};
use minijinja::{Environment, Value, context};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 模板来源类型
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum TemplateSource {
    BuiltIn,
    User,
}

/// 模板信息
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TemplateInfo {
    pub name: String,
    pub category: String,
    pub source: TemplateSource,
    pub description: String,
    pub required_vars: Vec<String>,
}

/// 模板上下文
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    pub vars: HashMap<String, Value>,
}

impl TemplateContext {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub fn insert<K: Into<String>, V: Into<Value>>(&mut self, key: K, value: V) {
        self.vars.insert(key.into(), value.into());
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let vars: HashMap<String, serde_json::Value> = serde_json::from_str(json)?;
        let mut context = Self::new();
        for (key, value) in vars {
            context.insert(key, Value::from_serialize(&value));
        }
        Ok(context)
    }
}

/// 模板验证结果
#[derive(Debug)]
#[allow(dead_code)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// 模板原子 trait
#[allow(dead_code)]
pub trait TemplateAtom {
    /// 列出可用模板
    fn list_templates(&self, category: Option<&str>) -> Result<Vec<TemplateInfo>>;

    /// 获取模板内容
    fn get_template(&self, name: &str) -> Result<String>;

    /// 渲染模板
    fn render(&self, template: &str, context: &TemplateContext) -> Result<String>;

    /// 渲染模板到文件
    fn render_to_file(
        &self,
        template: &str,
        context: &TemplateContext,
        output: &Path,
    ) -> Result<()>;

    /// 验证模板语法
    fn validate(&self, template: &str) -> Result<ValidationResult>;

    /// 添加用户自定义模板
    fn add_user_template(&self, name: &str, content: &str) -> Result<()>;

    /// 删除用户自定义模板
    fn remove_user_template(&self, name: &str) -> Result<()>;
}

/// 模板引擎实现
pub struct TemplateEngine {
    user_dir: PathBuf,
    builtin_templates: HashMap<String, String>,
    #[allow(dead_code)]
    undefined_behavior: UndefinedBehavior,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum UndefinedBehavior {
    Error,
    Warning,
    Ignore,
}

impl TemplateEngine {
    /// 创建新的模板引擎实例
    pub fn new(user_dir: PathBuf) -> Result<Self> {
        let mut engine = Self {
            user_dir: user_dir.clone(),
            builtin_templates: HashMap::new(),
            undefined_behavior: UndefinedBehavior::Error,
        };

        // 确保用户模板目录存在
        fs::create_dir_all(&user_dir)
            .with_context(|| format!("Failed to create user template directory: {:?}", user_dir))?;

        // 加载内置模板
        engine.load_builtin_templates()?;

        Ok(engine)
    }

    /// 设置未定义变量行为（保留用于未来扩展）
    #[allow(dead_code)]
    pub fn set_undefined_behavior(&mut self, behavior: UndefinedBehavior) {
        self.undefined_behavior = behavior;
    }

    /// 加载内置模板
    fn load_builtin_templates(&mut self) -> Result<()> {
        // Systemd 服务模板
        self.builtin_templates.insert(
            "systemd/simple-service.service.j2".to_string(),
            include_str!("../../../templates/systemd/simple-service.service.j2").to_string(),
        );

        // Crontab 模板
        self.builtin_templates.insert(
            "crontab/daily-task.cron.j2".to_string(),
            include_str!("../../../templates/crontab/daily-task.cron.j2").to_string(),
        );

        Ok(())
    }

    /// 创建 minijinja 环境
    fn create_env(&self) -> Result<Environment<'static>> {
        let mut env = Environment::new();

        // 内置模板已经存储在 self.builtin_templates 中，需要创建静态版本
        // 这里使用 Box::leak 将数据转换为 'static 生命周期
        // 注意：这会导致内存泄漏，但对于长期运行的应用是可接受的
        for (name, content) in &self.builtin_templates {
            let name_static: &'static str = Box::leak(name.clone().into_boxed_str());
            let content_static: &'static str = Box::leak(content.clone().into_boxed_str());
            env.add_template(name_static, content_static)
                .with_context(|| format!("Failed to add builtin template: {}", name))?;
        }

        if self.user_dir.exists() {
            for entry in fs::read_dir(&self.user_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("j2") {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| anyhow!("Invalid template filename: {:?}", path))?
                        .to_string();
                    let content = fs::read_to_string(&path)?;
                    let name_static: &'static str = Box::leak(name.into_boxed_str());
                    let content_static: &'static str = Box::leak(content.into_boxed_str());
                    env.add_template(name_static, content_static)
                        .with_context(|| format!("Failed to add user template from: {:?}", path))?;
                }
            }
        }

        Ok(env)
    }

    /// 解析模板所需的变量（简单实现）
    fn extract_required_vars(&self, template: &str) -> Vec<String> {
        let mut vars = Vec::new();
        let re = regex::Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:\||\}\})").unwrap();

        for cap in re.captures_iter(template) {
            if let Some(var) = cap.get(1) {
                let var_name = var.as_str().to_string();
                if !vars.contains(&var_name) {
                    vars.push(var_name);
                }
            }
        }

        vars
    }
}

impl TemplateAtom for TemplateEngine {
    fn list_templates(&self, category: Option<&str>) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // 列出内置模板
        for (name, content) in &self.builtin_templates {
            let category_name = name.split('/').next().unwrap_or("unknown").to_string();

            if let Some(filter) = category
                && category_name != filter
            {
                continue;
            }

            templates.push(TemplateInfo {
                name: name.clone(),
                category: category_name,
                source: TemplateSource::BuiltIn,
                description: format!("Built-in template: {}", name),
                required_vars: self.extract_required_vars(content),
            });
        }

        // 列出用户模板
        if self.user_dir.exists() {
            for entry in fs::read_dir(&self.user_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("j2") {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| anyhow!("Invalid template filename: {:?}", path))?
                        .to_string();

                    let content = fs::read_to_string(&path)?;
                    let category_name = "user".to_string();

                    if let Some(filter) = category
                        && category_name != filter
                    {
                        continue;
                    }

                    templates.push(TemplateInfo {
                        name: name.clone(),
                        category: category_name,
                        source: TemplateSource::User,
                        description: format!("User template: {}", name),
                        required_vars: self.extract_required_vars(&content),
                    });
                }
            }
        }

        Ok(templates)
    }

    fn get_template(&self, name: &str) -> Result<String> {
        // 首先尝试从内置模板获取
        if let Some(content) = self.builtin_templates.get(name) {
            return Ok(content.clone());
        }

        // 然后尝试从用户模板获取
        let user_path = self.user_dir.join(name);
        if user_path.exists() {
            return fs::read_to_string(&user_path)
                .with_context(|| format!("Failed to read user template: {}", name));
        }

        Err(anyhow!("Template not found: {}", name))
    }

    fn render(&self, template: &str, ctx: &TemplateContext) -> Result<String> {
        let env = self.create_env()?;

        // 尝试作为模板名称
        if let Ok(tmpl) = env.get_template(template) {
            let rendered = tmpl.render(context! { ..ctx.vars.clone() })?;
            return Ok(rendered);
        }

        // 否则作为模板内容直接渲染
        let tmpl = env
            .template_from_str(template)
            .with_context(|| "Failed to parse template")?;
        let rendered = tmpl.render(context! { ..ctx.vars.clone() })?;
        Ok(rendered)
    }

    fn render_to_file(&self, template: &str, ctx: &TemplateContext, output: &Path) -> Result<()> {
        let rendered = self.render(template, ctx)?;

        // 确保输出目录存在
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {:?}", parent))?;
        }

        fs::write(output, rendered)
            .with_context(|| format!("Failed to write rendered template to: {:?}", output))?;

        Ok(())
    }

    fn validate(&self, template: &str) -> Result<ValidationResult> {
        let env = self.create_env()?;

        // 尝试解析模板
        match env.template_from_str(template) {
            Ok(_) => Ok(ValidationResult {
                valid: true,
                errors: Vec::new(),
            }),
            Err(e) => Ok(ValidationResult {
                valid: false,
                errors: vec![e.to_string()],
            }),
        }
    }

    fn add_user_template(&self, name: &str, content: &str) -> Result<()> {
        // 验证模板语法
        let validation = self.validate(content)?;
        if !validation.valid {
            return Err(anyhow!(
                "Template validation failed: {}",
                validation.errors.join(", ")
            ));
        }

        // 写入用户模板文件
        let template_path = self.user_dir.join(name);
        fs::write(&template_path, content)
            .with_context(|| format!("Failed to write user template: {}", name))?;

        Ok(())
    }

    fn remove_user_template(&self, name: &str) -> Result<()> {
        let template_path = self.user_dir.join(name);
        if !template_path.exists() {
            return Err(anyhow!("User template not found: {}", name));
        }

        fs::remove_file(&template_path)
            .with_context(|| format!("Failed to remove user template: {}", name))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_engine() -> (TemplateEngine, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let engine = TemplateEngine::new(temp_dir.path().to_path_buf()).unwrap();
        (engine, temp_dir)
    }

    #[test]
    fn test_render_simple_template() {
        let (engine, _temp) = create_test_engine();

        let template = "Hello, {{ name }}!";
        let mut ctx = TemplateContext::new();
        ctx.insert("name", "World");

        let result = engine.render(template, &ctx).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_render_with_filter() {
        let (engine, _temp) = create_test_engine();

        let template = "{{ name | upper }}";
        let mut ctx = TemplateContext::new();
        ctx.insert("name", "hello");

        let result = engine.render(template, &ctx).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_render_with_condition() {
        let (engine, _temp) = create_test_engine();

        let template = "{% if enabled %}Enabled{% else %}Disabled{% endif %}";
        let mut ctx = TemplateContext::new();
        ctx.insert("enabled", true);

        let result = engine.render(template, &ctx).unwrap();
        assert_eq!(result, "Enabled");
    }

    #[test]
    fn test_render_with_loop() {
        let (engine, _temp) = create_test_engine();

        let template = "{% for item in items %}{{ item }}\n{% endfor %}";
        let mut ctx = TemplateContext::new();
        ctx.insert("items", vec!["a", "b", "c"]);

        let result = engine.render(template, &ctx).unwrap();
        assert_eq!(result, "a\nb\nc\n");
    }

    #[test]
    fn test_validate_template() {
        let (engine, _temp) = create_test_engine();

        let valid_template = "{{ name }}";
        let result = engine.validate(valid_template).unwrap();
        assert!(result.valid);

        let invalid_template = "{% if %}";
        let result = engine.validate(invalid_template).unwrap();
        assert!(!result.valid);
    }

    #[test]
    fn test_add_and_remove_user_template() {
        let (engine, _temp) = create_test_engine();

        let template_name = "test.j2";
        let template_content = "Hello, {{ name }}!";

        // 添加模板
        engine
            .add_user_template(template_name, template_content)
            .unwrap();

        // 验证模板存在
        let content = engine.get_template(template_name).unwrap();
        assert_eq!(content, template_content);

        // 删除模板
        engine.remove_user_template(template_name).unwrap();

        // 验证模板已删除
        assert!(engine.get_template(template_name).is_err());
    }

    #[test]
    fn test_list_templates() {
        let (engine, _temp) = create_test_engine();

        let templates = engine.list_templates(None).unwrap();
        assert!(!templates.is_empty());

        // 验证内置模板
        let systemd_templates: Vec<_> = templates
            .iter()
            .filter(|t| t.category == "systemd")
            .collect();
        assert!(!systemd_templates.is_empty());
    }

    #[test]
    fn test_render_to_file() {
        let (engine, temp) = create_test_engine();

        let template = "Hello, {{ name }}!";
        let mut ctx = TemplateContext::new();
        ctx.insert("name", "File");

        let output_path = temp.path().join("output.txt");
        engine.render_to_file(template, &ctx, &output_path).unwrap();

        let content = fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "Hello, File!");
    }

    /// 测试获取不存在的模板时的错误处理
    #[test]
    fn test_get_nonexistent_template() {
        let (engine, _temp) = create_test_engine();

        let result = engine.get_template("nonexistent_template.j2");
        assert!(result.is_err());
    }

    /// 测试渲染复杂嵌套变量
    #[test]
    fn test_render_nested_variables() {
        let (engine, _temp) = create_test_engine();

        let template = "User: {{ user.name }}, Age: {{ user.age }}";
        let mut ctx = TemplateContext::new();
        let mut user_map = HashMap::new();
        user_map.insert("name", Value::from("Alice"));
        user_map.insert("age", Value::from(30));
        ctx.insert("user", user_map);

        let result = engine.render(template, &ctx).unwrap();
        assert_eq!(result, "User: Alice, Age: 30");
    }
}
