// Systemd
export interface SystemdService {
  name: string;
  status: "running" | "stopped" | "failed";
  enabled: boolean;
  pid?: number;
  memory?: string;
  uptime?: string;
  description?: string;
  exec_start?: string;
  working_directory?: string;
  environment?: Record<string, string>;
  restart_policy?: string;
}

export interface SystemdLog {
  timestamp: string;
  level: "info" | "warning" | "error" | "debug";
  message: string;
  unit: string;
}

// Crontab
export interface CrontabTask {
  id: string;
  expression: string;
  command: string;
  enabled: boolean;
  description?: string;
  last_run?: string;
}

// Mise / Tool Manager
export interface MiseDependency {
  id: string;
  name: string;
  current_version: string;
  latest_version?: string;
  source: string;
  installed_versions?: string[];
}

export interface MiseTask {
  id: string;
  name: string;
  description?: string;
  command: string;
  source?: string;
}

// Nginx
export interface NginxProxy {
  id: string;
  path: string;
  proxy_type: "static" | "http" | "tcp";
  target: string;
  status: "active" | "inactive" | "error";
  root?: string;
  port?: number;
  built_in?: boolean;
}

// Cloudflare Tunnels
export interface CloudflareTunnel {
  id: string;
  name: string;
  domain: string;
  service_url: string;
  status: "connected" | "disconnected" | "degraded";
  uptime?: string;
}

// TTY
export interface TTYSession {
  id: string;
  name: string;
  command: string;
  url: string;
  status: "running" | "stopped";
  created_at: string;
  password?: boolean;
}

// Config
export interface ManagedDirectory {
  id: string;
  path: string;
  label: string;
  git_status?: "clean" | "dirty";
  branch?: string;
  uncommitted_changes?: number;
  last_commit_message?: string;
  last_commit_time?: string;
}

export interface ConfigStatus {
  path: string;
  git_status: "clean" | "dirty";
  branch: string;
  uncommitted_changes: number;
  last_commit?: string;
}

export interface ConfigChange {
  file: string;
  status: "modified" | "added" | "deleted";
  diff?: string;
}

export interface ConfigCommit {
  hash: string;
  message: string;
  author: string;
  timestamp: string;
}

// Settings
export interface SettingsConfig {
  nginx_port: number;
  config_dir: string;
  auto_commit: boolean;
  log_level: "debug" | "info" | "warn" | "error";
}

export interface ToolStatus {
  name: string;
  installed: boolean;
  version?: string;
  path?: string;
}

// Activity
export interface ActivityLog {
  id: string;
  timestamp: string;
  type: "systemd" | "crontab" | "nginx" | "cloudflare" | "tty" | "config" | "system";
  action: string;
  description: string;
}

// Dashboard
export interface DashboardStats {
  systemd_running: number;
  systemd_total: number;
  crontab_tasks: number;
  nginx_proxies: number;
  cloudflare_connected: number;
  cloudflare_total: number;
}
