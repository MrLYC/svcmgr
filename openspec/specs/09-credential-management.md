# 09 - 凭据管理模块

> 版本：2.0.0-draft
> 状态：设计中

## 1. 设计目标

### 1.1 为什么需要凭据管理

**核心需求**：
- **HTTP 代理认证**：每个代理路由可能需要不同的认证方式（Basic Auth, Bearer Token, API Key）
- **外部服务凭据**：数据库密码、API 密钥、OAuth token 等敏感信息
- **安全存储**：凭据不能明文存储在配置文件中
- **版本控制友好**：加密后的凭据可以安全地提交到 Git
- **统一管理**：所有凭据在一个地方管理，避免分散在多个配置文件中

### 1.2 基于 fnox 的设计决策

**为什么选择 fnox**：
- **已集成 mise 生态**：fnox 由 mise 作者开发，与 mise 深度集成
- **多种存储后端**：支持加密存储（age, AWS KMS, Azure KMS, GCP KMS）和远程存储（AWS Secrets Manager, 1Password, Bitwarden 等）
- **Git 友好**：加密后的凭据可以安全地提交到 Git
- **Shell 集成**：支持自动加载环境变量
- **成熟稳定**：活跃开发，社区支持良好

**设计策略**：
- ✅ **复用 fnox 核心能力**：作为 Rust 库依赖使用（fnox crate）
- ✅ **扩展凭据类型**：支持 HTTP 认证相关的凭据格式
- ✅ **统一配置接口**：在 svcmgr.toml 中引用 fnox 管理的凭据
- ❌ **不重新实现加密**：直接使用 fnox 的加密和存储逻辑

### 1.3 核心功能

| 功能 | 说明 |
|------|------|
| **凭据类型** | Basic Auth, Bearer Token, API Key, Custom Header |
| **加密存储** | 使用 age/AWS KMS/Azure KMS/GCP KMS 加密凭据 |
| **远程存储** | 支持 AWS Secrets Manager, 1Password, Bitwarden 等 |
| **凭据引用** | 配置文件中通过引用名称使用凭据 |
| **动态刷新** | 支持凭据过期自动刷新（如 OAuth token） |
| **审计日志** | 记录凭据访问日志 |
| **CLI 管理** | 提供 CLI 命令管理凭据 |

---

## 2. 配置格式

### 2.1 fnox 配置（fnox.toml）

svcmgr 使用标准的 fnox 配置文件管理凭据：

```toml
# .config/mise/svcmgr/fnox.toml

# 配置加密提供者
[providers]
age = { type = "age", recipients = ["age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p"] }

# 定义凭据
[secrets]
# Basic Auth 凭据（用户名和密码分开存储）
admin_username = { provider = "age", value = "age[...]encrypted_base64[...]" }
admin_password = { provider = "age", value = "age[...]encrypted_base64[...]" }

# Bearer Token
api_token = { provider = "age", value = "age[...]encrypted_base64[...]" }

# API Key
external_api_key = { provider = "age", value = "age[...]encrypted_base64[...]" }

# 从远程提供者读取（1Password 示例）
database_password = { provider = "1password", ref = "op://prod/database/password" }
```

**字段说明**：
- `provider`：加密或远程存储提供者
- `value`：加密后的凭据值（仅加密提供者需要）
- `ref`：远程提供者的引用路径（仅远程提供者需要）

### 2.2 svcmgr 凭据配置

在 svcmgr.toml 中定义凭据对象，引用 fnox 管理的 secrets：

