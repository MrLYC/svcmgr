use super::{EnvError, EnvScope};
use crate::ports::ConfigPort;
use regex::Regex;
use std::collections::{HashMap, HashSet};

const MAX_DEPTH: usize = 10;

pub struct VariableExpander<'a> {
    config_port: &'a dyn ConfigPort,
    cache: HashMap<(String, EnvScope), String>,
    var_pattern: Regex,
}

impl<'a> VariableExpander<'a> {
    pub async fn new(config_port: &'a dyn ConfigPort) -> Result<Self, EnvError> {
        let var_pattern = Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}")
            .map_err(|e| EnvError::ConfigError(format!("Invalid regex pattern: {}", e)))?;

        Ok(Self {
            config_port,
            cache: HashMap::new(),
            var_pattern,
        })
    }

    pub async fn expand(&mut self, value: &str, scope: &EnvScope) -> Result<String, EnvError> {
        let mut visiting = HashSet::new();
        self.expand_with_depth(value, scope, 0, &mut visiting).await
    }

    fn expand_with_depth<'life0, 'life1, 'life2, 'life3, 'async_trait>(
        &'life0 mut self,
        value: &'life1 str,
        scope: &'life2 EnvScope,
        depth: usize,
        visiting: &'life3 mut HashSet<String>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<String, EnvError>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        'life3: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            if depth > MAX_DEPTH {
                return Err(EnvError::MaxDepthExceeded {
                    key: format!("{:?}", scope),
                    depth,
                });
            }

            let escaped = value.replace(r"\$", "\x00");
            let mut result = escaped.clone();
            let captures: Vec<_> = self
                .var_pattern
                .captures_iter(&escaped)
                .map(|cap| {
                    let var_name = cap[1].to_string();
                    let full_match = cap[0].to_string();
                    (var_name, full_match)
                })
                .collect();
            for (var_name, full_match) in captures {
                if visiting.contains(&var_name) {
                    return Err(EnvError::CircularReference {
                        key: var_name.to_string(),
                        chain: visiting.iter().cloned().collect(),
                    });
                }
                let resolved_value = match self.resolve_var(&var_name, scope).await? {
                    Some(v) => v,
                    None => continue,
                };
                visiting.insert(var_name.to_string());
                let expanded = self
                    .expand_with_depth(&resolved_value, scope, depth + 1, visiting)
                    .await?;
                visiting.remove(&var_name);
                result = result.replace(&full_match, &expanded);
            }
            Ok(result.replace('\x00', "$"))
        })
    }

    async fn resolve_var(
        &mut self,
        key: &str,
        scope: &EnvScope,
    ) -> Result<Option<String>, EnvError> {
        let cache_key = (key.to_string(), scope.clone());

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(Some(cached.clone()));
        }

        let value = match scope {
            EnvScope::Task { name } => {
                if let Some(v) = self
                    .config_port
                    .get_task_env_var(name, key)
                    .await
                    .map_err(|e| EnvError::ConfigError(e.to_string()))?
                {
                    Some(v)
                } else {
                    self.config_port
                        .get_global_env_var(key)
                        .await
                        .map_err(|e| EnvError::ConfigError(e.to_string()))?
                }
            }
            EnvScope::Service { name } => {
                if let Some(v) = self
                    .config_port
                    .get_service_env_var(name, key)
                    .await
                    .map_err(|e| EnvError::ConfigError(e.to_string()))?
                {
                    Some(v)
                } else {
                    self.config_port
                        .get_global_env_var(key)
                        .await
                        .map_err(|e| EnvError::ConfigError(e.to_string()))?
                }
            }
            EnvScope::Global => self
                .config_port
                .get_global_env_var(key)
                .await
                .map_err(|e| EnvError::ConfigError(e.to_string()))?,
        };

        if let Some(ref v) = value {
            self.cache.insert(cache_key, v.clone());
        }

        Ok(value)
    }

    pub fn check_circular_reference(
        &self,
        key: &str,
        value: &str,
        _scope: &EnvScope,
    ) -> Result<(), EnvError> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut stack = vec![(key.to_string(), value.to_string())];

        while let Some((k, v)) = stack.pop() {
            let refs: Vec<String> = self
                .var_pattern
                .captures_iter(&v)
                .map(|cap| cap[1].to_string())
                .collect();

            for r in &refs {
                if r == key {
                    return Err(EnvError::CircularReference {
                        key: key.to_string(),
                        chain: vec![key.to_string(), r.clone()],
                    });
                }
            }

            graph.insert(k, refs);
        }

        Ok(())
    }
}
