use crate::atoms::systemd::{SystemdAtom, SystemdManager};
use crate::error::{Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tokio::fs;

// ========================================
// Data Structures
// ========================================

#[derive(Debug, Clone)]
pub struct HttpProxyConfig {
    pub location: String,
    pub upstream: String,
    pub websocket: bool,
    pub proxy_headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct TcpProxyConfig {
    pub listen_port: u16,
    pub upstream: String,
}

#[derive(Debug, Clone)]
pub struct StaticSiteConfig {
    pub location: String,
    pub root: PathBuf,
    pub autoindex: bool,
    pub index: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TtyRoute {
    pub name: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct NginxStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub worker_processes: u32,
    pub connections: u32,
}

// ========================================
// ProxyAtom Trait
// ========================================

pub trait ProxyAtom {
    fn start(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn stop(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn reload(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn status(&self) -> impl std::future::Future<Output = Result<NginxStatus>> + Send;
    fn test_config(&self) -> impl std::future::Future<Output = Result<bool>> + Send;
    
    fn add_http_proxy(&self, config: &HttpProxyConfig) -> Result<()>;
    fn remove_http_proxy(&self, location: &str) -> Result<()>;
    fn list_http_proxies(&self) -> Result<Vec<HttpProxyConfig>>;
    
    fn add_tcp_proxy(&self, config: &TcpProxyConfig) -> Result<()>;
    fn remove_tcp_proxy(&self, listen_port: u16) -> Result<()>;
    fn list_tcp_proxies(&self) -> Result<Vec<TcpProxyConfig>>;
    
    fn add_static_site(&self, config: &StaticSiteConfig) -> Result<()>;
    fn remove_static_site(&self, location: &str) -> Result<()>;
    fn list_static_sites(&self) -> Result<Vec<StaticSiteConfig>>;
    
    fn add_tty_route(&self, name: &str, port: u16) -> Result<()>;
    fn remove_tty_route(&self, name: &str) -> Result<()>;
    fn list_tty_routes(&self) -> Result<Vec<TtyRoute>>;
}

// ========================================
// NginxManager Implementation
// ========================================

pub struct NginxManager {
    config_dir: PathBuf,
    data_dir: PathBuf,
    systemd: SystemdManager,
}

impl NginxManager {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf, systemd: SystemdManager) -> Self {
        Self {
            config_dir,
            data_dir,
            systemd,
        }
    }

    pub fn default_config(systemd: SystemdManager) -> Result<Self> {
        let home = std::env::var("HOME")
            .map_err(|_| Error::Config("HOME environment variable not set".to_string()))?;
        let config_dir = PathBuf::from(&home).join(".config/svcmgr/nginx");
        let data_dir = PathBuf::from(&home).join(".local/share/svcmgr/nginx");
        Ok(Self::new(config_dir, data_dir, systemd))
    }

    fn nginx_conf_path(&self) -> PathBuf {
        self.config_dir.join("nginx.conf")
    }

    fn conf_d_path(&self, filename: &str) -> PathBuf {
        self.config_dir.join("conf.d").join(filename)
    }

    fn pid_file_path(&self) -> PathBuf {
        self.data_dir.join("run/nginx.pid")
    }

    async fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir).await?;
        fs::create_dir_all(self.config_dir.join("conf.d")).await?;
        fs::create_dir_all(self.data_dir.join("logs")).await?;
        fs::create_dir_all(self.data_dir.join("run")).await?;
        fs::create_dir_all(self.data_dir.join("cache")).await?;
        Ok(())
    }

    fn run_nginx(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("nginx").args(args).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: format!("nginx {}", args.join(" ")),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn generate_http_proxy_block(&self, config: &HttpProxyConfig) -> String {
        let mut block = format!("# HTTP Proxy for {}\n", config.location);
        block.push_str(&format!("location {} {{\n", config.location));
        block.push_str(&format!("    proxy_pass {};\n", config.upstream));
        block.push_str("    proxy_set_header Host $host;\n");
        block.push_str("    proxy_set_header X-Real-IP $remote_addr;\n");
        block.push_str("    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n");
        block.push_str("    proxy_set_header X-Forwarded-Proto $scheme;\n");

        if config.websocket {
            block.push_str("    proxy_http_version 1.1;\n");
            block.push_str("    proxy_set_header Upgrade $http_upgrade;\n");
            block.push_str("    proxy_set_header Connection \"upgrade\";\n");
            block.push_str("    proxy_read_timeout 86400;\n");
        }

        for (key, value) in &config.proxy_headers {
            block.push_str(&format!("    proxy_set_header {} \"{}\";\n", key, value));
        }

        block.push_str("}\n\n");
        block
    }

    fn parse_http_proxies(&self, content: &str) -> Vec<HttpProxyConfig> {
        let mut proxies = Vec::new();
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.starts_with("location ") && trimmed.ends_with('{') {
                let location = trimmed
                    .strip_prefix("location ")
                    .and_then(|s| s.strip_suffix(" {"))
                    .unwrap_or("")
                    .trim()
                    .to_string();

                let mut upstream = String::new();
                let mut websocket = false;
                let mut headers = HashMap::new();

                for inner_line in lines.by_ref() {
                    let inner_trimmed = inner_line.trim();
                    if inner_trimmed == "}" {
                        break;
                    }

                    if let Some(rest) = inner_trimmed.strip_prefix("proxy_pass ") {
                        upstream = rest
                            .trim_end_matches(';')
                            .trim()
                            .to_string();
                    } else if inner_trimmed.contains("Upgrade $http_upgrade") {
                        websocket = true;
                    } else if let Some(rest) = inner_trimmed.strip_prefix("proxy_set_header ") {
                        let rest = rest.trim_end_matches(';').trim();
                        if let Some((key, value)) = rest.split_once(' ') {
                            let value = value.trim_matches('"');
                            if !["Host", "X-Real-IP", "X-Forwarded-For", "X-Forwarded-Proto", "Upgrade", "Connection"].contains(&key) {
                                headers.insert(key.to_string(), value.to_string());
                            }
                        }
                    }
                }

                if !upstream.is_empty() {
                    proxies.push(HttpProxyConfig {
                        location,
                        upstream,
                        websocket,
                        proxy_headers: headers,
                    });
                }
            }
        }

        proxies
    }

    fn generate_tcp_proxy_block(&self, config: &TcpProxyConfig) -> String {
        format!(
            "# TCP proxy for port {}\nserver {{\n    listen {};\n    proxy_pass {};\n}}\n\n",
            config.listen_port, config.listen_port, config.upstream
        )
    }

    fn parse_tcp_proxies(&self, content: &str) -> Vec<TcpProxyConfig> {
        let mut proxies = Vec::new();
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed == "server {" {
                let mut listen_port = 0u16;
                let mut upstream = String::new();

                for inner_line in lines.by_ref() {
                    let inner_trimmed = inner_line.trim();
                    if inner_trimmed == "}" {
                        break;
                    }

                    if let Some(port_str) = inner_trimmed
                        .strip_prefix("listen ")
                        .and_then(|s| s.strip_suffix(';'))
                    {
                        listen_port = port_str.trim().parse().unwrap_or(0);
                    } else if let Some(ups) = inner_trimmed
                        .strip_prefix("proxy_pass ")
                        .and_then(|s| s.strip_suffix(';'))
                    {
                        upstream = ups.trim().to_string();
                    }
                }

                if listen_port > 0 && !upstream.is_empty() {
                    proxies.push(TcpProxyConfig {
                        listen_port,
                        upstream,
                    });
                }
            }
        }

        proxies
    }

    fn generate_static_site_block(&self, config: &StaticSiteConfig) -> String {
        let mut block = format!("# Static site for {}\n", config.location);
        block.push_str(&format!("location {} {{\n", config.location));
        block.push_str(&format!("    root {};\n", config.root.display()));

        if config.autoindex {
            block.push_str("    autoindex on;\n");
        }

        if !config.index.is_empty() {
            block.push_str(&format!("    index {};\n", config.index.join(" ")));
        }

        block.push_str("}\n\n");
        block
    }

    fn parse_static_sites(&self, content: &str) -> Vec<StaticSiteConfig> {
        let mut sites = Vec::new();
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.starts_with("location ") && trimmed.ends_with('{') {
                let location = trimmed
                    .strip_prefix("location ")
                    .and_then(|s| s.strip_suffix(" {"))
                    .unwrap_or("")
                    .trim()
                    .to_string();

                let mut root = PathBuf::new();
                let mut autoindex = false;
                let mut index = Vec::new();

                for inner_line in lines.by_ref() {
                    let inner_trimmed = inner_line.trim();
                    if inner_trimmed == "}" {
                        break;
                    }

                    if let Some(path_str) = inner_trimmed
                        .strip_prefix("root ")
                        .and_then(|s| s.strip_suffix(';'))
                    {
                        root = PathBuf::from(path_str.trim());
                    } else if inner_trimmed == "autoindex on;" {
                        autoindex = true;
                    } else if let Some(index_str) = inner_trimmed
                        .strip_prefix("index ")
                        .and_then(|s| s.strip_suffix(';'))
                    {
                        index = index_str
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect();
                    }
                }

                if !root.as_os_str().is_empty() {
                    sites.push(StaticSiteConfig {
                        location,
                        root,
                        autoindex,
                        index,
                    });
                }
            }
        }

        sites
    }

    fn generate_tty_route_block(&self, name: &str, port: u16) -> String {
        format!(
            "# TTY route for {}\nlocation /tty/{}/ {{\n    proxy_pass http://127.0.0.1:{}/;\n    proxy_http_version 1.1;\n    proxy_set_header Upgrade $http_upgrade;\n    proxy_set_header Connection \"upgrade\";\n    proxy_set_header Host $host;\n    proxy_read_timeout 86400;\n}}\n\n",
            name, name, port
        )
    }

    fn parse_tty_routes(&self, content: &str) -> Vec<TtyRoute> {
        let mut routes = Vec::new();
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.starts_with("location /tty/") && trimmed.ends_with('{') {
                if let Some(name_part) = trimmed.strip_prefix("location /tty/") {
                    if let Some(name) = name_part.strip_suffix("/ {") {
                        let name = name.trim().to_string();
                        let mut port = 0u16;

                        while let Some(inner_line) = lines.next() {
                            let inner_trimmed = inner_line.trim();
                            if inner_trimmed == "}" {
                                break;
                            }

                            if inner_trimmed.starts_with("proxy_pass http://127.0.0.1:") {
                                if let Some(port_part) = inner_trimmed
                                    .strip_prefix("proxy_pass http://127.0.0.1:")
                                {
                                    if let Some(port_str) = port_part.split('/').next() {
                                        port = port_str.parse().unwrap_or(0);
                                    }
                                }
                            }
                        }

                        if port > 0 {
                            routes.push(TtyRoute { name, port });
                        }
                    }
                }
            }
        }

        routes
    }

    async fn atomic_write(&self, path: &PathBuf, content: &str) -> Result<()> {
        let backup_path = path.with_extension("bak");
        
        if path.exists() {
            fs::copy(path, &backup_path).await?;
        }

        match fs::write(path, content).await {
            Ok(_) => {
                if backup_path.exists() {
                    let _ = fs::remove_file(&backup_path).await;
                }
                Ok(())
            }
            Err(e) => {
                if backup_path.exists() {
                    let _ = fs::copy(&backup_path, path).await;
                    let _ = fs::remove_file(&backup_path).await;
                }
                Err(e.into())
            }
        }
    }
}

