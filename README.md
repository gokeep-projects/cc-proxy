# CC Proxy

API 代理转发工具，将 Claude Code / Codex CLI 的请求转发到任意 OpenAI 兼容 API 提供商（DeepSeek、Qwen 等）。

## 功能

- **模型名映射**: 将 `claude-sonnet-4-20250514` 等模型名自动映射到 `deepseek-chat`、`qwen-plus` 等
- **Responses API 兼容**: 自动将 Codex CLI 的 `/v1/responses` 请求转换为 `/v1/chat/completions` 格式
- **流式转发**: 支持 SSE 流式响应的实时转换
- **GUI 界面**: 美观的管理界面，实时查看请求日志、配置提供商和映射规则
- **命令行模式**: 支持 `--headless` 无头模式运行
- **跨平台**: 支持 Windows 和 Linux

## 快速开始

### 1. 配置

编辑 `config.toml`:

```toml
[proxy]
host = "127.0.0.1"
port = 8080

[[providers]]
id = "deepseek"
name = "DeepSeek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-key"
default_model = "deepseek-chat"

[[model_mappings]]
from = "claude-sonnet-4-20250514"
to_provider = "deepseek"
to_model = "deepseek-chat"
```

### 2. 运行

**GUI 模式:**
```bash
./cc-proxy
```

**命令行模式:**
```bash
./cc-proxy --headless --config ./config.toml --port 8080
```

### 3. 使用

将客户端的 API base URL 指向代理:

```bash
# Claude Code
export ANTHROPIC_BASE_URL=http://127.0.0.1:8080

# Codex CLI
export OPENAI_BASE_URL=http://127.0.0.1:8080/v1
```

## 开发

```bash
# 安装依赖
pnpm install

# 开发模式
pnpm tauri dev

# 构建
pnpm tauri build
```

## 架构

```
Client (Claude Code / Codex CLI)
  │ POST /v1/responses 或 /v1/chat/completions
  ▼
cc-proxy (axum)
  │ 解析请求 → 模型映射 → 格式转换
  ▼
Provider (DeepSeek / Qwen / ...)
  │ 响应
  ▼
cc-proxy: 格式转回 → 返回客户端
```