```toml
# .config/mise/svcmgr/config.toml

# ========================================
# 凭据定义
# ========================================
[credentials.admin_basic]
type = "basic"
username_secret = "admin_username"  # 引用 fnox.toml 中的 secret
password_secret = "admin_password"
realm = "Admin Area"                # HTTP Basic Auth realm (可选)

[credentials.api_bearer]
type = "bearer"
token_secret = "api_token"          # 引用 fnox.toml 中的 secret

[credentials.external_api]
type = "api_key"
key_secret = "external_api_key"     # 引用 fnox.toml 中的 secret
header_name = "X-API-Key"           # HTTP 头名称
# 或者通过查询参数传递
# query_param = "api_key"

[credentials.custom_header]
type = "custom"
header_name = "X-Custom-Auth"
value_secret = "custom_auth_value"  # 引用 fnox.toml 中的 secret

# ========================================
# HTTP 路由配置（带认证）
# ========================================
[[http.routes]]
path = "/api"
backend = "api:http"
auth = "api_bearer"                 # 引用凭据名称

[[http.routes]]
path = "/admin"
backend = "admin:http"
auth = "admin_basic"                # 使用 Basic Auth

[[http.routes]]
path = "/external"
backend = "external:http"
auth = "external_api"               # 使用 API Key

[[http.routes]]
path = "/public"
backend = "frontend:http"
# auth 字段省略 = 无认证
```

### 2.3 凭据类型详解

#### 2.3.1 Basic Authentication

```toml
[credentials.basic_example]
type = "basic"
username_secret = "username_key"    # fnox secret 名称
password_secret = "password_key"    # fnox secret 名称
realm = "Restricted Area"           # 可选，默认 "Restricted"
```

**HTTP 请求示例**：
```http
GET /admin HTTP/1.1
Host: example.com
Authorization: Basic YWRtaW46cGFzc3dvcmQ=
```

#### 2.3.2 Bearer Token

```toml
[credentials.bearer_example]
type = "bearer"
token_secret = "bearer_token_key"   # fnox secret 名称
```

**HTTP 请求示例**：
```http
GET /api/users HTTP/1.1
Host: example.com
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

#### 2.3.3 API Key (Header)

```toml
[credentials.api_key_header]
type = "api_key"
key_secret = "api_key_value"        # fnox secret 名称
header_name = "X-API-Key"           # HTTP 头名称
```

**HTTP 请求示例**：
```http
GET /api/data HTTP/1.1
Host: example.com
X-API-Key: sk_live_abc123def456...
```

#### 2.3.4 API Key (Query Parameter)

```toml
[credentials.api_key_query]
type = "api_key"
key_secret = "api_key_value"        # fnox secret 名称
query_param = "api_key"             # 查询参数名称
```

**HTTP 请求示例**：
```http
GET /api/data?api_key=sk_live_abc123def456... HTTP/1.1
Host: example.com
```

#### 2.3.5 Custom Header

```toml
[credentials.custom_example]
type = "custom"
header_name = "X-Custom-Token"      # 自定义头名称
value_secret = "custom_value"       # fnox secret 名称
```

**HTTP 请求示例**：
```http
GET /api/custom HTTP/1.1
Host: example.com
X-Custom-Token: custom_auth_value_here
```

---

## 3. 实现设计

### 3.1 技术选型

**核心依赖**：
```toml
# Cargo.toml
[dependencies]
# fnox 作为库依赖
fnox = "0.1"  # 或最新版本

# 加密和哈希
age = "0.10"
sha2 = "0.10"
base64 = "0.21"

# 序列化
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
```

### 3.2 数据结构

```rust
// src/credentials/mod.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 凭据管理器
pub struct CredentialManager {
    /// fnox 配置
    fnox_config: FnoxConfig,
    /// 凭据定义
    credentials: HashMap<String, Credential>,
    /// 凭据缓存（解密后的值）
    cache: Arc<RwLock<HashMap<String, CachedCredential>>>,
}

/// 凭据定义（配置文件中的定义）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Credential {
    /// Basic Authentication
    Basic {
        username_secret: String,
        password_secret: String,
        realm: Option<String>,
    },
    /// Bearer Token
    Bearer {
        token_secret: String,
    },
    /// API Key (Header or Query)
    ApiKey {
        key_secret: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        header_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        query_param: Option<String>,
    },
    /// Custom Header
    Custom {
        header_name: String,
        value_secret: String,
    },
}

/// 缓存的凭据（解密后的值）
#[derive(Debug, Clone)]
pub struct CachedCredential {
    /// 解密后的凭据值
    value: CredentialValue,
    /// 缓存时间
    cached_at: Instant,
    /// 过期时间（如果有）
    expires_at: Option<Instant>,
}