impl ProxyAtom for NginxManager {
    async fn start(&self) -> Result<()> {
        self.ensure_dirs().await?;

        if !self.nginx_conf_path().exists() {
            return Err(Error::Config(
                "nginx.conf not found. Please create it first.".to_string(),
            ));
        }

        self.systemd.start("nginx").await
    }

    async fn stop(&self) -> Result<()> {
        self.systemd.stop("nginx").await
    }

    async fn reload(&self) -> Result<()> {
        let config_path = self.nginx_conf_path();
        let output = Command::new("nginx")
            .args(&["-s", "reload", "-c", &config_path.to_string_lossy()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: "nginx -s reload".to_string(),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }

        Ok(())
    }

    async fn status(&self) -> Result<NginxStatus> {
        let pid_file = self.pid_file_path();
        let running = pid_file.exists();
        let mut pid = None;

        if running {
            if let Ok(pid_content) = fs::read_to_string(&pid_file).await {
                pid = pid_content.trim().parse().ok();
            }
        }

        Ok(NginxStatus {
            running,
            pid,
            worker_processes: 0,
            connections: 0,
        })
    }

    async fn test_config(&self) -> Result<bool> {
        let config_path = self.nginx_conf_path();
        let output = Command::new("nginx")
            .args(&["-t", "-c", &config_path.to_string_lossy()])
            .output()?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(output.status.success()
            && stderr.contains("syntax is ok")
            && stderr.contains("test is successful"))
    }

    fn add_http_proxy(&self, config: &HttpProxyConfig) -> Result<()> {
        let path = self.conf_d_path("http-proxies.conf");
        let content = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        };

        let existing = self.parse_http_proxies(&content);
        if existing.iter().any(|p| p.location == config.location) {
            return Err(Error::DuplicateLocation {
                location: config.location.clone(),
            });
        }

        let block = self.generate_http_proxy_block(config);
        let new_content = content + &block;

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn remove_http_proxy(&self, location: &str) -> Result<()> {
        let path = self.conf_d_path("http-proxies.conf");
        if !path.exists() {
            return Err(Error::NotSupported(format!(
                "HTTP proxy {} not found",
                location
            )));
        }

        let content = std::fs::read_to_string(&path)?;
        let proxies = self.parse_http_proxies(&content);
        
        let filtered: Vec<_> = proxies
            .into_iter()
            .filter(|p| p.location != location)
            .collect();

        let mut new_content = String::new();
        for proxy in filtered {
            new_content.push_str(&self.generate_http_proxy_block(&proxy));
        }

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn list_http_proxies(&self) -> Result<Vec<HttpProxyConfig>> {
        let path = self.conf_d_path("http-proxies.conf");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&path)?;
        Ok(self.parse_http_proxies(&content))
    }

    fn add_tcp_proxy(&self, config: &TcpProxyConfig) -> Result<()> {
        let path = self.conf_d_path("tcp-proxies.conf");
        let content = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        };

        let existing = self.parse_tcp_proxies(&content);
        if existing.iter().any(|p| p.listen_port == config.listen_port) {
            return Err(Error::PortInUse {
                port: config.listen_port,
            });
        }

        let block = self.generate_tcp_proxy_block(config);
        let new_content = content + &block;

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn remove_tcp_proxy(&self, listen_port: u16) -> Result<()> {
        let path = self.conf_d_path("tcp-proxies.conf");
        if !path.exists() {
            return Err(Error::NotSupported(format!(
                "TCP proxy on port {} not found",
                listen_port
            )));
        }

        let content = std::fs::read_to_string(&path)?;
        let proxies = self.parse_tcp_proxies(&content);
        
        let filtered: Vec<_> = proxies
            .into_iter()
            .filter(|p| p.listen_port != listen_port)
            .collect();

        let mut new_content = String::new();
        for proxy in filtered {
            new_content.push_str(&self.generate_tcp_proxy_block(&proxy));
        }

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn list_tcp_proxies(&self) -> Result<Vec<TcpProxyConfig>> {
        let path = self.conf_d_path("tcp-proxies.conf");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&path)?;
        Ok(self.parse_tcp_proxies(&content))
    }

    fn add_static_site(&self, config: &StaticSiteConfig) -> Result<()> {
        let path = self.conf_d_path("static-sites.conf");
        let content = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        };

        let existing = self.parse_static_sites(&content);
        if existing.iter().any(|s| s.location == config.location) {
            return Err(Error::DuplicateLocation {
                location: config.location.clone(),
            });
        }

        let block = self.generate_static_site_block(config);
        let new_content = content + &block;

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn remove_static_site(&self, location: &str) -> Result<()> {
        let path = self.conf_d_path("static-sites.conf");
        if !path.exists() {
            return Err(Error::NotSupported(format!(
                "Static site {} not found",
                location
            )));
        }

        let content = std::fs::read_to_string(&path)?;
        let sites = self.parse_static_sites(&content);
        
        let filtered: Vec<_> = sites
            .into_iter()
            .filter(|s| s.location != location)
            .collect();

        let mut new_content = String::new();
        for site in filtered {
            new_content.push_str(&self.generate_static_site_block(&site));
        }

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn list_static_sites(&self) -> Result<Vec<StaticSiteConfig>> {
        let path = self.conf_d_path("static-sites.conf");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&path)?;
        Ok(self.parse_static_sites(&content))
    }

    fn add_tty_route(&self, name: &str, port: u16) -> Result<()> {
        let path = self.conf_d_path("tty-routes.conf");
        let content = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        };

        let existing = self.parse_tty_routes(&content);
        if existing.iter().any(|r| r.name == name) {
            return Err(Error::DuplicateLocation {
                location: format!("/tty/{}", name),
            });
        }

        let block = self.generate_tty_route_block(name, port);
        let new_content = content + &block;

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn remove_tty_route(&self, name: &str) -> Result<()> {
        let path = self.conf_d_path("tty-routes.conf");
        if !path.exists() {
            return Err(Error::NotSupported(format!("TTY route {} not found", name)));
        }

        let content = std::fs::read_to_string(&path)?;
        let routes = self.parse_tty_routes(&content);
        
        let filtered: Vec<_> = routes
            .into_iter()
            .filter(|r| r.name != name)
            .collect();

        let mut new_content = String::new();
        for route in filtered {
            new_content.push_str(&self.generate_tty_route_block(&route.name, route.port));
        }

        std::fs::write(&path, new_content)?;
        Ok(())
    }

    fn list_tty_routes(&self) -> Result<Vec<TtyRoute>> {
        let path = self.conf_d_path("tty-routes.conf");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&path)?;
        Ok(self.parse_tty_routes(&content))
    }
}

