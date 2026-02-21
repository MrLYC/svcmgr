import { createContext, useContext, useState, useCallback, ReactNode } from "react";

type Locale = "zh" | "en";

const translations: Record<string, Record<Locale, string>> = {
  // Nav
  "nav.dashboard": { zh: "仪表盘", en: "Dashboard" },
  "nav.services": { zh: "服务", en: "Services" },
  "nav.proxy": { zh: "代理", en: "Proxy" },
  "nav.tty": { zh: "终端", en: "Terminal" },
  "nav.config": { zh: "配置", en: "Config" },
  "nav.settings": { zh: "设置", en: "Settings" },
  "nav.systemd": { zh: "系统服务", en: "System Services" },
  "nav.crontab": { zh: "定时任务", en: "Crontab" },
  "nav.mise": { zh: "工具管理", en: "Tool Manager" },
  "nav.nginx": { zh: "Nginx 代理", en: "Nginx" },
  "nav.cloudflare": { zh: "Cloudflare 隧道", en: "Cloudflare Tunnels" },

  // Dashboard
  "dash.title": { zh: "仪表盘", en: "Dashboard" },
  "dash.systemd_services": { zh: "系统服务", en: "System Services" },
  "dash.crontab_tasks": { zh: "定时任务", en: "Crontab Tasks" },
  "dash.nginx_proxies": { zh: "Nginx 代理", en: "Nginx Proxies" },
  "dash.cf_tunnels": { zh: "CF 隧道", en: "CF Tunnels" },
  "dash.running": { zh: "运行中", en: "running" },
  "dash.active": { zh: "已启用", en: "active" },
  "dash.configured": { zh: "已配置", en: "configured" },
  "dash.connected": { zh: "已连接", en: "connected" },
  "dash.recent_activity": { zh: "最近活动", en: "Recent Activity" },
  "dash.service": { zh: "服务", en: "Service" },
  "dash.proxy": { zh: "代理", en: "Proxy" },

  // Common
  "common.create": { zh: "创建", en: "Create" },
  "common.delete": { zh: "删除", en: "Delete" },
  "common.save": { zh: "保存", en: "Save" },
  "common.cancel": { zh: "取消", en: "Cancel" },
  "common.loading": { zh: "加载中...", en: "Loading..." },
  "common.name": { zh: "名称", en: "Name" },
  "common.status": { zh: "状态", en: "Status" },
  "common.actions": { zh: "操作", en: "Actions" },
  "common.description": { zh: "描述", en: "Description" },
  "common.command": { zh: "命令", en: "Command" },
  "common.enabled": { zh: "已启用", en: "Enabled" },
  "common.type": { zh: "类型", en: "Type" },
  "common.target": { zh: "目标", en: "Target" },
  "common.path": { zh: "路径", en: "Path" },
  "common.domain": { zh: "域名", en: "Domain" },
  "common.uptime": { zh: "运行时长", en: "Uptime" },
  "common.created": { zh: "创建时间", en: "Created" },
  "common.save_changes": { zh: "保存更改", en: "Save Changes" },
  "common.back": { zh: "返回", en: "Back" },
  "common.edit": { zh: "编辑", en: "Edit" },
  "common.updated": { zh: "已更新", en: "Updated" },
  "common.confirm_delete": { zh: "确认删除", en: "Confirm Delete" },
  "common.confirm_delete_desc": { zh: "此操作不可撤销，确定要删除吗？", en: "This action cannot be undone. Are you sure you want to delete?" },
  "common.start": { zh: "启动", en: "Start" },
  "common.stop": { zh: "停止", en: "Stop" },
  "common.restart": { zh: "重启", en: "Restart" },
  "common.open": { zh: "打开", en: "Open" },

  // Systemd
  "systemd.title": { zh: "系统服务", en: "System Services" },
  "systemd.create_service": { zh: "创建服务", en: "Create Service" },
  "systemd.pid": { zh: "PID", en: "PID" },
  "systemd.memory": { zh: "内存", en: "Memory" },
  "systemd.service_name": { zh: "服务名称", en: "Service Name" },
  "systemd.restart_policy": { zh: "重启策略", en: "Restart Policy" },
  "systemd.exec_start": { zh: "启动命令", en: "ExecStart Command" },
  "systemd.working_dir": { zh: "工作目录", en: "Working Directory" },
  "systemd.env_vars": { zh: "环境变量（每行一个）", en: "Environment Variables (one per line)" },
  "systemd.form": { zh: "表单", en: "Form" },
  "systemd.preview": { zh: "预览", en: "Preview" },
  "systemd.action_completed": { zh: "服务操作已完成", en: "Service action completed" },
  "systemd.service_created": { zh: "服务已创建", en: "Service created" },
  "systemd.service_deleted": { zh: "服务已删除", en: "Service deleted" },
  "systemd.logs": { zh: "日志", en: "Logs" },
  "systemd.edit_service": { zh: "编辑服务", en: "Edit Service" },
  "systemd.service_updated": { zh: "服务已更新", en: "Service updated" },

  // Crontab
  "crontab.title": { zh: "定时任务", en: "Crontab Tasks" },
  "crontab.create_task": { zh: "创建任务", en: "Create Task" },
  "crontab.schedule": { zh: "调度", en: "Schedule" },
  "crontab.last_run": { zh: "上次执行", en: "Last Run" },
  "crontab.schedule_preset": { zh: "调度预设", en: "Schedule Preset" },
  "crontab.cron_expression": { zh: "Cron 表达式", en: "Cron Expression" },
  "crontab.task_created": { zh: "任务已创建", en: "Task created" },
  "crontab.task_deleted": { zh: "任务已删除", en: "Task deleted" },
  "crontab.edit_task": { zh: "编辑任务", en: "Edit Task" },
  "crontab.task_updated": { zh: "任务已更新", en: "Task updated" },
  "crontab.every_minute": { zh: "每分钟", en: "Every minute" },
  "crontab.every_5_min": { zh: "每5分钟", en: "Every 5 minutes" },
  "crontab.every_hour": { zh: "每小时", en: "Every hour" },
  "crontab.daily_midnight": { zh: "每天午夜", en: "Daily at midnight" },
  "crontab.daily_2am": { zh: "每天凌晨2点", en: "Daily at 2 AM" },
  "crontab.weekly_sunday": { zh: "每周日", en: "Weekly on Sunday" },
  "crontab.monthly": { zh: "每月", en: "Monthly" },

  // Mise / Tool Manager
  "mise.title": { zh: "工具管理", en: "Tool Manager" },
  "mise.dependencies": { zh: "依赖管理", en: "Dependencies" },
  "mise.tasks": { zh: "任务管理", en: "Tasks" },
  "mise.tool": { zh: "工具", en: "Tool" },
  "mise.current": { zh: "当前版本", en: "Current" },
  "mise.latest": { zh: "最新版本", en: "Latest" },
  "mise.source": { zh: "来源", en: "Source" },
  "mise.up_to_date": { zh: "已是最新", en: "up to date" },
  "mise.installed_versions": { zh: "已安装版本", en: "Installed Versions" },
  "mise.install_version": { zh: "安装新版本", en: "Install New Version" },
  "mise.install": { zh: "安装", en: "Install" },
  "mise.uninstall": { zh: "卸载工具", en: "Uninstall" },
  "mise.run_task": { zh: "执行任务", en: "Run Task" },
  "mise.active": { zh: "使用中", en: "Active" },
  "mise.inactive": { zh: "未使用", en: "Inactive" },
  "mise.activate": { zh: "激活", en: "Activate" },
  "mise.select_version": { zh: "选择版本", en: "Select version" },
  "mise.version_uninstalled": { zh: "版本已卸载", en: "Version uninstalled" },
  "mise.task_executed": { zh: "任务已执行", en: "Task executed" },
  "mise.add_dep": { zh: "添加依赖", en: "Add Dependency" },
  "mise.dep_added": { zh: "依赖已添加", en: "Dependency added" },
  "mise.dep_deleted": { zh: "依赖已删除", en: "Dependency deleted" },
  "mise.dep_updated": { zh: "版本已切换", en: "Version switched" },
  "mise.switch_version": { zh: "切换版本", en: "Switch Version" },
  "mise.new_version": { zh: "新版本", en: "New Version" },
  "mise.tool_name": { zh: "工具名称", en: "Tool Name" },
  "mise.version": { zh: "版本", en: "Version" },
  "mise.add_task": { zh: "添加任务", en: "Add Task" },
  "mise.edit_task": { zh: "编辑任务", en: "Edit Task" },
  "mise.task_created": { zh: "任务已创建", en: "Task created" },
  "mise.task_updated": { zh: "任务已更新", en: "Task updated" },
  "mise.task_deleted": { zh: "任务已删除", en: "Task deleted" },
  "mise.operation_log": { zh: "操作日志", en: "Operation Log" },
  "mise.installing": { zh: "正在安装", en: "Installing" },
  "mise.uninstalling": { zh: "正在卸载", en: "Uninstalling" },

  // Nginx
  "nginx.title": { zh: "Nginx 代理", en: "Nginx Proxies" },
  "nginx.create_proxy": { zh: "创建代理", en: "Create Proxy" },
  "nginx.proxy_type": { zh: "代理类型", en: "Proxy Type" },
  "nginx.static_files": { zh: "静态文件", en: "Static Files" },
  "nginx.http_proxy": { zh: "HTTP 代理", en: "HTTP Proxy" },
  "nginx.tcp_proxy": { zh: "TCP 代理", en: "TCP Proxy" },
  "nginx.root_dir": { zh: "根目录", en: "Root Directory" },
  "nginx.proxy_created": { zh: "代理已创建", en: "Proxy created" },
  "nginx.proxy_deleted": { zh: "代理已删除", en: "Proxy deleted" },
  "nginx.connectivity_test": { zh: "连通性测试", en: "Connectivity Test" },
  "nginx.edit_proxy": { zh: "编辑代理", en: "Edit Proxy" },
  "nginx.proxy_updated": { zh: "代理已更新", en: "Proxy updated" },
  "nginx.built_in": { zh: "系统内置", en: "Built-in" },
  "nginx.built_in_hint": { zh: "系统内置规则，不可删除", en: "Built-in rule, cannot be deleted" },

  // Cloudflare
  "cf.title": { zh: "Cloudflare 隧道", en: "Cloudflare Tunnels" },
  "cf.create_tunnel": { zh: "创建隧道", en: "Create Tunnel" },
  "cf.tunnel_name": { zh: "隧道名称", en: "Tunnel Name" },
  "cf.service_url": { zh: "服务 URL", en: "Service URL" },
  "cf.tunnel_created": { zh: "隧道已创建", en: "Tunnel created" },
  "cf.tunnel_deleted": { zh: "隧道已删除", en: "Tunnel deleted" },
  "cf.service": { zh: "服务地址", en: "Service" },
  "cf.edit_tunnel": { zh: "编辑隧道", en: "Edit Tunnel" },
  "cf.tunnel_updated": { zh: "隧道已更新", en: "Tunnel updated" },

  // TTY
  "tty.title": { zh: "终端", en: "Terminal" },
  "tty.start_session": { zh: "启动会话", en: "Start Session" },
  "tty.session_started": { zh: "会话已启动", en: "Session started" },
  "tty.create_session": { zh: "创建会话", en: "Create Session" },
  "tty.session_name": { zh: "会话名称", en: "Session Name" },
  "tty.require_password": { zh: "需要密码", en: "Require Password" },
  "tty.session_created": { zh: "会话已创建", en: "Session created" },
  "tty.session_deleted": { zh: "会话已删除", en: "Session deleted" },
  "tty.edit_session": { zh: "编辑会话", en: "Edit Session" },
  "tty.session_updated": { zh: "会话已更新", en: "Session updated" },

  // Config
  "config.title": { zh: "配置管理", en: "Configuration" },
  "config.commit_changes": { zh: "提交变更", en: "Commit Changes" },
  "config.pending_changes": { zh: "待提交变更", en: "Pending Changes" },
  "config.commit_history": { zh: "提交历史", en: "Commit History" },
  "config.uncommitted": { zh: "未提交变更", en: "uncommitted changes" },
  "config.managed_dirs": { zh: "管理目录", en: "Managed Directories" },
  "config.add_dir": { zh: "添加目录", en: "Add Directory" },
  "config.dir_path": { zh: "目录路径", en: "Directory Path" },
  "config.dir_added": { zh: "目录已添加", en: "Directory added" },
  "config.dir_removed": { zh: "目录已移除", en: "Directory removed" },
  "config.remove_dir": { zh: "移除", en: "Remove" },
  "config.commit_message": { zh: "提交消息", en: "Commit Message" },
  "config.files": { zh: "文件", en: "Files" },
  "config.commit": { zh: "提交", en: "Commit" },
  "config.hash": { zh: "哈希", en: "Hash" },
  "config.message": { zh: "消息", en: "Message" },
  "config.author": { zh: "作者", en: "Author" },
  "config.time": { zh: "时间", en: "Time" },
  "config.committed": { zh: "变更已提交", en: "Changes committed" },
  "config.rollback_done": { zh: "回滚完成", en: "Rollback completed" },
  "config.back": { zh: "返回", en: "Back" },
  "config.last_commit": { zh: "最近提交", en: "Last Commit" },
  "config.no_changes": { zh: "无变更", en: "No changes" },
  "config.changes_count": { zh: "处变更", en: "changes" },

  // Settings
  "settings.title": { zh: "设置", en: "Settings" },
  "settings.general": { zh: "基础配置", en: "General Configuration" },
  "settings.nginx_port": { zh: "Nginx 端口", en: "Nginx Port" },
  "settings.log_level": { zh: "日志级别", en: "Log Level" },
  "settings.config_dir": { zh: "配置目录", en: "Config Directory" },
  "settings.auto_commit": { zh: "自动提交变更", en: "Auto-commit Changes" },
  "settings.external_tools": { zh: "核心工具", en: "Core Tools" },
  "settings.not_installed": { zh: "未安装", en: "Not installed" },
  "settings.danger_zone": { zh: "危险操作", en: "Danger Zone" },
  "settings.danger_desc": { zh: "不可逆操作，请谨慎。", en: "Irreversible actions. Proceed with caution." },
  "settings.reset_confirm": { zh: "输入 RESET 确认系统重置。", en: "Type RESET to confirm system reset." },
  "settings.reset_system": { zh: "重置系统", en: "Reset System" },
  "settings.saved": { zh: "设置已保存", en: "Settings saved" },
  "settings.reset_done": { zh: "系统重置完成", en: "System reset completed" },
  "settings.language": { zh: "语言", en: "Language" },
};

interface I18nContextType {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: string) => string;
}

const I18nContext = createContext<I18nContextType>({
  locale: "zh",
  setLocale: () => {},
  t: (key) => key,
});

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocale] = useState<Locale>(() => {
    const saved = localStorage.getItem("svcmgr-locale");
    return (saved === "en" || saved === "zh") ? saved : "zh";
  });

  const changeLocale = useCallback((l: Locale) => {
    setLocale(l);
    localStorage.setItem("svcmgr-locale", l);
  }, []);

  const t = useCallback((key: string) => {
    return translations[key]?.[locale] ?? key;
  }, [locale]);

  return (
    <I18nContext.Provider value={{ locale, setLocale: changeLocale, t }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n() {
  return useContext(I18nContext);
}
