<script lang="ts">
  import { onMount } from "svelte";
  import { tick } from "svelte";
  import {
    getConfig, saveConfig, getLogs, clearLogs, getProxyStatus,
    startProxy, stopProxy, addOpLog, testProviderModels, testMapping,
    fetchProviderModels, generateSelfSignedCert, importCcSwitchConfig,
    PRESET_PROVIDERS, PROXY_MODELS,
    type Config, type Provider, type ModelMapping, type RequestLog,
    type ProxyStatus, type ModelTestResult, type TestResult
  } from "$lib/api";

  // State
  let theme = $state<string>(typeof localStorage !== "undefined" ? (localStorage.getItem("theme") || "light") : "light");
  let config = $state<Config>({ proxy: { host: "127.0.0.1", port: 8080, log_capacity: 1000, https: false, cert_path: "", key_path: "" }, providers: [], model_mappings: [] });
  let status = $state<ProxyStatus>({ running: false, host: "", port: 0, https: false });
  let logs = $state<RequestLog[]>([]);
  let autoScroll = $state(true);
  let logsPanelOpen = $state(false);
  let leftWidth = $state(60);
  let dragging = $state(false);

  // Modals
  let showProviderModal = $state(false);
  let showMappingModal = $state(false);
  let showConnModal = $state(false);
  let showImportModal = $state(false);
  let editingProvider = $state<Provider | null>(null);
  let editingMappingIdx = $state<number | null>(null);

  // Provider form
  let pForm = $state({ preset: "", name: "", base_url: "", api_key: "", models: [] as string[], newModel: "", showKey: false });
  let pError = $state("");
  let fetchingModels = $state(false);

  // Mapping form
  let mForm = $state({ from: "", to_provider: "", to_model: "" });

  // Import
  let importJson = $state("");
  let importError = $state("");

  // Provider test results: providerId -> ModelTestResult[]
  let providerTestResults = $state<Record<string, ModelTestResult[]>>({});
  let testingProvider = $state<Record<string, boolean>>({});

  // Mapping test results: idx -> {ok, ms, msg}
  let mappingTestResult = $state<Record<number, {ok:boolean,ms:number,msg:string}>>({});
  let testingMapping = $state<Record<number, boolean>>({});

  // API key visibility per provider row
  let showApiKey = $state<Record<string, boolean>>({});

  let logContainer: HTMLElement;
  let pollInterval: ReturnType<typeof setInterval>;


  onMount(async () => {
    config = await getConfig();
    status = await getProxyStatus();
    logs = await getLogs();
    pollInterval = setInterval(async () => {
      status = await getProxyStatus();
      const newLogs = await getLogs();
      if (newLogs.length !== logs.length) {
        logs = newLogs;
        if (autoScroll) {
          await tick();
          if (logContainer) logContainer.scrollTop = logContainer.scrollHeight;
        }
      }
    }, 1000);
    return () => clearInterval(pollInterval);
  });

  $effect(() => {
    if (typeof localStorage !== 'undefined') localStorage.setItem('theme', theme);
  });

  async function doSave() { await saveConfig(config); }

  async function handleStart() {
    const info = await startProxy();
    status = await getProxyStatus();
    logsPanelOpen = true;
    await addOpLog('启动代理', `${status.https ? 'HTTPS' : 'HTTP'} ${info.host}:${info.port} 启动成功`);
  }

  async function handleStop() {
    await stopProxy();
    status = await getProxyStatus();
    await addOpLog('停止代理', '代理服务已停止');
  }

  async function handleGenCert() {
    const cert = await generateSelfSignedCert();
    config.proxy.cert_path = cert.cert_path;
    config.proxy.key_path = cert.key_path;
    await doSave();
    await addOpLog('生成证书', `证书路径: ${cert.cert_path}`);
  }

  function openAddProvider() {
    editingProvider = null;
    pForm = { preset: '', name: '', base_url: '', api_key: '', models: [], newModel: '', showKey: false };
    pError = '';
    showProviderModal = true;
  }

  function openEditProvider(p) {
    editingProvider = p;
    pForm = { preset: '', name: p.name, base_url: p.base_url, api_key: p.api_key, models: [...p.models], newModel: '', showKey: false };
    pError = '';
    showProviderModal = true;
  }

  function applyPreset(key) {
    if (!key || !PRESET_PROVIDERS[key]) return;
    const pr = PRESET_PROVIDERS[key];
    pForm.name = pr.name;
    pForm.base_url = pr.base_url;
    pForm.models = [...pr.models];
  }

  async function saveProvider() {
    if (!pForm.name.trim()) { pError = '名称不能为空'; return; }
    if (!editingProvider && config.providers.some(p => p.name === pForm.name.trim())) {
      pError = '名称已存在'; return;
    }
    if (editingProvider) {
      const idx = config.providers.findIndex(p => p.id === editingProvider.id);
      if (idx >= 0) config.providers[idx] = { ...editingProvider, name: pForm.name, base_url: pForm.base_url, api_key: pForm.api_key, models: pForm.models };
    } else {
      const id = crypto.randomUUID();
      config.providers = [...config.providers, { id, name: pForm.name, base_url: pForm.base_url, api_key: pForm.api_key, models: pForm.models }];
    }
    await doSave();
    showProviderModal = false;
  }

  async function deleteProvider(id) {
    if (!confirm('确定删除此提供商？')) return;
    config.providers = config.providers.filter(p => p.id !== id);
    await doSave();
    await addOpLog('删除提供商', config.providers.find(p => p.id === id)?.name || id);
  }

  async function testProvider(id) {
    testingProvider = { ...testingProvider, [id]: true };
    const results = await testProviderModels(id);
    providerTestResults = { ...providerTestResults, [id]: results };
    testingProvider = { ...testingProvider, [id]: false };
  }

  function addModel() {
    const m = pForm.newModel.trim();
    if (m && !pForm.models.includes(m)) pForm.models = [...pForm.models, m];
    pForm.newModel = '';
  }

  function removeModel(m) { pForm.models = pForm.models.filter(x => x !== m); }

  async function fetchModels() {
    fetchingModels = true;
    try {
      const tempId = editingProvider?.id || '__temp__';
      const models = await fetchProviderModels(tempId, pForm.api_key);
      pForm.models = models;
    } catch(e) { pError = String(e); }
    fetchingModels = false;
  }

  function openAddMapping() {
    editingMappingIdx = null;
    mForm = { from: '', to_provider: '', to_model: '' };
    showMappingModal = true;
  }

  function openEditMapping(idx) {
    editingMappingIdx = idx;
    mForm = { ...config.model_mappings[idx] };
    showMappingModal = true;
  }

  async function saveMapping() {
    if (!mForm.from || !mForm.to_provider || !mForm.to_model) return;
    if (editingMappingIdx !== null) {
      config.model_mappings[editingMappingIdx] = { ...mForm };
    } else {
      config.model_mappings = [...config.model_mappings, { ...mForm }];
    }
    await doSave();
    showMappingModal = false;
  }

  async function deleteMapping(idx) {
    if (!confirm('确定删除此映射？')) return;
    const m = config.model_mappings[idx];
    config.model_mappings = config.model_mappings.filter((_, i) => i !== idx);
    await doSave();
    await addOpLog('删除映射', m.from);
  }

  async function testMappingBtn(idx) {
    testingMapping = { ...testingMapping, [idx]: true };
    try {
      const r = await testMapping(idx);
      mappingTestResult = { ...mappingTestResult, [idx]: { ok: r.success, ms: r.latency_ms, msg: r.message } };
    } catch(e) {
      mappingTestResult = { ...mappingTestResult, [idx]: { ok: false, ms: 0, msg: String(e) } };
    }
    testingMapping = { ...testingMapping, [idx]: false };
    setTimeout(() => {
      const copy = { ...mappingTestResult };
      delete copy[idx];
      mappingTestResult = copy;
    }, 5000);
  }

  async function handleClearLogs() { await clearLogs(); logs = []; }

  async function handleImport() {
    importError = '';
    try {
      config = await importCcSwitchConfig(importJson);
      await doSave();
      showImportModal = false;
      importJson = '';
    } catch(e) { importError = String(e); }
  }

  function startDrag(e) {
    dragging = true;
    const startX = e.clientX;
    const startW = leftWidth;
    const total = window.innerWidth;
    function onMove(ev) {
      const delta = ev.clientX - startX;
      const newW = Math.max(300, Math.min(total - 340, (startW / 100) * total + delta));
      leftWidth = (newW / total) * 100;
    }
    function onUp() {
      dragging = false;
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    }
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function formatTime(ts) {
    return new Date(ts).toTimeString().slice(0, 8);
  }

  function formatLog(log) {
    const t = formatTime(log.timestamp);
    if (log.method === 'OP') {
      const detail = (typeof log.response_body === 'string' && log.response_body) ? log.response_body : (log.model_in || '');
      return `[${t}] [操作] ${log.path} | ${detail} | ✓`;
    }
    if (log.latency_ms === 0 && log.status === 200) return `[${t}] [流式] ${log.method} ${log.path} | ${log.model_in} → ${log.model_out} | ${log.provider} | streaming`;
    const tokens = (log.prompt_tokens != null || log.completion_tokens != null) ? ` | ${log.prompt_tokens ?? 0}/${log.completion_tokens ?? 0} tokens` : '';
    return `[${t}] [请求] ${log.method} ${log.path} | ${log.model_in} → ${log.model_out} | ${log.provider} | HTTP ${log.status} | ${log.latency_ms}ms${tokens}`;
  }

  function getProviderById(id) { return config.providers.find(p => p.id === id); }

  function modelTestColor(providerId, model) {
    const results = providerTestResults[providerId];
    if (!results) return '';
    const r = results.find(x => x.model === model);
    if (!r) return '';
    return r.success ? 'background:#166534;color:#bbf7d0' : 'background:#7f1d1d;color:#fecaca';
  }

  function connBase() {
    const proto = status.https ? 'https' : 'http';
    return `${proto}://${status.host}:${status.port}`;
  }

  async function copyText(text) { await navigator.clipboard.writeText(text); }

  let mappingProviderModels = $derived(
    mForm.to_provider ? (getProviderById(mForm.to_provider)?.models ?? []) : []
  );
</script>

<div class="app" data-theme={theme}>
  <header>
    <span class="title">CC Proxy</span>
    <div class="hright">
      {#if status.running}
        <div class="running-badge">
          <span class="pulse-ring"></span>
          <span class="pulse-dot"></span>
          <span class="run-info">运行中 · 端口 {status.port} · {status.https ? 'HTTPS' : 'HTTP'}</span>
        </div>
        <button class="btn-conn" onclick={() => showConnModal = true}>连接信息</button>
        <button class="btn-danger" onclick={handleStop}>停止</button>
      {:else}
        <button class="btn-primary" onclick={handleStart}>&#9654; 启动</button>
      {/if}
      <button class="btn-icon" onclick={() => theme = theme === 'dark' ? 'light' : 'dark'}>
        {theme === 'dark' ? '☀️' : '🌙'}
      </button>
    </div>
  </header>
  <div class="main">
    <div class="panel left-panel" style="width:{leftWidth}%">
      <section>
        <h3>代理设置</h3>
        <div class="form-row">
          <label>端口</label>
          <input type="number" bind:value={config.proxy.port} onchange={doSave} />
        </div>
        <div class="form-row">
          <label>HTTPS</label>
          <input type="checkbox" bind:checked={config.proxy.https} onchange={doSave} />
          <span class="hint">启用后跳过证书校验，默认HTTP</span>
        </div>
      </section>
      <section>
        <div class="sec-header">
          <h3>提供商</h3>
          <div>
            <button onclick={() => showImportModal = true}>CC-Switch 导入</button>
            <button class="btn-primary" onclick={openAddProvider}>添加提供商</button>
          </div>
        </div>
        <table>
          <thead><tr><th>名称</th><th>Base URL</th><th>API Key</th><th>模型</th><th>操作</th></tr></thead>
          <tbody>
            {#each config.providers as p (p.id)}
              <tr>
                <td>{p.name}</td>
                <td class="url-cell">{p.base_url}</td>
                <td class="key-cell">
                  <span>{showApiKey[p.id] ? p.api_key : '••••••••'}</span>
                  <button class="btn-icon" onclick={() => showApiKey = { ...showApiKey, [p.id]: !showApiKey[p.id] }}>👁</button>
                </td>
                <td>
                  {#if providerTestResults[p.id]}
                    {#each p.models as m}
                      <span class="model-tag" style={modelTestColor(p.id, m)}>{m}</span>
                    {/each}
                  {:else}
                    {p.models.length > 3 ? `${p.models.length}个模型` : p.models.join(', ')}
                  {/if}
                </td>
                <td class="ops">
                  <button onclick={() => testProvider(p.id)} disabled={testingProvider[p.id]}>
                    {testingProvider[p.id] ? '测试中...' : '测试'}
                  </button>
                  <button onclick={() => openEditProvider(p)}>编辑</button>
                  <button class="btn-danger" onclick={() => deleteProvider(p.id)}>删除</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
      <section>
        <div class="sec-header">
          <h3>模型映射</h3>
          <button class="btn-primary" onclick={openAddMapping}>添加映射</button>
        </div>
        <table>
          <thead><tr><th>代理模型名</th><th>目标提供商</th><th>原始模型</th><th>操作</th></tr></thead>
          <tbody>
            {#each config.model_mappings as m, i (i)}
              <tr>
                <td>{m.from}</td>
                <td>{getProviderById(m.to_provider)?.name ?? m.to_provider}</td>
                <td>{m.to_model}</td>
                <td class="ops">
                  {#if mappingTestResult[i]}
                    <button style="color:{mappingTestResult[i].ok ? '#4ade80' : '#f87171'}">
                      {mappingTestResult[i].ok ? `✓ ${mappingTestResult[i].ms}ms` : `✗ ${mappingTestResult[i].msg}`}
                    </button>
                  {:else}
                    <button onclick={() => testMappingBtn(i)} disabled={testingMapping[i]}>
                      {testingMapping[i] ? '测试中...' : '测试'}
                    </button>
                  {/if}
                  <button onclick={() => openEditMapping(i)}>编辑</button>
                  <button class="btn-danger" onclick={() => deleteMapping(i)}>删除</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    </div>
    <div class="splitter" onmousedown={startDrag}></div>
    <div class="panel right-panel" style="width:{logsPanelOpen ? `calc(${100 - leftWidth}% - 4px)` : '40px'}">
      {#if !logsPanelOpen}
        <button class="collapse-btn" onclick={() => logsPanelOpen = true}>
          <span class="vert-text">日志</span>
          <span>◀</span>
        </button>
      {:else}
        <div class="log-header">
          <span class="log-title">📋 日志</span>
          <div>
            <label class="auto-label"><input type="checkbox" bind:checked={autoScroll} /> 自动滚动</label>
            <button onclick={handleClearLogs}>清空</button>
            <button onclick={() => logsPanelOpen = false}>收起 ▶</button>
          </div>
        </div>
        <div class="log-body" bind:this={logContainer}>
          {#each logs as log (log.id)}
            <div class="log-line">{formatLog(log)}</div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  {#if showProviderModal}
    <div class="modal-overlay" onclick={() => showProviderModal = false}>
      <div class="modal" onclick={(e) => e.stopPropagation()}>
        <h3>{editingProvider ? '编辑提供商' : '添加提供商'}</h3>
        <div class="form-row">
          <label>预设</label>
          <select bind:value={pForm.preset} onchange={() => applyPreset(pForm.preset)}>
            <option value="">选择预设</option>
            {#each Object.entries(PRESET_PROVIDERS) as [k, v]}
              <option value={k}>{v.name}</option>
            {/each}
          </select>
        </div>
        <div class="form-row">
          <label>名称</label>
          <input bind:value={pForm.name} />
        </div>
        <div class="form-row">
          <label>Base URL</label>
          <input bind:value={pForm.base_url} />
        </div>
        <div class="form-row">
          <label>API Key</label>
          <input type={pForm.showKey ? 'text' : 'password'} bind:value={pForm.api_key} />
          <button class="btn-icon" onclick={() => pForm.showKey = !pForm.showKey}>👁</button>
        </div>
        <div class="form-row">
          <label>模型</label>
          <div class="models-area">
            <div class="model-tags">
              {#each pForm.models as m}
                <span class="model-tag">{m}<button onclick={() => removeModel(m)}>×</button></span>
              {/each}
            </div>
            <div class="model-add">
              <input bind:value={pForm.newModel} placeholder="添加模型" onkeydown={(e) => e.key === 'Enter' && addModel()} />
              <button onclick={addModel}>添加</button>
              <button onclick={fetchModels} disabled={fetchingModels}>{fetchingModels ? '拉取中...' : '拉取模型'}</button>
            </div>
          </div>
        </div>
        {#if pError}<div class="error">{pError}</div>{/if}
        <div class="modal-footer">
          <button onclick={() => showProviderModal = false}>取消</button>
          <button class="btn-primary" onclick={saveProvider}>保存</button>
        </div>
      </div>
    </div>
  {/if}

  {#if showMappingModal}
    <div class="modal-overlay" onclick={() => showMappingModal = false}>
      <div class="modal" onclick={(e) => e.stopPropagation()}>
        <h3>{editingMappingIdx !== null ? '编辑映射' : '添加映射'}</h3>
        <div class="form-row">
          <label>代理模型名</label>
          <input list="proxy-models" bind:value={mForm.from} />
          <datalist id="proxy-models">
            {#each PROXY_MODELS as m}
              <option value={m}></option>
            {/each}
          </datalist>
        </div>
        <div class="form-row">
          <label>目标提供商</label>
          <select bind:value={mForm.to_provider} onchange={() => mForm.to_model = ''}>
            <option value="">选择提供商</option>
            {#each config.providers as p}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </div>
        <div class="form-row">
          <label>原始模型</label>
          <select bind:value={mForm.to_model}>
            <option value="">选择模型</option>
            {#each mappingProviderModels as m}
              <option value={m}>{m}</option>
            {/each}
          </select>
        </div>
        <div class="modal-footer">
          <button onclick={() => showMappingModal = false}>取消</button>
          <button class="btn-primary" onclick={saveMapping}>保存</button>
        </div>
      </div>
    </div>
  {/if}

  {#if showConnModal}
    <div class="modal-overlay" onclick={() => showConnModal = false}>
      <div class="modal" onclick={(e) => e.stopPropagation()}>
        <h3>连接信息</h3>
        {#each [['Base URL', connBase()], ['Chat', connBase() + '/v1/chat/completions'], ['Responses', connBase() + '/v1/responses'], ['Models', connBase() + '/v1/models']] as [label, url]}
          <div class="form-row">
            <label>{label}</label>
            <code>{url}</code>
            <button onclick={() => copyText(url)}>复制</button>
          </div>
        {/each}
        <h4>模型映射</h4>
        {#each config.model_mappings as m}
          <div class="map-row">{m.from} → {getProviderById(m.to_provider)?.name} / {m.to_model}</div>
        {/each}
        <h4>配置示例</h4>
        <pre class="code-block">base_url: {connBase()}/v1
api_key: any</pre>
        <div class="modal-footer">
          <button onclick={() => showConnModal = false}>关闭</button>
        </div>
      </div>
    </div>
  {/if}

  {#if showImportModal}
    <div class="modal-overlay" onclick={() => showImportModal = false}>
      <div class="modal" onclick={(e) => e.stopPropagation()}>
        <h3>导入 CC-Switch 配置</h3>
        <textarea bind:value={importJson} rows="10" placeholder="粘贴 JSON 配置..."></textarea>
        {#if importError}<div class="error">{importError}</div>{/if}
        <div class="modal-footer">
          <button onclick={() => showImportModal = false}>取消</button>
          <button class="btn-primary" onclick={handleImport}>导入</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    font-family: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    font-size: 13px;
    -webkit-font-smoothing: antialiased;
  }
  .app[data-theme="dark"] {
    --bg: #0f172a;
    --card: #1e293b;
    --text: #e2e8f0;
    --text-dim: #94a3b8;
    --border: #334155;
    --btn: #334155;
    --btn-hover: #475569;
    --input-bg: #0f172a;
    --accent: #6366f1;
    --accent-hover: #4f46e5;
  }
  .app[data-theme="light"] {
    --bg: #f1f5f9;
    --card: #fff;
    --text: #1e293b;
    --text-dim: #64748b;
    --border: #e2e8f0;
    --btn: #f1f5f9;
    --btn-hover: #e2e8f0;
    --input-bg: #fff;
    --accent: #6366f1;
    --accent-hover: #4f46e5;
  }
  .app { background: var(--bg); color: var(--text); }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    background: var(--card);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .title { font-size: 16px; font-weight: 700; background: linear-gradient(135deg, #6366f1, #8b5cf6); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
  .hright { display: flex; align-items: center; gap: 8px; }
  .main { display: flex; flex: 1; overflow: hidden; }
  .panel { overflow-y: auto; padding: 12px; }
  .left-panel { flex-shrink: 0; }
  .right-panel { flex-shrink: 0; display: flex; flex-direction: column; background: #0d1117; color: #c9d1d9; transition: width 0.2s; }
  .splitter { width: 4px; background: var(--border); cursor: col-resize; flex-shrink: 0; }
  .splitter:hover { background: #3b82f6; }
  section { background: var(--card); border: 1px solid var(--border); border-radius: 10px; padding: 16px; margin-bottom: 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }
  h3 { margin: 0 0 10px; font-size: 13px; font-weight: 600; letter-spacing: -0.2px; }
  h4 { margin: 10px 0 6px; font-size: 12px; color: var(--text-dim); }
  .sec-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
  .sec-header h3 { margin: 0; }
  .sec-header div { display: flex; gap: 6px; }
  .form-row { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
  .form-row label { min-width: 70px; font-size: 12px; color: var(--text-dim); font-weight: 500; }
  .hint { font-size: 11px; color: var(--text-dim); }
  input[type="text"], input[type="number"], input[type="password"], select, textarea {
    background: var(--input-bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 12px;
    flex: 1;
    transition: border-color 0.15s;
  }
  input:focus, select:focus, textarea:focus { border-color: var(--accent); outline: none; }
  input[type="checkbox"] { width: auto; flex: none; accent-color: var(--accent); }
  textarea { width: 100%; box-sizing: border-box; resize: vertical; }
  button {
    background: var(--btn);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 12px;
    cursor: pointer;
    font-size: 12px;
    white-space: nowrap;
    font-weight: 500;
    transition: all 0.15s;
  }
  button:hover { background: var(--btn-hover); }
  button:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-primary { background: var(--accent); color: #fff; border-color: var(--accent); }
  .btn-primary:hover { background: var(--accent-hover); }
  .btn-danger { background: #dc2626; color: #fff; border-color: #dc2626; font-size: 11px; padding: 3px 8px; }
  .btn-danger:hover { background: #b91c1c; }
  .btn-icon { background: transparent; border: none; padding: 2px 6px; cursor: pointer; opacity: 0.7; }
  .btn-icon:hover { opacity: 1; }
  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  th, td { padding: 8px 10px; border-bottom: 1px solid var(--border); text-align: left; }
  th { font-weight: 600; color: var(--text-dim); font-size: 11px; text-transform: uppercase; letter-spacing: 0.3px; }
  .url-cell { max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-dim); font-size: 11px; }
  .key-cell { display: flex; align-items: center; gap: 4px; font-family: monospace; font-size: 11px; }
  .ops { display: flex; gap: 4px; white-space: nowrap; }
  .model-tag { display: inline-block; background: var(--btn); border-radius: 4px; padding: 2px 8px; margin: 2px; font-size: 11px; border: 1px solid var(--border); }
  .model-tag button { background: none; border: none; padding: 0 3px; cursor: pointer; font-size: 11px; color: var(--text-dim); }
  .model-tag button:hover { color: #ef4444; }
  .models-area { flex: 1; }
  .model-tags { display: flex; flex-wrap: wrap; margin-bottom: 6px; }
  .model-add { display: flex; gap: 6px; }
  .running-badge {
    display: flex; align-items: center; gap: 8px;
    background: linear-gradient(135deg, #065f46, #047857);
    padding: 4px 14px; border-radius: 20px; position: relative;
    box-shadow: 0 0 12px rgba(16, 185, 129, 0.4);
  }
  .pulse-ring {
    position: absolute; left: 10px; width: 16px; height: 16px; border-radius: 50%;
    border: 2px solid #34d399; animation: ring 2s infinite;
  }
  @keyframes ring {
    0% { transform: scale(0.8); opacity: 1; }
    100% { transform: scale(1.8); opacity: 0; }
  }
  .pulse-dot {
    width: 10px; height: 10px; border-radius: 50%;
    background: #34d399;
    box-shadow: 0 0 8px #34d399;
    animation: pulse 1.5s infinite;
    display: inline-block;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
  .run-info { font-size: 13px; color: #d1fae5; font-weight: 500; }
  .btn-conn { background: #1e40af; color: #bfdbfe; border-color: #1e40af; border-radius: 4px; }
  .btn-conn:hover { background: #1d4ed8; }
  .collapse-btn {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    height: 100%; width: 40px; background: transparent; border: none; cursor: pointer;
    color: #c9d1d9; gap: 8px;
  }
  .vert-text { writing-mode: vertical-rl; font-size: 13px; }
  .log-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 8px 12px; border-bottom: 1px solid #30363d; flex-shrink: 0;
  }
  .log-title { font-size: 12px; font-weight: 600; color: #e6edf3; }
  .log-header div { display: flex; align-items: center; gap: 8px; }
  .log-header button { background: #21262d; color: #c9d1d9; border-color: #30363d; font-size: 11px; padding: 3px 8px; border-radius: 4px; }
  .log-header button:hover { background: #30363d; }
  .auto-label { font-size: 11px; color: #8b949e; display: flex; align-items: center; gap: 3px; cursor: pointer; }
  .log-body { flex: 1; overflow-y: auto; padding: 8px 12px; font-family: "SF Mono", "Fira Code", Consolas, monospace; font-size: 11px; line-height: 1.6; }
  .log-line { padding: 1px 0; white-space: pre-wrap; word-break: break-all; color: #8b949e; }
  .log-line:hover { color: #e6edf3; }
  .modal-overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.6);
    display: flex; align-items: center; justify-content: center; z-index: 100;
  }
  .modal {
    background: var(--card); border: 1px solid var(--border); border-radius: 12px;
    padding: 24px; min-width: 480px; max-width: 600px; max-height: 80vh; overflow-y: auto;
    box-shadow: 0 20px 60px rgba(0,0,0,0.3);
  }
  .modal h3 { margin: 0 0 16px; font-size: 15px; }
  .modal-footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  .error { color: #f87171; font-size: 13px; margin: 4px 0; }
  code { font-family: monospace; font-size: 12px; background: var(--btn); padding: 2px 6px; border-radius: 3px; flex: 1; }
  .map-row { font-size: 13px; padding: 3px 0; }
  pre.code-block { background: var(--btn); padding: 8px; border-radius: 4px; font-size: 12px; overflow-x: auto; }
</style>