/// 凭据值（解密后）
#[derive(Debug, Clone)]
pub enum CredentialValue {
    Basic {
        username: String,
        password: String,
        realm: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        key: String,
        header_name: Option<String>,
        query_param: Option<String>,
    },
    Custom {
        header_name: String,
        value: String,
    },
}

impl CredentialManager {
    /// 创建凭据管理器
    pub fn new(fnox_config_path: &Path, credentials: HashMap<String, Credential>) -> Result<Self> {
        let fnox_config = FnoxConfig::load(fnox_config_path)?;
        Ok(Self {
            fnox_config,
            credentials,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 获取凭据（自动解密和缓存）
    pub async fn get(&self, name: &str) -> Result<CredentialValue> {
        // 1. 检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(name) {
                // 检查是否过期
                if let Some(expires_at) = cached.expires_at {
                    if Instant::now() < expires_at {
                        return Ok(cached.value.clone());
                    }
                } else {
                    // 无过期时间，使用缓存
                    return Ok(cached.value.clone());
                }
            }
        }

        // 2. 从配置中获取凭据定义
        let credential = self.credentials.get(name)
            .ok_or_else(|| CredentialError::NotFound(name.to_string()))?;

        // 3. 从 fnox 解密 secrets
        let value = match credential {
            Credential::Basic { username_secret, password_secret, realm } => {
                let username = self.fnox_config.get_secret(username_secret).await?;
                let password = self.fnox_config.get_secret(password_secret).await?;
                CredentialValue::Basic {
                    username,
                    password,
                    realm: realm.clone().unwrap_or_else(|| "Restricted".to_string()),
                }
            }
            Credential::Bearer { token_secret } => {
                let token = self.fnox_config.get_secret(token_secret).await?;
                CredentialValue::Bearer { token }
            }
            Credential::ApiKey { key_secret, header_name, query_param } => {
                let key = self.fnox_config.get_secret(key_secret).await?;
                CredentialValue::ApiKey {
                    key,
                    header_name: header_name.clone(),
                    query_param: query_param.clone(),
                }
            }
            Credential::Custom { header_name, value_secret } => {
                let value = self.fnox_config.get_secret(value_secret).await?;
                CredentialValue::Custom {
                    header_name: header_name.clone(),
                    value,
                }
            }
        };

        // 4. 缓存凭据（默认 5 分钟过期）
        {
            let mut cache = self.cache.write().await;
            cache.insert(name.to_string(), CachedCredential {
                value: value.clone(),
                cached_at: Instant::now(),
                expires_at: Some(Instant::now() + Duration::from_secs(300)),
            });
        }

        Ok(value)
    }

    /// 清除缓存（用于凭据更新后）
    pub async fn clear_cache(&self, name: Option<&str>) {
        let mut cache = self.cache.write().await;
        if let Some(name) = name {
            cache.remove(name);
        } else {
            cache.clear();
        }
    }

    /// 验证凭据是否有效（尝试解密所有凭据）
    pub async fn validate_all(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::default();
        
        for (name, _) in &self.credentials {
            match self.get(name).await {
                Ok(_) => {
                    report.valid.push(name.clone());
                }
                Err(e) => {
                    report.invalid.push((name.clone(), e.to_string()));
                }
            }
        }
        
        Ok(report)
    }
}

/// fnox 配置（简化封装）
pub struct FnoxConfig {
    /// fnox 配置文件路径
    config_path: PathBuf,
    /// fnox 实例
    fnox: fnox::Fnox,
}

impl FnoxConfig {
    /// 加载 fnox 配置
    pub fn load(config_path: &Path) -> Result<Self> {
        let fnox = fnox::Fnox::load(config_path)?;
        Ok(Self {
            config_path: config_path.to_path_buf(),
            fnox,
        })
    }

    /// 获取 secret（解密）
    pub async fn get_secret(&self, key: &str) -> Result<String> {
        self.fnox.get(key).await
            .map_err(|e| CredentialError::FnoxError(e.to_string()))
    }
}

/// 凭据验证报告
#[derive(Debug, Default)]
pub struct ValidationReport {
    pub valid: Vec<String>,
    pub invalid: Vec<(String, String)>,
}

/// 凭据错误
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("凭据 '{0}' 未找到")]
    NotFound(String),
    
    #[error("fnox 错误: {0}")]
    FnoxError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}
