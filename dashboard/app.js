// scanprojects dashboard — vanilla JS, no build step
const FRAMEWORK_COLORS = {
  nextjs: '#000000', vite: '#646CFF', express: '#68A063',
  'create-react-app': '#61DAFB', 'rust-web': '#DEA584', rust: '#DEA584',
  go: '#00ADD8', django: '#092E20', fastapi: '#009688', rails: '#CC0000',
  'docker-compose': '#2496ED', fly: '#7B36ED', node: '#68A063', flask: '#000000',
};
const FRAMEWORK_LABELS = {
  nextjs: 'Next', vite: 'Vite', express: 'Expr', 'create-react-app': 'React',
  'rust-web': 'Rust', rust: 'Rust', go: 'Go', django: 'Djng', fastapi: 'Fast',
  rails: 'Rails', 'docker-compose': 'Dock', fly: 'Fly', node: 'Node', flask: 'Flask',
};

let projects = [];
// envSecrets: [{key, value, file, file_path, projectId, projectName}]
let envSecrets = [];
// storeSecrets: [{key, projectName: string|null}]
let storeSecrets = [];
let revealedSecrets = {}; // "key@scope" → plaintext value
let editingSecret = null; // "key@scope" currently being edited
let filter = '';
let ws = null;
let connected = false;
let lastScan = null;
let toasts = [];

// Pending-update state — changes are buffered here instead of triggering
// a full re-render on every scan cycle, which caused the 5 s flicker.
let pendingCount = 0;
let pendingTimer = null;

const $ = (sel) => document.querySelector(sel);
const app = () => $('#app');

function render() {
  // Consume any buffered pending state — render IS the flush.
  pendingCount = 0;
  clearTimeout(pendingTimer);

  const filtered = projects.filter(p =>
    !filter || p.name.toLowerCase().includes(filter.toLowerCase()) ||
    (p.framework || '').toLowerCase().includes(filter.toLowerCase()) ||
    p.ports.some(port => String(port).includes(filter))
  );

  const identified = filtered.filter(p => p.framework || p.path);
  const unresolved = filtered.filter(p => !p.framework && !p.path);

  const projectCount = projects.length;
  const portCount = projects.reduce((sum, p) => sum + p.ports.length, 0);
  const scanAgo = lastScan ? timeSince(lastScan) : '...';
  const scanStale = lastScan && (Date.now() - lastScan) > 15000;

  let html = `
    ${!connected ? '<div class="banner warning">Connection lost. Reconnecting...</div>' : ''}
    <div class="header">
      <h1>scanprojects</h1>
      <div class="stats">
        ${projectCount} project${projectCount !== 1 ? 's' : ''} &middot;
        ${portCount} port${portCount !== 1 ? 's' : ''} &middot;
        <span class="live-dot"></span>scanned <span id="scan-time" class="${scanStale ? 'stale' : ''}">${scanAgo}</span>
      </div>
    </div>
    <div class="search">
      <input type="text" placeholder="Filter by name, framework, or port..."
             value="${filter}" oninput="window._filter(this.value)"
             aria-label="Filter projects">
    </div>`;

  if (projects.length === 0 && connected) {
    html += `
      <div class="empty-state">
        <h2>No projects detected</h2>
        <p>Start a dev server in another terminal and it will appear here automatically.</p>
        <code>$ npm run dev</code>
        <code>$ cargo run</code>
        <code>$ python manage.py runserver</code>
      </div>`;
  } else {
    if (!connected && projects.length === 0) {
      html += `
        <div class="skeleton-row"><div class="skeleton" style="width:24px;height:16px"></div><div class="skeleton" style="width:160px;height:16px"></div><div class="skeleton" style="width:48px;height:20px"></div><div class="skeleton" style="width:60px;height:16px"></div></div>
        <div class="skeleton-row"><div class="skeleton" style="width:24px;height:16px"></div><div class="skeleton" style="width:120px;height:16px"></div><div class="skeleton" style="width:48px;height:20px"></div><div class="skeleton" style="width:60px;height:16px"></div></div>
        <div class="skeleton-row"><div class="skeleton" style="width:24px;height:16px"></div><div class="skeleton" style="width:140px;height:16px"></div><div class="skeleton" style="width:48px;height:20px"></div><div class="skeleton" style="width:60px;height:16px"></div></div>`;
    }

    if (identified.length > 0) {
      html += '<div class="project-list" role="grid">';
      identified.forEach((p, i) => { html += projectRow(p, i); });
      html += '</div>';
    }

    if (unresolved.length > 0) {
      html += '<div class="section-label">Unresolved Ports</div>';
      html += '<div class="project-list unresolved" role="grid">';
      unresolved.forEach((p, i) => { html += projectRow(p, identified.length + i); });
      html += '</div>';
    }
  }

  // Secrets panel — populated async after render
  html += `<div id="secrets-panel">${secretsSection()}</div>`;

  // Toasts
  toasts.forEach(t => {
    html += `<div class="toast ${t.type}">${t.message}</div>`;
  });

  app().innerHTML = html;
}

