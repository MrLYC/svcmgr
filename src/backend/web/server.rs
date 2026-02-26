use crate::git::versioning::GitVersioning;
use crate::ports::mise_port::ConfigPort;
use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{info, Level};

/// HTTP 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpConfig {
    /// 绑定地址
    #[serde(default = "default_bind")]
    pub bind: String,

    /// 监听端口
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_bind() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_port(),
        }
    }
}

/// 全局应用状态(用于依赖注入)
#[derive(Clone)]
pub struct AppState {
    /// Git 版本控制
    pub git_versioning: Arc<tokio::sync::Mutex<GitVersioning>>,
    /// 配置端口(用于读写配置文件)
    pub config_port: Arc<dyn ConfigPort>,
    /// 配置文件根目录
    pub config_dir: PathBuf,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(
        git_versioning: GitVersioning,
        config_port: Arc<dyn ConfigPort>,
        config_dir: PathBuf,
    ) -> Self {
        Self {
            git_versioning: Arc::new(tokio::sync::Mutex::new(git_versioning)),
            config_port,
            config_dir,
        }
    }

    /// 创建用于开发/测试的默认状态(使用mock adapter)
    ///
    /// 注意: 此方法使用 MockMiseAdapter 和临时 Git 仓库,
    /// 仅用于开发和测试环境。生产代码应使用 AppState::new()
    /// 并传入真实的依赖项。
    pub fn for_development() -> Self {
        use crate::adapters::mock::MockMiseAdapter;
        use tempfile::TempDir;

        // 创建临时目录用于Git
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_dir = temp_dir.path().to_path_buf();

        // 初始化Git仓库
        let git = GitVersioning::init(&config_dir).expect("Failed to init git");

        // 创建 mock adapter (使用 MiseMock + MiseVersion)
        use crate::mocks::mise::MiseMock;
        use crate::ports::MiseVersion;
        let mock = MiseMock::new(config_dir.clone());
        let version = MiseVersion::new(2026, 2, 17); // 模拟最新版本
        let config_port: Arc<dyn ConfigPort> = Arc::new(MockMiseAdapter::new(mock, version));

        // 注意:temp_dir会在这里被drop,但git仓库已初始化,测试期间目录仍然存在
        std::mem::forget(temp_dir); // 防止提前删除

        Self {
            git_versioning: Arc::new(tokio::sync::Mutex::new(git)),
            config_port,
            config_dir,
        }
    }
}

/// 统一 API 错误类型
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// 错误代码（大写下划线格式，如 SERVICE_NOT_FOUND）
    pub code: String,
    /// 错误消息（人类可读）
    pub message: String,
    /// 额外详细信息（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// 请求追踪 ID（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            request_id: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::new(
            "RESOURCE_NOT_FOUND",
            format!("{} not found", resource.into()),
        )
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new("INTERNAL_ERROR", message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new("BAD_REQUEST", message)
    }
}

/// 统一错误响应格式
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.code.as_str() {
            "RESOURCE_NOT_FOUND"
            | "SERVICE_NOT_FOUND"
            | "TASK_NOT_FOUND"
            | "SCHEDULED_TASK_NOT_FOUND" => StatusCode::NOT_FOUND,
            "BAD_REQUEST" | "INVALID_INPUT" | "VALIDATION_ERROR" => StatusCode::BAD_REQUEST,
            "CONFLICT" | "ALREADY_EXISTS" => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorResponse { error: self });
        (status, body).into_response()
    }
}

/// 统一成功响应格式
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            pagination: None,
        }
    }

    pub fn with_pagination(mut self, pagination: Pagination) -> Self {
        self.pagination = Some(pagination);
        self
    }
}

/// 分页信息
#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

impl Pagination {
    pub fn new(page: u32, per_page: u32, total: u64) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;
        Self {
            page,
            per_page,
            total,
            total_pages,
        }
    }
}

/// HTTP 服务器主结构
pub struct HttpServer {
    config: HttpConfig,
    pub router: Router,
    /// 反向代理服务(可选)
    pub proxy: Option<Arc<crate::web::proxy::ProxyService>>,
}

impl HttpServer {
    /// 创建新的 HTTP 服务器实例
    pub fn new(config: HttpConfig) -> Self {
        // 创建测试用的 AppState (实际生产环境应传入真实依赖)
        let app_state = AppState::for_development();
        let router = Self::build_router(app_state);
        Self {
            config,
            router,
            proxy: None,
        }
    }

    /// 创建带代理服务的 HTTP 服务器
    pub fn with_proxy(config: HttpConfig, routes: Vec<crate::config::models::RouteConfig>) -> Self {
        // 创建测试用的 AppState (实际生产环境应传入真实依赖)
        let app_state = AppState::for_development();
        let router = Self::build_router(app_state);
        let proxy = Arc::new(crate::web::proxy::ProxyService::new(routes));
        Self {
            config,
            router,
            proxy: Some(proxy),
        }
    }

    /// 注册后端服务(服务启动时调用)
    pub async fn register_backend(&self, service: &str, port: &str, addr: std::net::SocketAddr) {
        if let Some(proxy) = &self.proxy {
            proxy.register_backend(service, port, addr).await;
        }
    }

    /// 注销后端服务(服务停止时调用)
    pub async fn unregister_backend(&self, service: &str, port: &str) {
        if let Some(proxy) = &self.proxy {
            proxy.unregister_backend(service, port).await;
        }
    }

    /// 更新后端健康状态
    pub async fn update_backend_health(&self, service: &str, port: &str, healthy: bool) {
        if let Some(proxy) = &self.proxy {
            proxy.update_backend_health(service, port, healthy).await;
        }
    }

    /// 构建路由
    fn build_router(app_state: AppState) -> Router {
        Router::new()
            // 健康检查端点
            .route("/health", get(health_check))
            // API v1 路由 (注入 AppState)
            .nest("/api/v1", crate::web::api::api_routes(app_state))
            // 配置中间件
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(Level::INFO)),
            )
            .layer(CorsLayer::permissive())
            // 全局 404 处理
            .fallback(handle_404)
    }

    /// 启动 HTTP 服务器
    pub async fn start(self) -> anyhow::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.bind, self.config.port).parse()?;

        info!("Starting HTTP server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;

        axum::serve(listener, self.router)
            .await
            .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

        Ok(())
    }
}

/// 健康检查端点处理器
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// 全局 404 处理器
async fn handle_404(req: Request) -> impl IntoResponse {
    ApiError::not_found(format!("Route: {}", req.uri().path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_config_default() {
        let config = HttpConfig::default();
        assert_eq!(config.bind, "127.0.0.1");
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn test_api_error_serialization() {
        let error = ApiError::new("TEST_ERROR", "Test error message").with_request_id("req_123");

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("TEST_ERROR"));
        assert!(json.contains("Test error message"));
        assert!(json.contains("req_123"));
    }

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse::new(serde_json::json!({"key": "value"}));
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("data"));
        assert!(json.contains("key"));
    }

    #[test]
    fn test_pagination() {
        let pagination = Pagination::new(1, 20, 100);
        assert_eq!(pagination.page, 1);
        assert_eq!(pagination.per_page, 20);
        assert_eq!(pagination.total, 100);
        assert_eq!(pagination.total_pages, 5);
    }
}
