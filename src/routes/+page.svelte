<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    getConfig, saveConfig, getLogs, clearLogs, getProxyStatus,
    startProxy, stopProxy, addOpLog, testProviderModels, testMapping,
    fetchProviderModels, generateSelfSignedCert, importCcSwitchConfig,
    PRESET_PROVIDERS, PROXY_MODELS,
    type Config, type Provider, type ModelMapping, type RequestLog,
    type ProxyStatus, type ProxyInfo, type TestResult, type ModelTestResult, type CertInfo
  } from "$lib/api";

  // ── theme ────────────────────────────────────────────────────────────────────
  let theme = $state<"dark" | "light">("dark");

  // ── config / proxy state ─────────────────────────────────────────────────────
  let config = $state<Config | null>(null);
  let logs = $state<RequestLog[]>([]);
  let status = $state<ProxyStatus | null>(null);
  let loading = $state(false);
  let toast = $state("");

  // ── log panel ────────────────────────────────────────────────────────────────
  let logsExpanded = $state(false);
  let logContainer = $state<HTMLElement | null>(null);
  let autoScroll = $state(true);
  let selectedLog = $state<RequestLog | null>(null);

  // ── provider modal ───────────────────────────────────────────────────────────
  let showProviderModal = $state(false);
  let providerPreset = $state("deepseek");
  let editingProviderIdx = $state<number | null>(null);
  let providerForm = $state<Provider>({ id: "", name: "", base_url: "", api_key: "", models: [] });
  let showProviderKey = $state(false);
  let fetchingModels = $state(false);
  let testingAllModels = $state(false);
  let modelTestResults = $state<ModelTestResult[]>([]);

  // ── provider inline edit ─────────────────────────────────────────────────────
  let inlineProviderIdx = $state<number | null>(null);
  let inlineProviderForm = $state<Provider>({ id: "", name: "", base_url: "", api_key: "", models: [] });
  let showInlineKey = $state(false);
  let inlineFetchingModels = $state(false);

  // ── provider key visibility per row ───────────────────────────────────────────
  let visibleKeys = $state<Record<number, boolean>>({});

  // ── mapping modal ────────────────────────────────────────────────────────────
  let showMappingModal = $state(false);
  let editingMappingIdx = $state<number | null>(null);
  let mappingForm = $state<ModelMapping>({ from: "", to_provider: "", to_model: "" });
  let mappingFetchedModels = $state<string[]>([]);
  let mappingFetchingModels = $state(false);

  // ── mapping inline edit ──────────────────────────────────────────────────────
  let inlineMappingIdx = $state<number | null>(null);
  let inlineMappingForm = $state<ModelMapping>({ from: "", to_provider: "", to_model: "" });
  let inlineMappingFetchedModels = $state<string[]>([]);

  // ── connection info modal ────────────────────────────────────────────────────
  let showConnInfo = $state(false);

  // ── import cc-switch dialog ──────────────────────────────────────────────────
  let showImportDialog = $state(false);
  let importJson = $state("");

  // ── test states ──────────────────────────────────────────────────────────────
  let testingMapping = $state<number | null>(null);
  let mappingBtnLabel = $state<Record<number, string>>({});

  // ── cert info ────────────────────────────────────────────────────────────────
  let certInfo = $state<CertInfo | null>(null);

  // ── lifecycle ────────────────────────────────────────────────────────────────
  onMount(() => {
    const saved = localStorage.getItem("theme");
    if (saved === "light" || saved === "dark") theme = saved;
    loadAll();
    const iv = setInterval(refresh, 3000);
    return () => clearInterval(iv);
  });

  $effect(() => {
    localStorage.setItem("theme", theme);
  });

  $effect(() => {
    // Force reactivity on logs change
    logs;
    tick().then(() => {
      if (logContainer && autoScroll) {
        logContainer.scrollTop = logContainer.scrollHeight;
      }
    });
  });

  function onLogScroll() {
    if (!logContainer) return;
    const el = logContainer;
    autoScroll = (el.scrollHeight - el.scrollTop - el.clientHeight) < 50;
  }

  async function loadAll() {
    try {
      [config, status, logs] = await Promise.all([getConfig(), getProxyStatus(), getLogs(200)]);
      if (config?.proxy.cert_path) {
        certInfo = { cert_path: config.proxy.cert_path, key_path: config.proxy.key_path, generated: true };
      }
    } catch (e) { console.error(e); }
  }

  async function refresh() {
    try {
      status = await getProxyStatus();
      logs = await getLogs(200);
    } catch {}
  }

  function showToast(msg: string) { toast = msg; setTimeout(() => toast = "", 3000); }

  // ── proxy control ────────────────────────────────────────────────────────────
  async function handleStart() {
    loading = true;
    try {
      await startProxy();
      status = await getProxyStatus();
      await addOpLog("启动代理", `端口 ${status?.port}`);
      logsExpanded = true;
      showToast("代理已启动");
    } catch (e: any) { showToast("启动失败: " + e); }
    loading = false;
  }

  async function handleStop() {
    loading = true;
    try {
      await stopProxy();
      await addOpLog("停止代理", "");
      status = await getProxyStatus();
      showToast("代理已停止");
    } catch (e: any) { showToast("停止失败: " + e); }
    loading = false;
  }

  async function onPortChange() {
    if (!config) return;
    await saveConfig(config);
    await addOpLog("修改端口", String(config.proxy.port));
  }

  async function onHttpsToggle() {
    if (!config) return;
    config.proxy.https = !config.proxy.https;
    await saveConfig(config);
    await addOpLog(config.proxy.https ? "启用HTTPS" : "禁用HTTPS", "");
  }

  async function handleGenerateCert() {
    try {
      certInfo = await generateSelfSignedCert();
      if (config) {
        config.proxy.cert_path = certInfo.cert_path;
        config.proxy.key_path = certInfo.key_path;
        await saveConfig(config);
      }
      showToast("证书已生成");
    } catch (e: any) { showToast("生成证书失败: " + e); }
  }

  // ── provider modal ───────────────────────────────────────────────────────────
  function openAddProvider() {
    editingProviderIdx = null;
    providerPreset = "deepseek";
    const p = PRESET_PROVIDERS["deepseek"];
    providerForm = { id: "deepseek", name: p.name, base_url: p.base_url, api_key: "", models: [...p.models] };
    showProviderKey = false;
    modelTestResults = [];
    showProviderModal = true;
  }

  function openEditProvider(idx: number) {
    if (!config) return;
    editingProviderIdx = idx;
    const prov = config.providers[idx];
    const presetKey = Object.keys(PRESET_PROVIDERS).find(
      k => k !== "custom" && PRESET_PROVIDERS[k].name === prov.name
    );
    providerPreset = presetKey || "custom";
    providerForm = { ...prov, models: [...prov.models] };
    showProviderKey = false;
    modelTestResults = [];
    showProviderModal = true;
  }

  function onPresetChange() {
    const p = PRESET_PROVIDERS[providerPreset];
    if (!p) return;
    if (providerPreset === "custom") {
      providerForm = { id: "", name: providerForm.name, base_url: "", api_key: providerForm.api_key, models: providerForm.models };
    } else {
      providerForm = {
        id: providerPreset,
        name: p.name,
        base_url: p.base_url,
        api_key: providerForm.api_key,
        models: [...p.models]
      };
    }
  }

  async function doFetchModels() {
    if (!providerForm.api_key || !providerForm.base_url) {
      showToast("请先填写 API Key 和 Base URL");
      return;
    }
    fetchingModels = true;
    try {
      const models = await fetchProviderModels(providerForm.id, providerForm.api_key);
      providerForm.models = models;
      showToast(`拉取到 ${models.length} 个模型`);
    } catch (e: any) {
      showToast("拉取模型失败: " + e);
    }
    fetchingModels = false;
  }

  async function doTestAllModels() {
    if (!providerForm.id) return;
    testingAllModels = true;
    modelTestResults = [];
    try {
      modelTestResults = await testProviderModels(providerForm.id);
    } catch (e: any) {
      showToast("测试失败: " + e);
    }
    testingAllModels = false;
  }

  function addModelTag() {
    const input = (document.getElementById("modal-add-model-input") as HTMLInputElement);
    if (!input) return;
    const v = input.value.trim();
    if (v && !providerForm.models.includes(v)) {
      providerForm.models = [...providerForm.models, v];
    }
    input.value = "";
  }

  function removeModelTag(model: string) {
    providerForm.models = providerForm.models.filter(m => m !== model);
  }

  async function saveProviderModal() {
    if (!config) return;
    const name = providerForm.name.trim();
    if (!name || !providerForm.base_url || !providerForm.api_key) {
      showToast("请填写完整信息");
      return;
    }

    if (editingProviderIdx === null) {
      const dup = config.providers.some(p => p.name.toLowerCase() === name.toLowerCase());
      if (dup) { showToast("提供商名称已存在"); return; }
    }

    const id = providerForm.id || name.toLowerCase().replace(/\s+/g, "_");
    const entry: Provider = { ...providerForm, id, name, models: [...providerForm.models] };
    if (editingProviderIdx !== null) {
      config.providers[editingProviderIdx] = entry;
      await saveConfig(config);
      await addOpLog("编辑提供商", name);
      showToast("已更新");
    } else {
      config.providers = [...config.providers, entry];
      await saveConfig(config);
      await addOpLog("添加提供商", name);
      showToast("已添加");
    }
    showProviderModal = false;
  }

  // ── provider inline edit ─────────────────────────────────────────────────────
  function startInlineProvider(idx: number) {
    if (!config) return;
    inlineProviderIdx = idx;
    inlineProviderForm = { ...config.providers[idx], models: [...config.providers[idx].models] };
    showInlineKey = false;
  }

  function addInlineModelTag(idx: number) {
    const input = (document.getElementById(`inline-add-model-${idx}`) as HTMLInputElement);
    if (!input) return;
    const v = input.value.trim();
    if (v && !inlineProviderForm.models.includes(v)) {
      inlineProviderForm.models = [...inlineProviderForm.models, v];
    }
    input.value = "";
  }

  function removeInlineModelTag(model: string) {
    inlineProviderForm.models = inlineProviderForm.models.filter(m => m !== model);
  }

  async function saveInlineProvider() {
    if (!config || inlineProviderIdx === null) return;
    const name = inlineProviderForm.name.trim();
    if (!name || !inlineProviderForm.base_url || !inlineProviderForm.api_key) {
      showToast("请填写完整信息"); return;
    }

    config.providers[inlineProviderIdx] = { ...inlineProviderForm, models: [...inlineProviderForm.models] };
    await saveConfig(config);
    await addOpLog("编辑提供商", name);
    inlineProviderIdx = null;
    showToast("已保存");
  }

  async function deleteProvider(idx: number) {
    if (!config) return;
    const name = config.providers[idx].name;
    config.providers = config.providers.filter((_, i) => i !== idx);
    const newVis: Record<number, boolean> = {};
    for (const k of Object.keys(visibleKeys)) {
      const ki = parseInt(k);
      if (ki > idx) newVis[ki - 1] = visibleKeys[ki] ?? false;
      else if (ki < idx) newVis[ki] = visibleKeys[ki] ?? false;
    }
    visibleKeys = newVis;
    await saveConfig(config);
    await addOpLog("删除提供商", name);
    showToast("已删除");
  }

  // ── mapping modal ────────────────────────────────────────────────────────────
  function openAddMapping() {
    editingMappingIdx = null;
    const firstId = config?.providers[0]?.id || "";
    mappingForm = { from: "", to_provider: firstId, to_model: "" };
    mappingFetchedModels = [];
    mappingFetchingModels = false;
    showMappingModal = true;
    // Auto-fetch if we have a provider with api key
    if (firstId) autoFetchMappingModels(firstId);
  }

  function openEditMapping(idx: number) {
    if (!config) return;
    editingMappingIdx = idx;
    mappingForm = { ...config.model_mappings[idx] };
    mappingFetchedModels = [];
    mappingFetchingModels = false;
    showMappingModal = true;
    if (mappingForm.to_provider) autoFetchMappingModels(mappingForm.to_provider);
  }

  async function autoFetchMappingModels(providerId: string) {
    const prov = config?.providers.find(p => p.id === providerId);
    if (!prov || !prov.api_key) return;
    mappingFetchingModels = true;
    try {
      mappingFetchedModels = await fetchProviderModels(providerId, prov.api_key);
    } catch { mappingFetchedModels = []; }
    mappingFetchingModels = false;
  }

  async function onMappingProviderChange() {
    // Fetch models from the selected provider
    const prov = config?.providers.find(p => p.id === mappingForm.to_provider);
    if (!prov || !prov.api_key) {
      mappingFetchedModels = prov?.models ?? [];
      return;
    }
    mappingFetchingModels = true;
    try {
      mappingFetchedModels = await fetchProviderModels(mappingForm.to_provider, prov.api_key);
    } catch { mappingFetchedModels = prov.models; }
    mappingFetchingModels = false;
  }

  async function saveMappingModal() {
    if (!config || !mappingForm.from || !mappingForm.to_provider || !mappingForm.to_model) {
      showToast("请填写完整信息"); return;
    }
    if (editingMappingIdx !== null) {
      config.model_mappings[editingMappingIdx] = { ...mappingForm };
      await saveConfig(config);
      await addOpLog("编辑映射", mappingForm.from);
      showToast("已更新");
    } else {
      config.model_mappings = [...config.model_mappings, { ...mappingForm }];
      await saveConfig(config);
      await addOpLog("添加映射", mappingForm.from);
      showToast("已添加");
    }
    showMappingModal = false;
  }

  // ── mapping inline edit ──────────────────────────────────────────────────────
  function startInlineMapping(idx: number) {
    if (!config) return;
    inlineMappingIdx = idx;
    inlineMappingForm = { ...config.model_mappings[idx] };
    inlineMappingFetchedModels = [];
    // Try to prefetch
    const prov = config.providers.find(p => p.id === inlineMappingForm.to_provider);
    inlineMappingFetchedModels = prov?.models ?? [];
  }

  async function onInlineMappingProviderChange() {
    const prov = config?.providers.find(p => p.id === inlineMappingForm.to_provider);
    if (!prov || !prov.api_key) {
      inlineMappingFetchedModels = prov?.models ?? [];
      return;
    }
    try {
      inlineMappingFetchedModels = await fetchProviderModels(inlineMappingForm.to_provider, prov.api_key);
    } catch { inlineMappingFetchedModels = prov.models; }
  }

  async function saveInlineMapping() {
    if (!config || inlineMappingIdx === null) return;
    if (!inlineMappingForm.from || !inlineMappingForm.to_provider || !inlineMappingForm.to_model) {
      showToast("请填写完整信息"); return;
    }
    config.model_mappings[inlineMappingIdx] = { ...inlineMappingForm };
    await saveConfig(config);
    await addOpLog("编辑映射", inlineMappingForm.from);
    inlineMappingIdx = null;
    showToast("已保存");
  }

  async function deleteMapping(idx: number) {
    if (!config) return;
    const from = config.model_mappings[idx].from;
    config.model_mappings = config.model_mappings.filter((_, i) => i !== idx);
    await saveConfig(config);
    await addOpLog("删除映射", from);
    showToast("已删除");
  }

  // ── test mapping ─────────────────────────────────────────────────────────────
  async function handleTestMapping(idx: number) {
    testingMapping = idx;
    let r: TestResult;
    try { r = await testMapping(idx); }
    catch (e: any) { r = { success: false, message: String(e), latency_ms: 0 }; }
    testingMapping = null;
    const label = r.success
      ? (r.latency_ms <= 2000 ? `OK ${r.latency_ms}ms` : `SLOW ${r.latency_ms}ms`)
      : `FAIL ${r.message.slice(0, 20)}`;
    mappingBtnLabel[idx] = label;
    setTimeout(() => { mappingBtnLabel[idx] = ""; }, 5000);
  }

  function mapTestBtnClass(idx: number) {
    const label = mappingBtnLabel[idx];
    if (!label) return "btn-sm btn-test";
    if (label.startsWith("OK")) return "btn-sm btn-test test-ok-btn";
    if (label.startsWith("SLOW")) return "btn-sm btn-test test-warn-btn";
    return "btn-sm btn-test test-fail-btn";
  }

  function modelTestClass(r: ModelTestResult) {
    if (r.success) return r.latency_ms <= 2000 ? "mt-ok" : "mt-warn";
    return "mt-fail";
  }

  // ── logs ─────────────────────────────────────────────────────────────────────
  async function handleClearLogs() { await clearLogs(); logs = []; }

  // ── copy ─────────────────────────────────────────────────────────────────────
  async function copy(text: string) {
    try { await navigator.clipboard.writeText(text); } catch {}
    showToast("已复制");
  }

  // ── import cc-switch ─────────────────────────────────────────────────────────
  async function handleImport() {
    if (!importJson.trim()) { showToast("请输入配置 JSON"); return; }
    try {
      config = await importCcSwitchConfig(importJson.trim());
      showToast("配置已导入");
      showImportDialog = false;
      importJson = "";
    } catch (e: any) { showToast("导入失败: " + e); }
  }

  function openImportDialog() {
    importJson = "";
    showImportDialog = true;
  }

  // ── helpers ──────────────────────────────────────────────────────────────────
  function getProviderName(id: string): string {
    return config?.providers.find(p => p.id === id)?.name ?? id;
  }