function projectRow(p, index) {
  const fw = p.framework || '?';
  const color = FRAMEWORK_COLORS[fw] || '#888888';
  const label = FRAMEWORK_LABELS[fw] || '?';
  const ports = p.ports.map(port => `:${port}`).join(' ');
  const uptime = formatUptime(p.uptime_seconds || 0);
  const delay = index * 50;

  // Dark mode needs light text on dark badge colors
  const textColor = isLightColor(color) ? '#000' : '#fff';

  return `
    <div class="project-row" role="row" style="animation-delay:${delay}ms"
         tabindex="0" aria-label="${p.name} on ${ports}">
      <button class="fav-btn ${p.favorite ? 'active' : ''}"
              onclick="window._toggleFav(${p.id})"
              aria-label="${p.favorite ? 'Unfavorite' : 'Favorite'} ${p.name}">
        ${p.favorite ? '★' : '☆'}
      </button>
      <span class="project-name">${esc(p.name)}</span>
      <span class="badge" style="background:${color};color:${textColor}">${label}</span>
      <span class="ports">${ports || '-'}</span>
      <span class="uptime">${uptime}</span>
      <div class="actions">
        ${p.ports.length > 0 ? `<button class="open-btn" onclick="window._openInBrowser(${p.ports[0]})" aria-label="Open in browser">Open</button>` : ''}
        ${p.start_cmd ? `<button class="restart-btn" onclick="window._restart(${p.id}, '${esc(p.name)}')" aria-label="Restart ${esc(p.name)}">Restart</button>` : ''}
        <button class="kill-btn" onclick="window._kill(${p.id}, '${esc(p.name)}')"
                aria-label="Kill ${esc(p.name)}">Kill</button>
      </div>
      ${p.start_cmd ? `<div class="start-cmd">$ ${esc(p.start_cmd)}</div>` : ''}
    </div>`;
}

