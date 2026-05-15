# CC Proxy

AI 模型代理路由工具 — 将 Claude Code / Codex CLI 的请求转发到 DeepSeek、Qwen、GLM 等 OpenAI 兼容的模型提供商。

## 功能特性

- **模型映射** — 将 `claude-sonnet-4-20250514` 等模型名映射到实际提供商的模型
- **多提供商** — 支持 DeepSeek、OpenAI、Qwen、GLM、MiniMax 等，可自定义
- **协议转换** — 自动转换 Responses API ↔ Chat Completions API
- **流式转发** — 完整支持 SSE 流式响应，解决 Codex/DeepSeek 兼容性问题
- **模拟请求** — 内置类 Postman 功能，支持代码生成（curl/python/go/rust）
- **心跳检测** — 定时检测提供商可用性，异常时状态变黄
- **日志系统** — 实时日志流，支持导出，记录所有操作和请求
- **系统托盘** — 关闭窗口最小化到托盘，支持开机自启
- **CLI 模式** — 支持无 GUI 的 headless 模式运行
- **暗黑/明亮主题** — 可切换，设置持久化

## 快速开始

### GUI 模式

双击 `cc-proxy.exe` 或安装 `CC Proxy_2.0.0_x64-setup.exe`。

### CLI 模式

```bash
cc-proxy --headless --port 9528 --config config.toml
```

### 参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--config, -c` | 配置文件路径 | `config.toml` |
| `--headless` | 无 GUI 模式 | false |
| `--host` | 监听地址 | `127.0.0.1` |
| `--port, -p` | 监听端口 | `9528` |
| `--log-capacity` | 日志容量 | `10000` |

## 配置示例

```toml
[proxy]
host = "127.0.0.1"
port = 9528
log_capacity = 10000
heartbeat_interval = 30
autostart = true
language = "zh"
theme = "light"

[[providers]]
id = "deepseek"
name = "DeepSeek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-key"
models = ["deepseek-chat", "deepseek-v4-pro", "deepseek-v4-flash"]

[[model_mappings]]
from = "claude-sonnet-4-20250514"
to_provider = "deepseek"
to_model = "deepseek-v4-pro"

[[model_mappings]]
from = "gpt-4o"
to_provider = "deepseek"
to_model = "deepseek-v4-pro"
```

## 使用方式

### Claude Code

```bash
export ANTHROPIC_BASE_URL=http://127.0.0.1:9528
claude
```

### Codex CLI

```bash
export OPENAI_BASE_URL=http://127.0.0.1:9528/v1
codex
```

### API 端点

| 端点 | 说明 |
|------|------|
| `GET /v1/models` | 获取可用模型列表 |
| `POST /v1/chat/completions` | Chat Completions API |
| `POST /v1/responses` | Responses API (自动转换) |

## 技术栈

- **后端**: Rust + Tauri 2 + Axum + Tokio
- **前端**: Svelte 5 + Tailwind CSS 4
- **HTTP**: reqwest (连接池复用, 高并发)
- **构建**: Vite + SvelteKit

## 开发

```bash
# 安装依赖
pnpm install

# 开发模式
pnpm tauri dev

# 构建发布
pnpm tauri build
```

## 许可证

MIT
