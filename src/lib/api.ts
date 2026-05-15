import { invoke } from "@tauri-apps/api/core";

export interface ProxyConfig {
  host: string;
  port: number;
  log_capacity: number;
  https: boolean;
  cert_path: string;
  key_path: string;
  heartbeat_interval: number;
  autostart: boolean;
  language: string;
  theme: string;
}

export interface Provider {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  models: string[];
}

export interface ModelMapping {
  from: string;
  to_provider: string;
  to_model: string;
}

export interface Config {
  proxy: ProxyConfig;
  providers: Provider[];
  model_mappings: ModelMapping[];
}

export interface RequestLog {
  id: string;
  timestamp: string;
  method: string;
  path: string;
  model_in: string;
  model_out: string;
  provider: string;
  status: number;
  latency_ms: number;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  request_body: any;
  response_body: any;
}

export interface ProxyStatus {
  running: boolean;
  host: string;
  port: number;
  https: boolean;
}

export interface ProxyInfo {
  host: string;
  port: number;
  https: boolean;
}

export interface TestResult {
  success: boolean;
  message: string;
  latency_ms: number;
}

export interface ModelTestResult {
  model: string;
  success: boolean;
  latency_ms: number;
  message: string;
}

export interface CertInfo {
  cert_path: string;
  key_path: string;
  generated: boolean;
}

export const PRESET_PROVIDERS: Record<string, { name: string; base_url: string; models: string[] }> = {
  deepseek: { name: "DeepSeek", base_url: "https://api.deepseek.com", models: ["deepseek-chat", "deepseek-reasoner", "deepseek-v4-pro", "deepseek-v4-flash"] },
  openai: { name: "OpenAI", base_url: "https://api.openai.com", models: ["gpt-4o", "gpt-4o-mini", "o3", "o4-mini"] },
  claude: { name: "Claude (Anthropic)", base_url: "https://api.anthropic.com", models: ["claude-sonnet-4-20250514", "claude-opus-4-5-20250414", "claude-haiku-3-5-20241022"] },
  glm: { name: "GLM (智谱)", base_url: "https://open.bigmodel.cn/api/paas", models: ["glm-4-plus", "glm-4-flash", "glm-4"] },
  minimax: { name: "MiniMax", base_url: "https://api.minimax.chat", models: ["abab6.5s-chat", "abab5.5-chat"] },
  qwen: { name: "Qwen (通义千问)", base_url: "https://dashscope.aliyuncs.com/compatible-mode", models: ["qwen-plus", "qwen-max", "qwen-turbo"] },
  custom: { name: "自定义", base_url: "", models: [] },
};

export const PROXY_MODELS = [
  "claude-sonnet-4-20250514",
  "claude-opus-4-5-20250414",
  "claude-haiku-3-5-20241022",
  "gpt-4o",
  "gpt-4o-mini",
  "o3",
  "o4-mini",
];

export async function getConfig(): Promise<Config> { return invoke("get_config"); }
export async function saveConfig(config: Config): Promise<void> { return invoke("save_config", { config }); }
export async function getLogs(limit?: number): Promise<RequestLog[]> { return invoke("get_logs", { limit: limit ?? 200 }); }
export async function clearLogs(): Promise<void> { return invoke("clear_logs"); }
export async function getProxyStatus(): Promise<ProxyStatus> { return invoke("get_proxy_status"); }
export async function startProxy(): Promise<ProxyInfo> { return invoke("start_proxy"); }
export async function stopProxy(): Promise<void> { return invoke("stop_proxy"); }
export async function addOpLog(action: string, detail: string): Promise<void> { return invoke("add_op_log", { action, detail }); }
export async function testProviderModels(providerId: string): Promise<ModelTestResult[]> { return invoke("test_provider_models", { providerId }); }
export async function testMapping(mappingIdx: number): Promise<TestResult> { return invoke("test_mapping", { mappingIdx }); }
export async function fetchProviderModels(providerId: string, apiKey: string): Promise<string[]> { return invoke("fetch_provider_models", { providerId, apiKey }); }
export async function generateSelfSignedCert(): Promise<CertInfo> { return invoke("generate_self_signed_cert"); }
export async function importCcSwitchConfig(jsonStr: string): Promise<Config> { return invoke("import_cc_switch_config", { jsonStr }); }
export async function heartbeatCheck(): Promise<HeartbeatResult[]> { return invoke("heartbeat_check"); }

export interface HeartbeatResult {
  mapping_from: string;
  ok: boolean;
}
