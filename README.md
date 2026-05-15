<div align="center">

<img src="icons/128x128.png" alt="CC Proxy" width="120" />

# CC Proxy

### AI 模型代理路由引擎

一键将 Claude Code / Codex CLI / Cursor 等 AI 编码工具
无缝接入 DeepSeek · Qwen · GLM · OpenAI 等任意 OpenAI 兼容模型

[![Version](https://img.shields.io/badge/version-2.0.0-646cff?style=flat-square)](https://github.com)
[![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80+-orange?style=flat-square)](https://www.rust-lang.org)
[![Svelte](https://img.shields.io/badge/svelte-5-ff3e00?style=flat-square)](https://svelte.dev)
[![Tauri](https://img.shields.io/badge/tauri-2-67d6b5?style=flat-square)](https://tauri.app)

</div>

---

## 快速开始

### 安装

```bash
# 方式一：安装包（推荐）
双击 CC Proxy_2.0.0_x64-setup.exe 安装

# 方式二：便携版
直接运行 cc-proxy.exe
```

### 连接 AI 工具

启动 CC Proxy 后，将 AI 工具的 API 地址指向本地代理：

```bash
# Claude Code
export ANTHROPIC_BASE_URL=http://127.0.0.1:9528
claude

# Codex CLI
export OPENAI_BASE_URL=http://127.0.0.1:9528/v1
codex

# Cursor / VS Code
# 在设置中将 API Base URL 设置为 http://127.0.0.1:9528/v1
```

就这么简单 —— 无需修改任何代码，所有请求由 CC Proxy 自动路由到对应的模型提供商。

### CLI 模式

无需 GUI，轻量运行：

```bash
cc-proxy --headless --port 9528 --config config.toml
```

---

## 命令行参数

| 参数 | 简写 | 说明 | 默认值 |
|------|------|------|--------|
| `--config` | `-c` | 配置文件路径 | `config.toml` |
| `--headless` | — | 无 GUI 模式（无窗口、无托盘） | `false` |
| `--host` | — | 监听地址 | `127.0.0.1` |
| `--port` | `-p` | 监听端口 | `9528` |
| `--log-capacity` | — | 内存日志最大条数 | `10000` |

---

## 配置

CC Proxy 使用 TOML 配置文件，位于可执行文件同目录下 `config.toml`。首次启动自动生成。

```toml
[proxy]
host = "127.0.0.1"          # 监听地址
port = 9528                  # 监听端口
log_capacity = 10000         # 日志容量
heartbeat_interval = 180     # 心跳检测间隔（秒）
autostart = true             # 开机自启
language = "zh"              # 界面语言（zh / en）
theme = "light"              # 主题（light / dark）
https = false                # 是否启用 HTTPS

# ── 模型提供商 ──
[[providers]]
id = "deepseek"
name = "DeepSeek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-api-key"
models = ["deepseek-chat", "deepseek-v4-pro", "deepseek-v4-flash"]

[[providers]]
id = "qwen"
name = "Qwen"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-your-api-key"
models = ["qwen-plus", "qwen-max"]

# ── 模型映射（代理模型 → 提供商模型）──
[[model_mappings]]
from = "claude-sonnet-4-20250514"
to_provider = "deepseek"
to_model = "deepseek-v4-pro"

[[model_mappings]]
from = "gpt-4o"
to_provider = "qwen"
to_model = "qwen-max"

[[model_mappings]]
from = "claude-opus-4-5-20250414"
to_provider = "deepseek"
to_model = "deepseek-v4-pro"
```

### 映射规则

- **精确匹配** — 请求中的 `model` 字段与 `from` 完全匹配时，路由到对应提供商的 `to_model`
- **兜底策略** — 无匹配时，使用第一个 provider 的第一个模型

---

## API 端点

启动代理后，提供以下 HTTP 端点：

| 方法 | 端点 | 说明 |
|------|------|------|
| `GET` | `/v1/models` | 获取可用代理模型列表 |
| `POST` | `/v1/chat/completions` | OpenAI Chat Completions API |
| `POST` | `/v1/responses` | OpenAI Responses API（自动转换为 Chat API） |
| `POST` | `/v1/messages` | Anthropic Messages API（自动检测 `/anthropic` 后缀的 Base URL） |

### 协议转换

CC Proxy 在底层自动完成以下格式转换：

```
┌──────────────────┐          ┌──────────────────┐
│  Responses API   │   ←──→   │ Chat Completions │
│  (Claude Code)   │          │  (OpenAI 兼容)    │
└──────────────────┘          └──────────────────┘
         │                            │
         │    Anthropic Messages       │
         └────────   ─────────────────┘
              (DeepSeek /anthropic)
```

- **Resolutions → Chat** — `instructions` → `system` message, `input` array → `messages` array, tools 转换
- **Chat → Responses** — message content → `output_text`, tool calls → `function_call`, usage token 重映射
- **Anthropic ↔ OpenAI** — `input_tokens` / `output_tokens` ↔ `prompt_tokens` / `completion_tokens`, content blocks 聚合
- **SSE 流式** — stream chunk 实时转换为 Responses SSE 事件的完整生命周期（`output_text.delta` → `output_text.done` → `output_item.done` → `response.completed`）

---

## 功能特性

<table>
<tr><td>

**模型映射**
将 Claude / GPT 等模型名透明映射到任意 provider 的实际模型，支持一个 provider 对应多个代理模型名。

**多提供商支持**
内置 DeepSeek、OpenAI、Anthropic、Qwen（通义千问）、GLM（智谱）、MiniMax 预设，一键切换。支持自定义任意 OpenAI 兼容提供商。

</td><td>

**协议自动转换**
Responses API ↔ Chat Completions API、Anthropic Messages ↔ OpenAI Chat 格式全自动转换，无需关心底层协议差异。

**SSE 流式转发**
完整 SSE 事件流代理，解决 Codex CLI 与 DeepSeek 等 provider 的流式兼容性问题。支持 stream_options 按需注入。

</td></tr>
<tr><td>

**内置 API 调试器**
类 Postman 的模拟请求面板 —— 选择模型、输入消息、一键发送，实时查看 JSON 响应。支持生成 cURL / Python / Go / Rust 代码片段，一键复制。

**实时日志流**
每条请求记录完整的模型映射链、状态码、延迟、Token 用量。支持导出 TXT，可选全部 / 最近 100 条 / 最近 50 条。

</td><td>

**心跳检测**
定时检测所有映射链路可用性，连续失败 3 次状态灯变为黄色告警，成功即恢复绿色脉冲。

**系统托盘 & 开机自启**
关闭窗口自动隐藏到系统托盘，后台静默运行。支持配置开机自启，重启电脑无需手动启动。

</td></tr>
<tr><td>

**暗黑 / 明亮主题**
两套主题，一键切换，设置自动持久化。

**CC-Switch 兼容**
支持一键导入 CC-Switch 的 JSON 配置，无缝迁移。

</td><td>

**HTTPS 支持**
内置自签名证书生成器，一键开启 HTTPS，支持 localhost / 127.0.0.1 / ::1。

**双模式运行**
GUI 桌面应用 + CLI headless 模式，满足本地使用和服务器部署两种场景。

</td></tr>
</table>

---

## 预设提供商

| 提供商 | Base URL | 预设模型 |
|--------|----------|----------|
| **DeepSeek** | `https://api.deepseek.com` | `deepseek-chat`, `deepseek-reasoner`, `deepseek-v4-pro`, `deepseek-v4-flash` |
| **OpenAI** | `https://api.openai.com` | `gpt-4o`, `gpt-4o-mini`, `o3`, `o4-mini` |
| **Anthropic** | `https://api.anthropic.com` | `claude-sonnet-4-20250514`, `claude-opus-4-5-20250414`, `claude-haiku-3-5-20241022` |
| **Qwen（通义千问）** | `https://dashscope.aliyuncs.com/compatible-mode` | `qwen-plus`, `qwen-max`, `qwen-turbo` |
| **GLM（智谱）** | `https://open.bigmodel.cn/api/paas` | `glm-4-plus`, `glm-4-flash`, `glm-4` |
| **MiniMax** | `https://api.minimax.chat` | `abab6.5s-chat`, `abab5.5-chat` |

> 支持手动添加任意 OpenAI 兼容提供商，支持从 API 自动拉取模型列表。

---

## 技术架构

```
┌──────────────────────────────────────────────────┐
│                    Desktop App                    │
│  ┌─────────────┐          ┌───────────────────┐  │
│  │   Svelte 5   │  ◄──►   │   Tauri 2 (Rust)   │  │
│  │  Tailwind 4  │  IPC    │   Axum HTTP Server │  │
│  └─────────────┘          └─────────┬─────────┘  │
│                                     │             │
│                    ┌────────────────┼──────┐      │
│                    │  Reverse Proxy │      │      │
│                    │  ──────────────│──    │      │
│                    │  • 模型映射    │      │      │
│                    │  • 协议转换    │      │      │
│                    │  • SSE 流      │      │      │
│                    │  • 心跳检测    │      │      │
│                    │  • 日志记录    │      │      │
│                    └──────────┬─────┘      │      │
└───────────────────────────────┼────────────┘      │
                                │                    │
                    ┌───────────▼──────────────────┐ │
                    │   Upstream Providers         │ │
                    │   DeepSeek · Qwen · OpenAI   │ │
                    │   GLM · MiniMax · Custom     │ │
                    └──────────────────────────────┘ │
```

| 层 | 技术 | 职责 |
|----|------|------|
| GUI 框架 | **Tauri 2** | 窗口管理、系统托盘、开机自启、IPC |
| 前端 | **Svelte 5** + **Tailwind CSS 4** | 响应式 UI，暗黑/明亮主题 |
| HTTP 服务 | **Axum 0.7** (Rust) | 高性能异步 HTTP 代理，路由分发 |
| HTTP 客户端 | **reqwest 0.12** | 连接池复用（32 hosts），高并发转发 |
| 流式处理 | **futures-util** + SSE | 零拷贝字节流，实时协议转换 |
| 配置 | **TOML** + **Serde** | 类型安全配置管理 |
| 日志 | 内存环形队列 (`VecDeque`) | 容量可控，按时间排序 |
| 构建 | **Vite** + **pnpm** | 前端构建，Tauri bundler 打包 |

---

## 开发

```bash
# 环境要求
# - Rust 1.80+
# - Node.js 20+
# - pnpm 9+

# 安装前端依赖
pnpm install

# 开发模式（热重载）
pnpm tauri dev

# 生产构建
pnpm tauri build

# 仅构建前端
pnpm build
```

---

## 项目结构

```
cc-proxy/
├── src/                        # Svelte 前端
│   ├── routes/
│   │   ├── +page.svelte        # 主界面
│   │   └── +layout.svelte      # 布局
│   └── lib/
│       └── api.ts              # Tauri IPC 接口层
├── src-tauri/                  # Rust 后端
│   ├── src/
│   │   ├── main.rs             # 入口 / CLI 解析
│   │   ├── lib.rs              # Tauri 应用初始化 / 状态管理
│   │   ├── config.rs           # 配置结构体与持久化
│   │   ├── commands.rs         # Tauri IPC 命令
│   │   ├── store.rs            # 日志存储（环形队列）
│   │   └── proxy/
│   │       ├── mod.rs          # Axum 路由注册
│   │       ├── handler.rs      # 代理核心逻辑
│   │       └── convert.rs      # 协议转换引擎
│   ├── Cargo.toml
│   └── tauri.conf.json         # Tauri 配置
├── config.toml                 # 用户配置文件
└── package.json
```

---

## 许可证

MIT — 自由使用、修改、分发。
