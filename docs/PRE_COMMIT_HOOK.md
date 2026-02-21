# Pre-commit Hook 配置说明

## 简介

本项目已配置 Git pre-commit hook，在每次 `git commit` 前自动运行代码质量检查，确保提交的代码符合项目规范。

## 检查项目

Pre-commit hook 会按顺序执行以下检查：

### 1. 代码格式检查 (cargo fmt)
- **检查内容**: 验证代码格式是否符合 Rust 标准
- **失败时**: 提示运行 `cargo fmt --all` 修复
- **对应 CI**: `.github/workflows/ci.yml` 第 52 行

### 2. 代码质量检查 (cargo clippy)
- **检查内容**: 静态分析代码，检测潜在问题和不规范写法
- **允许的警告**: `dead_code`, `unused_imports`, `single_component_path_imports`
- **失败时**: 修复 clippy 提示的警告
- **对应 CI**: `.github/workflows/ci.yml` 第 55 行

### 3. 单元测试 (cargo test --lib)
- **检查内容**: 运行所有库单元测试
- **失败时**: 修复失败的测试
- **对应 CI**: `.github/workflows/ci.yml` 第 61 行

## 使用方法

### 正常提交流程

```bash
# 1. 修改代码
vim src/atoms/example.rs

# 2. 添加到暂存区
git add src/atoms/example.rs

# 3. 提交（会自动触发 pre-commit hook）
git commit -m "feat: add example feature"

# Pre-commit hook 会自动运行以下检查：
# 🔍 Running pre-commit checks...
# 📝 Checking code formatting...
# ✅ Code formatting check passed
# 🔧 Running clippy...
# ✅ Clippy check passed
# 🧪 Running tests...
# ✅ All tests passed
# ✨ All pre-commit checks passed! Proceeding with commit...
```

### 如果检查失败

#### 格式检查失败
```bash
# 自动修复格式问题
cargo fmt --all

# 重新提交
git add -u
git commit -m "..."
```

#### Clippy 检查失败
```bash
# 查看详细警告
cargo clippy --all-targets --all-features

# 修复代码问题
vim src/...

# 重新提交
git add -u
git commit -m "..."
```

#### 测试失败
```bash
# 运行测试查看详情
cargo test --lib

# 修复测试或代码
vim src/...
vim tests/...

# 重新提交
git add -u
git commit -m "..."
```

### 跳过 Pre-commit Hook（不推荐）

在紧急情况下，可以使用 `--no-verify` 跳过检查：

```bash
git commit --no-verify -m "emergency fix"
```

⚠️ **警告**: 跳过 pre-commit 会导致 CI 失败，请谨慎使用！

## 手动运行检查

在提交前，你也可以手动运行这些检查：

```bash
# 完整的 pre-commit 检查
.git/hooks/pre-commit

# 或者分步运行
cargo fmt --all -- --check    # 格式检查
cargo clippy -- -D warnings   # 代码质量
cargo test --lib              # 单元测试
```

## Hook 文件位置

- **文件路径**: `.git/hooks/pre-commit`
- **权限**: 可执行 (`chmod +x`)
- **类型**: Bash 脚本

## 与 CI 的关系

Pre-commit hook 的检查项与 GitHub Actions CI 完全一致：

| 检查项 | Pre-commit Hook | GitHub Actions CI |
|-------|-----------------|-------------------|
| 代码格式 | `cargo fmt --check` | ✅ 第 52 行 |
| Clippy | `cargo clippy` | ✅ 第 55 行 |
| 单元测试 | `cargo test --lib` | ✅ 第 61 行 |
| 构建 | ❌ 未包含（太慢） | ✅ 第 58 行 |
| 文档测试 | ❌ 未包含（可选） | ✅ 第 64 行 |
| 覆盖率 | ❌ 未包含（可选） | ✅ 第 66-87 行 |

## 故障排除

### Hook 未执行

```bash
# 检查文件是否存在
ls -la .git/hooks/pre-commit

# 检查是否可执行
chmod +x .git/hooks/pre-commit
```

### Hook 执行太慢

Pre-commit hook 可能需要 30-60 秒运行完成（取决于代码量）。如果觉得太慢，可以：

1. **临时跳过**: `git commit --no-verify`
2. **优化 hook**: 移除 `cargo test` 检查，仅保留 fmt 和 clippy
3. **使用 CI**: 完全依赖 GitHub Actions

### 误报问题

如果 hook 检查失败但代码实际正确：

1. 清理缓存: `cargo clean`
2. 更新依赖: `cargo update`
3. 重新运行: `.git/hooks/pre-commit`

## 维护

Pre-commit hook 需要定期维护，确保与 CI 配置保持同步：

- 当 `.github/workflows/ci.yml` 更新时，同步更新 `.git/hooks/pre-commit`
- 测试新增的检查项是否正常工作
- 考虑添加更多检查（如 `cargo doc`, `cargo audit`）

## 团队协作

⚠️ **注意**: `.git/hooks/` 目录不会被 Git 追踪，每个开发者需要手动配置 hook。

推荐方式：

1. 将 hook 脚本放在 `scripts/pre-commit.sh`（已追踪）
2. 开发者手动链接: `ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit`
3. 或在 `README.md` 中说明配置步骤

---

**当前状态**: ✅ Pre-commit hook 已配置并测试通过  
**CI 状态**: ✅ 最新 CI run (22252728208) 成功通过  
**测试覆盖**: 59 个单元测试全部通过