</script>

<div class="app" data-theme={theme}>
  {#if toast}<div class="toast">{toast}</div>{/if}

  <!-- ── Header ── -->
  <header>
    <div class="header-left">
      <h1>CC Proxy</h1>
      <button class="theme-toggle" onclick={() => theme = theme === "dark" ? "light" : "dark"}
        title={theme === "dark" ? "切换到亮色模式" : "切换到暗色模式"}>
        {theme === "dark" ? "☀" : "🌙"}
      </button>
    </div>
    <div class="header-right">
      {#if status?.running}
        <div class="status-indicator" title="代理运行中">
          <span class="status-ring"></span>
          <span class="status-dot-inner"></span>
        </div>
        <span class="status-text">运行中 · 端口 {status.port} · {status.https ? "HTTPS" : "HTTP"}</span>
        <button class="btn-info" onclick={() => showConnInfo = true}>连接信息</button>
        <button class="btn-stop" onclick={handleStop} disabled={loading}>{loading ? "停止中..." : "停止"}</button>
      {:else}
        <button class="btn-start" onclick={handleStart} disabled={loading}>
          {#if loading}<span class="spinner"></span> 启动中...{:else}启动{/if}
        </button>
      {/if}
    </div>
  </header>

  <!-- ── Split Layout ── -->
  <div class="split-layout">
    <!-- LEFT: Config -->
    <div class="panel panel-left">

      <!-- Proxy Settings -->
      <section class="section">
        <div class="section-header"><h2>代理设置</h2></div>
        {#if config}
          <div class="card">
            <div class="proxy-settings-row">
              <label class="port-label">
                端口
                <input type="number" bind:value={config.proxy.port} onchange={onPortChange} min="1" max="65535" />
              </label>
              <label class="https-label">
                <input type="checkbox" checked={config.proxy.https} onchange={onHttpsToggle} />
                HTTPS
              </label>
              {#if config.proxy.https}
                <button class="btn-sm" onclick={handleGenerateCert}>
                  {certInfo?.generated ? "重新生成证书" : "生成证书"}
                </button>
                {#if certInfo?.generated}
                  <span class="cert-path" title={certInfo.cert_path}>证书已就绪</span>
                {/if}
              {/if}
            </div>
          </div>
        {/if}
      </section>

      <!-- Providers -->
      <section class="section">
        <div class="section-header">
          <h2>提供商</h2>
          <div class="section-header-actions">
            <button class="btn-secondary" onclick={openImportDialog}>导入配置</button>
            <button class="btn-primary" onclick={openAddProvider}>+ 添加提供商</button>
          </div>
        </div>
        {#if config && config.providers.length > 0}
          <div class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>名称</th>
                  <th>Base URL</th>
                  <th>API Key</th>
                  <th>模型</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {#each config.providers as provider, idx (provider.id)}
                  {#if inlineProviderIdx === idx}
                    <!-- Inline edit row -->
                    <tr class="editing-row">
                      <td><input bind:value={inlineProviderForm.name} placeholder="名称" /></td>
                      <td><input bind:value={inlineProviderForm.base_url} placeholder="https://..." /></td>
                      <td>
                        <div class="input-toggle">
                          <input type={showInlineKey ? "text" : "password"} bind:value={inlineProviderForm.api_key} placeholder="sk-..." />
                          <button type="button" class="toggle-btn" onclick={() => showInlineKey = !showInlineKey}>
                            {showInlineKey ? "🔒" : "👁"}
                          </button>
                        </div>
                      </td>
                      <td>
                        <div class="model-tags">
                          {#each inlineProviderForm.models as m}
                            <span class="model-tag">
                              {m}
                              <button class="model-tag-remove" onclick={() => removeInlineModelTag(m)}>&times;</button>
                            </span>
                          {/each}
                          <div class="model-tag-add-row">
                            <input id="inline-add-model-{idx}" class="model-tag-input" placeholder="添加模型" />
                            <button class="btn-sm btn-save" onclick={() => addInlineModelTag(idx)}>+</button>
                          </div>
                        </div>
                      </td>
                      <td class="act">
                        <button class="btn-sm btn-save" onclick={saveInlineProvider}>保存</button>
                        <button class="btn-sm" onclick={() => { inlineProviderIdx = null; showInlineKey = false; }}>取消</button>
                      </td>
                    </tr>
                  {:else}
                    <!-- Normal row -->
                    <tr>
                      <td><strong>{provider.name}</strong></td>
                      <td class="cell-url" title={provider.base_url}>{provider.base_url}</td>
                      <td class="cell-key">
                        <span class="key-mask">{visibleKeys[idx] ? provider.api_key : "•".repeat(Math.min(provider.api_key.length || 12, 20))}</span>
                        <button class="icon-btn" onclick={() => visibleKeys = { ...visibleKeys, [idx]: !visibleKeys[idx] }}
                          title={visibleKeys[idx] ? "隐藏" : "显示"}>
                          {visibleKeys[idx] ? "🔒" : "👁"}
                        </button>
                        <button class="icon-btn" onclick={() => copy(provider.api_key)} title="复制">📋</button>
                      </td>
                      <td>
                        <div class="model-tags">
                          {#each provider.models as m}
                            <span class="model-tag model-tag-readonly">{m}</span>
                          {/each}
                          {#if provider.models.length === 0}
                            <span class="model-tag-empty">-</span>
                          {/if}
                        </div>
                      </td>
                      <td class="act">
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
          <p class="empty">暂无提供商，点击上方按钮添加</p>
        {/if}
      </section>

      <!-- Model Mappings -->
      <section class="section">
        <div class="section-header">
          <h2>模型映射</h2>
          <button class="btn-primary" onclick={openAddMapping}>+ 添加映射</button>
        </div>
        {#if config && config.model_mappings.length > 0}
          <div class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>代理模型名</th>
                  <th>目标提供商</th>
                  <th>原始模型</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {#each config.model_mappings as mapping, idx}
                  {#if inlineMappingIdx === idx}
                    <tr class="editing-row">
                      <td>
                        <input list="il-proxy-models" bind:value={inlineMappingForm.from} placeholder="claude-sonnet-4-20250514" />
                        <datalist id="il-proxy-models">
                          {#each PROXY_MODELS as m}<option value={m}></option>{/each}
                        </datalist>
                      </td>
                      <td>
                        <select bind:value={inlineMappingForm.to_provider} onchange={onInlineMappingProviderChange}>
                          {#each config.providers as p}<option value={p.id}>{p.name}</option>{/each}
                        </select>
                      </td>
                      <td>
                        <input list="il-target-models-{idx}" bind:value={inlineMappingForm.to_model} placeholder="model-name" />
                        <datalist id="il-target-models-{idx}">
                          {#each inlineMappingFetchedModels as m}<option value={m}></option>{/each}
                        </datalist>
                      </td>
                      <td class="act">
                        <button class="btn-sm btn-save" onclick={saveInlineMapping}>保存</button>
                        <button class="btn-sm" onclick={() => inlineMappingIdx = null}>取消</button>
                      </td>
                    </tr>
                  {:else}
                    <tr>
                      <td><code>{mapping.from}</code></td>
                      <td>{getProviderName(mapping.to_provider)}</td>
                      <td><code>{mapping.to_model}</code></td>
                      <td class="act">
                        <button
                          class={mapTestBtnClass(idx)}
                          onclick={() => handleTestMapping(idx)}
                          disabled={testingMapping === idx || !!mappingBtnLabel[idx]}
                        >
                          {mappingBtnLabel[idx] || (testingMapping === idx ? "..." : "测试")}
                        </button>
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
          <p class="empty">暂无映射，点击上方按钮添加</p>
        {/if}
      </section>
    </div>

    <!-- RIGHT: Log Panel -->
    <div class="panel panel-right" class:collapsed={!logsExpanded}>
      {#if logsExpanded}
        <div class="log-header">
          <button class="log-collapse-btn" onclick={() => (logsExpanded = false)}>
            日志
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <path d="M4 15l8-8 8 8"/>
            </svg>
          </button>
          <div class="log-controls">
            <label class="autoscroll-label">
              <input type="checkbox" checked={autoScroll} onchange={() => autoScroll = !autoScroll} />
              自动滚动
            </label>
            <button class="btn-secondary" onclick={handleClearLogs}>清空</button>
          </div>
        </div>
        {#if logs.length === 0}
          <div class="log-terminal-empty">
            <span class="log-empty-msg">等待请求...</span>
          </div>
        {:else}
          <div class="log-terminal" bind:this={logContainer} onscroll={onLogScroll}>
            {#each logs as log (log.id)}
              <div
                class="log-line"
                class:log-clickable={log.method !== "OP"}
                onclick={() => { if (log.method !== "OP") selectedLog = log; }}
                role={log.method !== "OP" ? "button" : undefined}
                tabindex={log.method !== "OP" ? 0 : undefined}
                onkeydown={(e: KeyboardEvent) => { if (log.method !== "OP" && e.key === "Enter") selectedLog = log; }}
              >
                <span class="log-time">{new Date(log.timestamp).toLocaleTimeString()}</span>
                {#if log.method === "OP"}
                  <span class="badge badge-op">OP</span>
                  <span class="log-msg">{log.path}</span>
                  {#if log.response_body && typeof log.response_body === "string" && log.response_body}
                    <span class="log-detail">{log.response_body}</span>
                  {/if}
                {:else}
                  <span class="badge badge-method">{log.method}</span>
                  <span class="log-msg">{log.path}</span>
                  {#if log.model_in}
                    <span class="log-model">{log.model_in}{log.model_out ? " → " + log.model_out : ""}</span>
                  {/if}
                  {#if log.provider}
                    <span class="log-provider">{log.provider}</span>
                  {/if}
                  <span class="badge {log.status >= 400 ? 'badge-err' : 'badge-ok'}">{log.status}</span>
                  <span class="log-lat">{log.latency_ms}ms</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        <div class="log-collapse-bar">
          <button class="log-expand-btn" onclick={() => (logsExpanded = true)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <path d="M15 6l-6 6 6 6"/>
            </svg>
            <span>日志</span>
            {#if logs.length > 0}<span class="log-collapse-count">{logs.length}</span>{/if}
          </button>
        </div>
      {/if}
    </div>
  </div>

  <!-- ── Log Detail Modal ── -->
  {#if selectedLog}
    <div class="overlay" role="dialog" aria-modal="true" onclick={() => selectedLog = null}>
      <div class="modal" onclick={(e: MouseEvent) => e.stopPropagation()} role="document">
        <div class="modal-head">
          <h3>请求详情</h3>
          <button class="modal-close-btn" onclick={() => selectedLog = null} aria-label="关闭">&times;</button>
        </div>
        <div class="modal-body">
          <div class="detail-grid">
            <div>
              <h4>Request</h4>
              <pre>{JSON.stringify(selectedLog.request_body, null, 2)}</pre>
            </div>
            <div>
              <h4>Response</h4>
              <pre>{JSON.stringify(selectedLog.response_body, null, 2)}</pre>
            </div>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Provider Modal ── -->
  {#if showProviderModal}
    <div class="overlay" role="dialog" aria-modal="true" onclick={() => showProviderModal = false}>
      <div class="modal modal-wide" onclick={(e: MouseEvent) => e.stopPropagation()} role="document">
        <div class="modal-head">
          <h3>{editingProviderIdx !== null ? "编辑提供商" : "添加提供商"}</h3>
          <button class="modal-close-btn" onclick={() => showProviderModal = false} aria-label="关闭">&times;</button>
        </div>
        <div class="modal-body">
          <!-- Preset + Name -->
          <div class="form-row">
            <label>
              预设
              <select bind:value={providerPreset} onchange={onPresetChange}>
                {#each Object.entries(PRESET_PROVIDERS) as [key, val]}
                  <option value={key}>{val.name}</option>
                {/each}
              </select>
            </label>
            <label>
              名称
              <input bind:value={providerForm.name} placeholder="提供商名称" />
            </label>
          </div>
          <!-- Base URL -->
          <div class="form-row">
            <label style="flex:1">
              Base URL
              <input bind:value={providerForm.base_url} placeholder="https://api.example.com" />
            </label>
          </div>
          <!-- API Key -->
          <div class="form-row">
            <label style="flex:1">
              API Key
              <div class="input-toggle">
                <input type={showProviderKey ? "text" : "password"} bind:value={providerForm.api_key} placeholder="sk-..." />
                <button type="button" class="toggle-btn" onclick={() => showProviderKey = !showProviderKey}>
                  {showProviderKey ? "🔒" : "👁"}
                </button>
              </div>
            </label>
          </div>
          <!-- Models -->
          <div class="form-row">
            <label style="flex:1">
              <div class="models-label-row">
                <span>模型列表</span>
                <button class="btn-sm" onclick={doFetchModels} disabled={fetchingModels}>
                  {fetchingModels ? "拉取中..." : "拉取模型"}
                </button>
                <button class="btn-sm btn-test" onclick={doTestAllModels} disabled={testingAllModels || providerForm.models.length === 0}>
                  {testingAllModels ? "测试中..." : "测试所有模型"}
                </button>
              </div>
              <div class="model-tags model-tags-editable">
                {#each providerForm.models as m}
                  <span class="model-tag">
                    {m}
                    <button class="model-tag-remove" onclick={() => removeModelTag(m)}>&times;</button>
                  </span>
                {/each}
                <div class="model-tag-add-row">
                  <input id="modal-add-model-input" class="model-tag-input" placeholder="输入模型名" />
                  <button class="btn-sm btn-save" onclick={addModelTag}>+</button>
                </div>
              </div>
            </label>
          </div>
          <!-- Model test results -->
          {#if modelTestResults.length > 0}
            <div class="model-test-results">
              <div class="model-test-header">模型测试结果</div>
              {#each modelTestResults as r}
                <div class="model-test-row {modelTestClass(r)}">
                  <code>{r.model}</code>
                  <span>{r.success ? r.latency_ms + "ms" : r.message}</span>
                </div>
              {/each}
            </div>
          {/if}
          <!-- Actions -->
          <div class="form-actions">
            <button class="btn-primary" onclick={saveProviderModal}>
              {editingProviderIdx !== null ? "更新" : "添加"}
            </button>
            <button class="btn-secondary" onclick={() => showProviderModal = false}>取消</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Mapping Modal ── -->
  {#if showMappingModal}
    <div class="overlay" role="dialog" aria-modal="true" onclick={() => showMappingModal = false}>
      <div class="modal" onclick={(e: MouseEvent) => e.stopPropagation()} role="document">
        <div class="modal-head">
          <h3>{editingMappingIdx !== null ? "编辑映射" : "添加映射"}</h3>
          <button class="modal-close-btn" onclick={() => showMappingModal = false} aria-label="关闭">&times;</button>
        </div>
        <div class="modal-body">
          <!-- Proxy model name -->
          <div class="form-row">
            <label style="flex:1">
              代理模型名
              <input list="modal-proxy-models" bind:value={mappingForm.from} placeholder="claude-sonnet-4-20250514" />
              <datalist id="modal-proxy-models">
                {#each PROXY_MODELS as m}<option value={m}></option>{/each}
              </datalist>
            </label>
          </div>
          <!-- Target provider -->
          <div class="form-row">
            <label style="flex:1">
              目标提供商
              {#if config}
                <select bind:value={mappingForm.to_provider} onchange={onMappingProviderChange}>
                  {#each config.providers as p}<option value={p.id}>{p.name}</option>{/each}
                </select>
              {/if}
            </label>
          </div>
          <!-- Target model -->
          <div class="form-row">
            <label style="flex:1">
              原始模型
              {#if mappingFetchingModels}
                <div class="fetching-hint">正在拉取模型列表...</div>
              {/if}
              <input list="modal-target-models" bind:value={mappingForm.to_model} placeholder="model-name" />
              <datalist id="modal-target-models">
                {#each mappingFetchedModels as m}<option value={m}></option>{/each}
              </datalist>
            </label>
          </div>
          <div class="form-actions">
            <button class="btn-primary" onclick={saveMappingModal}>
              {editingMappingIdx !== null ? "更新" : "添加"}
            </button>
            <button class="btn-secondary" onclick={() => showMappingModal = false}>取消</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Connection Info Modal ── -->
  {#if showConnInfo && config && status}
    <div class="overlay" role="dialog" aria-modal="true" onclick={() => showConnInfo = false}>
      <div class="modal" onclick={(e: MouseEvent) => e.stopPropagation()} role="document">
        <div class="modal-head">
          <h3>连接信息</h3>
          <button class="modal-close-btn" onclick={() => showConnInfo = false} aria-label="关闭">&times;</button>
        </div>
        <div class="modal-body conn-info">
          <div class="ci-row">
            <span class="ci-label">服务地址</span>
            <code>http://{status.host}:{status.port}</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}`)}>复制</button>
          </div>
          <div class="ci-row">
            <span class="ci-label">Chat 接口</span>
            <code>http://{status.host}:{status.port}/v1/chat/completions</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}/v1/chat/completions`)}>复制</button>
          </div>
          <div class="ci-row">
            <span class="ci-label">Responses 接口</span>
            <code>http://{status.host}:{status.port}/v1/responses</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}/v1/responses`)}>复制</button>
          </div>
          <div class="ci-row">
            <span class="ci-label">Models 接口</span>
            <code>http://{status.host}:{status.port}/v1/models</code>
            <button class="btn-copy" onclick={() => copy(`http://${status!.host}:${status!.port}/v1/models`)}>复制</button>
          </div>
          <hr class="ci-divider" />
          <div class="ci-section-label">可用模型映射</div>
          {#each config.model_mappings as m}
            <div class="ci-row ci-model-row">
              <code>{m.from}</code>
              <span class="ci-arrow">&rarr;</span>
              <span>{getProviderName(m.to_provider)}</span>
              <code>{m.to_model}</code>
              <button class="btn-copy-sm" onclick={() => copy(m.from)}>复制</button>
            </div>
          {/each}
          {#if config.model_mappings.length === 0}
            <div class="ci-row"><span class="text-muted">暂无映射</span></div>
          {/if}
          <hr class="ci-divider" />
          <div class="ci-section-label">配置示例</div>
          <div class="ci-row ci-pre-row">
            <pre class="ci-pre">ANTHROPIC_BASE_URL=http://{status.host}:{status.port}
# Codex / OpenAI 兼容
OPENAI_BASE_URL=http://{status.host}:{status.port}/v1</pre>
            <button class="btn-copy" onclick={() => copy(`ANTHROPIC_BASE_URL=http://${status!.host}:${status!.port}\nOPENAI_BASE_URL=http://${status!.host}:${status!.port}/v1`)}>复制</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Import CC-Switch Dialog ── -->
  {#if showImportDialog}
    <div class="overlay" role="dialog" aria-modal="true" onclick={() => showImportDialog = false}>
      <div class="modal" onclick={(e: MouseEvent) => e.stopPropagation()} role="document">
        <div class="modal-head">
          <h3>导入 CC-Switch 配置</h3>
          <button class="modal-close-btn" onclick={() => showImportDialog = false} aria-label="关闭">&times;</button>
        </div>
        <div class="modal-body">
          <div class="form-row">
            <label style="flex:1">
              粘贴 CC-Switch 配置 JSON
              <textarea bind:value={importJson} rows="12" placeholder="粘贴 CC-Switch 配置 JSON 内容"></textarea>
            </label>
          </div>
          <div class="form-actions">
            <button class="btn-primary" onclick={handleImport}>导入</button>
            <button class="btn-secondary" onclick={() => showImportDialog = false}>取消</button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* ═══════════════════════════════════════════════════════════════════════════
     CSS Variables (Dark theme defaults)
     ═══════════════════════════════════════════════════════════════════════════ */
  .app {
    --bg-app: #111827;
    --bg-panel: #111827;
    --bg-card: #1f2937;
    --bg-input: #374151;
    --bg-input-focus: #374151;
    --bg-table-head: #1f2937;
    --bg-table-hover: #1a2332;
    --bg-editing-row: #1a2744;
    --bg-overlay: rgba(0, 0, 0, 0.7);
    --bg-toast: #374151;
    --text-primary: #e5e7eb;
    --text-secondary: #9ca3af;
    --text-muted: #6b7280;
    --text-toast: #e5e7eb;
    --border: #374151;
    --border-light: #1f2937;
    --border-input: #4b5563;
    --accent: #818cf8;
    --accent-hover: #6366f1;
    --accent-light: rgba(129, 140, 248, 0.1);
    --accent-bg: rgba(129, 140, 248, 0.15);
    --danger: #f87171;
    --danger-hover: #ef4444;
    --danger-light: rgba(248, 113, 113, 0.1);
    --danger-border: rgba(248, 113, 113, 0.3);
    --success: #4ade80;
    --success-light: rgba(74, 222, 128, 0.1);
    --success-bg: rgba(74, 222, 128, 0.15);
    --warn: #fbbf24;
    --warn-bg: rgba(251, 191, 36, 0.15);
    --modal-bg: #1f2937;
    --modal-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    --header-bg: #1f2937;
    --header-border: #374151;
    --badge-ok-bg: rgba(74, 222, 128, 0.15);
    --badge-ok-color: #4ade80;
    --badge-err-bg: rgba(248, 113, 113, 0.15);
    --badge-err-color: #f87171;
    --badge-op-bg: rgba(129, 140, 248, 0.15);
    --badge-op-color: #818cf8;
    --badge-method-bg: #374151;
    --badge-method-color: #9ca3af;
    --btn-secondary-bg: #374151;
    --btn-secondary-color: #d1d5db;
    --btn-secondary-hover-bg: #4b5563;
    --btn-sm-bg: #374151;
    --btn-sm-border: #4b5563;
    --btn-sm-hover-bg: #4b5563;
    --code-bg: #374151;
    --modal-head-border: #374151;
    --status-text-color: #4ade80;
    --toggle-btn-opacity: 0.8;
    --model-tag-bg: rgba(129, 140, 248, 0.15);
    --model-tag-color: #a5b4fc;
    --model-tag-border: rgba(129, 140, 248, 0.3);
    --ring-color: #4ade80;
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Light theme overrides
     ═══════════════════════════════════════════════════════════════════════════ */
  .app[data-theme="light"] {
    --bg-app: #f5f7fa;
    --bg-panel: #f5f7fa;
    --bg-card: #ffffff;
    --bg-input: #fafbfc;
    --bg-input-focus: #ffffff;
    --bg-table-head: #f8f9fb;
    --bg-table-hover: #f8f9fb;
    --bg-editing-row: #eef2ff;
    --bg-overlay: rgba(0, 0, 0, 0.45);
    --bg-toast: #1a1a2e;
    --text-primary: #1a1a2e;
    --text-secondary: #555555;
    --text-muted: #888888;
    --text-toast: #ffffff;
    --border: #e8ecf0;
    --border-light: #f3f4f6;
    --border-input: #dddddd;
    --accent: #667eea;
    --accent-hover: #5a6fd6;
    --accent-light: #eef2ff;
    --accent-bg: #dbeafe;
    --danger: #ef4444;
    --danger-hover: #dc2626;
    --danger-light: #fef2f2;
    --danger-border: #fecaca;
    --success: #16a34a;
    --success-light: #f0fdf4;
    --success-bg: #dcfce7;
    --warn: #b45309;
    --warn-bg: #fef3c7;
    --modal-bg: #ffffff;
    --modal-shadow: 0 20px 60px rgba(0, 0, 0, 0.2);
    --header-bg: #ffffff;
    --header-border: #e8ecf0;
    --badge-ok-bg: #dcfce7;
    --badge-ok-color: #166534;
    --badge-err-bg: #fef2f2;
    --badge-err-color: #991b1b;
    --badge-op-bg: #dbeafe;
    --badge-op-color: #1d4ed8;
    --badge-method-bg: #e4e7ec;
    --badge-method-color: #555555;
    --btn-secondary-bg: #e4e7ec;
    --btn-secondary-color: #555555;
    --btn-secondary-hover-bg: #d8dbe0;
    --btn-sm-bg: #ffffff;
    --btn-sm-border: #dddddd;
    --btn-sm-hover-bg: #f5f5f5;
    --code-bg: #f0f2f5;
    --modal-head-border: #e8ecf0;
    --status-text-color: #16a34a;
    --toggle-btn-opacity: 0.7;
    --model-tag-bg: #eef2ff;
    --model-tag-color: #4338ca;
    --model-tag-border: #c7d2fe;
    --ring-color: #16a34a;
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Global reset
     ═══════════════════════════════════════════════════════════════════════════ */
  :global(html, body) {
    margin: 0; padding: 0; height: 100%;
    font-family: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", "Helvetica Neue", Arial, sans-serif;
    -webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale;
  }

  .app {
    height: 100vh; display: flex; flex-direction: column;
    background: var(--bg-app); color: var(--text-primary); overflow: hidden;
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Header
     ═══════════════════════════════════════════════════════════════════════════ */
  header {
    display: flex; align-items: center; gap: 16px; padding: 10px 20px;
    background: var(--header-bg); border-bottom: 1px solid var(--header-border);
    flex-shrink: 0;
  }

  .header-left {
    display: flex; align-items: center; gap: 10px;
  }

  header h1 {
    margin: 0; font-size: 1.15rem;
    background: linear-gradient(135deg, #667eea, #764ba2);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
    background-clip: text;
    font-weight: 700; letter-spacing: -0.3px;
  }

  .theme-toggle {
    background: none; border: 1px solid var(--border); border-radius: 6px;
    font-size: 1rem; cursor: pointer; padding: 3px 7px; line-height: 1;
    color: var(--text-primary);
  }
  .theme-toggle:hover { background: var(--bg-input); }

  .header-right { margin-left: auto; display: flex; align-items: center; gap: 10px; }

  /* ═══════════════════════════════════════════════════════════════════════════
     Animated Status Ring Indicator
     ═══════════════════════════════════════════════════════════════════════════ */
  .status-indicator {
    position: relative;
    width: 18px; height: 18px;
    display: flex; align-items: center; justify-content: center;
    flex-shrink: 0;
  }

  .status-ring {
    position: absolute; inset: -4px;
    border-radius: 50%;
    border: 2px solid var(--ring-color);
    animation: status-pulse-ring 2s ease-out infinite;
    opacity: 0;
  }

  .status-dot-inner {
    width: 8px; height: 8px; border-radius: 50%;
    background: var(--ring-color);
    box-shadow: 0 0 8px var(--ring-color), 0 0 16px var(--ring-color);
    animation: status-pulse-dot 2s ease-in-out infinite;
  }

  @keyframes status-pulse-ring {
    0% { transform: scale(0.8); opacity: 0.9; }
    100% { transform: scale(1.8); opacity: 0; }
  }

  @keyframes status-pulse-dot {
    0%, 100% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.2); opacity: 0.7; }
  }

  .status-text { font-size: 0.8rem; color: var(--status-text-color); font-weight: 500; }

  /* ═══════════════════════════════════════════════════════════════════════════
     Split layout
     ═══════════════════════════════════════════════════════════════════════════ */
  .split-layout { display: flex; flex: 1; overflow: hidden; }

  .panel-left {
    flex: 1; overflow-y: auto; padding: 20px; min-width: 0;
    background: var(--bg-panel);
  }

  .panel-right {
    flex: 0 0 46%; overflow: hidden; display: flex; flex-direction: column;
    border-left: 1px solid var(--border);
    background: var(--bg-panel);
  }
  .panel-right.collapsed {
    flex: 0 0 46px; width: 46px;
  }

  @media (max-width: 700px) {
    .split-layout { flex-direction: column; }
    .panel-left { flex: 1; min-height: 200px; border-bottom: 1px solid var(--border); }
    .panel-right.collapsed { flex: 0 0 38px; width: auto; border-left: none; border-top: 1px solid var(--border); }
    .panel-right:not(.collapsed) { flex: 0 0 50%; min-height: 200px; }
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Sections, Cards, Tables
     ═══════════════════════════════════════════════════════════════════════════ */
  .section { margin-bottom: 28px; }
  .section-header {
    display: flex; justify-content: space-between; align-items: center;
    margin-bottom: 10px; gap: 8px;
  }
  .section-header h2 {
    margin: 0; font-size: 0.95rem; font-weight: 600;
    color: var(--text-primary); letter-spacing: -0.2px;
  }
  .section-header-actions {
    display: flex; gap: 6px; align-items: center;
  }

  .card {
    background: var(--bg-card); border-radius: 9px; padding: 14px 18px;
    border: 1px solid var(--border);
  }
  .proxy-settings-row {
    display: flex; align-items: center; gap: 16px; flex-wrap: wrap;
  }
  .port-label { display: flex; align-items: center; gap: 10px; font-size: 0.82rem; color: var(--text-secondary); font-weight: 500; }
  .port-label input { width: 90px; }
  .https-label {
    display: flex; align-items: center; gap: 4px; font-size: 0.82rem;
    color: var(--text-secondary); font-weight: 500; cursor: pointer; user-select: none;
  }
  .https-label input { width: auto; accent-color: var(--accent); }
  .cert-path { font-size: 0.72rem; color: var(--success); font-weight: 500; }

  .table-wrap {
    background: var(--bg-card); border-radius: 9px; overflow: hidden;
    border: 1px solid var(--border);
  }
  table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
  th {
    text-align: left; padding: 9px 12px; background: var(--bg-table-head);
    color: var(--text-secondary); font-weight: 600;
    border-bottom: 1px solid var(--border); white-space: nowrap;
    font-size: 0.76rem; letter-spacing: -0.1px;
  }
  td {
    padding: 7px 12px; border-bottom: 1px solid var(--border-light);
    vertical-align: middle; color: var(--text-primary);
  }
  tbody tr:last-child td { border-bottom: none; }
  tbody tr:hover { background: var(--bg-table-hover); }
  tbody tr.editing-row { background: var(--bg-editing-row); }

  .cell-url { color: var(--text-muted); font-size: 0.76rem; max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell-key { color: var(--text-muted); display: flex; align-items: center; gap: 2px; }
  .key-mask { letter-spacing: 2px; font-size: 0.72rem; color: var(--text-muted); }
  code {
    font-size: 0.76rem; background: var(--code-bg); padding: 2px 5px; border-radius: 3px;
    font-family: "SF Mono", "Fira Code", "Fira Mono", "Roboto Mono", "Cascadia Code", Consolas, monospace;
    color: var(--text-primary);
  }
  .empty { text-align: center; color: var(--text-muted); padding: 28px; margin: 0; font-size: 0.82rem; }
  .text-muted { color: var(--text-muted); font-size: 0.8rem; }

  /* ═══════════════════════════════════════════════════════════════════════════
     Model Tags
     ═══════════════════════════════════════════════════════════════════════════ */
  .model-tags {
    display: flex; flex-wrap: wrap; gap: 4px; align-items: center;
  }
  .model-tags-editable {
    margin-top: 4px;
  }
  .model-tag {
    display: inline-flex; align-items: center; gap: 3px;
    background: var(--model-tag-bg); color: var(--model-tag-color);
    border: 1px solid var(--model-tag-border);
    border-radius: 12px; padding: 1px 8px;
    font-size: 0.7rem; font-weight: 500;
    white-space: nowrap;
  }
  .model-tag-readonly {
    cursor: default;
  }
  .model-tag-remove {
    background: none; border: none; cursor: pointer;
    color: var(--model-tag-color); font-size: 0.85rem;
    padding: 0; line-height: 1; opacity: 0.6;
    margin-left: 1px;
  }
  .model-tag-remove:hover { opacity: 1; }
  .model-tag-empty {
    font-size: 0.72rem; color: var(--text-muted);
  }
  .model-tag-add-row {
    display: flex; gap: 2px; align-items: center;
  }
  .model-tag-input {
    width: 100px !important; padding: 2px 6px !important; font-size: 0.7rem !important;
    border-radius: 12px !important;
  }

  .models-label-row {
    display: flex; align-items: center; gap: 8px; margin-bottom: 4px;
  }

  .fetching-hint {
    font-size: 0.7rem; color: var(--accent); font-style: italic; margin-bottom: 2px;
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Model test results
     ═══════════════════════════════════════════════════════════════════════════ */
  .model-test-results {
    margin-top: 8px; padding: 8px;
    background: var(--bg-input); border-radius: 6px;
    max-height: 160px; overflow-y: auto;
  }
  .model-test-header {
    font-size: 0.72rem; font-weight: 600; color: var(--text-secondary);
    margin-bottom: 6px;
  }
  .model-test-row {
    display: flex; align-items: center; gap: 8px;
    padding: 3px 6px; border-radius: 4px; font-size: 0.7rem;
    margin-bottom: 2px;
  }
  .model-test-row.mt-ok { background: var(--success-light); color: var(--success); }
  .model-test-row.mt-warn { background: var(--warn-bg); color: var(--warn); }
  .model-test-row.mt-fail { background: var(--danger-light); color: var(--danger); }
  .model-test-row code { font-size: 0.68rem; background: transparent; padding: 0; }

  /* ═══════════════════════════════════════════════════════════════════════════
     Buttons
     ═══════════════════════════════════════════════════════════════════════════ */
  .btn-primary {
    padding: 6px 16px; background: var(--accent); color: #fff;
    border: none; border-radius: 6px; font-size: 0.8rem; cursor: pointer; font-weight: 500;
  }
  .btn-primary:hover { background: var(--accent-hover); }

  .btn-secondary {
    padding: 5px 12px; background: var(--btn-secondary-bg);
    color: var(--btn-secondary-color); border: none; border-radius: 6px;
    font-size: 0.78rem; cursor: pointer; font-weight: 500;
  }
  .btn-secondary:hover { background: var(--btn-secondary-hover-bg); }

  .btn-start {
    padding: 7px 18px; background: var(--accent); color: #fff;
    border: none; border-radius: 7px; font-size: 0.82rem; font-weight: 600;
    cursor: pointer; display: flex; align-items: center; gap: 5px;
    letter-spacing: -0.1px;
  }
  .btn-start:hover { background: var(--accent-hover); }
  .btn-start:disabled { opacity: 0.6; cursor: not-allowed; }

  .btn-stop {
    padding: 7px 14px; background: var(--danger); color: #fff;
    border: none; border-radius: 7px; font-size: 0.82rem; cursor: pointer; font-weight: 500;
  }
  .btn-stop:hover { background: var(--danger-hover); }
  .btn-stop:disabled { opacity: 0.6; cursor: not-allowed; }

  .btn-info {
    padding: 6px 12px; background: var(--accent-bg); color: #818cf8;
    border: none; border-radius: 6px; font-size: 0.78rem; cursor: pointer; font-weight: 500;
  }
  .app[data-theme="light"] .btn-info { color: #1d4ed8; }
  .btn-info:hover { filter: brightness(0.95); }

  .btn-sm {
    padding: 3px 9px; border: 1px solid var(--btn-sm-border);
    background: var(--btn-sm-bg); border-radius: 4px;
    font-size: 0.72rem; cursor: pointer; color: var(--text-primary);
  }
  .btn-sm:hover { background: var(--btn-sm-hover-bg); }
  .btn-del { color: var(--danger); border-color: var(--danger-border); }
  .btn-del:hover { background: var(--danger-light); }
  .btn-save { color: var(--success); border-color: rgba(22, 163, 74, 0.3); }
  .btn-save:hover { background: var(--success-light); }
  .btn-test { color: var(--accent); border-color: rgba(99, 102, 241, 0.3); min-width: 46px; transition: all 0.2s; }
  .btn-test:hover { background: var(--accent-light); }

  .test-ok-btn { color: #16a34a !important; border-color: #bbf7d0 !important; background: #f0fdf4 !important; font-weight: 600; }
  .app[data-theme="dark"] .test-ok-btn { background: #052e16 !important; border-color: #166534 !important; color: #4ade80 !important; }
  .test-warn-btn { color: #b45309 !important; border-color: #fde68a !important; background: #fffbeb !important; font-weight: 600; }
  .app[data-theme="dark"] .test-warn-btn { background: #451a03 !important; border-color: #92400e !important; color: #fbbf24 !important; }
  .test-fail-btn { color: #dc2626 !important; border-color: #fecaca !important; background: #fef2f2 !important; font-weight: 600; }
  .app[data-theme="dark"] .test-fail-btn { background: #450a0a !important; border-color: #991b1b !important; color: #f87171 !important; }

  .act { white-space: nowrap; display: flex; align-items: center; gap: 3px; flex-wrap: wrap; }
  .icon-btn {
    background: none; border: none; cursor: pointer; font-size: 0.82rem;
    padding: 1px 3px; opacity: 0.5; line-height: 1;
  }
  .icon-btn:hover { opacity: 1; }

  .btn-copy {
    padding: 4px 9px; background: var(--accent); color: #fff;
    border: none; border-radius: 4px; font-size: 0.7rem; cursor: pointer; white-space: nowrap;
  }
  .btn-copy:hover { background: var(--accent-hover); }
  .btn-copy-sm {
    padding: 2px 6px; background: var(--bg-input); color: var(--text-secondary);
    border: 1px solid var(--border-input); border-radius: 3px; font-size: 0.66rem; cursor: pointer;
  }
  .btn-copy-sm:hover { background: var(--border); }

  .spinner {
    width: 13px; height: 13px; border: 2px solid rgba(255,255,255,0.3);
    border-top-color: #fff; border-radius: 50%; animation: spin 0.6s linear infinite;
    display: inline-block;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ═══════════════════════════════════════════════════════════════════════════
     Forms
     ═══════════════════════════════════════════════════════════════════════════ */
  input:not([type="checkbox"]), select, textarea {
    padding: 5px 9px; border: 1px solid var(--border-input); border-radius: 5px;
    font-size: 0.8rem; background: var(--bg-input); outline: none;
    width: 100%; box-sizing: border-box; font-family: inherit; color: var(--text-primary);
  }
  input:focus, select:focus, textarea:focus { border-color: var(--accent); background: var(--bg-input-focus); }
  textarea { resize: vertical; font-family: "SF Mono", "Fira Code", Consolas, monospace; font-size: 0.72rem; }

  .input-toggle { display: flex; position: relative; }
  .input-toggle input { flex: 1; padding-right: 30px; }
  .toggle-btn {
    position: absolute; right: 5px; top: 50%; transform: translateY(-50%);
    background: none; border: none; cursor: pointer; font-size: 0.85rem;
    padding: 0; line-height: 1; opacity: var(--toggle-btn-opacity);
  }
  .toggle-btn:hover { opacity: 1; }

  /* ═══════════════════════════════════════════════════════════════════════════
     Modals
     ═══════════════════════════════════════════════════════════════════════════ */
  .overlay {
    position: fixed; inset: 0; background: var(--bg-overlay);
    display: flex; align-items: center; justify-content: center; z-index: 100;
  }
  .modal {
    background: var(--modal-bg); border-radius: 11px; width: 90vw; max-width: 560px;
    max-height: 82vh; display: flex; flex-direction: column;
    box-shadow: var(--modal-shadow);
  }
  .modal-wide {
    max-width: 660px;
  }
  .modal-head {
    display: flex; justify-content: space-between; align-items: center;
    padding: 13px 18px; border-bottom: 1px solid var(--modal-head-border);
  }
  .modal-head h3 { margin: 0; font-size: 0.95rem; color: var(--text-primary); }
  .modal-close-btn {
    background: none; border: none; font-size: 1.2rem; cursor: pointer;
    color: var(--text-muted); line-height: 1; padding: 0;
  }
  .modal-close-btn:hover { color: var(--text-primary); }
  .modal-body { padding: 18px; overflow-y: auto; flex: 1; }

  .form-row { display: flex; gap: 10px; margin-bottom: 12px; flex-wrap: wrap; }
  .form-row label {
    display: flex; flex-direction: column; gap: 4px;
    font-size: 0.78rem; color: var(--text-secondary); min-width: 160px;
  }
  .form-actions { display: flex; gap: 8px; margin-top: 4px; }

  .detail-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .detail-grid h4 { margin: 0 0 6px; font-size: 0.82rem; color: var(--text-secondary); }
  .detail-grid pre {
    background: var(--bg-input); padding: 10px; border-radius: 5px;
    font-size: 0.7rem; overflow: auto; max-height: 44vh; margin: 0;
    color: var(--text-primary);
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Badges
     ═══════════════════════════════════════════════════════════════════════════ */
  .badge { padding: 2px 7px; border-radius: 4px; font-size: 0.7rem; font-weight: 600; }
  .badge-ok { background: var(--badge-ok-bg); color: var(--badge-ok-color); }
  .badge-err { background: var(--badge-err-bg); color: var(--badge-err-color); }
  .badge-op { background: var(--badge-op-bg); color: var(--badge-op-color); }
  .badge-method { background: var(--badge-method-bg); color: var(--badge-method-color); }

  /* ═══════════════════════════════════════════════════════════════════════════
     Log Terminal (ALWAYS dark - terminal style)
     ═══════════════════════════════════════════════════════════════════════════ */
  .log-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 10px 16px; flex-shrink: 0;
  }
  .log-collapse-btn {
    background: none; border: 1px solid var(--border); border-radius: 5px;
    padding: 3px 10px; font-size: 0.78rem; cursor: pointer;
    color: var(--text-primary); font-weight: 500;
    display: flex; align-items: center; gap: 4px;
  }
  .log-collapse-btn:hover { background: var(--bg-input); }

  .log-controls { display: flex; align-items: center; gap: 10px; }
  .autoscroll-label {
    display: flex; align-items: center; gap: 4px;
    font-size: 0.72rem; color: var(--text-muted); cursor: pointer; user-select: none;
  }
  .autoscroll-label input { width: auto; accent-color: var(--accent); }

  .log-terminal {
    flex: 1; overflow-y: auto;
    font-family: "SF Mono", "Fira Code", "Fira Mono", "Roboto Mono", "Cascadia Code", Consolas, monospace;
    font-size: 0.72rem;
    background: #1e1e2e; color: #cdd6f4;
    border-radius: 0 0 6px 6px;
  }
  .log-terminal-empty {
    flex: 1; overflow-y: auto;
    font-family: "SF Mono", "Fira Code", Consolas, monospace;
    font-size: 0.72rem;
    background: #1e1e2e; color: #cdd6f4;
    display: flex; align-items: center; justify-content: center;
    border-radius: 0 0 6px 6px;
  }
  .log-empty-msg { color: #6c7086; font-size: 0.8rem; }

  .log-line {
    display: flex; align-items: center; gap: 6px;
    padding: 2px 10px; border-bottom: 1px solid rgba(255,255,255,0.04);
    min-height: 22px; white-space: nowrap;
  }
  .log-line:hover { background: rgba(255,255,255,0.05); }
  .log-clickable { cursor: pointer; }
  .log-time { color: #6c7086; font-size: 0.66rem; flex-shrink: 0; width: 56px; text-align: right; }
  .log-msg { color: #a6adc8; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .log-model { color: #89b4fa; flex-shrink: 0; max-width: 220px; overflow: hidden; text-overflow: ellipsis; }
  .log-provider { color: #a6e3a1; flex-shrink: 0; max-width: 100px; overflow: hidden; text-overflow: ellipsis; font-size: 0.66rem; }
  .log-lat { color: #6c7086; flex-shrink: 0; font-size: 0.66rem; }
  .log-detail { color: #a6adc8; font-size: 0.66rem; flex-shrink: 0; max-width: 120px; overflow: hidden; text-overflow: ellipsis; }

  .log-line .badge { font-size: 0.62rem; padding: 1px 5px; border-radius: 3px; flex-shrink: 0; }
  .log-line .badge-op { background: rgba(137,180,250,0.15); color: #89b4fa; }
  .log-line .badge-method { background: rgba(166,173,200,0.1); color: #a6adc8; }
  .log-line .badge-ok { background: rgba(166,227,161,0.12); color: #a6e3a1; }
  .log-line .badge-err { background: rgba(243,139,168,0.12); color: #f38ba8; }

  /* ═══════════════════════════════════════════════════════════════════════════
     Collapse Bar (when logs are hidden)
     ═══════════════════════════════════════════════════════════════════════════ */
  .log-collapse-bar {
    flex: 1; display: flex; align-items: center; justify-content: center;
    background: var(--bg-card);
    padding: 6px;
  }
  .log-expand-btn {
    display: flex; flex-direction: column; align-items: center; gap: 4px;
    background: none; border: none; cursor: pointer; color: var(--text-secondary);
    font-size: 0.78rem; font-family: inherit; padding: 8px;
    border-radius: 6px; transition: background 0.15s;
  }
  .log-expand-btn:hover { background: var(--bg-input); color: var(--text-primary); }
  .log-collapse-count {
    font-size: 0.62rem; color: var(--accent); font-weight: 600;
    background: var(--accent-bg); padding: 1px 7px; border-radius: 8px;
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Connection Info
     ═══════════════════════════════════════════════════════════════════════════ */
  .conn-info { display: flex; flex-direction: column; gap: 10px; }
  .ci-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .ci-label {
    font-size: 0.72rem; color: var(--text-muted); font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.4px; min-width: 130px;
  }
  .ci-row code { font-size: 0.78rem; background: var(--code-bg); padding: 5px 9px; border-radius: 4px; flex: 1; }
  .ci-pre-row { align-items: flex-start; }
  .ci-pre {
    font-size: 0.76rem; background: var(--code-bg); padding: 9px 11px;
    border-radius: 4px; margin: 0; flex: 1; white-space: pre-wrap;
    font-family: "SF Mono", "Fira Code", Consolas, monospace; color: var(--text-primary);
  }
  .ci-arrow { color: var(--accent); font-weight: bold; }
  .ci-model-row { font-size: 0.78rem; }
  .ci-divider { border: none; border-top: 1px solid var(--border); margin: 4px 0; }
  .ci-section-label {
    font-size: 0.72rem; color: var(--text-muted); font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.4px; margin-bottom: 2px;
  }

  /* ═══════════════════════════════════════════════════════════════════════════
     Toast
     ═══════════════════════════════════════════════════════════════════════════ */
  .toast {
    position: fixed; top: 56px; right: 20px;
    background: var(--bg-toast); color: var(--text-toast);
    padding: 9px 18px; border-radius: 7px; font-size: 0.82rem;
    z-index: 999; animation: fadeIn 0.2s;
  }
  @keyframes fadeIn { from { opacity:0; transform:translateY(-6px); } to { opacity:1; } }
</style>
