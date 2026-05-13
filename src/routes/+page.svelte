<script lang="ts">
  import { onMount, afterUpdate, tick } from "svelte";
  import {
    getConfig, saveConfig, getLogs, clearLogs, getProxyStatus,
    startProxy, stopProxy, addOpLog, testProvider, testMapping,
    PRESET_PROVIDERS, PROXY_MODELS,
    type Config, type Provider, type ModelMapping, type RequestLog, type ProxyStatus, type TestResult
  } from "$lib/api";

  // ── state ──────────────────────────────────────────────────────────────────
  let config = $state<Config | null>(null);
  let logs = $state<RequestLog[]>([]);
  let status = $state<ProxyStatus | null>(null);
  let loading = $state(false);
  let selectedLog = $state<RequestLog | null>(null);
  let toast = $state("");

  // provider modal
  let showProviderModal = $state(false);
  let providerPreset = $state("deepseek");
  let customProviderName = $state("");
  let editingProviderIdx = $state<number | null>(null);
  let providerForm = $state<Provider>({ id: "", name: "", base_url: "", api_key: "", default_model: "" });
  let showProviderKey = $state(false);

  // provider inline edit
  let inlineProviderIdx = $state<number | null>(null);
  let inlineProviderForm = $state<Provider>({ id: "", name: "", base_url: "", api_key: "", default_model: "" });
  let showInlineKey = $state(false);

  // mapping modal
  let showMappingModal = $state(false);
  let editingMappingIdx = $state<number | null>(null);
  let mappingForm = $state<ModelMapping>({ from: "", to_provider: "", to_model: "" });

  // mapping inline edit
  let inlineMappingIdx = $state<number | null>(null);
  let inlineMappingForm = $state<ModelMapping>({ from: "", to_provider: "", to_model: "" });

  // connection info modal
  let showConnInfo = $state(false);

  // test states
  let testingProvider = $state<string | null>(null);
  let providerTestResult = $state<Record<string, TestResult>>({});
  let testingMapping = $state<number | null>(null);
  let mappingTestResult = $state<Record<number, TestResult>>({});

  // ── lifecycle ──────────────────────────────────────────────────────────────
  onMount(() => {
    loadAll();
    const iv = setInterval(refresh, 3000);
    return () => clearInterval(iv);
  });

  async function loadAll() {
    try {
      [config, status, logs] = await Promise.all([getConfig(), getProxyStatus(), getLogs(100)]);
    } catch (e) { console.error(e); }
  }

  async function refresh() {
    try {
      status = await getProxyStatus();
      logs = await getLogs(100);
    } catch {}
  }

  function showToast(msg: string) { toast = msg; setTimeout(() => toast = "", 3000); }

  // ── proxy control ──────────────────────────────────────────────────────────
  async function handleStart() {
    loading = true;
    try {
      await startProxy();
      status = await getProxyStatus();
      await addOpLog("启动代理", `端口 ${status?.port}`);
      showToast("✓ 代理已启动");
    } catch (e: any) { showToast("✗ " + e); }
    loading = false;
  }

  async function handleStop() {
    loading = true;
    try {
      await stopProxy();
      await addOpLog("停止代理", "");
      status = await getProxyStatus();
      showToast("✓ 代理已停止");
    } catch (e: any) { showToast("✗ " + e); }
    loading = false;
  }

  async function onPortChange() {
    if (!config) return;
    await saveConfig(config);
    await addOpLog("修改端口", String(config.proxy.port));
  }

  // ── provider modal ─────────────────────────────────────────────────────────
  function openAddProvider() {
    editingProviderIdx = null;
    providerPreset = "deepseek";
    customProviderName = "";
    const p = PRESET_PROVIDERS["deepseek"];
    providerForm = { id: "deepseek", name: p.name, base_url: p.base_url, api_key: "", default_model: p.models[0] || "" };
    showProviderKey = false;
    showProviderModal = true;
  }

  function onPresetChange() {
    const p = PRESET_PROVIDERS[providerPreset];
    if (!p) return;
    if (providerPreset === "custom") {
      providerForm = { id: "", name: customProviderName, base_url: "", api_key: "", default_model: "" };
    } else {
      providerForm = { id: providerPreset, name: p.name, base_url: p.base_url, api_key: providerForm.api_key, default_model: p.models[0] || "" };
    }
  }

  async function saveProviderModal() {
    if (!config) return;
    const name = providerPreset === "custom" ? customProviderName : providerForm.name;
    if (!name || !providerForm.base_url || !providerForm.api_key) { showToast("请填写完整信息"); return; }
    const id = providerForm.id || name.toLowerCase().replace(/\s+/g, "_");
    const entry: Provider = { ...providerForm, id, name };
    if (editingProviderIdx !== null) {
      config.providers[editingProviderIdx] = entry;
      await saveConfig(config);
      await addOpLog("编辑提供商", name);
      showToast("✓ 已更新");
    } else {
      config.providers = [...config.providers, entry];
      await saveConfig(config);
      await addOpLog("添加提供商", name);
      showToast("✓ 已添加");
    }
    showProviderModal = false;
  }

  // ── provider inline edit ───────────────────────────────────────────────────
  function startInlineProvider(idx: number) {
    if (!config) return;
    inlineProviderIdx = idx;
    inlineProviderForm = { ...config.providers[idx] };
    showInlineKey = false;
  }

  async function saveInlineProvider() {
    if (!config || inlineProviderIdx === null) return;
    config.providers[inlineProviderIdx] = { ...inlineProviderForm };
    await saveConfig(config);
    await addOpLog("编辑提供商", inlineProviderForm.name);
    inlineProviderIdx = null;
    showToast("✓ 已保存");
  }

  async function deleteProvider(idx: number) {
    if (!config) return;
    const name = config.providers[idx].name;
    config.providers = config.providers.filter((_, i) => i !== idx);
    await saveConfig(config);
    await addOpLog("删除提供商", name);
    showToast("✓ 已删除");
  }

  // ── mapping modal ──────────────────────────────────────────────────────────
  function openAddMapping() {
    editingMappingIdx = null;
    mappingForm = { from: "", to_provider: config?.providers[0]?.id || "", to_model: "" };
    showMappingModal = true;
  }

  async function saveMappingModal() {
    if (!config || !mappingForm.from || !mappingForm.to_provider || !mappingForm.to_model) { showToast("请填写完整信息"); return; }
    if (editingMappingIdx !== null) {
      config.model_mappings[editingMappingIdx] = { ...mappingForm };
      await saveConfig(config);
      await addOpLog("编辑映射", mappingForm.from);
      showToast("✓ 已更新");
    } else {
      config.model_mappings = [...config.model_mappings, { ...mappingForm }];
      await saveConfig(config);
      await addOpLog("添加映射", mappingForm.from);
      showToast("✓ 已添加");
    }
    showMappingModal = false;
  }

  // ── mapping inline edit ────────────────────────────────────────────────────
  function startInlineMapping(idx: number) {
    if (!config) return;
    inlineMappingIdx = idx;
    inlineMappingForm = { ...config.model_mappings[idx] };
  }

  async function saveInlineMapping() {
    if (!config || inlineMappingIdx === null) return;
    config.model_mappings[inlineMappingIdx] = { ...inlineMappingForm };
    await saveConfig(config);
    await addOpLog("编辑映射", inlineMappingForm.from);
    inlineMappingIdx = null;
    showToast("✓ 已保存");
  }

  async function deleteMapping(idx: number) {
    if (!config) return;
    const from = config.model_mappings[idx].from;
    config.model_mappings = config.model_mappings.filter((_, i) => i !== idx);
    await saveConfig(config);
    await addOpLog("删除映射", from);
    showToast("✓ 已删除");
  }

  // ── test ───────────────────────────────────────────────────────────────────
  async function handleTestProvider(id: string) {
    testingProvider = id;
    try { providerTestResult[id] = await testProvider(id); }
    catch (e: any) { providerTestResult[id] = { success: false, message: String(e), latency_ms: 0 }; }
    testingProvider = null;
  }

  async function handleTestMapping(idx: number) {
    testingMapping = idx;
    try { mappingTestResult[idx] = await testMapping(idx); }
    catch (e: any) { mappingTestResult[idx] = { success: false, message: String(e), latency_ms: 0 }; }
    testingMapping = null;
  }

  function testColor(r: TestResult) {
    if (!r.success) return "test-fail";
    return r.latency_ms <= 2000 ? "test-ok" : "test-warn";
  }

  // ── logs ───────────────────────────────────────────────────────────────────
  async function handleClearLogs() { await clearLogs(); logs = []; }

  // ── copy ───────────────────────────────────────────────────────────────────
  function copy(text: string) { navigator.clipboard.writeText(text); showToast("✓ Copied"); }