function secretsSection() {
  const projectOptions = projects
    .map(p => `<option value="${esc(p.name)}">${esc(p.name)}</option>`)
    .join('');

  let html = `
    <div class="section-label-row">
      <span class="section-label">Secrets</span>
      <form class="add-secret-form" onsubmit="window._setSecret(event)">
        <input name="key" placeholder="KEY" required autocomplete="off" spellcheck="false">
        <input name="value" type="password" placeholder="value" required autocomplete="new-password">
        <select name="project">
          <option value="">global (store)</option>
          ${projectOptions}
        </select>
        <button type="submit" class="set-btn">Add to store</button>
      </form>
    </div>`;

  const hasEnv = envSecrets.length > 0;
  const hasStore = storeSecrets.length > 0;

  if (!hasEnv && !hasStore) {
    html += `<div class="secrets-empty">No secrets found. Start a project with a .env file or add one to the store.</div>`;
    return html;
  }

  // ── .env file secrets grouped by project ──────────────────────────────────
  if (hasEnv) {
    // Group by project
    const byProject = {};
    envSecrets.forEach(s => {
      const k = s.projectName;
      if (!byProject[k]) byProject[k] = [];
      byProject[k].push(s);
    });

    Object.entries(byProject).forEach(([projectName, entries]) => {
      html += `<div class="section-label" style="padding-left:16px">${esc(projectName)}</div>`;
      entries.forEach(({ key, value, file, file_path, projectId }) => {
        const scopeKey = `${key}@env:${projectId}`;
        const revealed = revealedSecrets[scopeKey];
        const isEditing = editingSecret === scopeKey;
        html += `
          <div class="secret-row">
            <span class="scope-tag file-tag" title="${esc(file_path)}">${esc(file)}</span>
            <span class="secret-key">${esc(key)}</span>
            ${isEditing
              ? `<form class="secret-edit-form" onsubmit="window._saveEnvEdit(event,'${esc(file_path)}',${projectId},'${esc(key)}')">
                   <input class="secret-edit-input" name="val" value="${esc(value)}" autocomplete="off" spellcheck="false">
                   <button type="submit" class="set-btn">Save</button>
                   <button type="button" class="reveal-btn" onclick="window._cancelEdit()">Cancel</button>
                 </form>`
              : `<span class="secret-value-cell ${revealed ? 'revealed' : ''}">
                   ${revealed ? esc(value) : '••••••••'}
                 </span>`
            }
            <div class="secret-actions">
              ${!isEditing ? `
                <button class="reveal-btn"
                  onclick="window._toggleEnvReveal('${esc(key)}',${projectId})">
                  ${revealed ? 'Hide' : 'Reveal'}
                </button>
                <button class="reveal-btn"
                  onclick="window._editEnvSecret('${scopeKey}')">
                  Edit
                </button>` : ''}
            </div>
          </div>`;
      });
    });
  }

  // ── Encrypted store secrets ────────────────────────────────────────────────
  if (hasStore) {
    html += `<div class="section-label" style="padding-left:16px">Encrypted store</div>`;
    storeSecrets.forEach(({ key, projectName }) => {
      const scopeKey = `${key}@store:${projectName || ''}`;
      const revealed = revealedSecrets[scopeKey];
      const scopeLabel = projectName || 'global';
      const isGlobal = !projectName;
      html += `
        <div class="secret-row">
          <span class="scope-tag ${isGlobal ? 'global' : ''}">${esc(scopeLabel)}</span>
          <span class="secret-key">${esc(key)}</span>
          <span class="secret-value-cell ${revealed ? 'revealed' : ''}">
            ${revealed ? esc(revealed) : '••••••••'}
          </span>
          <div class="secret-actions">
            <button class="reveal-btn"
              onclick="window._revealStoreSecret('${esc(key)}', ${projectName ? `'${esc(projectName)}'` : 'null'})">
              ${revealed ? 'Hide' : 'Reveal'}
            </button>
            <button class="delete-secret-btn"
              onclick="window._deleteSecret('${esc(key)}', ${projectName ? `'${esc(projectName)}'` : 'null'})">
              Delete
            </button>
          </div>
        </div>`;
    });
  }

  return html;
}

function _refreshSecretsPanel() {
  const panel = document.getElementById('secrets-panel');
  if (panel) panel.innerHTML = secretsSection();
}

async function loadSecrets() {
  try {
    // Load env file secrets for every project in parallel
    const envResults = await Promise.all(
      projects.map(async p => {
        const entries = await fetch(`/projects/${p.id}/env`).then(r => r.json()).catch(() => []);
        return entries.map(e => ({ ...e, projectId: p.id, projectName: p.name }));
      })
    );
    envSecrets = envResults.flat();

    // Load encrypted store secrets (global + per-project)
    const globalKeys = await fetch('/secrets').then(r => r.json()).catch(() => []);
    const storePerProject = await Promise.all(
      projects.map(async p => {
        const keys = await fetch(`/secrets?project=${encodeURIComponent(p.name)}`)
          .then(r => r.json()).catch(() => []);
        return keys.map(k => ({ key: k, projectName: p.name }));
      })
    );
    storeSecrets = [
      ...globalKeys.map(k => ({ key: k, projectName: null })),
      ...storePerProject.flat(),
    ];

    _refreshSecretsPanel();
  } catch (_) {
    // silently ignore — panel stays with previous state
  }
}

// Actions
window._filter = (val) => { filter = val; render(); };

