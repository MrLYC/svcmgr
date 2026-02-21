

# Add Breadcrumb Navigation and Improve Consistency

## Overview

All detail views currently use a plain back-arrow button without context. This plan adds a unified breadcrumb component to every detail view, and fixes several consistency/usability issues across modules.

## 1. Create a Reusable PageBreadcrumb Component

Create `src/components/PageBreadcrumb.tsx` using the existing `breadcrumb.tsx` UI primitives. It accepts an array of breadcrumb items (label + optional onClick) and renders them consistently.

```
Dashboard / Services / System Services / nginx-app
```

The last item is rendered as the current page (non-clickable). Previous items are clickable links or callbacks.

## 2. Add Breadcrumbs to All Detail Views

Replace the `ArrowLeft` button + title pattern in every detail view with the new `PageBreadcrumb` component:

| Module | List Breadcrumb | Detail Breadcrumb |
|--------|----------------|-------------------|
| System Services | Services > System Services | Services > System Services > {name} |
| Crontab Tasks | Services > Crontab | Services > Crontab > {name} |
| Tool Manager (Dep) | Services > Tool Manager | Services > Tool Manager > {dep.name} |
| Tool Manager (Task) | Services > Tool Manager | Services > Tool Manager > {task.name} |
| Nginx Proxies | Proxy > Nginx | Proxy > Nginx > {path} |
| Cloudflare Tunnels | Proxy > Cloudflare | Proxy > Cloudflare > {name} |
| Terminal | Terminal | Terminal > {session.name} |
| Config Management | Config | Config > {dir.label} |

## 3. Consistency and Usability Fixes

### 3a. Delete Confirmation Dialog
Currently, all delete buttons execute immediately with no confirmation. Add an `AlertDialog` confirmation before destructive actions (delete service, delete task, delete proxy, delete tunnel, delete session, remove config dir).

### 3b. Standardize Detail View Header Layout
All detail views will follow the same structure:
1. Breadcrumb row at the top
2. Title + status/actions row below
3. Info card (metadata summary)
4. Edit card (form)

### 3c. Consistent Button Labels (i18n)
Some detail views use hardcoded English ("Start", "Stop", "Restart", "Open"). Add missing i18n keys and replace hardcoded strings.

## 4. i18n Updates

Add new translation keys:
- `common.confirm_delete` / `common.confirm_delete_desc` for the delete confirmation dialog
- `common.start` / `common.stop` / `common.restart` / `common.open` for action buttons
- Navigation-related breadcrumb labels (reuse existing `nav.*` keys)

## Technical Details

### Files to Create
- `src/components/PageBreadcrumb.tsx` - Reusable breadcrumb wrapper

### Files to Modify
- `src/pages/SystemdServices.tsx` - Add breadcrumb, delete confirmation, i18n for action buttons
- `src/pages/CrontabTasks.tsx` - Add breadcrumb, delete confirmation
- `src/pages/MiseTasks.tsx` - Add breadcrumb, delete confirmation
- `src/pages/NginxProxies.tsx` - Add breadcrumb, delete confirmation
- `src/pages/CloudflareTunnels.tsx` - Add breadcrumb, delete confirmation
- `src/pages/TTYSessions.tsx` - Add breadcrumb, delete confirmation, i18n for "Open"
- `src/pages/ConfigManagement.tsx` - Add breadcrumb, delete confirmation for dir removal
- `src/i18n/index.tsx` - Add new translation keys

### Implementation Pattern

Each detail view will change from:

```tsx
<div className="flex items-center gap-3">
  <Button variant="ghost" size="icon" onClick={onBack}>
    <ArrowLeft className="h-4 w-4" />
  </Button>
  <div>
    <h1>...</h1>
  </div>
</div>
```

To:

```tsx
<PageBreadcrumb items={[
  { label: t("nav.services"), onClick: () => navigate("/") },
  { label: t("nav.systemd"), onClick: onBack },
  { label: service.name },
]} />
<div className="flex items-center justify-between">
  <h1>...</h1>
  <div>...actions...</div>
</div>
```

Delete buttons will be wrapped with `AlertDialog` for confirmation before executing the mutation.