</script>

<div class="app">
  {#if toast}<div class="toast">{toast}</div>{/if}

  <header>
    <h1>CC Proxy</h1>
    <div class="header-right">
      {#if status?.running}
        <span class="status-dot"></span>
        <span class="status-text">Running &middot; Port {status.port}</span>
        <button class="btn-info" onclick={() => showConnInfo = true}>Connection Info</button>
        <button class="btn-stop" onclick={handleStop} disabled={loading}>{loading ? "Stopping..." : "Stop"}</button>
      {:else}
        <button class="btn-start" onclick={handleStart} disabled={loading}>
          {#if loading}<span class="spinner"></span> Starting...{:else}Start{/if}
        </button>
      {/if}
    </div>
  </header>

  <div class="split-layout">
    <!-- LEFT PANEL: Config -->
    <div class="panel panel-left">

      <!-- Proxy Settings -->
      <section class="section">
        <div class="section-header"><h2>Proxy Settings</h2></div>
        {#if config}
          <div class="card">
            <label class="port-label">Port
              <input type="number" bind:value={config.proxy.port} onchange={onPortChange} style="width:100px" />
            </label>
          </div>
        {/if}
      </section>

      <!-- Providers -->
      <section class="section">
        <div class="section-header">
          <h2>Providers</h2>
          <button class="btn-primary" onclick={openAddProvider}>+ 添加</button>
        </div>
        {#if config && config.providers.length > 0}
          <div class="table-wrap">
            <table>
              <thead><tr><th>名称</th><th>Base URL</th><th>API Key</th><th>默认模型</th><th>操作</th></tr></thead>
              <tbody>
                {#each config.providers as provider, idx}
                  {#if inlineProviderIdx === idx}
                    <tr class="editing-row">
                      <td><input bind:value={inlineProviderForm.name} /></td>
                      <td><input bind:value={inlineProviderForm.base_url} /></td>
                      <td>
                        <div class="input-toggle">
                          <input type={showInlineKey ? "text" : "password"} bind:value={inlineProviderForm.api_key} />
                          <button type="button" class="toggle-btn" onclick={() => showInlineKey = !showInlineKey}>{showInlineKey ? "🔒" : "👁"}</button>
                        </div>
                      </td>
                      <td><input bind:value={inlineProviderForm.default_model} /></td>
                      <td class="act">
                        <button class="btn-sm btn-save" onclick={saveInlineProvider}>保存</button>
                        <button class="btn-sm" onclick={() => inlineProviderIdx = null}>取消</button>
                      </td>
                    </tr>
                  {:else}
                    <tr>
                      <td><strong>{provider.name}</strong></td>
                      <td class="cell-url">{provider.base_url}</td>
                      <td class="cell-key">{"•".repeat(Math.min(provider.api_key.length, 12))}</td>
                      <td><code>{provider.default_model}</code></td>
                      <td class="act">
                        <button class="btn-sm btn-test" onclick={() => handleTestProvider(provider.id)} disabled={testingProvider === provider.id}>
                          {testingProvider === provider.id ? "..." : "测试"}
                        </button>
                        {#if providerTestResult[provider.id]}
                          {@const r = providerTestResult[provider.id]}
                          <span class="test-badge {testColor(r)}" title={r.message}>{r.success ? "✓" : "✗"} {r.latency_ms}ms</span>
                        {/if}
                        <button class="btn-sm" onclick={() => startInlineProvider(idx)}>编辑</button>
                        <button class="btn-sm btn-del" onclick={() => deleteProvider(idx)}>删除</button>
                      </td>
                    </tr>
                  {/if}
                {/each}
              </tbody>
            </table>
          </div>
        {:else if config}
          <p class="empty">暂无提供商</p>
        {/if}
      </section>

      <!-- Model Mappings -->
      <section class="section">
        <div class="section-header">
          <h2>Model Mappings</h2>
          <button class="btn-primary" onclick={openAddMapping}>+ 添加</button>
        </div>
        {#if config && config.model_mappings.length > 0}
          <div class="table-wrap">
            <table>
              <thead><tr><th>代理模型名</th><th>目标提供商</th><th>原始模型</th><th>操作</th></tr></thead>
              <tbody>
                {#each config.model_mappings as mapping, idx}
                  {#if inlineMappingIdx === idx}
                    <tr class="editing-row">
                      <td>
                        <input list="il-proxy-models" bind:value={inlineMappingForm.from} />
                        <datalist id="il-proxy-models">{#each PROXY_MODELS as m}<option value={m}></option>{/each}</datalist>
                      </td>
                      <td>
                        <select bind:value={inlineMappingForm.to_provider}>
                          {#each config.providers as p}<option value={p.id}>{p.name}</option>{/each}
                        </select>
                      </td>
                      <td>
                        <input list="il-target-models" bind:value={inlineMappingForm.to_model} />
                        <datalist id="il-target-models">{#each (PRESET_PROVIDERS[inlineMappingForm.to_provider]?.models ?? []) as m}<option value={m}></option>{/each}</datalist>
                      </td>
                      <td class="act">
                        <button class="btn-sm btn-save" onclick={saveInlineMapping}>保存</button>
                        <button class="btn-sm" onclick={() => inlineMappingIdx = null}>取消</button>
                      </td>
                    </tr>
                  {:else}
                    <tr>
                      <td><code>{mapping.from}</code></td>
                      <td>{config.providers.find(p => p.id === mapping.to_provider)?.name ?? mapping.to_provider}</td>
                      <td><code>{mapping.to_model}</code></td>
                      <td class="act">
                        <button class="btn-sm btn-test" onclick={() => handleTestMapping(idx)} disabled={testingMapping === idx}>
                          {testingMapping === idx ? "..." : "测试"}
                        </button>
                        {#if mappingTestResult[idx]}
                          {@const r = mappingTestResult[idx]}
                          <span class="test-badge {testColor(r)}" title={r.message}>{r.success ? "✓" : "✗"} {r.latency_ms}ms</span>
                        {/if}
                        <button class="btn-sm" onclick={() => startInlineMapping(idx)}>编辑</button>
                        <button class="btn-sm btn-del" onclick={() => deleteMapping(idx)}>删除</button>
                      </td>
                    </tr>
                  {/if}
                {/each}
              </tbody>
            </table>
          </div>
        {:else if config}
          <p class="empty">暂无映射</p>
        {/if}
      </section>
    </div>

    <!-- RIGHT PANEL: Logs -->
    <div class="panel panel-right">
      <div class="section-header">
        <h2>Logs</h2>
        <button class="btn-secondary" onclick={handleClearLogs}>Clear</button>
      </div>
      {#if logs.length === 0}
        <p class="empty">No logs yet</p>
      {:else}
        <div class="log-list">
          {#each logs as log}
            <div class="log-row {log.method === 'OP' ? '' : 'log-clickable'}" onclick={() => log.method !== 'OP' && (selectedLog = log)}>
              <span class="log-time">{new Date(log.timestamp).toLocaleTimeString()}</span>
              {#if log.method === 'OP'}
                <span class="badge badge-op">OP</span>
                <span class="log-path">{log.path}</span>
              {:else}
                <span class="badge badge-method">{log.method}</span>
                <span class="log-path">{log.path}</span>
                {#if log.model_in}<span class="log-model">{log.model_in}{log.model_out ? " → " + log.model_out : ""}</span>{/if}
                <span class="badge {log.status >= 400 ? 'badge-err' : 'badge-ok'}">{log.status}</span>
                <span class="log-lat">{log.latency_ms}ms</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Log Detail Modal -->
  {#if selectedLog}
    <div class="overlay" role="dialog" aria-modal="true">
      <div class="modal">
        <div class="modal-head">
          <h3>Request Detail</h3>
          <button onclick={() => selectedLog = null}>✕</button>
        </div>
        <div class="modal-body">
          <div class="detail-grid">
            <div><h4>Request</h4><pre>{JSON.stringify(selectedLog.request_body, null, 2)}</pre></div>
            <div><h4>Response</h4><pre>{JSON.stringify(selectedLog.response_body, null, 2)}</pre></div>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Provider Modal -->
  {#if showProviderModal}
    <div class="overlay" role="dialog" aria-modal="true">
      <div class="modal">
        <div class="modal-head">
          <h3>{editingProviderIdx !== null ? "编辑提供商" : "添加提供商"}</h3>
          <button onclick={() => showProviderModal = false}>✕</button>
        </div>
        <div class="modal-body">
          <div class="form-row">
            <label>Provider
              <select bind:value={providerPreset} onchange={onPresetChange}>
                {#each Object.entries(PRESET_PROVIDERS) as [key, val]}
                  <option value={key}>{val.name}</option>
                {/each}
              </select>
            </label>
            {#if providerPreset === "custom"}
              <label>Name <input bind:value={customProviderName} placeholder="My Provider" /></label>
            {/if}
          </div>
          <div class="form-row">
            <label>Base URL <input bind:value={providerForm.base_url} placeholder="https://api.example.com" /></label>
          </div>
          <div class="form-row">
            <label>API Key
              <div class="input-toggle">
                <input type={showProviderKey ? "text" : "password"} bind:value={providerForm.api_key} placeholder="sk-..." />
                <button type="button" class="toggle-btn" onclick={() => showProviderKey = !showProviderKey}>{showProviderKey ? "🔒" : "👁"}</button>
              </div>
            </label>
            <label>Default Model
              <input list="modal-def-models" bind:value={providerForm.default_model} placeholder="model-name" />
              <datalist id="modal-def-models">{#each (PRESET_PROVIDERS[providerPreset]?.models ?? []) as m}<option value={m}></option>{/each}</datalist>
            </label>
          </div>
          <div class="form-actions">
            <button class="btn-primary" onclick={saveProviderModal}>{editingProviderIdx !== null ? "更新" : "添加"}</button>
            <button class="btn-secondary" onclick={() => showProviderModal = false}>取消</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Mapping Modal -->
  {#if showMappingModal}
    <div class="overlay" role="dialog" aria-modal="true">
      <div class="modal">
        <div class="modal-head">
          <h3>{editingMappingIdx !== null ? "编辑映射" : "添加映射"}</h3>
          <button onclick={() => showMappingModal = false}>✕</button>
        </div>
        <div class="modal-body">
          <div class="form-row">
            <label>代理模型名
              <input list="modal-proxy-models" bind:value={mappingForm.from} placeholder="claude-sonnet-4-20250514" />
              <datalist id="modal-proxy-models">{#each PROXY_MODELS as m}<option value={m}></option>{/each}</datalist>
            </label>
            <label>目标提供商
              {#if config}
                <select bind:value={mappingForm.to_provider}>
                  {#each config.providers as p}<option value={p.id}>{p.name}</option>{/each}
                </select>
              {/if}
            </label>
            <label>原始模型
              <input list="modal-target-models" bind:value={mappingForm.to_model} placeholder="model-name" />
              <datalist id="modal-target-models">{#each (PRESET_PROVIDERS[mappingForm.to_provider]?.models ?? []) as m}<option value={m}></option>{/each}</datalist>
            </label>
          </div>
          <div class="form-actions">
            <button class="btn-primary" onclick={saveMappingModal}>{editingMappingIdx !== null ? "更新" : "添加"}</button>
            <button class="btn-secondary" onclick={() => showMappingModal = false}>取消</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Connection Info Modal -->
  {#if showConnInfo && config && status}
    <div class="overlay" role="dialog" aria-modal="true">
      <div class="modal">
        <div class="modal-head">
          <h3>Connection Info</h3>
          <button onclick={() => showConnInfo = false}>✕</button>
        </div>
        <div class="modal-body conn-info">
          <div class="ci-row">
            <span class="ci-label">Base URL</span>
            <code>http://{status.host}:{status.port}</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}`)}>Copy</button>
          </div>
          <div class="ci-row">
            <span class="ci-label">Chat Completions</span>
            <code>http://{status.host}:{status.port}/v1/chat/completions</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}/v1/chat/completions`)}>Copy</button>
          </div>
          <div class="ci-row">
            <span class="ci-label">Responses API</span>
            <code>http://{status.host}:{status.port}/v1/responses</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}/v1/responses`)}>Copy</button>
          </div>
          <div class="ci-row">
            <span class="ci-label">Models</span>
            <code>http://{status.host}:{status.port}/v1/models</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}/v1/models`)}>Copy</button>
          </div>
          <hr class="ci-divider" />
          <div class="ci-section-label">Available Model Mappings</div>
          {#each config.model_mappings as m}
            <div class="ci-row ci-model-row">
              <code>{m.from}</code>
              <span class="ci-arrow">→</span>
              <span>{config.providers.find(p => p.id === m.to_provider)?.name ?? m.to_provider}</span>
              <code>{m.to_model}</code>
              <button class="btn-copy-sm" onclick={() => copy(m.from)}>Copy</button>
            </div>
          {/each}
          <hr class="ci-divider" />
          <div class="ci-section-label">Config Example</div>
          <div class="ci-row ci-pre-row">
            <pre class="ci-pre">ANTHROPIC_BASE_URL=http://{status.host}:{status.port}
# Codex CLI
OPENAI_BASE_URL=http://{status.host}:{status.port}/v1</pre>
            <button class="btn-copy" onclick={() => copy(`ANTHROPIC_BASE_URL=http://${status!.host}:${status!.port}
OPENAI_BASE_URL=http://${status!.host}:${status!.port}/v1`)}>Copy</button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html, body) { margin: 0; padding: 0; height: 100%; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
  .app { height: 100vh; display: flex; flex-direction: column; background: #f5f7fa; color: #1a1a2e; overflow: hidden; }
  header { display: flex; align-items: center; gap: 16px; padding: 10px 20px; background: #fff; border-bottom: 1px solid #e8ecf0; flex-shrink: 0; }
  header h1 { margin: 0; font-size: 1.15rem; background: linear-gradient(135deg, #667eea, #764ba2); -webkit-background-clip: text; -webkit-text-fill-color: transparent; font-weight: 700; }
  .header-right { margin-left: auto; display: flex; align-items: center; gap: 10px; }
  .status-dot { width: 8px; height: 8px; border-radius: 50%; background: #16a34a; animation: pulse 2s infinite; flex-shrink: 0; }
  .status-text { font-size: 0.8rem; color: #16a34a; font-weight: 500; }
  @keyframes pulse { 0%,100% { opacity:1; } 50% { opacity:0.5; } }
  .btn-start { padding: 7px 18px; background: #667eea; color: #fff; border: none; border-radius: 7px; font-size: 0.82rem; font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 5px; }
  .btn-start:hover { background: #5a6fd6; } .btn-start:disabled { opacity: 0.6; cursor: not-allowed; }
  .btn-stop { padding: 7px 14px; background: #ef4444; color: #fff; border: none; border-radius: 7px; font-size: 0.82rem; cursor: pointer; }
  .btn-stop:hover { background: #dc2626; } .btn-stop:disabled { opacity: 0.6; cursor: not-allowed; }
  .btn-info { padding: 6px 12px; background: #dbeafe; color: #1d4ed8; border: none; border-radius: 6px; font-size: 0.78rem; cursor: pointer; font-weight: 500; }
  .btn-info:hover { background: #bfdbfe; }
  .spinner { width: 13px; height: 13px; border: 2px solid rgba(255,255,255,0.3); border-top-color: #fff; border-radius: 50%; animation: spin 0.6s linear infinite; display: inline-block; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .toast { position: fixed; top: 56px; right: 20px; background: #1a1a2e; color: #fff; padding: 9px 18px; border-radius: 7px; font-size: 0.82rem; z-index: 999; animation: fadeIn 0.2s; }
  @keyframes fadeIn { from { opacity:0; transform:translateY(-6px); } to { opacity:1; } }
  .split-layout { display: flex; flex: 1; overflow: hidden; }
  .panel { display: flex; flex-direction: column; overflow-y: auto; padding: 20px; }
  .panel-left { flex: 55; border-right: 1px solid #e0e4ea; background: #f5f7fa; }
  .panel-right { flex: 45; background: #f0f2f5; }
  @media (max-width: 700px) { .split-layout { flex-direction: column; } .panel-left { border-right: none; border-bottom: 1px solid #e0e4ea; } }
  .section { margin-bottom: 28px; }
  .section-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
  .section-header h2 { margin: 0; font-size: 0.95rem; font-weight: 600; color: #333; }
  .card { background: #fff; border-radius: 9px; padding: 14px 18px; border: 1px solid #e8ecf0; }
  .port-label { display: flex; align-items: center; gap: 10px; font-size: 0.82rem; color: #555; font-weight: 500; }
  .table-wrap { background: #fff; border-radius: 9px; overflow: hidden; border: 1px solid #e8ecf0; }
  table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
  th { text-align: left; padding: 9px 12px; background: #f8f9fb; color: #666; font-weight: 600; border-bottom: 1px solid #e8ecf0; white-space: nowrap; }
  td { padding: 7px 12px; border-bottom: 1px solid #f3f4f6; vertical-align: middle; }
  tbody tr:last-child td { border-bottom: none; }
  tbody tr:hover { background: #f8f9fb; }
  tbody tr.editing-row { background: #f0f4ff; }
  .cell-url { color: #888; font-size: 0.76rem; max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell-key { color: #bbb; letter-spacing: 2px; font-size: 0.72rem; }
  code { font-size: 0.76rem; background: #f0f2f5; padding: 2px 5px; border-radius: 3px; font-family: monospace; }
  .empty { text-align: center; color: #bbb; padding: 28px; margin: 0; font-size: 0.82rem; }
  .badge { padding: 2px 7px; border-radius: 4px; font-size: 0.7rem; font-weight: 600; }
  .badge-ok { background: #dcfce7; color: #166534; }
  .badge-err { background: #fef2f2; color: #991b1b; }
  .badge-op { background: #dbeafe; color: #1d4ed8; }
  .badge-method { background: #f0f2f5; color: #555; }
  .btn-primary { padding: 6px 16px; background: #667eea; color: #fff; border: none; border-radius: 6px; font-size: 0.8rem; cursor: pointer; font-weight: 500; }
  .btn-primary:hover { background: #5a6fd6; }
  .btn-secondary { padding: 5px 12px; background: #f0f2f5; color: #555; border: none; border-radius: 6px; font-size: 0.78rem; cursor: pointer; }
  .btn-secondary:hover { background: #e4e7ec; }
  .btn-sm { padding: 3px 9px; border: 1px solid #ddd; background: #fff; border-radius: 4px; font-size: 0.72rem; cursor: pointer; }
  .btn-sm:hover { background: #f5f5f5; }
  .btn-del { color: #ef4444; border-color: #fecaca; }
  .btn-del:hover { background: #fef2f2; }
  .btn-save { color: #16a34a; border-color: #bbf7d0; }
  .btn-save:hover { background: #f0fdf4; }
  .btn-test { color: #667eea; border-color: #c7d2fe; }
  .btn-test:hover { background: #eef2ff; }
  .act { white-space: nowrap; display: flex; align-items: center; gap: 3px; flex-wrap: wrap; }
  .test-badge { font-size: 0.7rem; font-weight: 600; padding: 1px 5px; border-radius: 3px; }
  .test-ok { color: #16a34a; background: #dcfce7; }
  .test-warn { color: #b45309; background: #fef3c7; }
  .test-fail { color: #dc2626; background: #fef2f2; }
  input, select { padding: 5px 9px; border: 1px solid #ddd; border-radius: 5px; font-size: 0.8rem; background: #fafbfc; outline: none; width: 100%; box-sizing: border-box; }
  input:focus, select:focus { border-color: #667eea; background: #fff; }
  .input-toggle { display: flex; position: relative; }
  .input-toggle input { flex: 1; padding-right: 30px; }
  .toggle-btn { position: absolute; right: 5px; top: 50%; transform: translateY(-50%); background: none; border: none; cursor: pointer; font-size: 0.9rem; padding: 0; line-height: 1; }
  .form-row { display: flex; gap: 10px; margin-bottom: 12px; flex-wrap: wrap; }
  .form-row label { display: flex; flex-direction: column; gap: 4px; font-size: 0.78rem; color: #666; flex: 1; min-width: 160px; }
  .form-actions { display: flex; gap: 8px; margin-top: 4px; }
  .overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.45); display: flex; align-items: center; justify-content: center; z-index: 100; }
  .modal { background: #fff; border-radius: 11px; width: 90vw; max-width: 660px; max-height: 82vh; display: flex; flex-direction: column; box-shadow: 0 20px 60px rgba(0,0,0,0.2); }
  .modal-head { display: flex; justify-content: space-between; align-items: center; padding: 13px 18px; border-bottom: 1px solid #e8ecf0; }
  .modal-head h3 { margin: 0; font-size: 0.95rem; }
  .modal-head button { background: none; border: none; font-size: 1.1rem; cursor: pointer; color: #888; line-height: 1; }
  .modal-head button:hover { color: #333; }
  .modal-body { padding: 18px; overflow-y: auto; flex: 1; }
  .detail-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .detail-grid h4 { margin: 0 0 6px; font-size: 0.82rem; color: #666; }
  .detail-grid pre { background: #f8f9fb; padding: 10px; border-radius: 5px; font-size: 0.7rem; overflow: auto; max-height: 44vh; margin: 0; }
  .log-list { display: flex; flex-direction: column; gap: 2px; }
  .log-row { display: flex; align-items: center; gap: 6px; padding: 5px 8px; border-radius: 5px; font-size: 0.76rem; flex-wrap: wrap; }
  .log-row:hover { background: rgba(0,0,0,0.04); }
  .log-clickable { cursor: pointer; }
  .log-time { color: #999; font-size: 0.72rem; white-space: nowrap; }
  .log-path { color: #444; font-family: monospace; font-size: 0.74rem; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .log-model { color: #667eea; font-size: 0.72rem; font-family: monospace; }
  .log-lat { color: #888; font-size: 0.7rem; white-space: nowrap; }
  .conn-info { display: flex; flex-direction: column; gap: 10px; }
  .ci-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .ci-label { font-size: 0.72rem; color: #888; font-weight: 600; text-transform: uppercase; letter-spacing: 0.4px; min-width: 130px; }
  .ci-row code { font-size: 0.78rem; background: #f0f2f5; padding: 5px 9px; border-radius: 4px; flex: 1; }
  .ci-pre-row { align-items: flex-start; }
  .ci-pre { font-size: 0.76rem; background: #f0f2f5; padding: 9px 11px; border-radius: 4px; margin: 0; flex: 1; white-space: pre-wrap; font-family: monospace; }
  .ci-arrow { color: #667eea; font-weight: bold; }
  .ci-model-row { font-size: 0.78rem; }
  .ci-divider { border: none; border-top: 1px solid #e8ecf0; margin: 4px 0; }
  .ci-section-label { font-size: 0.72rem; color: #888; font-weight: 600; text-transform: uppercase; letter-spacing: 0.4px; margin-bottom: 4px; }
  .btn-copy { padding: 4px 9px; background: #667eea; color: #fff; border: none; border-radius: 4px; font-size: 0.7rem; cursor: pointer; white-space: nowrap; }
  .btn-copy:hover { background: #5a6fd6; }
  .btn-copy-sm { padding: 2px 6px; background: #f0f2f5; color: #555; border: 1px solid #ddd; border-radius: 3px; font-size: 0.66rem; cursor: pointer; }
  .btn-copy-sm:hover { background: #e4e7ec; }
</style>