window._toggleFav = async (id) => {
  const p = projects.find(p => p.id === id);
  if (!p) return;
  p.favorite = !p.favorite;
  render();
  await fetch(`/projects/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ favorite: p.favorite }),
  });
};

window._kill = async (id, name) => {
  const btn = event.target;
  btn.disabled = true;
  btn.textContent = 'Killing...';
  try {
    const resp = await fetch(`/projects/${id}/kill`, { method: 'POST' });
    if (resp.ok) {
      showToast(`Killed: ${name}`, 'success');
      const p = projects.find(p => p.id === id);
      if (p) { p.ports = []; p.pids = []; }
      render();
    } else {
      const data = await resp.json().catch(() => ({}));
      showToast(`Kill failed: ${data.message || 'Unknown error'}`, 'error');
      btn.disabled = false;
      btn.textContent = 'Kill';
    }
  } catch (e) {
    showToast(`Kill failed: ${e.message}`, 'error');
    btn.disabled = false;
    btn.textContent = 'Kill';
  }
};

window._restart = async (id, name) => {
  const btn = event.target;
  btn.disabled = true;
  btn.textContent = 'Restarting...';
  try {
    const resp = await fetch(`/projects/${id}/restart`, { method: 'POST' });
    if (resp.ok) {
      showToast(`Restarting: ${name}`, 'success');
    } else {
      const data = await resp.json().catch(() => ({}));
      showToast(`Restart failed: ${data.message || 'No start command configured'}`, 'error');
    }
  } catch (e) {
    showToast(`Restart failed: ${e.message}`, 'error');
  } finally {
    btn.disabled = false;
    btn.textContent = 'Restart';
  }
};

window._openInBrowser = (port) => {
  window.open(`http://localhost:${port}`, '_blank');
};

// ── Env-file secret actions ───────────────────────────────────────────────────

window._toggleEnvReveal = (key, projectId) => {
  const scopeKey = `${key}@env:${projectId}`;
  if (revealedSecrets[scopeKey]) {
    delete revealedSecrets[scopeKey];
  } else {
    // Value is already in envSecrets state — no round-trip needed
    const entry = envSecrets.find(s => s.key === key && s.projectId === projectId);
    if (entry) revealedSecrets[scopeKey] = entry.value;
  }
  _refreshSecretsPanel();
};

window._editEnvSecret = (scopeKey) => {
  editingSecret = scopeKey;
  _refreshSecretsPanel();
  // Focus the input after render
  requestAnimationFrame(() => {
    const input = document.querySelector('.secret-edit-input');
    if (input) { input.focus(); input.select(); }
  });
};

window._cancelEdit = () => {
  editingSecret = null;
  _refreshSecretsPanel();
};

window._saveEnvEdit = async (event, filePath, projectId, key) => {
  event.preventDefault();
  const form = event.target;
  const newValue = form.val.value;
  const btn = form.querySelector('[type=submit]');
  btn.disabled = true;
  btn.textContent = 'Saving...';
  try {
    const resp = await fetch(
      `/projects/${projectId}/env/${encodeURIComponent(key)}`,
      {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ value: newValue, file_path: filePath }),
      }
    );
    if (resp.ok) {
      // Update local state
      const entry = envSecrets.find(s => s.key === key && s.projectId === projectId);
      if (entry) entry.value = newValue;
      const scopeKey = `${key}@env:${projectId}`;
      if (revealedSecrets[scopeKey]) revealedSecrets[scopeKey] = newValue;
      editingSecret = null;
      _refreshSecretsPanel();
      showToast(`Saved: ${key}`, 'success');
    } else {
      const data = await resp.json().catch(() => ({}));
      showToast(`Save failed: ${data.error || 'unknown error'}`, 'error');
      btn.disabled = false;
      btn.textContent = 'Save';
    }
  } catch (e) {
    showToast(`Save failed: ${e.message}`, 'error');
    btn.disabled = false;
    btn.textContent = 'Save';
  }
};

// ── Encrypted store secret actions ────────────────────────────────────────────

window._revealStoreSecret = async (key, projectName) => {
  const scopeKey = `${key}@store:${projectName || ''}`;
  if (revealedSecrets[scopeKey]) {
    delete revealedSecrets[scopeKey];
    _refreshSecretsPanel();
    return;
  }
  const qs = projectName ? `?project=${encodeURIComponent(projectName)}` : '';
  try {
    const data = await fetch(`/secrets/${encodeURIComponent(key)}${qs}`).then(r => r.json());
    revealedSecrets[scopeKey] = data.value;
    _refreshSecretsPanel();
  } catch (_) {
    showToast('Failed to reveal secret', 'error');
  }
};

window._deleteSecret = async (key, projectName) => {
  const qs = projectName ? `?project=${encodeURIComponent(projectName)}` : '';
  try {
    const resp = await fetch(`/secrets/${encodeURIComponent(key)}${qs}`, { method: 'DELETE' });
    if (resp.ok) {
      delete revealedSecrets[`${key}@store:${projectName || ''}`];
      storeSecrets = storeSecrets.filter(s => !(s.key === key && s.projectName === projectName));
      _refreshSecretsPanel();
      showToast(`Deleted: ${key}`, 'success');
    } else {
      showToast('Delete failed', 'error');
    }
  } catch (e) {
    showToast(`Delete failed: ${e.message}`, 'error');
  }
};