```

### 3.3 HTTP 认证中间件

```rust
// src/web/middleware/auth.rs

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use base64::{Engine as _, engine::general_purpose};

/// HTTP 认证中间件
pub async fn auth_middleware(
    State(cred_manager): State<Arc<CredentialManager>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 从路由扩展中获取认证配置
    let auth_config = req.extensions().get::<AuthConfig>().cloned();
    
    if let Some(config) = auth_config {
        // 获取凭据
        let credential = cred_manager.get(&config.credential_name)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // 验证请求
        match credential {
            CredentialValue::Basic { username, password, realm } => {
                verify_basic_auth(&req, &username, &password, &realm)?;
            }
            CredentialValue::Bearer { token } => {
                verify_bearer_token(&req, &token)?;
            }
            CredentialValue::ApiKey { key, header_name, query_param } => {
                verify_api_key(&req, &key, header_name.as_deref(), query_param.as_deref())?;
            }
            CredentialValue::Custom { header_name, value } => {
                verify_custom_header(&req, &header_name, &value)?;
            }
        }
    }
    
    Ok(next.run(req).await)
}

/// 验证 Basic Auth
fn verify_basic_auth(
    req: &Request,
    expected_username: &str,
    expected_password: &str,
    realm: &str,
) -> Result<(), StatusCode> {
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    if !auth_header.starts_with("Basic ") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    let encoded = &auth_header[6..];
    let decoded = general_purpose::STANDARD.decode(encoded)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let credentials = String::from_utf8(decoded)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    let parts: Vec<&str> = credentials.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    let (username, password) = (parts[0], parts[1]);
    
    if username == expected_username && password == expected_password {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// 验证 Bearer Token
fn verify_bearer_token(req: &Request, expected_token: &str) -> Result<(), StatusCode> {
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    let token = &auth_header[7..];
    
    if token == expected_token {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// 验证 API Key
fn verify_api_key(
    req: &Request,
    expected_key: &str,
    header_name: Option<&str>,
    query_param: Option<&str>,
) -> Result<(), StatusCode> {
    // 优先检查 header
    if let Some(header) = header_name {
        if let Some(value) = req.headers().get(header).and_then(|v| v.to_str().ok()) {
            return if value == expected_key {
                Ok(())
            } else {
                Err(StatusCode::UNAUTHORIZED)
            };
        }
    }
    
    // 检查查询参数
    if let Some(param) = query_param {
        if let Some(query) = req.uri().query() {
            for pair in query.split('&') {
                if let Some((key, value)) = pair.split_once('=') {
                    if key == param {
                        return if value == expected_key {
                            Ok(())
                        } else {
                            Err(StatusCode::UNAUTHORIZED)
                        };
                    }
                }
            }
        }
    }
    
    Err(StatusCode::UNAUTHORIZED)
}

/// 验证自定义 Header
fn verify_custom_header(
    req: &Request,
    header_name: &str,
    expected_value: &str,
) -> Result<(), StatusCode> {
    let value = req.headers()
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    if value == expected_value {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// 认证配置（存储在请求扩展中）
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub credential_name: String,
}
```

---

## 4. CLI 命令

### 4.1 凭据管理命令

```bash
# ========================================
# 凭据初始化
# ========================================
# 初始化 fnox 配置（生成 age 密钥）
svcmgr credential init

# 使用现有 age 密钥初始化
svcmgr credential init --age-key ~/.age/key.txt

# ========================================
# 设置凭据
# ========================================
# 交互式设置 secret（会提示输入值）
svcmgr credential set admin_username

# 从命令行设置 secret
svcmgr credential set admin_password --value "secure_password"

# 从文件读取 secret
svcmgr credential set api_key --file ~/.secrets/api_key.txt

# 从环境变量读取
svcmgr credential set db_password --from-env DB_PASSWORD

# ========================================
# 获取凭据
# ========================================
# 查看凭据（解密）
svcmgr credential get admin_username

# 查看所有凭据（仅名称）
svcmgr credential list

# 查看凭据详情（包括引用关系）
svcmgr credential list --detailed

# ========================================
# 删除凭据
# ========================================
# 删除 secret
svcmgr credential delete api_key

# 删除所有未使用的 secrets
svcmgr credential cleanup

# ========================================
# 验证凭据
# ========================================
# 验证所有凭据是否可以解密
svcmgr credential validate

# 输出示例：
# ✓ admin_username: OK
# ✓ admin_password: OK
# ✗ api_key: Decryption failed
# ✓ db_password: OK (from 1Password)

# ========================================
# 旋转凭据（更新值）
# ========================================
# 交互式更新凭据
svcmgr credential rotate admin_password

# 自动生成新值（适用于 token/密钥）
svcmgr credential rotate api_token --generate

# ========================================
# 导入/导出
# ========================================
# 导出凭据（加密状态，可以共享给团队）
svcmgr credential export --output team-secrets.toml

# 导入凭据
svcmgr credential import team-secrets.toml

# ========================================
# 审计日志
# ========================================
# 查看凭据访问日志
svcmgr credential audit

# 输出示例：
# 2026-02-23 12:30:15 | admin_username | accessed by user@host
# 2026-02-23 12:30:16 | admin_password | accessed by user@host
# 2026-02-23 12:35:20 | api_key | accessed by user@host
```

### 4.2 CLI 实现

```rust
// src/cli/commands/credential.rs

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct CredentialCommand {
    #[command(subcommand)]
    pub subcommand: CredentialSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum CredentialSubcommand {
    /// 初始化 fnox 配置
    Init {
        /// 使用现有 age 密钥文件
        #[arg(long)]
        age_key: Option<PathBuf>,
    },
    
    /// 设置 secret
    Set {
        /// Secret 名称
        name: String,
        
        /// Secret 值
        #[arg(long)]
        value: Option<String>,
        
        /// 从文件读取
        #[arg(long)]
        file: Option<PathBuf>,
        
        /// 从环境变量读取
        #[arg(long)]
        from_env: Option<String>,
    },
    
    /// 获取 secret
    Get {
        /// Secret 名称
        name: String,
    },
    
    /// 列出所有 secrets
    List {
        /// 显示详细信息
        #[arg(long)]
        detailed: bool,
    },
    
    /// 删除 secret
    Delete {
        /// Secret 名称
        name: String,
    },
    
    /// 清理未使用的 secrets
    Cleanup,
    
    /// 验证所有凭据
    Validate,
    
    /// 旋转凭据（更新值）
    Rotate {
        /// 凭据名称
        name: String,
        
        /// 自动生成新值
        #[arg(long)]
        generate: bool,
    },
    
    /// 导出凭据
    Export {
        /// 输出文件
        #[arg(long)]
        output: PathBuf,
    },
    
    /// 导入凭据
    Import {
        /// 输入文件
        file: PathBuf,
    },
    
    /// 查看审计日志
    Audit {
        /// 过滤凭据名称
        #[arg(long)]
        name: Option<String>,
        
        /// 显示最近 N 条
        #[arg(long, default_value = "50")]
        limit: usize,
    },
}

impl CredentialCommand {
    pub async fn execute(&self, config: &Config) -> Result<()> {
        match &self.subcommand {
            CredentialSubcommand::Init { age_key } => {
                self.init(config, age_key.as_deref()).await
            }
            CredentialSubcommand::Set { name, value, file, from_env } => {
                self.set(config, name, value.as_deref(), file.as_deref(), from_env.as_deref()).await
            }
            CredentialSubcommand::Get { name } => {
                self.get(config, name).await
            }
            CredentialSubcommand::List { detailed } => {
                self.list(config, *detailed).await
            }
            CredentialSubcommand::Delete { name } => {
                self.delete(config, name).await
            }
            CredentialSubcommand::Cleanup => {
                self.cleanup(config).await
            }
            CredentialSubcommand::Validate => {
                self.validate(config).await
            }
            CredentialSubcommand::Rotate { name, generate } => {
                self.rotate(config, name, *generate).await
            }
            CredentialSubcommand::Export { output } => {
                self.export(config, output).await
            }
            CredentialSubcommand::Import { file } => {
                self.import(config, file).await
            }
            CredentialSubcommand::Audit { name, limit } => {
                self.audit(config, name.as_deref(), *limit).await
            }
        }
    }
    
    async fn init(&self, config: &Config, age_key: Option<&Path>) -> Result<()> {
        // 初始化 fnox 配置
        todo!()
    }
    
    // ... 其他方法实现
}
```

---

## 5. 与 HTTP 代理集成

### 5.1 配置示例

```toml
# .config/mise/svcmgr/config.toml

# ========================================
# 凭据定义
# ========================================
[credentials.admin]
type = "basic"
username_secret = "admin_user"
password_secret = "admin_pass"
realm = "Admin Dashboard"

[credentials.api_auth]
type = "bearer"
token_secret = "api_token"

# ========================================
# HTTP 路由（带认证）
# ========================================
[[http.routes]]
path = "/admin"
backend = "admin_panel:http"
auth = "admin"                      # 使用 Basic Auth

[[http.routes]]
path = "/api"
backend = "api_server:http"
auth = "api_auth"                   # 使用 Bearer Token

[[http.routes]]
path = "/public"
backend = "frontend:http"
# 无 auth 字段 = 公开访问
```

### 5.2 路由构建时加载认证

```rust
// src/web/proxy.rs

impl ProxyServer {
    fn build_router(
        config: &ProxyConfig,
        cred_manager: Arc<CredentialManager>,
    ) -> Result<Router> {
        let mut router = Router::new();
        
        for route in &config.routes {
            let handler = Self::create_route_handler(route, cred_manager.clone());
            
            // 如果配置了认证，添加认证中间件
            let handler = if let Some(ref auth_name) = route.auth {
                handler.layer(middleware::from_fn_with_state(
                    cred_manager.clone(),
                    move |State(cm): State<Arc<CredentialManager>>, mut req: Request, next: Next| async move {
                        // 将认证配置添加到请求扩展
                        req.extensions_mut().insert(AuthConfig {
                            credential_name: auth_name.clone(),
                        });
                        auth_middleware(State(cm), req, next).await
                    }
                ))
            } else {
                handler
            };
            
            router = router.route(&route.path, handler);
        }
        
        Ok(router)
    }
}
```

---

## 6. 安全最佳实践

### 6.1 凭据轮换策略

**推荐轮换周期**：
- **高风险凭据**（生产数据库密码）：30-90 天
- **中风险凭据**（API Token）：90-180 天
- **低风险凭据**（开发环境密码）：180-365 天

**自动化轮换**：
```toml
# .config/mise/svcmgr/config.toml

[credentials.prod_db_password]
type = "custom"
value_secret = "db_password"
rotation_policy = {
    enabled = true,
    interval_days = 90,
    auto_rotate = false,           # false = 提醒，true = 自动旋转
    notify = ["admin@example.com"]
}
```

### 6.2 审计日志

凭据访问自动记录到审计日志：

```
2026-02-23T12:30:15Z | INFO | credential=admin_username | action=get | user=alice | host=dev-machine | result=success
2026-02-23T12:30:16Z | INFO | credential=admin_password | action=get | user=alice | host=dev-machine | result=success
2026-02-23T12:35:20Z | WARN | credential=api_key | action=get | user=bob | host=prod-server | result=failed | reason=decryption_error
```

### 6.3 访问控制

限制凭据访问权限：

```toml
[credentials.prod_api_key]
type = "bearer"
token_secret = "prod_token"
access_control = {
    allowed_users = ["alice", "bob"],
    allowed_hosts = ["prod-server-1", "prod-server-2"],
    require_mfa = true
}
```

### 6.4 加密最佳实践

**推荐配置**：
```toml
# fnox.toml

# 开发环境：使用 age（快速，无外部依赖）
[providers.age]
type = "age"
recipients = ["age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p"]

# 生产环境：使用云 KMS（审计、自动轮换）
[providers.aws_kms]
type = "aws-kms"
key_id = "arn:aws:kms:us-east-1:123456789012:key/12345678-1234-1234-1234-123456789012"
region = "us-east-1"

# 团队共享：使用多个 age 接收者
[providers.team_age]
type = "age"
recipients = [
    "age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p",  # alice
    "age1ytm269gvr6j8h8yqgd8ggrv9hde4sgh2r3jh5g7hqkr8afjm2hmqkr8jzl",  # bob
]
```

---

## 7. 实施计划

### Phase 1: 基础凭据管理（2 周）
- [ ] 集成 fnox 库
- [ ] 实现 CredentialManager
- [ ] 实现基本的 CLI 命令（init, set, get, list）
- [ ] 支持 age 加密提供者

### Phase 2: HTTP 认证集成（1 周）
- [ ] 实现 HTTP 认证中间件
- [ ] 支持 Basic Auth, Bearer Token, API Key
- [ ] 集成到 HTTP 代理路由

### Phase 3: 高级功能（2 周）
- [ ] 凭据缓存和刷新
- [ ] 审计日志
- [ ] 凭据验证和轮换
- [ ] 访问控制

### Phase 4: 远程提供者（1 周）
- [ ] 支持 AWS Secrets Manager
- [ ] 支持 1Password
- [ ] 支持 Bitwarden

**总计**：6 周

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_auth_credential() {
        let fnox_config = create_test_fnox_config().await;
        let mut credentials = HashMap::new();
        credentials.insert("admin".to_string(), Credential::Basic {
            username_secret: "admin_user".to_string(),
            password_secret: "admin_pass".to_string(),
            realm: Some("Admin".to_string()),
        });
        
        let manager = CredentialManager::new(&fnox_config, credentials).unwrap();
        let value = manager.get("admin").await.unwrap();
        
        match value {
            CredentialValue::Basic { username, password, realm } => {
                assert_eq!(username, "admin");
                assert_eq!(password, "secret");
                assert_eq!(realm, "Admin");
            }
            _ => panic!("Expected Basic credential"),
        }
    }

    #[tokio::test]
    async fn test_credential_caching() {
        let manager = create_test_manager().await;
        
        // 第一次获取（从 fnox 解密）
        let start = Instant::now();
        let _ = manager.get("admin").await.unwrap();
        let first_duration = start.elapsed();
        
        // 第二次获取（从缓存）
        let start = Instant::now();
        let _ = manager.get("admin").await.unwrap();
        let second_duration = start.elapsed();
        
        // 缓存应该更快
        assert!(second_duration < first_duration);
    }
}
```

### 8.2 集成测试

```rust
#[tokio::test]
async fn test_http_basic_auth() {
    // 启动测试代理服务器
    let config = ProxyConfig {
        routes: vec![
            RouteConfig {
                path: "/admin".to_string(),
                backend: "backend:http".to_string(),
                auth: Some("admin".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    
    let proxy = ProxyServer::start(config, create_test_cred_manager()).await.unwrap();
    
    // 无认证 -> 401
    let client = reqwest::Client::new();
    let resp = client.get(&format!("http://{}/admin", proxy.addr())).send().await.unwrap();
    assert_eq!(resp.status(), 401);
    
    // 错误凭据 -> 401
    let resp = client
        .get(&format!("http://{}/admin", proxy.addr()))
        .basic_auth("admin", Some("wrong_password"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    
    // 正确凭据 -> 200
    let resp = client
        .get(&format!("http://{}/admin", proxy.addr()))
        .basic_auth("admin", Some("secret"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}
```

---

## 9. 参考资料

- [fnox 官方文档](https://fnox.jdx.dev)
- [fnox GitHub](https://github.com/jdx/fnox)
- [age 加密规范](https://age-encryption.org)
- [HTTP Authentication: Basic and Digest Access Authentication (RFC 7617)](https://tools.ietf.org/html/rfc7617)
- [The OAuth 2.0 Authorization Framework: Bearer Token Usage (RFC 6750)](https://tools.ietf.org/html/rfc6750)
- [01-config-design.md](./01-config-design.md) - svcmgr 配置格式
- [05-web-service.md](./05-web-service.md) - HTTP 代理设计
- [10-api-overview.md](./10-api-overview.md) - API 认证机制
