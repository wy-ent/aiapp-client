// aiapp container — frontend logic
// Handles navigation, app management, store, and the app sandbox runner.

// --- i18n ---
const I18N = {
  en: {
    desktop: 'Desktop', store: 'Store', settings: 'Settings',
    my_apps: 'My Apps', app_store: 'App Store',
    no_apps: 'No apps installed. Browse the Store to get started.',
    loading: 'Loading...',
    search_placeholder: 'Search apps...',
    backend_url: 'Backend URL', language: 'Language',
    platform: 'Platform', renderer: 'Renderer',
    clear_data: 'Clear All Data', run: 'Run', install: 'Install',
    uninstall: 'Uninstall', installed: 'Installed', open: 'Open',
  },
  zh: {
    desktop: '桌面', store: '商店', settings: '设置',
    my_apps: '我的应用', app_store: '应用商店',
    no_apps: '还没有安装应用，去商店看看吧。',
    loading: '加载中...',
    search_placeholder: '搜索应用...',
    backend_url: '后端地址', language: '语言',
    platform: '平台', renderer: '渲染器',
    clear_data: '清除所有数据', run: '运行', install: '安装',
    uninstall: '卸载', installed: '已安装', open: '打开',
  }
};

// --- State ---
const state = {
  lang: 'en',
  backendUrl: 'http://localhost:8080',
  installed: [],  // [{ id, name, description, category, icon }]
  storeApps: [],  // [{ id, name, description, category, ... }]
};

// --- Init ---
async function init() {
  // Load saved settings
  try {
    const saved = await bridge('storage_get', { key: 'settings' });
    if (saved) {
      const s = JSON.parse(new TextDecoder().decode(saved));
      if (s.lang) state.lang = s.lang;
      if (s.backendUrl) state.backendUrl = s.backendUrl;
    }
    const installed = await bridge('storage_get', { key: 'installed' });
    if (installed) {
      state.installed = JSON.parse(new TextDecoder().decode(installed));
    }
  } catch (e) {
    console.warn('Init failed, using defaults:', e);
  }

  // Set i18n
  applyI18n();

  // Set settings values
  document.getElementById('setting-backend-url').value = state.backendUrl;
  document.getElementById('setting-lang').value = state.lang;
  document.getElementById('setting-platform').textContent = await bridge('platform');
  document.getElementById('setting-renderer').textContent = await bridge('renderer_mode');

  // Navigation
  document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('click', () => {
      document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
      document.querySelectorAll('.tab-content').forEach(t => t.classList.remove('active'));
      item.classList.add('active');
      const tab = document.getElementById('tab-' + item.dataset.tab);
      if (tab) tab.classList.add('active');
    });
  });

  // Settings events
  document.getElementById('setting-backend-url').addEventListener('change', e => {
    state.backendUrl = e.target.value;
    saveSettings();
  });
  document.getElementById('setting-lang').addEventListener('change', e => {
    state.lang = e.target.value;
    applyI18n();
    saveSettings();
  });
  document.getElementById('btn-clear-data').addEventListener('click', async () => {
    state.installed = [];
    await bridge('storage_set', { key: 'installed', value: new TextEncoder().encode('[]') });
    renderDesktop();
  });

  // Store search
  document.getElementById('store-search').addEventListener('input', e => {
    renderStore(e.target.value);
  });

  // Close app overlay
  document.getElementById('btn-close-app').addEventListener('click', closeApp);

  // Initial render
  renderDesktop();
  loadStore();
}

// --- JS Bridge ---
async function bridge(cmd, args = {}) {
  if (window.__TAURI__) {
    return await window.__TAURI__.invoke(cmd, args);
  }
  // Fallback for development (no Tauri)
  console.warn(`[bridge] ${cmd}`, args);
  return null;
}

// --- i18n ---
function applyI18n() {
  const t = I18N[state.lang] || I18N.en;
  document.querySelectorAll('[data-i18n]').forEach(el => {
    const key = el.dataset.i18n;
    if (t[key]) el.textContent = t[key];
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
    const key = el.dataset.i18nPlaceholder;
    if (t[key]) el.placeholder = t[key];
  });
}