window._setSecret = async (event) => {
  event.preventDefault();
  const form = event.target;
  const key = form.key.value.trim();
  const value = form.value.value;
  const project = form.project.value || undefined;
  const btn = form.querySelector('.set-btn');
  btn.disabled = true;
  btn.textContent = 'Saving...';
  try {
    const resp = await fetch('/secrets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ key, value, project }),
    });
    if (resp.ok) {
      form.key.value = '';
      form.value.value = '';
      showToast(`Stored: ${key}`, 'success');
      await loadSecrets();
    } else {
      showToast('Failed to store secret', 'error');
    }
  } catch (e) {
    showToast(`Failed: ${e.message}`, 'error');
  } finally {
    btn.disabled = false;
    btn.textContent = 'Add to store';
  }
};

// Flush pending updates immediately (called by badge click or auto-timer).
window._flushPending = () => {
  pendingCount = 0;
  clearTimeout(pendingTimer);
  render();
};

function showToast(message, type) {
  const toast = { message, type, id: Date.now() };
  toasts.push(toast);
  render();
  setTimeout(() => {
    toasts = toasts.filter(t => t.id !== toast.id);
    render();
  }, 5000);
}

// Buffer a data change instead of re-rendering immediately.
// Shows a ↻ badge in the header; auto-flushes after 3 s of quiet.
function deferRender() {
  pendingCount++;
  _injectOrUpdateBadge();
  clearTimeout(pendingTimer);
  pendingTimer = setTimeout(window._flushPending, 3000);
}

function _injectOrUpdateBadge() {
  let badge = document.getElementById('update-badge');
  if (!badge) {
    const header = document.querySelector('.header');
    if (!header) return;
    badge = document.createElement('button');
    badge.id = 'update-badge';
    badge.className = 'update-badge';
    badge.title = 'Updates ready — click to apply';
    badge.onclick = window._flushPending;
    header.appendChild(badge);
  }
  badge.textContent = pendingCount > 1 ? `↻ ${pendingCount}` : '↻';
}

// WebSocket
function connectWs() {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  ws = new WebSocket(`${proto}//${location.host}/ws`);

  ws.onopen = () => { connected = true; render(); };

  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    switch (msg.type) {
      case 'full_sync':
        // Full sync on connect/reconnect — apply immediately, clear any stale pending.
        projects = msg.data;
        pendingCount = 0;
        clearTimeout(pendingTimer);
        render();
        loadSecrets();
        break;
      case 'project_added':
        projects.push(msg.data);
        deferRender();
        break;
      case 'project_updated':
        projects = projects.map(p => p.id === msg.data.id ? msg.data : p);
        deferRender();
        break;
      case 'project_removed':
        projects = projects.filter(p => p.id !== msg.id);
        deferRender();
        break;
      case 'scan_completed':
        // Just update the timestamp — the setInterval handles the display.
        // No render: this is the heartbeat that was causing the 5 s flicker.
        lastScan = Date.now();
        break;
    }
  };

  ws.onclose = () => {
    connected = false;
    render();
    setTimeout(connectWs, 3000);
  };

  ws.onerror = () => { ws.close(); };
}

// Helpers
function formatUptime(seconds) {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${h}h ${m}m`;
}

function timeSince(ts) {
  const seconds = Math.floor((Date.now() - ts) / 1000);
  if (seconds < 5) return 'just now';
  if (seconds < 60) return `${seconds}s ago`;
  return `${Math.floor(seconds / 60)}m ago`;
}

function esc(str) {
  const div = document.createElement('div');
  div.textContent = str || '';
  return div.innerHTML;
}

function isLightColor(hex) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return (r * 299 + g * 587 + b * 114) / 1000 > 128;
}

// Update the scan timestamp every second without a full re-render.
setInterval(() => {
  if (lastScan) {
    const el = document.getElementById('scan-time');
    if (el) {
      el.textContent = timeSince(lastScan);
      el.className = (Date.now() - lastScan) > 15000 ? 'stale' : '';
    }
  }
}, 1000);

// Boot
connectWs();
render();