// ========================================
// Unit Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manager() -> NginxManager {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-nginx");
        let data_dir = tmpdir.join("data");
        let systemd_dir = tmpdir.join("systemd");
        NginxManager::new(
            tmpdir.clone(),
            data_dir,
            SystemdManager::new(systemd_dir, false),
        )
    }

    #[test]
    fn test_nginx_manager_creation() {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-nginx");
        let data_dir = tmpdir.join("data");
        let systemd_dir = tmpdir.join("systemd");
        let manager = NginxManager::new(
            tmpdir.clone(),
            data_dir.clone(),
            SystemdManager::new(systemd_dir, false),
        );
        assert_eq!(manager.config_dir, tmpdir);
        assert_eq!(manager.data_dir, data_dir);
    }

    #[test]
    fn test_http_proxy_config_generation() {
        let manager = create_test_manager();
        let config = HttpProxyConfig {
            location: "/api".to_string(),
            upstream: "http://localhost:3000".to_string(),
            websocket: false,
            proxy_headers: HashMap::new(),
        };

        let block = manager.generate_http_proxy_block(&config);
        assert!(block.contains("location /api"));
        assert!(block.contains("proxy_pass http://localhost:3000"));
        assert!(block.contains("proxy_set_header Host $host"));
    }

    #[test]
    fn test_websocket_proxy_config() {
        let manager = create_test_manager();
        let config = HttpProxyConfig {
            location: "/ws".to_string(),
            upstream: "http://localhost:8080".to_string(),
            websocket: true,
            proxy_headers: HashMap::new(),
        };

        let block = manager.generate_http_proxy_block(&config);
        assert!(block.contains("proxy_http_version 1.1"));
        assert!(block.contains("Upgrade $http_upgrade"));
        assert!(block.contains("Connection \"upgrade\""));
    }

    #[test]
    fn test_tcp_proxy_config() {
        let manager = create_test_manager();
        let config = TcpProxyConfig {
            listen_port: 5432,
            upstream: "postgres.internal:5432".to_string(),
        };

        let block = manager.generate_tcp_proxy_block(&config);
        assert!(block.contains("listen 5432"));
        assert!(block.contains("proxy_pass postgres.internal:5432"));
    }

    #[test]
    fn test_static_site_config() {
        let manager = create_test_manager();
        let config = StaticSiteConfig {
            location: "/static".to_string(),
            root: PathBuf::from("/var/www/html"),
            autoindex: true,
            index: vec!["index.html".to_string(), "index.htm".to_string()],
        };

        let block = manager.generate_static_site_block(&config);
        assert!(block.contains("location /static"));
        assert!(block.contains("root /var/www/html"));
        assert!(block.contains("autoindex on"));
        assert!(block.contains("index index.html index.htm"));
    }

    #[test]
    fn test_tty_route_generation() {
        let manager = create_test_manager();
        let block = manager.generate_tty_route_block("terminal1", 7681);
        assert!(block.contains("location /tty/terminal1/"));
        assert!(block.contains("proxy_pass http://127.0.0.1:7681/"));
        assert!(block.contains("proxy_http_version 1.1"));
        assert!(block.contains("Upgrade $http_upgrade"));
    }

    #[test]
    fn test_parse_http_proxies() {
        let manager = create_test_manager();
        let content = r#"# HTTP Proxy for /api
location /api {
    proxy_pass http://localhost:3000;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}

# HTTP Proxy for /ws
location /ws {
    proxy_pass http://localhost:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 86400;
}
"#;

        let proxies = manager.parse_http_proxies(content);
        assert_eq!(proxies.len(), 2);
        assert_eq!(proxies[0].location, "/api");
        assert_eq!(proxies[0].upstream, "http://localhost:3000");
        assert!(!proxies[0].websocket);
        assert_eq!(proxies[1].location, "/ws");
        assert!(proxies[1].websocket);
    }

    #[test]
    fn test_parse_tcp_proxies() {
        let manager = create_test_manager();
        let content = r#"# TCP proxy for port 5432
server {
    listen 5432;
    proxy_pass postgres.internal:5432;
}

# TCP proxy for port 3306
server {
    listen 3306;
    proxy_pass mysql.internal:3306;
}
"#;

        let proxies = manager.parse_tcp_proxies(content);
        assert_eq!(proxies.len(), 2);
        assert_eq!(proxies[0].listen_port, 5432);
        assert_eq!(proxies[0].upstream, "postgres.internal:5432");
        assert_eq!(proxies[1].listen_port, 3306);
    }
}