// --- Desktop ---
function renderDesktop() {
  const grid = document.getElementById('app-grid');
  if (state.installed.length === 0) {
    grid.innerHTML = `<div class="app-grid-empty">${I18N[state.lang].no_apps}</div>`;
    return;
  }
  grid.innerHTML = state.installed.map(app => `
    <div class="app-card" data-app-id="${app.id}">
      <div class="app-card-icon">${(app.name || '?')[0].toUpperCase()}</div>
      <div class="app-card-name">${escHtml(app.name)}</div>
      <div class="app-card-desc">${escHtml(app.description || '')}</div>
      <div class="app-card-actions">
        <button class="app-card-btn btn-run" data-action="run">${I18N[state.lang].run}</button>
        <button class="app-card-btn btn-uninstall" data-action="uninstall">${I18N[state.lang].uninstall}</button>
      </div>
    </div>
  `).join('');

  // Event delegation
  grid.querySelectorAll('[data-action="run"]').forEach(btn => {
    btn.addEventListener('click', e => {
      const card = e.target.closest('.app-card');
      runApp(card.dataset.appId);
    });
  });
  grid.querySelectorAll('[data-action="uninstall"]').forEach(btn => {
    btn.addEventListener('click', async e => {
      const card = e.target.closest('.app-card');
      const id = card.dataset.appId;
      state.installed = state.installed.filter(a => a.id !== id);
      await bridge('storage_set', {
        key: 'installed',
        value: new TextEncoder().encode(JSON.stringify(state.installed))
      });
      renderDesktop();
    });
  });
}

// --- Store ---
async function loadStore() {
  try {
    const res = await fetch(`${state.backendUrl}/api/apps`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    state.storeApps = await res.json();
    renderStore();
  } catch (e) {
    document.getElementById('store-list').innerHTML =
      `<div class="app-grid-empty">Failed to load store: ${e.message}</div>`;
  }
}

function renderStore(query = '') {
  const list = document.getElementById('store-list');
  const filtered = query
    ? state.storeApps.filter(a =>
        (a.name || '').toLowerCase().includes(query.toLowerCase()) ||
        (a.description || '').toLowerCase().includes(query.toLowerCase()))
    : state.storeApps;

  if (filtered.length === 0) {
    list.innerHTML = `<div class="app-grid-empty">${I18N[state.lang].no_apps}</div>`;
    return;
  }

  const installedIds = new Set(state.installed.map(a => a.id));

  list.innerHTML = filtered.map(app => {
    const isInstalled = installedIds.has(app.id);
    return `
      <div class="app-card" data-app-id="${app.id}">
        <div class="app-card-icon">${(app.name || '?')[0].toUpperCase()}</div>
        <div class="app-card-name">${escHtml(app.name)}</div>
        <div class="app-card-desc">${escHtml(app.description || '')}</div>
        <div class="app-card-actions">
          ${isInstalled
            ? `<button class="app-card-btn btn-run" data-action="store-run">${I18N[state.lang].open}</button>`
            : `<button class="app-card-btn btn-install" data-action="store-install">${I18N[state.lang].install}</button>`
          }
        </div>
      </div>
    `;
  }).join('');

  list.querySelectorAll('[data-action="store-run"]').forEach(btn => {
    btn.addEventListener('click', e => {
      const card = e.target.closest('.app-card');
      runApp(card.dataset.appId);
    });
  });
  list.querySelectorAll('[data-action="store-install"]').forEach(btn => {
    btn.addEventListener('click', async e => {
      const card = e.target.closest('.app-card');
      const app = state.storeApps.find(a => a.id === card.dataset.appId);
      if (!app) return;
      state.installed.push(app);
      await bridge('storage_set', {
        key: 'installed',
        value: new TextEncoder().encode(JSON.stringify(state.installed))
      });
      renderStore();
      renderDesktop();
    });
  });
}

// --- App Runner ---
async function runApp(appId) {
  const app = state.installed.find(a => a.id === appId) ||
              state.storeApps.find(a => a.id === appId);
  if (!app) return;

  document.getElementById('overlay-app-title').textContent = app.name || 'App';
  document.getElementById('app-overlay').classList.remove('hidden');

  const sandbox = document.getElementById('app-sandbox');

  // Inject the app's HTML into the sandbox iframe
  // The sandbox loads the app UI and WASM from the backend
  sandbox.src = `${state.backendUrl}/app/${appId}/run`;
}

function closeApp() {
  const sandbox = document.getElementById('app-sandbox');
  sandbox.src = 'about:blank';
  document.getElementById('app-overlay').classList.add('hidden');
}

// --- Settings ---
async function saveSettings() {
  await bridge('storage_set', {
    key: 'settings',
    value: new TextEncoder().encode(JSON.stringify({
      lang: state.lang,
      backendUrl: state.backendUrl,
    }))
  });
}

// --- Helpers ---
function escHtml(s) {
  if (!s) return '';
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

// --- Start ---
document.addEventListener('DOMContentLoaded', init);