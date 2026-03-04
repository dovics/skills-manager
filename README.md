# Skills Manager

> 一个统一的 AI 助手技能管理工具

Skills Manager 是一个命令行工具，用于统一管理多个 AI 助手的技能（skills）。它通过符号链接的方式，将分散在不同位置的技能集中管理，让维护和更新变得更加简单。

## 支持的 AI 工具

- [Claude Code](https://github.com/anthropics/claude-code)
- [OpenFang](https://github.com/yourusername/openfang)
- [OpenClaw](https://github.com/yourusername/openclaw)
- [ZeroClaw](https://github.com/yourusername/zeroclaw)

## 特性

- 🔍 **自动发现**: 自动扫描系统中的技能目录
- 📦 **统一管理**: 将所有技能集中到一个工作空间
- 🔗 **符号链接**: 在原位置创建符号链接，透明访问
- ⚙️ **灵活配置**: 支持多种工具和格式
- 🔐 **安全备份**: 自动备份原文件
- ✅ **完整性验证**: 验证技能和链接状态
- 🖥️ **TUI 界面**: 交互式终端界面，可视化选择和管理技能

## 安装

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/dovics/skills.git
cd skills

# 构建
cargo build --release

# 安装到系统
sudo cp target/release/skills /usr/local/bin/
```

### 使用 Cargo 安装

```bash
cargo install --path .
```

## 快速开始

### 1. 初始化

```bash
skills init
```

这将创建默认配置文件和工作空间：

- 配置文件: `~/.config/skills/config.yaml`
- 工作空间: `~/.skills/workspace/`

### 2. 添加工具

编辑配置文件添加你的工具：

```bash
skills config
vim ~/.config/skills/config.yaml
```

配置示例：

```yaml
workspace: ~/.skills/workspace

tools:
  claude-code:
    name: claude-code
    path: ~/.claude/skills
    enabled: true
    priority: 10
```

### 3. 扫描现有技能

```bash
# 递归扫描配置的路径
skills scan --recursive

# 扫描特定路径
skills scan --path ~/.openclaw/skills
```

### 4. 同步技能到工作空间

```bash
# 同步所有技能
skills sync

# 同步特定工具的技能
skills sync --tool openclaw

# 预览将要执行的操作
skills sync --dry-run
```

### 5. 使用 TUI 界面（推荐）

TUI（交互式终端界面）是最简单的使用方式：

```bash
skills tui
```

启动 TUI 后：
1. **左侧面板**显示可用的工具（如 claude-code、openclaw 等）
2. **右侧面板**显示当前工具的技能列表
3. 使用方向键或 `hjkl` 导航
4. 按 `Space` 切换技能的启用/禁用状态
5. 按 `Enter` 应用更改（将选中的技能移动到工作空间并创建符号链接）
6. 按 `q` 退出

**键盘快捷键：**
- `↑/k`: 向上移动
- `↓/j`: 向下移动
- `←/h`: 上一个工具
- `→/l`: 下一个工具
- `Tab`: 下一个工具
- `Space`: 切换技能状态
- `Enter`: 应用更改
- `?`: 显示帮助
- `q`: 退出

## 命令参考

### `skills init [-f]`

初始化技能管理器。

- `-f, --force`: 强制重新初始化（覆盖现有配置）

### `skills scan [-p PATH] [-r]`

扫描目录中的技能。

- `-p, --path PATH`: 指定扫描路径
- `-r, --recursive`: 递归扫描

### `skills add <name> <path> [-t TOOL]`

添加技能到管理列表。

- `name`: 技能名称
- `path`: 技能路径
- `-t, --tool TOOL`: 所属工具

### `skills remove <name>`

从管理列表中移除技能。

### `skills list [-d]`

列出所有管理的技能。

- `-d, --detailed`: 显示详细信息

### `skills sync [-t TOOL] [--dry-run]`

同步技能到工作空间并创建符号链接。

- `-t, --tool TOOL`: 仅同步指定工具
- `--dry-run`: 预览操作

### `skills link [--dry-run]`

为所有技能创建符号链接。

- `--dry-run`: 预览操作

### `skills unlink <name> [--dry-run]`

取消技能链接，恢复到原位置。

- `--dry-run`: 预览操作

### `skills verify`

验证所有技能和链接状态。

### `skills config [-p]`

显示或编辑配置。

- `-p, --show-path`: 仅显示配置文件路径

### `skills tui`

启动交互式 TUI 界面进行可视化的技能管理。

## 工作原理

```
原始位置                      工作空间                    符号链接
──────────────────────────────────────────────────────────────────
~/.openclaw/skills/github/
                                    ↓ 移动
~/.skills/workspace/github/  ◄─────┘
                                    ↓ 创建链接
~/.openclaw/skills/github/ → ~/.skills/workspace/github/
```

1. **扫描**: 发现各工具的技能目录
2. **移动**: 将技能移动到统一工作空间
3. **链接**: 在原位置创建符号链接
4. **透明访问**: 工具仍然可以在原位置访问技能

## 配置文件

配置文件位于 `~/.config/skills/config.yaml`：

```yaml
# 工作空间目录
workspace: ~/.skills/workspace

# AI 工具配置
tools:
  claude-code:
    name: claude-code
    path: ~/.claude/skills
    enabled: true
    priority: 10

  openclaw:
    name: openclaw
    path: ~/.openclaw/skills
    enabled: true
    priority: 5

# 管理的技能
skills:
  github:
    name: github
    path: ~/.openclaw/skills/github
    tool: openclaw
```

## 开发

### 构建

```bash
cargo build
```

### 运行

```bash
cargo run -- --help
```

### 测试

```bash
cargo test
```

### 添加新功能

1. 在 `src/` 目录中创建或修改模块
2. 在 `src/main.rs` 中添加新的 CLI 命令
3. 更新此 README 文档

## 常见问题

### Q: 如何备份现有技能？

A: 在同步前，工具会自动备份现有文件到带 `.backup` 后缀的文件。

### Q: 如何恢复被替换的文件？

A: 查找备份文件（`.backup` 后缀）并手动恢复。

### Q: 符号链接跨文件系统有效吗？

A: 是的，符号链接可以在不同文件系统间工作。

### Q: 如何取消所有链接？

A: 使用 `skills unlink <name>` 对每个技能，或删除工作空间并手动恢复。

## 贡献

欢迎贡献！请提交 Pull Request 或创建 Issue。

## 许可证

MIT License

## 作者

dovics

---

**让 AI 技能管理更简单！** 🚀
