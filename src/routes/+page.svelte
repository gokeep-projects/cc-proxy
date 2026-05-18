<script lang="ts">
  import { onMount } from "svelte";
  import { tick } from "svelte";
  import {
    getConfig, saveConfig, getLogs, clearLogs, getProxyStatus,
    startProxy, stopProxy, addOpLog, testProviderModels, testMapping,
    fetchProviderModels, importCcSwitchConfig,
    PRESET_PROVIDERS, PROXY_MODELS,
    type Config, type Provider, type RequestLog,
    type ProxyStatus, type ModelTestResult
  } from "$lib/api";

  let dark = $state(typeof localStorage !== "undefined" ? localStorage.getItem("theme") === "dark" : false);
  let config = $state<Config>({ proxy: { host: "127.0.0.1", port: 8080, log_capacity: 1000, https: false, cert_path: "", key_path: "" }, providers: [], model_mappings: [] });
  let status = $state<ProxyStatus>({ running: false, host: "", port: 0, https: false });
  let logs = $state<RequestLog[]>([]);
  let autoScroll = $state(true);
  let logsOpen = $state(false);
  let leftPct = $state(68);

  let showProviderModal = $state(false);
  let showMappingModal = $state(false);
  let showConnModal = $state(false);
  let showImportModal = $state(false);
  let showSimModal = $state(false);
  let editingProvider = $state<Provider | null>(null);
  let editingMappingIdx = $state<number | null>(null);

  let pForm = $state({ preset: "", name: "", base_url: "", api_key: "", models: [] as string[], newModel: "", showKey: false });
  let pError = $state("");
  let fetchingModels = $state(false);
  let mForm = $state({ from: "", to_provider: "", to_model: "" });
  let importJson = $state("");
  let importError = $state("");

  let simModel = $state("");
  let simMessage = $state("Hello!");
  let simResponse = $state("");
  let simLoading = $state(false);

  let providerTestResults = $state<Record<string, ModelTestResult[]>>({});
  let testingProvider = $state<Record<string, boolean>>({});
  let mappingTestResult = $state<Record<number, {ok:boolean,ms:number,msg:string}>>({});
  let testingMapping = $state<Record<number, boolean>>({});
  let showApiKey = $state<Record<string, boolean>>({});

  let logContainer: HTMLElement;

  onMount(async () => {
    config = await getConfig();
    status = await getProxyStatus();
    logs = await getLogs();
    // Apply saved theme
    dark = config.proxy.theme === "dark";
    // Start heartbeat if proxy already running
    if (status.running) startHeartbeat();
    const iv = setInterval(async () => {
      status = await getProxyStatus();
      const nl = await getLogs();
      if (nl.length !== logs.length) {
        logs = nl;
        if (autoScroll) { await tick(); if (logContainer) logContainer.scrollTop = logContainer.scrollHeight; }
      }
    }, 1000);
    return () => { clearInterval(iv); stopHeartbeat(); };
  });

  $effect(() => { if (typeof localStorage !== "undefined") localStorage.setItem("theme", dark ? "dark" : "light"); });

  async function doSave() { await saveConfig(config); }

  async function handleStart() {
    const info = await startProxy();
    status = await getProxyStatus();
    logsOpen = true;
    startHeartbeat();
    await addOpLog("启动代理", "HTTP " + info.host + ":" + info.port + " 启动成功");
  }

  async function handleStop() {
    await stopProxy();
    status = await getProxyStatus();
    stopHeartbeat();
    await addOpLog("停止代理", "代理服务已停止");
  }

  function openAddProvider() {
    editingProvider = null;
    pForm = { preset: "", name: "", base_url: "", api_key: "", models: [], newModel: "", showKey: false };
    pError = ""; showProviderModal = true;
  }

  function openEditProvider(p: Provider) {
    editingProvider = p;
    pForm = { preset: "", name: p.name, base_url: p.base_url, api_key: p.api_key, models: [...p.models], newModel: "", showKey: false };
    pError = ""; showProviderModal = true;
  }

  function applyPreset(key: string) {
    if (!key || !PRESET_PROVIDERS[key]) return;
    const pr = PRESET_PROVIDERS[key];
    pForm.name = pr.name; pForm.base_url = pr.base_url; pForm.models = [...pr.models];
  }

  async function saveProvider() {
    if (!pForm.name.trim()) { pError = "名称不能为空"; return; }
    if (!editingProvider && config.providers.some(p => p.name === pForm.name.trim())) { pError = "名称已存在"; return; }
    if (editingProvider) {
      const idx = config.providers.findIndex(p => p.id === editingProvider!.id);
      if (idx >= 0) config.providers[idx] = { ...editingProvider, name: pForm.name, base_url: pForm.base_url, api_key: pForm.api_key, models: pForm.models };
    } else {
      config.providers = [...config.providers, { id: crypto.randomUUID(), name: pForm.name, base_url: pForm.base_url, api_key: pForm.api_key, models: pForm.models }];
    }
    await doSave(); showProviderModal = false;
  }

  async function deleteProvider(id: string) {
    if (!await customConfirm("确定删除此提供商？")) return;
    config.providers = config.providers.filter(p => p.id !== id);
    await doSave();
    await addOpLog('删除提供商', config.providers.find(p => p.id === id)?.name || id);
  }

  async function testProvider(id: string) {
    testingProvider = { ...testingProvider, [id]: true };
    const results = await testProviderModels(id);
    providerTestResults = { ...providerTestResults, [id]: results };
    testingProvider = { ...testingProvider, [id]: false };
  }

  function addModel() {
    const m = pForm.newModel.trim();
    if (m && !pForm.models.includes(m)) pForm.models = [...pForm.models, m];
    pForm.newModel = "";
  }

  function removeModel(m: string) { pForm.models = pForm.models.filter(x => x !== m); }

  async function fetchModels() {
    fetchingModels = true;
    try {
      pForm.models = await fetchProviderModels(editingProvider?.id || "__temp__", pForm.api_key);
    } catch(e) { pError = String(e); }
    fetchingModels = false;
  }

  function openAddMapping() { editingMappingIdx = null; mForm = { from: "", to_provider: "", to_model: "" }; showMappingModal = true; }
  function openEditMapping(idx: number) { editingMappingIdx = idx; mForm = { ...config.model_mappings[idx] }; showMappingModal = true; }

  async function saveMapping() {
    if (!mForm.from || !mForm.to_provider || !mForm.to_model) return;
    if (editingMappingIdx !== null) config.model_mappings[editingMappingIdx] = { ...mForm };
    else config.model_mappings = [...config.model_mappings, { ...mForm }];
    await doSave(); showMappingModal = false;
  }

  async function deleteMapping(idx: number) {
    if (!await customConfirm("确定删除此映射？")) return;
    const m = config.model_mappings[idx];
    config.model_mappings = config.model_mappings.filter((_, i) => i !== idx);
    await doSave(); await addOpLog("删除映射", m.from);
  }

  async function testMappingBtn(idx: number) {
    testingMapping = { ...testingMapping, [idx]: true };
    try {
      const r = await testMapping(idx);
      mappingTestResult = { ...mappingTestResult, [idx]: { ok: r.success, ms: r.latency_ms, msg: r.message } };
    } catch(e) { mappingTestResult = { ...mappingTestResult, [idx]: { ok: false, ms: 0, msg: String(e) } }; }
    testingMapping = { ...testingMapping, [idx]: false };
    setTimeout(() => { const c = { ...mappingTestResult }; delete c[idx]; mappingTestResult = c; }, 5000);
  }

  async function handleClearLogs() { await clearLogs(); logs = []; }

  async function handleImport() {
    importError = "";
    try { config = await importCcSwitchConfig(importJson); await doSave(); showImportModal = false; importJson = ""; }
    catch(e) { importError = String(e); }
  }

  let simApiType = $state<"chat" | "responses">("chat");

  async function sendSimRequest() {
    simLoading = true; simResponse = "";
    try {
      let url = "http://127.0.0.1:" + config.proxy.port;
      let body: any;

      if (simApiType === "responses") {
        url += "/v1/responses";
        body = { model: simModel, input: simMessage, stream: false };
      } else {
        url += "/v1/chat/completions";
        body = { model: simModel, messages: [{ role: "user", content: simMessage }], stream: false };
      }

      const res = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body)
      });
      simResponse = JSON.stringify(await res.json(), null, 2);
    } catch(e: any) {
      simResponse = "请求失败: " + e.message + "\n\n提示: 请确保代理已启动且端口正确";
    }
    simLoading = false;
  }

  function startDrag(e: MouseEvent) {
    const startX = e.clientX, startW = leftPct, total = window.innerWidth;
    function onMove(ev: MouseEvent) {
      leftPct = (Math.max(400, Math.min(total - 280, (startW / 100) * total + ev.clientX - startX)) / total) * 100;
    }
    function onUp() { window.removeEventListener("mousemove", onMove); window.removeEventListener("mouseup", onUp); }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  function fmtTime(ts: string) { return new Date(ts).toTimeString().slice(0, 8); }

  function fmtLog(log: RequestLog): { text: string; cls: string } {
    const t = fmtTime(log.timestamp);
    if (log.method === "OP") {
      const detail = (typeof log.response_body === "string" && log.response_body) ? log.response_body : (log.model_in || "");
      return { text: "[" + t + "] ✦ " + log.path + " — " + detail + " ✓", cls: "text-blue-400" };
    }
    const isStream = log.latency_ms === 0 && log.status === 200 && !log.prompt_tokens;
    if (isStream) return { text: "[" + t + "] ⇄ " + log.method + " " + log.path + " | " + log.model_in + " → " + log.model_out + " | " + log.provider + " | streaming", cls: "text-gray-500" };
    const tokens = (log.prompt_tokens != null || log.completion_tokens != null) ? " | " + (log.prompt_tokens ?? 0) + "/" + (log.completion_tokens ?? 0) + " tokens" : "";
    const ok = log.status >= 200 && log.status < 300;
    return { text: "[" + t + "] → " + log.method + " " + log.path + " | " + log.model_in + " → " + log.model_out + " | " + log.provider + " | " + log.status + " | " + log.latency_ms + "ms" + tokens, cls: ok ? "text-green-400" : "text-red-400" };
  }

  function getProviderById(id: string) { return config.providers.find(p => p.id === id); }

  function modelTestCls(providerId: string, model: string) {
    const r = providerTestResults[providerId]?.find(x => x.model === model);
    if (!r) return "bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300";
    return r.success ? "bg-green-800 text-green-200" : "bg-red-800 text-red-200";
  }

  function connBase() { return "http://127.0.0.1:" + status.port; }
  async function copyText(text: string) { await navigator.clipboard.writeText(text); }

  let mappingProviderModels = $derived(mForm.to_provider ? (getProviderById(mForm.to_provider)?.models ?? []) : []);

  // ── Settings ──
  let showSettings = $state(false);

  // ── Heartbeat ──
  let heartbeatStatus = $state<"ok" | "warn" | "unknown">("unknown");
  let heartbeatFailCount = $state(0);
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;

  async function runHeartbeat() {
    if (!status.running || config.model_mappings.length === 0) return;
    try {
      const { heartbeatCheck } = await import("$lib/api");
      const results = await heartbeatCheck();
      const allOk = results.every(r => r.ok);
      if (allOk) {
        heartbeatFailCount = 0;
        heartbeatStatus = "ok";
        await addOpLog("心跳检测", "全部正常 ✓");
      } else {
        heartbeatFailCount++;
        const failed = results.filter(r => !r.ok).map(r => r.mapping_from).join(", ");
        await addOpLog("心跳检测", `异常(${heartbeatFailCount}次): ${failed}`);
        if (heartbeatFailCount >= 3) heartbeatStatus = "warn";
      }
    } catch { heartbeatFailCount++; if (heartbeatFailCount >= 3) heartbeatStatus = "warn"; }
  }

  function startHeartbeat() {
    stopHeartbeat();
    heartbeatStatus = "ok";
    heartbeatFailCount = 0;
    const interval = (config.proxy.heartbeat_interval || 30) * 1000;
    heartbeatTimer = setInterval(runHeartbeat, interval);
  }

  function stopHeartbeat() {
    if (heartbeatTimer) { clearInterval(heartbeatTimer); heartbeatTimer = null; }
    heartbeatStatus = "unknown";
  }

  async function saveSettings() {
    await doSave();
    // Restart heartbeat with new interval
    if (status.running) startHeartbeat();
  }

  // ── Custom confirm dialog ──
  let confirmMsg = $state("");
  let confirmResolve: ((v: boolean) => void) | null = null;
  function customConfirm(msg: string): Promise<boolean> {
    confirmMsg = msg;
    return new Promise(resolve => { confirmResolve = resolve; });
  }
  function confirmYes() { confirmResolve?.(true); confirmMsg = ""; }
  function confirmNo() { confirmResolve?.(false); confirmMsg = ""; }

  // ── Code generation for simulate ──
  let simCodeLang = $state<"raw" | "curl" | "python" | "go" | "rust">("curl");
  function genCode(): string {
    const base = "http://127.0.0.1:" + config.proxy.port;
    const endpoint = simApiType === "responses" ? "/v1/responses" : "/v1/chat/completions";
    const url = base + endpoint;
    const body = simApiType === "responses"
      ? JSON.stringify({ model: simModel, input: simMessage, stream: false }, null, 2)
      : JSON.stringify({ model: simModel, messages: [{ role: "user", content: simMessage }], stream: false }, null, 2);

    if (simCodeLang === "raw") {
      return `POST ${endpoint} HTTP/1.1\nHost: 127.0.0.1:${config.proxy.port}\nContent-Type: application/json\n\n${body}`;
    }
    if (simCodeLang === "curl") {
      return `curl -X POST '${url}' \\\n  -H 'Content-Type: application/json' \\\n  -d '${JSON.stringify(JSON.parse(body))}'`;
    }
    if (simCodeLang === "python") {
      return `import requests\n\nresp = requests.post("${url}",\n    json=${body})\nprint(resp.json())`;
    }
    if (simCodeLang === "go") {
      return `body := \`${body}\`\nresp, _ := http.Post("${url}", "application/json", strings.NewReader(body))\ndefer resp.Body.Close()\ndata, _ := io.ReadAll(resp.Body)\nfmt.Println(string(data))`;
    }
    return `let resp = reqwest::Client::new()\n    .post("${url}")\n    .header("Content-Type", "application/json")\n    .body(r#"${JSON.stringify(JSON.parse(body))}"#)\n    .send().await?;\nprintln!("{}", resp.text().await?);`;
  }

  // ── Log export ──
  let showExportModal = $state(false);
  let exportRange = $state<"all" | "last100" | "last50">("all");
  function exportLogs() {
    const data = exportRange === "all" ? logs : logs.slice(-(exportRange === "last100" ? 100 : 50));
    const text = data.map(l => fmtLog(l).text).join("\n");
    const blob = new Blob([text], { type: "text/plain" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = "cc-proxy-logs-" + new Date().toISOString().slice(0, 10) + ".txt";
    a.click();
    showExportModal = false;
  }
</script>

<div class={dark ? "dark" : ""} style="height:100vh;display:flex;flex-direction:column;font-family:Inter,-apple-system,sans-serif;font-size:13px;" class:bg-slate-100={!dark} class:bg-slate-900={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
  <header class="h-12 flex items-center justify-between px-4 border-b flex-shrink-0" class:bg-white={!dark} class:bg-slate-800={dark} class:border-slate-200={!dark} class:border-slate-700={dark}>
    <span class="text-base font-bold bg-gradient-to-r from-indigo-500 to-purple-500 bg-clip-text text-transparent">CC Proxy</span>
    <div class="flex items-center gap-2">
      {#if status.running}
        <button class="flex items-center gap-2 px-4 py-1.5 rounded-full cursor-pointer border-none transition-all" class:bg-gradient-to-r={heartbeatStatus !== "warn"} class:from-emerald-900={heartbeatStatus !== "warn"} class:to-emerald-700={heartbeatStatus !== "warn"} class:shadow-[0_0_12px_rgba(16,185,129,0.4)]={heartbeatStatus !== "warn"} class:bg-yellow-700={heartbeatStatus === "warn"} class:shadow-[0_0_12px_rgba(234,179,8,0.4)]={heartbeatStatus === "warn"} onclick={() => showConnModal = true}>
          <span class="relative flex h-3 w-3">
            <span class="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75" class:bg-emerald-400={heartbeatStatus !== "warn"} class:bg-yellow-400={heartbeatStatus === "warn"}></span>
            <span class="relative inline-flex rounded-full h-3 w-3" class:bg-emerald-400={heartbeatStatus !== "warn"} class:shadow-[0_0_8px_#34d399]={heartbeatStatus !== "warn"} class:bg-yellow-400={heartbeatStatus === "warn"} class:shadow-[0_0_8px_#eab308]={heartbeatStatus === "warn"}></span>
          </span>
          <span class="text-xs font-medium" class:text-emerald-100={heartbeatStatus !== "warn"} class:text-yellow-100={heartbeatStatus === "warn"}>{heartbeatStatus === "warn" ? "映射异常" : "运行中"} · 端口 {status.port}</span>
        </button>
        <button class="px-3 py-1.5 text-xs rounded bg-red-600 text-white border border-red-600 hover:bg-red-700 cursor-pointer" onclick={handleStop}>停止</button>
      {:else}
        <button class="px-3 py-1 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={handleStart}>▶ 启动</button>
      {/if}
      <button class="px-2 py-1 text-sm rounded border cursor-pointer" class:bg-slate-100={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => dark = !dark}>{dark ? "☀️" : "🌙"}</button>
    </div>
  </header>
  <div class="flex flex-1 overflow-hidden">
    <div class="overflow-y-auto p-4 space-y-4" style="width:{logsOpen ? leftPct + '%' : 'calc(100% - 40px)'}">

      <!-- Proxy Settings -->
      <div class="rounded-xl border p-4" class:bg-white={!dark} class:bg-slate-800={dark} class:border-slate-200={!dark} class:border-slate-700={dark}>
        <div class="flex items-center gap-4">
          <label class="text-xs font-medium" class:text-slate-500={!dark} class:text-slate-400={dark}>端口</label>
          <input type="number" bind:value={config.proxy.port} onchange={doSave} class="w-24 rounded border px-2 py-1 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark} />
        </div>
      </div>

      <!-- Providers -->
      <div class="rounded-xl border p-4" class:bg-white={!dark} class:bg-slate-800={dark} class:border-slate-200={!dark} class:border-slate-700={dark}>
        <div class="flex justify-between items-center mb-3">
          <h3 class="text-xs font-semibold uppercase tracking-wide" class:text-slate-500={!dark} class:text-slate-400={dark}>提供商</h3>
          <div class="flex gap-1.5">
            <button class="px-2 py-1 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={openAddProvider}>添加</button>
          </div>
        </div>
        <table class="w-full text-xs border-collapse">
          <thead>
            <tr class:text-slate-400={!dark} class:text-slate-500={dark}>
              <th class="text-left py-1.5 px-2 font-semibold">名称</th>
              <th class="text-left py-1.5 px-2 font-semibold">Base URL</th>
              <th class="text-left py-1.5 px-2 font-semibold">API Key</th>
              <th class="text-left py-1.5 px-2 font-semibold">模型</th>
              <th class="text-left py-1.5 px-2 font-semibold">操作</th>
            </tr>
          </thead>
          <tbody>
            {#each config.providers as p (p.id)}
              <tr class="border-t" class:border-slate-100={!dark} class:border-slate-700={dark}>
                <td class="py-2 px-2">{p.name}</td>
                <td class="py-2 px-2 max-w-[160px] truncate" class:text-slate-400={!dark} class:text-slate-500={dark}>{p.base_url}</td>
                <td class="py-2 px-2 font-mono text-[11px] w-32">
                  <span class="inline-block w-20 overflow-hidden">{showApiKey[p.id] ? p.api_key : "••••••••••••"}</span>
                  <button class="ml-1 opacity-60 hover:opacity-100 cursor-pointer bg-transparent border-none text-xs" onclick={() => showApiKey = { ...showApiKey, [p.id]: !showApiKey[p.id] }}>{showApiKey[p.id] ? "🔒" : "👁"}</button>
                </td>
                <td class="py-2 px-2">
                  {#if providerTestResults[p.id]}
                    <div class="flex flex-wrap gap-1">
                      {#each p.models as m}
                        <span class="px-1.5 py-0.5 rounded-full text-[10px] {modelTestCls(p.id, m)}">{m}</span>
                      {/each}
                    </div>
                  {:else}
                    <span class:text-slate-500={!dark} class:text-slate-400={dark}>{p.models.length > 3 ? p.models.length + "个模型" : p.models.join(", ")}</span>
                  {/if}
                </td>
                <td class="py-2 px-2">
                  <div class="flex gap-1">
                    <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => testProvider(p.id)} disabled={testingProvider[p.id]}>{testingProvider[p.id] ? "测试中..." : "测试"}</button>
                    <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => openEditProvider(p)}>编辑</button>
                    <button class="px-2 py-0.5 text-[11px] rounded bg-red-600 text-white border border-red-600 hover:bg-red-700 cursor-pointer" onclick={() => deleteProvider(p.id)}>删除</button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <!-- Model Mappings -->
      <div class="rounded-xl border p-4" class:bg-white={!dark} class:bg-slate-800={dark} class:border-slate-200={!dark} class:border-slate-700={dark}>
        <div class="flex justify-between items-center mb-3">
          <h3 class="text-xs font-semibold uppercase tracking-wide" class:text-slate-500={!dark} class:text-slate-400={dark}>模型映射</h3>
          <div class="flex gap-1.5">
            <button class="px-2 py-1 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={openAddMapping}>添加</button>
          </div>
        </div>
        <table class="w-full text-xs border-collapse">
          <thead>
            <tr class:text-slate-400={!dark} class:text-slate-500={dark}>
              <th class="text-left py-1.5 px-2 font-semibold">代理模型名</th>
              <th class="py-1.5 px-1 w-6"></th>
              <th class="text-left py-1.5 px-2 font-semibold">目标提供商</th>
              <th class="text-left py-1.5 px-2 font-semibold">原始模型</th>
              <th class="text-left py-1.5 px-2 font-semibold">操作</th>
            </tr>
          </thead>
          <tbody>
            {#each config.model_mappings as m, i (i)}
              <tr class="border-t" class:border-slate-100={!dark} class:border-slate-700={dark}>
                <td class="py-2 px-2 font-mono text-indigo-500">{m.from}</td>
                <td class="py-1 px-1 text-center">
                  <svg class="w-5 h-5 mx-auto {status.running ? 'animate-flow-svg' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="color: #818cf8;">
                    <line x1="5" y1="12" x2="19" y2="12"/>
                    <polyline points="14 7 19 12 14 17"/>
                  </svg>
                </td>
                <td class="py-2 px-2">{getProviderById(m.to_provider)?.name ?? m.to_provider}</td>
                <td class="py-2 px-2 font-mono">{m.to_model}</td>
                <td class="py-2 px-2">
                  <div class="flex gap-1">
                    {#if mappingTestResult[i]}
                      <button class="px-2 py-0.5 text-[11px] rounded border cursor-default" style="color:{mappingTestResult[i].ok ? '#4ade80' : '#f87171'}">{mappingTestResult[i].ok ? "✓ " + mappingTestResult[i].ms + "ms" : "✗ " + mappingTestResult[i].msg}</button>
                    {:else}
                      <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => testMappingBtn(i)} disabled={testingMapping[i]}>{testingMapping[i] ? "..." : "测试"}</button>
                    {/if}
                    <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => { simModel = m.from; simMessage = "Hello!"; showSimModal = true; }}>模拟</button>
                    <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => openEditMapping(i)}>编辑</button>
                    <button class="px-2 py-0.5 text-[11px] rounded bg-red-600 text-white border border-red-600 hover:bg-red-700 cursor-pointer" onclick={() => deleteMapping(i)}>删除</button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <!-- Splitter -->
    {#if logsOpen}
      <div class="w-1 flex-shrink-0 cursor-col-resize hover:bg-indigo-500 transition-colors" class:bg-slate-200={!dark} class:bg-slate-700={dark} onmousedown={startDrag}></div>
    {/if}

    <!-- Log Panel -->
    {#if logsOpen}
      <div class="flex flex-col overflow-hidden" class:bg-gray-900={dark} class:text-gray-300={dark} class:bg-slate-50={!dark} class:text-slate-700={!dark} style="width:calc({100 - leftPct}% - 4px)">
        <div class="flex justify-between items-center px-3 py-2 border-b flex-shrink-0" class:border-gray-700={dark} class:bg-gray-800={dark} class:border-slate-200={!dark} class:bg-slate-100={!dark}>
          <span class="text-xs font-semibold" class:text-gray-200={dark} class:text-slate-600={!dark}>📋 日志 <span class="ml-1 px-1.5 py-0.5 rounded-full text-[10px]" class:bg-indigo-900={dark} class:text-indigo-300={dark} class:bg-indigo-100={!dark} class:text-indigo-600={!dark}>{logs.length}</span></span>
          <div class="flex items-center gap-2">
            <label class="flex items-center gap-1 text-[11px] cursor-pointer" class:text-gray-400={dark} class:text-slate-500={!dark}>
              <input type="checkbox" bind:checked={autoScroll} class="accent-indigo-500" />
              自动
            </label>
            <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-gray-700={dark} class:text-gray-300={dark} class:border-gray-600={dark} class:bg-slate-200={!dark} class:text-slate-600={!dark} class:border-slate-300={!dark} onclick={() => showExportModal = true}>导出</button>
            <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-gray-700={dark} class:text-gray-300={dark} class:border-gray-600={dark} class:bg-slate-200={!dark} class:text-slate-600={!dark} class:border-slate-300={!dark} onclick={handleClearLogs}>清空</button>
            <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-gray-700={dark} class:text-gray-300={dark} class:border-gray-600={dark} class:bg-slate-200={!dark} class:text-slate-600={!dark} class:border-slate-300={!dark} onclick={() => logsOpen = false}>✕</button>
          </div>
        </div>
        <div class="flex-1 overflow-y-auto p-3 font-mono text-[11px] leading-relaxed relative" bind:this={logContainer}>
          {#each logs as log (log.id)}
            {@const l = fmtLog(log)}
            <div class="py-px whitespace-pre-wrap break-all {l.cls}">{l.text}</div>
          {/each}
        </div>
        {#if !autoScroll}
          <button class="absolute bottom-3 right-3 w-7 h-7 rounded-full flex items-center justify-center shadow-lg cursor-pointer bg-indigo-600 text-white border-none text-xs" onclick={() => { if (logContainer) logContainer.scrollTop = logContainer.scrollHeight; autoScroll = true; }}>↓</button>
        {/if}
      </div>
    {:else}
      <button class="w-10 flex-shrink-0 flex flex-col items-center justify-center border-l cursor-pointer gap-1 transition-colors" class:bg-gray-900={dark} class:border-gray-700={dark} class:text-gray-500={dark} class:hover:text-gray-200={dark} class:bg-slate-100={!dark} class:border-slate-200={!dark} class:text-slate-400={!dark} onclick={() => logsOpen = true}>
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M15 18l-6-6 6-6"/></svg>
        <span style="writing-mode:vertical-rl" class="text-[10px] tracking-wide">日志</span>
        {#if logs.length > 0}
          <span class="px-1 py-0.5 rounded text-[9px]" class:bg-indigo-900={dark} class:text-indigo-300={dark} class:bg-indigo-100={!dark} class:text-indigo-600={!dark}>{logs.length}</span>
        {/if}
      </button>
    {/if}
  </div>

  <!-- Provider Modal -->
  {#if showProviderModal}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div class="rounded-2xl shadow-2xl p-6 w-[560px] max-h-[80vh] overflow-y-auto" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark} onclick={(e) => e.stopPropagation()}>
        <h3 class="text-sm font-semibold mb-4">{editingProvider ? "编辑提供商" : "添加提供商"}</h3>
        <div class="space-y-3">
          <div class="flex items-center gap-2">
            <label class="w-20 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>预设</label>
            <select bind:value={pForm.preset} onchange={() => applyPreset(pForm.preset)} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
              <option value="">选择预设</option>
              {#each Object.entries(PRESET_PROVIDERS) as [k, v]}
                <option value={k}>{v.name}</option>
              {/each}
            </select>
          </div>
          <div class="flex items-center gap-2">
            <label class="w-20 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>名称</label>
            <input bind:value={pForm.name} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark} />
          </div>
          <div class="flex items-center gap-2">
            <label class="w-20 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>Base URL</label>
            <input bind:value={pForm.base_url} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark} />
          </div>
          <div class="flex items-center gap-2">
            <label class="w-20 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>API Key</label>
            <input type={pForm.showKey ? "text" : "password"} bind:value={pForm.api_key} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark} />
            <button class="opacity-60 hover:opacity-100 bg-transparent border-none cursor-pointer text-sm" onclick={() => pForm.showKey = !pForm.showKey}>👁</button>
          </div>
          <div class="flex gap-2">
            <label class="w-20 text-xs shrink-0 pt-1" class:text-slate-500={!dark} class:text-slate-400={dark}>模型</label>
            <div class="flex-1">
              <div class="flex flex-wrap gap-1 mb-2">
                {#each pForm.models as m}
                  <span class="flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] bg-indigo-100 text-indigo-700 dark:bg-indigo-900/40 dark:text-indigo-300">
                    {m}
                    <button class="hover:text-red-500 bg-transparent border-none cursor-pointer leading-none" onclick={() => removeModel(m)}>×</button>
                  </span>
                {/each}
              </div>
              <div class="flex gap-1.5">
                <input bind:value={pForm.newModel} placeholder="添加模型" class="flex-1 rounded border px-2 py-1 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark} onkeydown={(e) => e.key === "Enter" && addModel()} />
                <button class="px-2 py-1 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={addModel}>添加</button>
                <button class="px-2 py-1 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={fetchModels} disabled={fetchingModels}>{fetchingModels ? "拉取中..." : "拉取模型"}</button>
              </div>
            </div>
          </div>
        </div>
        {#if pError}<p class="text-red-400 text-xs mt-2">{pError}</p>{/if}
        <div class="flex justify-end gap-2 mt-4">
          <button class="px-3 py-1.5 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => showProviderModal = false}>取消</button>
          <button class="px-3 py-1.5 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={saveProvider}>保存</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Mapping Modal -->
  {#if showMappingModal}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div class="rounded-2xl shadow-2xl p-6 w-[480px]" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
        <h3 class="text-sm font-semibold mb-4">{editingMappingIdx !== null ? "编辑映射" : "添加映射"}</h3>
        <div class="space-y-3">
          <div class="flex items-center gap-2">
            <label class="w-24 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>代理模型名</label>
            <input list="proxy-models" bind:value={mForm.from} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark} />
            <datalist id="proxy-models">
              {#each PROXY_MODELS as m}<option value={m}></option>{/each}
            </datalist>
          </div>
          <div class="flex items-center gap-2">
            <label class="w-24 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>目标提供商</label>
            <select bind:value={mForm.to_provider} onchange={() => mForm.to_model = ""} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
              <option value="">选择提供商</option>
              {#each config.providers as p}<option value={p.id}>{p.name}</option>{/each}
            </select>
          </div>
          <div class="flex items-center gap-2">
            <label class="w-24 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>原始模型</label>
            <select bind:value={mForm.to_model} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
              <option value="">选择模型</option>
              {#each mappingProviderModels as m}<option value={m}>{m}</option>{/each}
            </select>
          </div>
        </div>
        <div class="flex justify-end gap-2 mt-4">
          <button class="px-3 py-1.5 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => showMappingModal = false}>取消</button>
          <button class="px-3 py-1.5 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={saveMapping}>保存</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Connection Info Modal -->
  {#if showConnModal}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onclick={() => showConnModal = false}>
      <div class="rounded-2xl shadow-2xl p-6 w-[520px] max-h-[80vh] overflow-y-auto" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark} onclick={(e) => e.stopPropagation()}>
        <h3 class="text-sm font-semibold mb-4">连接信息</h3>
        {#each [["Base URL", connBase()], ["Chat", connBase() + "/v1/chat/completions"], ["Models", connBase() + "/v1/models"]] as [label, url]}
          <div class="flex items-center gap-2 mb-2">
            <span class="w-20 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>{label}</span>
            <code class="flex-1 font-mono text-xs px-2 py-1 rounded" class:bg-slate-100={!dark} class:bg-slate-900={dark}>{url}</code>
            <button class="px-2 py-1 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => copyText(url)}>复制</button>
          </div>
        {/each}
        <p class="text-xs font-semibold mt-3 mb-1" class:text-slate-500={!dark} class:text-slate-400={dark}>模型映射</p>
        {#each config.model_mappings as m}
          <div class="text-xs py-0.5">{m.from} → {getProviderById(m.to_provider)?.name} / {m.to_model}</div>
        {/each}
        <p class="text-xs font-semibold mt-3 mb-1" class:text-slate-500={!dark} class:text-slate-400={dark}>配置示例</p>
        <pre class="text-xs font-mono p-2 rounded" class:bg-slate-100={!dark} class:bg-slate-900={dark}>base_url: {connBase()}/v1{"\n"}api_key: any</pre>
        <div class="flex justify-end mt-4">
          <button class="px-3 py-1.5 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => showConnModal = false}>关闭</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Import Modal -->
  {#if showImportModal}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onclick={() => showImportModal = false}>
      <div class="rounded-2xl shadow-2xl p-6 w-[520px]" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark} onclick={(e) => e.stopPropagation()}>
        <h3 class="text-sm font-semibold mb-4">导入 CC-Switch 配置</h3>
        <textarea bind:value={importJson} rows={10} placeholder="粘贴 JSON 配置..." class="w-full rounded border px-2 py-1.5 text-xs font-mono resize-y" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark}></textarea>
        {#if importError}<p class="text-red-400 text-xs mt-1">{importError}</p>{/if}
        <div class="flex justify-end gap-2 mt-4">
          <button class="px-3 py-1.5 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => showImportModal = false}>取消</button>
          <button class="px-3 py-1.5 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={handleImport}>导入</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Simulate Request Modal -->
  {#if showSimModal}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div class="rounded-2xl shadow-2xl p-6 w-[800px] max-h-[90vh] overflow-y-auto" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
        <div class="flex justify-between items-center mb-4">
          <h3 class="text-sm font-semibold">模拟请求</h3>
          <button class="text-lg cursor-pointer border-none bg-transparent" class:text-slate-400={!dark} class:text-slate-500={dark} onclick={() => showSimModal = false}>✕</button>
        </div>
        <div class="flex items-center gap-2 mb-3 p-2 rounded border" class:bg-slate-50={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-700={dark}>
          <span class="text-xs font-mono px-2 py-1 rounded bg-indigo-600 text-white">POST</span>
          <span class="flex-1 text-xs font-mono" class:text-slate-500={!dark} class:text-slate-400={dark}>{connBase()}{simApiType === "responses" ? "/v1/responses" : "/v1/chat/completions"}</span>
        </div>
        <div class="flex items-center gap-2 mb-3">
          <label class="w-16 text-xs shrink-0" class:text-slate-500={!dark} class:text-slate-400={dark}>接口</label>
          <select bind:value={simApiType} class="rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
            <option value="chat">Chat Completions</option>
            <option value="responses">Responses API</option>
          </select>
          <label class="w-16 text-xs shrink-0 ml-2" class:text-slate-500={!dark} class:text-slate-400={dark}>模型</label>
          <select bind:value={simModel} class="flex-1 rounded border px-2 py-1.5 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
            <option value="">选择模型</option>
            {#each config.model_mappings as m}<option value={m.from}>{m.from}</option>{/each}
          </select>
        </div>
        <textarea bind:value={simMessage} rows={4} placeholder="消息内容..." class="w-full rounded border px-2 py-1.5 text-xs mb-3 resize-y" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} class:text-slate-800={!dark} class:text-slate-200={dark}></textarea>
        <div class="flex gap-2 mb-3">
          <button class="px-4 py-1.5 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={sendSimRequest} disabled={simLoading}>{simLoading ? "发送中..." : "▶ 发送"}</button>
        </div>
        {#if simResponse}
          <pre class="bg-gray-900 text-green-400 text-[11px] font-mono p-3 rounded overflow-x-auto max-h-60 overflow-y-auto">{simResponse}</pre>
        {/if}
        <div class="mt-3 border-t pt-3" class:border-slate-200={!dark} class:border-slate-700={dark}>
          <div class="flex items-center gap-2 mb-2">
            <span class="text-xs font-medium" class:text-slate-500={!dark} class:text-slate-400={dark}>生成代码</span>
            {#each (["raw", "curl", "python", "go", "rust"] as const) as lang}
              <button class="px-2 py-0.5 text-[11px] rounded border cursor-pointer" class:bg-indigo-600={simCodeLang === lang} class:text-white={simCodeLang === lang} class:border-indigo-600={simCodeLang === lang} class:bg-slate-50={simCodeLang !== lang && !dark} class:bg-slate-700={simCodeLang !== lang && dark} class:border-slate-200={simCodeLang !== lang && !dark} class:border-slate-600={simCodeLang !== lang && dark} onclick={() => simCodeLang = lang}>{lang}</button>
            {/each}
            <button class="px-2 py-0.5 text-[11px] rounded bg-indigo-600 text-white border border-indigo-600 cursor-pointer" onclick={() => copyText(genCode())}>复制</button>
          </div>
          <pre class="text-[11px] font-mono p-2 rounded overflow-x-auto" class:bg-slate-100={!dark} class:bg-slate-900={dark}>{genCode()}</pre>
        </div>
      </div>
    </div>
  {/if}

  <!-- Settings Modal -->
  {#if showSettings}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div class="rounded-2xl shadow-2xl p-6 w-[420px]" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
        <div class="flex justify-between items-center mb-4">
          <h3 class="text-sm font-semibold">⚙️ 设置</h3>
          <button class="text-lg cursor-pointer border-none bg-transparent" class:text-slate-400={!dark} class:text-slate-500={dark} onclick={() => showSettings = false}>✕</button>
        </div>
        <div class="space-y-4">
          <!-- Theme -->
          <div class="flex items-center justify-between">
            <span class="text-xs font-medium">主题</span>
            <div class="flex gap-1">
              <button class="px-3 py-1 text-xs rounded border cursor-pointer" class:bg-indigo-600={!dark} class:text-white={!dark} class:border-indigo-600={!dark} class:bg-slate-700={dark} class:border-slate-600={dark} onclick={() => { dark = false; config.proxy.theme = "light"; saveSettings(); }}>☀️ 明亮</button>
              <button class="px-3 py-1 text-xs rounded border cursor-pointer" class:bg-indigo-600={dark} class:text-white={dark} class:border-indigo-600={dark} class:bg-slate-50={!dark} class:border-slate-200={!dark} onclick={() => { dark = true; config.proxy.theme = "dark"; saveSettings(); }}>🌙 暗黑</button>
            </div>
          </div>
          <!-- Language -->
          <div class="flex items-center justify-between">
            <span class="text-xs font-medium">语言</span>
            <select bind:value={config.proxy.language} onchange={saveSettings} class="rounded border px-2 py-1 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark}>
              <option value="zh">中文</option>
              <option value="en">English</option>
            </select>
          </div>
          <!-- Autostart -->
          <div class="flex items-center justify-between">
            <span class="text-xs font-medium">开机自启</span>
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="checkbox" bind:checked={config.proxy.autostart} onchange={saveSettings} class="accent-indigo-500" />
              <span class="text-xs" class:text-slate-500={!dark} class:text-slate-400={dark}>{config.proxy.autostart ? "已开启" : "已关闭"}</span>
            </label>
          </div>
          <!-- Heartbeat -->
          <div class="flex items-center justify-between">
            <span class="text-xs font-medium">心跳间隔 (秒)</span>
            <input type="number" bind:value={config.proxy.heartbeat_interval} onchange={saveSettings} min="5" max="300" class="w-20 rounded border px-2 py-1 text-xs" class:bg-white={!dark} class:bg-slate-900={dark} class:border-slate-200={!dark} class:border-slate-600={dark} />
          </div>
          <!-- Divider -->
          <div class="border-t pt-3" class:border-slate-200={!dark} class:border-slate-700={dark}>
            <p class="text-xs font-semibold mb-2" class:text-slate-500={!dark} class:text-slate-400={dark}>关于</p>
            <div class="text-xs space-y-1" class:text-slate-600={!dark} class:text-slate-400={dark}>
              <p><strong>产品:</strong> CC Proxy</p>
              <p><strong>版本:</strong> 2.0.0</p>
              <p><strong>描述:</strong> AI 模型代理路由工具</p>
              <p><strong>协议:</strong> Claude/Codex → OpenAI 兼容</p>
              <p><strong>技术栈:</strong> Rust + Tauri + Svelte + Tailwind</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Custom Confirm Dialog -->
  {#if confirmMsg}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div class="rounded-xl shadow-2xl p-5 w-[320px]" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
        <p class="text-sm mb-4">{confirmMsg}</p>
        <div class="flex justify-end gap-2">
          <button class="px-3 py-1.5 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={confirmNo}>取消</button>
          <button class="px-3 py-1.5 text-xs rounded bg-red-600 text-white border border-red-600 hover:bg-red-700 cursor-pointer" onclick={confirmYes}>确定</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Export Logs Modal -->
  {#if showExportModal}
    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div class="rounded-xl shadow-2xl p-5 w-[320px]" class:bg-white={!dark} class:bg-slate-800={dark} class:text-slate-800={!dark} class:text-slate-200={dark}>
        <h3 class="text-sm font-semibold mb-3">导出日志</h3>
        <div class="space-y-2 mb-4">
          {#each ([["all", "全部"], ["last100", "最近100条"], ["last50", "最近50条"]] as const) as [val, label]}
            <label class="flex items-center gap-2 text-xs cursor-pointer">
              <input type="radio" bind:group={exportRange} value={val} class="accent-indigo-500" />
              {label}
            </label>
          {/each}
        </div>
        <div class="flex justify-end gap-2">
          <button class="px-3 py-1.5 text-xs rounded border cursor-pointer" class:bg-slate-50={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => showExportModal = false}>取消</button>
          <button class="px-3 py-1.5 text-xs rounded bg-indigo-600 text-white border border-indigo-600 hover:bg-indigo-700 cursor-pointer" onclick={exportLogs}>导出</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Settings button (bottom-left) -->
  <button class="fixed bottom-4 left-4 w-8 h-8 rounded-full flex items-center justify-center shadow-lg cursor-pointer border z-40" class:bg-white={!dark} class:bg-slate-700={dark} class:border-slate-200={!dark} class:border-slate-600={dark} onclick={() => showSettings = true}>
    <svg class="w-4 h-4" class:text-slate-500={!dark} class:text-slate-300={dark} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>
  </button>
</div>

<style>
  :global(.overflow-y-auto) {
    scrollbar-width: none;
    -ms-overflow-style: none;
  }
  :global(.overflow-y-auto::-webkit-scrollbar) {
    display: none;
  }
  @keyframes flow {
    0% { transform: translateX(0); opacity: 0.6; }
    50% { transform: translateX(3px); opacity: 1; }
    100% { transform: translateX(0); opacity: 0.6; }
  }
  .animate-flow-svg {
    animation: flow 0.8s ease-in-out infinite;
  }
</style>
