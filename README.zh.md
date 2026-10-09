# AeroBrowser — 面向 AI 智能体的个人浏览器

[![CI](https://github.com/DevAnimecx/aerobrowser/actions/workflows/ci.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/actions/workflows/ci.yml)
[![Release](https://github.com/DevAnimecx/aerobrowser/actions/workflows/release.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/releases)
[![License: BSL-1.1](https://img.shields.io/badge/License-BSL--1.1-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-Stdio%20%7C%20HTTP%20%7C%20SSE-green)](https://modelcontextprotocol.io)

**62 个 MCP 工具。真正受沙箱隔离的浏览器。一条命令验证安装。**

AeroBrowser 通过 [Model Context Protocol](https://modelcontextprotocol.io) 把 AI 智能体接入真实浏览器。内置受隔离的托管引擎、批量工具调用、增量快照、持久会话，以及一步式助手配置。

最初由 [DevAnimecx/aerobrowser](https://github.com/DevAnimecx/aerobrowser) 创建 · 由 Blackvault Technology 维护 · 作者: Adarsh Kushwah

---

## 语言

- [English](README.md) · [中文](README.zh.md) · [Español](README.es.md)

---

## 安装

预编译二进制文件以 GitHub Releases 发布，并附带 `SHA256SUMS` 校验文件。

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.sh | sh
```

```powershell
# Windows PowerShell（5.1 或更高版本）
irm https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.ps1 | iex; Install-AeroBrowser -Agent opencode
```

两个安装脚本都会通过 GitHub API 解析最新版本、校验 SHA-256、在写入 `PATH` 之前确认下载的是原生可执行文件，并在二进制无法运行时将其删除。

更愿意从源码构建？

```sh
git clone https://github.com/DevAnimecx/aerobrowser.git
cd aerobrowser && cargo build --release --locked
```

---

## 验证安装

```sh
aero doctor
```

`aero doctor` 会检查引擎、沙箱支持、浏览器与守护进程端口、认证令牌，以及所有已知助手的配置路径。每一项失败的检查都会给出具体的修复方法。加 `--json` 可获得机器可读输出。

---

## 接入你的助手

```sh
aero connect opencode          # OpenCode
aero connect claude-code       # Claude Code
aero connect cursor            # Cursor
aero connect codex             # Codex
aero connect claude-desktop    # Claude Desktop
```

适配器覆盖 OpenCode、Claude Code、Claude Desktop、Cursor、VS Code、Codex、Windsurf、Cline、Roo Code、Hermes，以及通用的 `aero-mcp.json`。**所有适配器的配置生成均已实现并通过测试；但在验证环境中没有任何客户端真正对实时服务器运行过。**「写出了配置文件」与「已完成端到端验证」是两回事。

---

## AeroBrowser 的实际能力

### 更少的往返

`batch` 在单条 MCP 消息中、基于同一个浏览器按顺序执行多个工具，并逐条返回结果：

```json
{"calls": [
  {"name": "navigate", "arguments": {"url": "https://example.com/login"}},
  {"name": "snapshot", "arguments": {"interactive_only": true}},
  {"name": "fill", "arguments": {"ref": "@e1_4", "value": "user@example.com"}},
  {"name": "click", "arguments": {"ref": "@e1_5"}}
]}
```

设置 `stop_on_error: false` 可在失败后继续执行。

### 只返回变化的部分

`snapshot { "delta": true }` 只返回相较上一次快照新增或变化的元素，以及已消失引用的 tombstone 列表，而不是整棵可访问性树：

```json
{"text": "+ button 登录 [ref=e4_2]\n~ changed textbox 邮箱 [ref=e1_4]\n- removed [ref=e2_7]",
 "elements": [...], "changed": [...], "removed": ["e2_7"],
 "delta": {"baseline": false, "added": 1, "changed": 1, "removed": 1}}
```

首次调用、文档变更或 URL 变更都会返回完整快照，并带有 `"baseline": false` 与原因说明——恰恰在智能体上下文最少的时候，残缺的回答最没有价值。

### 持久会话

`save_state` / `restore_state` 导出 cookies、标签页、存储与滚动位置，并在浏览器启动前将其写入新的上下文。这**不是**进程快照，状态也未在静态存储时加密。用途是免去重复登录，而不是保护机密。

### 失败即关闭的沙箱

在 Linux x86_64 上，托管的 Moli 引擎运行在独立工作进程中，浏览器启动前会应用 Landlock 文件系统策略与 seccomp-BPF 系统调用过滤。cgroup v2 可施加严格的内存与进程数上限，OOM 只会终止工作进程，不影响宿主机。

当某项策略未实现时，AeroBrowser 会拒绝运行，而不会降级。`base`、`web-dev`、`test` 三个 profile 已声明但**未实现**，会直接报错。内核级网络出口边界并不存在：`cdp_raw` 的方法白名单限定在当前目标，不是防火墙。macOS 的 Seatbelt 路径未经验证。Windows 没有 AppContainer 或 Job Object，请使用外部浏览器，并将隔离视为浏览器自身的责任。

完整信任边界见 [SECURITY.md](SECURITY.md)，未覆盖的部分同样列在其中；明确缺失的功能见 [docs/ROADMAP.md](docs/ROADMAP.md)。

---

## 工具（62 个）

| 类别 | 工具 |
|---|---|
| 导航 | `navigate` `snapshot` `reload` `go_back` `go_forward` `wait_for` `scroll` |
| 交互 | `click` `double_click` `right_click` `fill` `type` `press_key` `hover` `drag` `upload_file` |
| 元素 | `get_text` `get_attribute` `get_bounding_box` `get_computed_style` `get_dom` `get_title` `get_url` `evaluate` |
| 表单 | `select_option` `check` `uncheck` `set_viewport` |
| 媒体 | `screenshot` `annotate_screenshot` `save_pdf` |
| 标签页 | `list_tabs` `new_tab` `switch_tab` `close_tab` `close` |
| 状态 | `save_state` `restore_state` `get_cookies` `set_cookie` `clear_cookies` |
| 测试 | `test_run` `test_assert_text` `test_assert_attr` `test_assert_url` `test_assert_visible` `test_report` |
| 网络 | `intercept_request` `get_network_log` `save_har` `get_console_logs` |
| 性能 | `get_performance` `get_memory_usage` `set_user_agent` `set_geolocation` |
| 诊断 | `server_status` `server_logs` `server_profile_list` `server_profile_set` `server_shutdown` |
| 协议 | `batch` `cdp_raw` |

`aero tools` 可打印机器可读的工具清单。

---

## 引擎

| 引擎 | 平台 | 隔离 | 适用场景 |
|---|---|---|---|
| **Moli**（托管） | Linux x86_64 | Landlock + seccomp + cgroup v2 | 不可信页面、生产环境 |
| **Chrome / Edge / Chromium**（外部） | 全部 | 浏览器自身 | 开发、可信页面 |
| **Lightpanda** | 全部 | 未验证 | 实验性；需 `--engine-path` |

---

## 测试配方

```
open https://example.com/login
wait_for #email
fill #email "user@example.com"
click #submit
assert_text .welcome "Welcome back"
assert_url "https://example.com/dashboard"
```

```sh
aero test examples/login.aero --report results.html
```

`results.html` 是自包含的报告，支持浅色/深色主题、相对最慢步骤的耗时条、可展开的错误详情，且不记录参数——可以安全附到评审中。

---

## 文档

| 文档 | 内容 |
|---|---|
| [快速开始](docs/QUICKSTART.md) | 首次可用配置 |
| [安装](docs/INSTALL.md) | 所有平台与助手 |
| [API 参考](docs/API.md) | 62 个工具的 schema 与 DSL 语法 |
| [助手技能](docs/AGENT-SKILL.md) | 智能体应如何使用 AeroBrowser |
| [架构](docs/ARCHITECTURE.md) | 系统设计 |
| [安全](SECURITY.md) | 信任边界与已知限制 |
| [部署](docs/DEPLOYMENT.md) | 守护进程、cgroup、传输方式 |
| [路线图](docs/ROADMAP.md) | 已实现与待完成，诚实标注 |
| [0.5 发布说明](docs/RELEASE-0.5.md) | 本次变更 |

---

## 参与贡献

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked --all-targets
python3 scripts/check-encoding.py     # 拒绝乱码
python3 scripts/check-docs.py         # 校验链接有效
```

详见 [CONTRIBUTING.md](CONTRIBUTING.md)。

---

## 许可证

Business Source License 1.1，附附加使用许可。个人、研究、教育及年收入低于
1000 万美元的企业内部使用免费。超过该门槛的生产使用，以及将浏览器自动化作为
服务对外提供，需要商业授权。

完整条款见 [LICENSE](LICENSE)。