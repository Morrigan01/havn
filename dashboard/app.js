// havn dashboard — vanilla JS, no build step
const FRAMEWORK_COLORS = {
  nextjs: '#000000', vite: '#646CFF', express: '#68A063',
  'create-react-app': '#61DAFB', 'rust-web': '#DEA584', rust: '#DEA584',
  go: '#00ADD8', django: '#092E20', fastapi: '#009688', rails: '#CC0000',
  'docker-compose': '#2496ED', fly: '#7B36ED', node: '#68A063', flask: '#000000',
};
// Simple Icons slugs — https://simpleicons.org
const FRAMEWORK_ICONS = {
  nextjs: 'nextdotjs', vite: 'vite', express: 'express',
  'create-react-app': 'react', 'rust-web': 'rust', rust: 'rust',
  go: 'go', django: 'django', fastapi: 'fastapi', rails: 'rubyonrails',
  'docker-compose': 'docker', fly: 'flydotio', node: 'nodedotjs', flask: 'flask',
};
const FRAMEWORK_NAMES = {
  nextjs: 'Next.js', vite: 'Vite', express: 'Express',
  'create-react-app': 'Create React App', 'rust-web': 'Rust (web)', rust: 'Rust',
  go: 'Go', django: 'Django', fastapi: 'FastAPI', rails: 'Ruby on Rails',
  'docker-compose': 'Docker Compose', fly: 'Fly.io', node: 'Node.js', flask: 'Flask',
};

let projects = [];
// envSecrets: [{key, value, file, file_path, projectId, projectName}]
let envSecrets = [];
// storeSecrets: [{key, projectName: string|null}]
let storeSecrets = [];
let revealedSecrets = {};       // "key@scope" → plaintext value
let editingSecret = null;       // "key@scope" currently being edited
let collapsedProjects = new Set(); // project names collapsed in secrets panel
let dismissedDuplicates = new Set(); // keys user dismissed from duplicate suggestions
let restartingProjects = new Set(); // project ids currently restarting
let filter = '';
let ws = null;
let connected = false;
let lastScan = null;
let toasts = [];

// Pending-update state — changes are buffered here instead of triggering
// a full re-render on every scan cycle, which caused the 5 s flicker.
let pendingCount = 0;
let pendingTimer = null;
let activeTab = 'projects'; // 'projects' | 'secrets' | 'profiles'

// ─── Detail panel ──────────────────────────────────────────────────────────
let selectedProjectId = null;
let panelData = {};  // id → { git, health, resources, logs }

// ─── Profiles ──────────────────────────────────────────────────────────────
let profiles = [];

const $ = (sel) => document.querySelector(sel);
const app = () => $('#app');

// ─── Theme ────────────────────────────────────────────────────────────────────
function initTheme() {
  const saved = localStorage.getItem('theme'); // 'light' | 'dark' | null
  if (saved) document.documentElement.classList.add(saved);
}

function isDark() {
  if (document.documentElement.classList.contains('dark')) return true;
  if (document.documentElement.classList.contains('light')) return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

window._toggleTheme = () => {
  const root = document.documentElement;
  const dark = isDark();
  root.classList.remove('dark', 'light');
  root.classList.add(dark ? 'light' : 'dark');
  localStorage.setItem('theme', dark ? 'light' : 'dark');
  // Update the button label without a full re-render
  const btn = document.getElementById('theme-btn');
  if (btn) btn.textContent = dark ? '○ Light' : '● Dark';
};

initTheme();

function render() {
  pendingCount = 0;
  clearTimeout(pendingTimer);

  const filtered = projects.filter(p =>
    !filter || p.name.toLowerCase().includes(filter.toLowerCase()) ||
    (p.framework || '').toLowerCase().includes(filter.toLowerCase()) ||
    p.ports.some(port => String(port).includes(filter))
  );

  const identified = filtered.filter(p => p.framework || p.path);
  const unresolved = filtered.filter(p => !p.framework && !p.path);
  const activeCount = projects.filter(p => p.ports.length > 0).length;
  const portCount = projects.reduce((sum, p) => sum + p.ports.length, 0);
  const totalSecrets = envSecrets.length + storeSecrets.length;
  const scanAgo = lastScan ? timeSince(lastScan) : '...';
  const scanStale = lastScan && (Date.now() - lastScan) > 15000;

  let html = `
    ${!connected ? '<div class="banner warning">Connection lost. Reconnecting...</div>' : ''}
    <div class="layout">
      <aside class="rail">
        <div class="rail-brand">havn</div>
        <div class="rail-stats">
          <div class="rail-stat-block">
            <span class="rail-count">${String(projects.length).padStart(2, '0')}</span>
            <span class="rail-label">projects</span>
            <span class="rail-sublabel">${activeCount} active</span>
          </div>
          <div class="rail-stat-block">
            <span class="rail-count">${String(portCount).padStart(2, '0')}</span>
            <span class="rail-label">ports</span>
          </div>
        </div>
        <div class="rail-scan">
          <span class="live-dot"></span>
          <span id="scan-time" class="${scanStale ? 'stale' : ''}">${scanAgo}</span>
        </div>
        <nav class="rail-tabs">
          <button class="rail-tab ${activeTab === 'projects' ? 'active' : ''}"
                  onclick="window._setTab('projects')">Projects</button>
          <button class="rail-tab ${activeTab === 'secrets' ? 'active' : ''}"
                  onclick="window._setTab('secrets')">
            Secrets${totalSecrets > 0 ? ` <span class="secrets-count">${totalSecrets}</span>` : ''}
          </button>
          <button class="rail-tab ${activeTab === 'profiles' ? 'active' : ''}"
                  onclick="window._setTab('profiles')">Profiles${profiles.length > 0 ? ` <span class="secrets-count">${profiles.length}</span>` : ''}</button>
        </nav>
        <div class="rail-bottom">
          <input class="rail-search" type="text" placeholder="Filter…" value="${filter}"
                 oninput="window._filter(this.value)" aria-label="Filter projects"
                 style="width:100%;padding:6px 10px;border:1px solid var(--border-2);border-radius:3px;background:var(--bg-primary);color:var(--text-primary);font-family:var(--font-mono);font-size:12px;outline:none;">
          <button id="theme-btn" class="theme-btn" onclick="window._toggleTheme()">
            ${isDark() ? '● Dark' : '○ Light'}
          </button>
        </div>
      </aside>
      <div class="content-area${selectedProjectId ? ' panel-open' : ''}">
      <main class="board">`;

  if (activeTab === 'secrets') {
    html += `<div id="secrets-panel">${secretsSection()}</div>`;
  } else if (activeTab === 'profiles') {
    html += profilesBoard();
  } else {
    if (projects.length === 0 && connected) {
      html += `
        <div class="empty-state">
          <h2>No projects detected</h2>
          <p>Start a dev server in another terminal.</p>
          <code>$ npm run dev</code>
          <code>$ cargo run</code>
          <code>$ python manage.py runserver</code>
        </div>`;
    } else if (!connected && projects.length === 0) {
      html += `
        <div class="skeleton-row"><div class="skeleton" style="width:160px;height:15px"></div><div class="skeleton" style="width:48px;height:20px"></div><div class="skeleton" style="width:60px;height:13px"></div></div>
        <div class="skeleton-row"><div class="skeleton" style="width:120px;height:15px"></div><div class="skeleton" style="width:48px;height:20px"></div><div class="skeleton" style="width:60px;height:13px"></div></div>
        <div class="skeleton-row"><div class="skeleton" style="width:140px;height:15px"></div><div class="skeleton" style="width:48px;height:20px"></div><div class="skeleton" style="width:60px;height:13px"></div></div>`;
    } else {
      identified.forEach((p, i) => { html += projectCard(p, i); });
      if (unresolved.length > 0) {
        html += `<div class="section-label" style="margin-top:16px">Unresolved Ports</div>`;
        unresolved.forEach((p, i) => { html += projectCard(p, identified.length + i, true); });
      }
    }
  }

  const selectedProject = selectedProjectId ? projects.find(p => p.id === selectedProjectId) : null;
  html += `
      </main>
      ${selectedProject ? renderDetailPanel(selectedProject) : ''}
      </div>
    </div>`;

  toasts.forEach(t => { html += `<div class="toast ${t.type}">${t.message}</div>`; });

  // Preserve filter input focus + cursor position across the innerHTML replace.
  const activeEl = document.activeElement;
  const wasFilterFocused = activeEl && activeEl.getAttribute('aria-label') === 'Filter projects';
  const selStart = wasFilterFocused ? activeEl.selectionStart : null;
  const selEnd   = wasFilterFocused ? activeEl.selectionEnd   : null;

  app().innerHTML = html;

  if (wasFilterFocused) {
    const input = document.querySelector('[aria-label="Filter projects"]');
    if (input) {
      input.focus();
      input.setSelectionRange(selStart, selEnd);
    }
  }
}

function frameworkBadge(fw) {
  const color = FRAMEWORK_COLORS[fw] || '#888888';
  const textColor = isLightColor(color) ? '#000' : '#fff';
  const iconSlug = FRAMEWORK_ICONS[fw];
  const fwName = FRAMEWORK_NAMES[fw] || fw;
  if (iconSlug) {
    const iconColor = textColor === '#fff' ? 'ffffff' : '000000';
    return `<span class="badge" style="background:${color}" data-tooltip="${fwName}">
      <img src="https://cdn.simpleicons.org/${iconSlug}/${iconColor}" width="13" height="13" alt="${fwName}" loading="lazy">
    </span>`;
  }
  return `<span class="badge" style="background:${color};color:${textColor}" data-tooltip="${fwName}">${fwName}</span>`;
}

function projectCard(p, index, dim = false) {
  const fw = p.framework || '?';
  const ports = p.ports.map(port => `:${port}`).join(' ');
  const uptime = formatUptime(p.uptime_seconds || 0);
  const delay = index * 40;
  const isRestarting = restartingProjects.has(p.id);

  return `
    <div class="card ${dim ? 'card-dim' : ''} ${isRestarting ? 'card-restarting' : ''} ${selectedProjectId === p.id ? 'card-selected' : ''}" style="animation-delay:${delay}ms"
         tabindex="0" aria-label="${p.name}${ports ? ' on ' + ports : ''}">
      ${isRestarting ? `<div class="restart-overlay"><span class="restart-spinner"></span> Restarting…</div>` : ''}
      <div class="card-top" onclick="window._selectProject(${p.id})" style="cursor:pointer">
        <button class="fav-btn ${p.favorite ? 'active' : ''}" onclick="window._toggleFav(${p.id})"
                aria-label="${p.favorite ? 'Unfavorite' : 'Favorite'} ${p.name}">
          ${p.favorite ? '★' : '☆'}
        </button>
        ${(()=>{
          const slash = p.name.indexOf('/');
          if (slash === -1) return `<span class="project-name">${esc(p.name)}</span>`;
          const parent = p.name.slice(0, slash);
          const child  = p.name.slice(slash + 1);
          return `<span class="project-name">
            <span class="project-parent">${esc(parent)}/</span>${esc(child)}
          </span>`;
        })()}
        ${frameworkBadge(fw)}
      </div>
      <div class="card-bottom">
        ${p.start_cmd ? `<span class="start-cmd">$ ${esc(p.start_cmd)}</span>` : ''}
        ${p.ports.length > 1
          ? /* multi-process: one row per port/pid */
            p.ports.map((port, i) => {
              const pid = p.pids[i];
              const isRowRestarting = restartingProjects.has(`${p.id}:${port}`);
              return `<div class="process-row">
                <span class="ports">:${port}</span>
                <span class="uptime">${uptime}</span>
                <div class="card-actions">
                  <button class="open-btn" onclick="window._openInBrowser(${port})">Open</button>
                  ${p.start_cmd ? `<button class="restart-btn" ${isRowRestarting ? 'disabled' : ''} onclick="window._restartProcess(${p.id},${port},'${esc(p.name)}',${pid})">${isRowRestarting ? 'Restarting…' : 'Restart'}</button>` : ''}
                  <button class="kill-hold-btn" ${isRowRestarting ? 'disabled' : ''}
                    onmousedown="window._startKillHold(this,${p.id},'${esc(p.name)}',${port})"
                    onmouseup="window._cancelKillHold(this)"
                    onmouseleave="window._cancelKillHold(this)"
                    ontouchstart="window._startKillHold(this,${p.id},'${esc(p.name)}',${port})"
                    ontouchend="window._cancelKillHold(this)">Kill</button>
                </div>
              </div>`;
            }).join('')
          : /* single process: original layout */
            `<div class="process-row">
              <span class="ports">${ports || '—'}</span>
              <span class="uptime">${uptime}</span>
              <div class="card-actions">
                ${p.ports.length > 0 ? `<button class="open-btn" onclick="window._openInBrowser(${p.ports[0]})">Open</button>` : ''}
                ${p.start_cmd ? `<button class="restart-btn" ${isRestarting ? 'disabled' : ''} onclick="window._restart(${p.id},'${esc(p.name)}')">${isRestarting ? 'Restarting…' : 'Restart'}</button>` : ''}
                <button class="kill-hold-btn" ${isRestarting ? 'disabled' : ''}
                  onmousedown="window._startKillHold(this,${p.id},'${esc(p.name)}')"
                  onmouseup="window._cancelKillHold(this)"
                  onmouseleave="window._cancelKillHold(this)"
                  ontouchstart="window._startKillHold(this,${p.id},'${esc(p.name)}')"
                  ontouchend="window._cancelKillHold(this)">Kill</button>
              </div>
            </div>`
        }
      </div>
    </div>`;
}

function secretsSection() {
  const projectOptions = projects
    .map(p => `<option value="${esc(p.name)}">${esc(p.name)}</option>`)
    .join('');

  const totalCount = envSecrets.length + storeSecrets.length;
  const countLabel = totalCount > 0 ? ` <span class="secrets-count">${totalCount}</span>` : '';

  let html = `
    <div class="section-label-row">
      <span class="section-label">Secrets${countLabel}</span>
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

  const globalStore = storeSecrets.filter(s => !s.projectName);
  const hasEnv = envSecrets.length > 0;

  if (!hasEnv && storeSecrets.length === 0) {
    html += `<div class="secrets-empty">No secrets found. Start a project with a .env file or add one to the store.</div>`;
    return html;
  }

  // ── Global store secrets — always visible at top ───────────────────────────
  if (globalStore.length > 0) {
    globalStore.forEach(({ key }) => {
      const scopeKey = `${key}@store:`;
      const revealed = revealedSecrets[scopeKey];
      html += `
        <div class="secret-row">
          <span class="scope-tag global">global</span>
          <span class="secret-key">${esc(key)}</span>
          <span class="secret-value-cell ${revealed ? 'revealed' : ''}">${revealed ? esc(revealed) : '••••••••'}</span>
          <div class="secret-actions">
            <button class="reveal-btn" onclick="window._revealStoreSecret('${esc(key)}',null)">${revealed ? 'Hide' : 'Reveal'}</button>
            <button class="delete-secret-btn" onclick="window._deleteSecret('${esc(key)}',null)">Delete</button>
          </div>
        </div>`;
    });
  }

  // ── Duplicate key suggestions ─────────────────────────────────────────────
  const globalKeySet = new Set(globalStore.map(s => s.key));
  const duplicates = findDuplicateKeys().filter(
    ({ key, occurrences }) =>
      !dismissedDuplicates.has(key) &&
      !globalKeySet.has(key) &&
      occurrences.every(o => o.value === occurrences[0].value)
  );
  if (duplicates.length > 0) {
    html += `<div class="duplicates-banner">
      <span class="duplicates-title">⚡ ${duplicates.length} key${duplicates.length > 1 ? 's' : ''} with identical values across projects — promote to global?</span>
      <div class="duplicates-list">`;
    duplicates.forEach(({ key, occurrences }) => {
      const names = occurrences.map(o => esc(o.projectName)).join(', ');
      const safeVal = encodeURIComponent(occurrences[0].value);
      html += `
        <div class="duplicate-row">
          <span class="secret-key">${esc(key)}</span>
          <span class="duplicates-projects">${names}</span>
          <div class="secret-actions">
            <button class="set-btn" onclick="window._promoteToGlobal('${esc(key)}','${safeVal}')">Add to global</button>
            <button class="reveal-btn" onclick="window._dismissDuplicate('${esc(key)}')">Dismiss</button>
          </div>
        </div>`;
    });
    html += `</div></div>`;
  }

  // ── Per-project sections — each collapsible ────────────────────────────────
  // Build a unified list of project names that have any secrets
  const projectNames = [
    ...new Set([
      ...envSecrets.map(s => s.projectName),
      ...storeSecrets.filter(s => s.projectName).map(s => s.projectName),
    ])
  ];

  projectNames.forEach(projectName => {
    const projEnv = envSecrets.filter(s => s.projectName === projectName);
    const projStore = storeSecrets.filter(s => s.projectName === projectName);
    const count = projEnv.length + projStore.length;
    const collapsed = collapsedProjects.has(projectName);

    html += `
      <div class="project-secrets-header" onclick="window._toggleProjectSecrets('${esc(projectName)}')">
        <span class="collapse-arrow ${collapsed ? 'collapsed' : ''}">▾</span>
        <span class="project-secrets-name">${esc(projectName)}</span>
        <span class="secrets-count">${count}</span>
      </div>`;

    if (!collapsed) {
      projEnv.forEach(({ key, value, file, file_path, projectId }) => {
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
              : `<span class="secret-value-cell ${revealed ? 'revealed' : ''}">${revealed ? esc(value) : '••••••••'}</span>`
            }
            <div class="secret-actions">
              ${!isEditing ? `
                <button class="reveal-btn" onclick="window._toggleEnvReveal('${esc(key)}',${projectId})">${revealed ? 'Hide' : 'Reveal'}</button>
                <button class="reveal-btn" onclick="window._editEnvSecret('${scopeKey}')">Edit</button>` : ''}
            </div>
          </div>`;
      });

      projStore.forEach(({ key }) => {
        const scopeKey = `${key}@store:${projectName}`;
        const revealed = revealedSecrets[scopeKey];
        html += `
          <div class="secret-row">
            <span class="scope-tag">store</span>
            <span class="secret-key">${esc(key)}</span>
            <span class="secret-value-cell ${revealed ? 'revealed' : ''}">${revealed ? esc(revealed) : '••••••••'}</span>
            <div class="secret-actions">
              <button class="reveal-btn" onclick="window._revealStoreSecret('${esc(key)}','${esc(projectName)}')">${revealed ? 'Hide' : 'Reveal'}</button>
              <button class="delete-secret-btn" onclick="window._deleteSecret('${esc(key)}','${esc(projectName)}')">Delete</button>
            </div>
          </div>`;
      });
    }
  });

  return html;
}

function _refreshSecretsPanel() {
  if (activeTab === 'secrets') {
    const panel = document.getElementById('secrets-panel');
    if (panel) panel.innerHTML = secretsSection();
  }
}

// Returns [{key, occurrences: [{projectName, value}]}] for keys in 2+ projects.
function findDuplicateKeys() {
  // Gather all project-scoped keys with values (env files only — store values need a fetch)
  const byKey = {};
  envSecrets.forEach(({ key, value, projectName }) => {
    if (!byKey[key]) byKey[key] = [];
    // Avoid listing the same project twice if multiple .env* files share a key
    if (!byKey[key].find(o => o.projectName === projectName)) {
      byKey[key].push({ projectName, value });
    }
  });
  return Object.entries(byKey)
    .filter(([, occ]) => occ.length >= 2)
    .map(([key, occurrences]) => ({ key, occurrences }));
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

    // Default-collapse all project sections
    const projectNames = [...new Set([
      ...envSecrets.map(s => s.projectName),
      ...storeSecrets.filter(s => s.projectName).map(s => s.projectName),
    ])];
    projectNames.forEach(n => collapsedProjects.add(n));

    _refreshSecretsPanel();
  } catch (_) {
    // silently ignore — panel stays with previous state
  }
}

window._setTab = (tab) => {
  activeTab = tab;
  selectedProjectId = null;
  render();
  if (tab === 'secrets') loadSecrets();
  if (tab === 'profiles') loadProfiles();
};

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

// Hold-to-kill: 600 ms hold gesture, progress bar fills, then fires.
let _killHoldTimer = null;

// port is optional — if provided, kills only the process on that port; otherwise kills all.
window._startKillHold = (btn, id, name, port = null) => {
  btn.classList.add('holding');
  _killHoldTimer = setTimeout(async () => {
    btn.classList.remove('holding');
    btn.classList.add('killing');
    btn.textContent = '…';
    try {
      const killUrl = port != null ? `/kill/${port}` : `/projects/${id}/kill`;
      const resp = await fetch(killUrl, { method: 'POST' });
      if (resp.ok) {
        const p = projects.find(p => p.id === id);
        if (p) {
          if (port != null) {
            const idx = p.ports.indexOf(port);
            if (idx >= 0) { p.pids.splice(idx, 1); p.ports.splice(idx, 1); }
          } else {
            p.ports = []; p.pids = [];
          }
        }
        render();
      } else {
        showToast(`Kill failed`, 'error');
        btn.classList.remove('killing');
        btn.textContent = 'Kill';
      }
    } catch (e) {
      showToast(`Kill failed: ${e.message}`, 'error');
      btn.classList.remove('killing');
      btn.textContent = 'Kill';
    }
  }, 600);
};

window._cancelKillHold = (btn) => {
  clearTimeout(_killHoldTimer);
  btn.classList.remove('holding');
};

window._restart = async (id, name) => {
  restartingProjects.add(id);
  render();
  try {
    const resp = await fetch(`/projects/${id}/restart`, { method: 'POST' });
    if (!resp.ok) {
      const data = await resp.json().catch(() => ({}));
      showToast(`Restart failed: ${data.message || 'No start command configured'}`, 'error');
      restartingProjects.delete(id);
      render();
      return;
    }
  } catch (e) {
    showToast(`Restart failed: ${e.message || 'Server unreachable'}`, 'error');
    restartingProjects.delete(id);
    render();
    return;
  }

  // Poll until project has new PIDs (or timeout after 15s)
  const prevPids = (projects.find(p => p.id === id)?.pids || []).join(',');
  const deadline = Date.now() + 15000;
  const poll = async () => {
    if (Date.now() > deadline) {
      showToast(`${name} restart timed out`, 'error');
      restartingProjects.delete(id);
      render();
      return;
    }
    try {
      const r = await fetch('/projects');
      if (r.ok) {
        const updated = await r.json();
        const proj = updated.find(p => p.id === id);
        if (proj && proj.pids.length > 0 && proj.pids.join(',') !== prevPids) {
          projects = updated;
          restartingProjects.delete(id);
          showToast(`${name} is back online`, 'success');
          render();
          return;
        }
      }
    } catch (_) { /* server may briefly be unreachable */ }
    setTimeout(poll, 1000);
  };
  setTimeout(poll, 1500); // give the process a moment to spawn
};

// Restart a single process identified by port number within a multi-process project.
// oldPid is the PID from the registry snapshot — used only to detect when the new
// process has come up (different PID on same port).
window._restartProcess = async (id, port, name, oldPid) => {
  const key = `${id}:${port}`;
  restartingProjects.add(key);
  render();
  try {
    const resp = await fetch(`/projects/${id}/processes/${port}/restart`, { method: 'POST' });
    if (!resp.ok) {
      const data = await resp.json().catch(() => ({}));
      showToast(`Restart failed: ${data.message || 'No start command configured'}`, 'error');
      restartingProjects.delete(key);
      render();
      return;
    }
  } catch (e) {
    showToast(`Restart failed: ${e.message || 'Server unreachable'}`, 'error');
    restartingProjects.delete(key);
    render();
    return;
  }

  // Poll until this port comes back with a PID different from the one we killed.
  const deadline = Date.now() + 20000;
  const poll = async () => {
    if (Date.now() > deadline) {
      showToast(`:${port} restart timed out`, 'error');
      restartingProjects.delete(key);
      render();
      return;
    }
    try {
      const r = await fetch('/projects');
      if (r.ok) {
        const updated = await r.json();
        const proj = updated.find(p => p.id === id);
        if (proj) {
          const idx = proj.ports.indexOf(port);
          // Port is back and has a new PID (or the port reappeared after disappearing)
          if (idx >= 0 && proj.pids[idx] !== oldPid) {
            projects = updated;
            restartingProjects.delete(key);
            showToast(`:${port} is back online`, 'success');
            render();
            return;
          }
        }
      }
    } catch (_) {}
    setTimeout(poll, 1000);
  };
  setTimeout(poll, 2000); // vite takes a moment to start
};

window._openInBrowser = (port) => {
  window.open(`http://localhost:${port}`, '_blank');
};

window._dismissDuplicate = (key) => {
  dismissedDuplicates.add(key);
  _refreshSecretsPanel();
};

window._promoteToGlobal = async (key, encodedVal) => {
  const value = decodeURIComponent(encodedVal);
  const resp = await fetch('/secrets', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ key, value }),
  });
  if (resp.ok) {
    dismissedDuplicates.add(key);
    showToast(`${key} added to global store`, 'success');
    await loadSecrets();
  } else {
    showToast(`Failed to promote ${key}`, 'error');
  }
};

window._toggleProjectSecrets = (projectName) => {
  if (collapsedProjects.has(projectName)) {
    collapsedProjects.delete(projectName);
  } else {
    collapsedProjects.add(projectName);
  }
  _refreshSecretsPanel();
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
    const rail = document.querySelector('.rail-search');
    if (!rail) return;
    badge = document.createElement('button');
    badge.id = 'update-badge';
    badge.className = 'update-badge';
    badge.title = 'Updates ready — click to apply';
    badge.onclick = window._flushPending;
    rail.appendChild(badge);
  }
  badge.textContent = pendingCount > 1 ? `↻ ${pendingCount} updates` : '↻ update';
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
        loadProfiles();
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

// ─── Detail Panel ─────────────────────────────────────────────────────────────

function renderDetailPanel(p) {
  const d = panelData[p.id] || {};
  return `
    <aside class="detail-panel">
      <div class="panel-header">
        <span class="panel-title">${esc(p.name)}</span>
        <button class="panel-close-btn" onclick="window._closePanel()">✕</button>
      </div>
      <div class="panel-body">
        <div class="panel-section">
          <div class="panel-section-title">Path</div>
          <div class="panel-path">${esc(p.path || '—')}</div>
        </div>
        ${renderGitSection(p.id, d.git)}
        ${renderHealthSection(p.id, d.health)}
        ${renderResourcesSection(p.id, d.resources)}
        <div class="panel-section">
          <div class="panel-section-title">Terminal</div>
          <button class="panel-action-btn" onclick="window._openTerminal(${p.id})">Open in Terminal</button>
        </div>
        ${renderLogsSection(p.id, d.logs)}
      </div>
    </aside>`;
}

function renderGitSection(id, git) {
  const refreshBtn = `<button class="panel-refresh-btn" onclick="event.stopPropagation();window._fetchPanelData(${id},'git')">↻</button>`;
  if (!git) {
    return `<div class="panel-section">
      <div class="panel-section-title">Git ${refreshBtn}</div>
      <div class="panel-loading">Loading…</div>
    </div>`;
  }
  if (git.error) {
    return `<div class="panel-section">
      <div class="panel-section-title">Git ${refreshBtn}</div>
      <div class="panel-muted">Not a git repo</div>
    </div>`;
  }
  return `<div class="panel-section">
    <div class="panel-section-title">Git ${refreshBtn}</div>
    <div class="panel-git-row">
      <span class="panel-git-branch">⎇ ${esc(git.branch || 'unknown')}</span>
      ${git.dirty ? '<span class="panel-git-dirty">● dirty</span>' : '<span class="panel-git-clean">✓ clean</span>'}
    </div>
    ${(git.ahead > 0 || git.behind > 0) ? `<div class="panel-git-ahead-behind">
      ${git.ahead > 0 ? `<span class="panel-git-ahead">↑${git.ahead}</span>` : ''}
      ${git.behind > 0 ? `<span class="panel-git-behind">↓${git.behind}</span>` : ''}
    </div>` : ''}
  </div>`;
}

function renderHealthSection(id, health) {
  const refreshBtn = `<button class="panel-refresh-btn" onclick="event.stopPropagation();window._fetchPanelData(${id},'health')">↻</button>`;
  if (!health) {
    return `<div class="panel-section">
      <div class="panel-section-title">Health ${refreshBtn}</div>
      <div class="panel-loading">Loading…</div>
    </div>`;
  }
  const checks = Array.isArray(health) ? health : [];
  const rows = checks.map(c => {
    const ok = c.status === 'up';
    return `
    <div class="panel-health-row">
      <span class="panel-health-dot ${ok ? 'ok' : 'fail'}"></span>
      <span class="panel-health-port">:${c.port}</span>
      <span class="panel-health-status">${esc(c.status)}${c.status_code ? ' (' + c.status_code + ')' : ''}</span>
      ${c.latency_ms != null ? `<span class="panel-health-latency">${c.latency_ms}ms</span>` : ''}
    </div>`;
  }).join('');
  return `<div class="panel-section">
    <div class="panel-section-title">Health ${refreshBtn}</div>
    ${rows || '<div class="panel-muted">No ports</div>'}
  </div>`;
}

function renderResourcesSection(id, resources) {
  const refreshBtn = `<button class="panel-refresh-btn" onclick="event.stopPropagation();window._fetchPanelData(${id},'resources')">↻</button>`;
  if (!resources) {
    return `<div class="panel-section">
      <div class="panel-section-title">Resources ${refreshBtn}</div>
      <div class="panel-loading">Loading…</div>
    </div>`;
  }
  const procs = Array.isArray(resources) ? resources : [];
  const rows = procs.map(r => {
    const memMb = r.mem_rss_kb != null ? (r.mem_rss_kb / 1024).toFixed(1) : null;
    return `
    <div class="panel-resource-row">
      <span class="panel-resource-pid">pid ${r.pid}</span>
      <span class="panel-resource-cpu">${r.cpu_percent != null ? r.cpu_percent.toFixed(1) + '% cpu' : '—'}</span>
      <span class="panel-resource-mem">${memMb != null ? memMb + ' MB' : '—'}</span>
    </div>`;
  }).join('');
  return `<div class="panel-section">
    <div class="panel-section-title">Resources ${refreshBtn}</div>
    ${rows || '<div class="panel-muted">No processes</div>'}
  </div>`;
}

function renderLogsSection(id, logs) {
  const lines = logs || [];
  return `<div class="panel-section panel-logs-section">
    <div class="panel-section-title" style="display:flex;align-items:center;justify-content:space-between">
      <span>Logs</span>
      <span style="display:flex;gap:6px">
        <button class="panel-refresh-btn" onclick="event.stopPropagation();window._refreshLogs(${id})">↻</button>
        <button class="panel-refresh-btn" onclick="event.stopPropagation();window._clearLogs(${id})">Clear</button>
      </span>
    </div>
    <div class="panel-log-box" id="panel-log-box-${id}">
      ${lines.length === 0
        ? '<div class="panel-muted">No logs yet. Logs appear when a process is restarted via havn.</div>'
        : lines.map(l => `<div class="log-line">
            <span class="log-ts">${esc((l.ts || '').slice(11, 19))}</span>
            <span class="log-stream ${l.stream === 'stderr' ? 'stderr' : ''}">${l.stream === 'stderr' ? 'err' : 'out'}</span>
            <span class="log-text">${esc(l.text)}</span>
          </div>`).join('')
      }
    </div>
  </div>`;
}

window._selectProject = (id) => {
  if (selectedProjectId === id) {
    selectedProjectId = null;
    render();
    return;
  }
  selectedProjectId = id;
  if (!panelData[id]) panelData[id] = {};
  render();
  window._fetchPanelData(id, 'git');
  window._fetchPanelData(id, 'health');
  window._fetchPanelData(id, 'resources');
  window._refreshLogs(id);
};

window._closePanel = () => {
  selectedProjectId = null;
  render();
};

window._fetchPanelData = async (id, section) => {
  try {
    const resp = await fetch(`/projects/${id}/${section}`);
    if (!panelData[id]) panelData[id] = {};
    if (!resp.ok) {
      panelData[id][section] = { error: `HTTP ${resp.status}` };
    } else {
      panelData[id][section] = await resp.json();
    }
    if (selectedProjectId === id) render();
  } catch (e) {
    if (!panelData[id]) panelData[id] = {};
    panelData[id][section] = { error: e.message };
    if (selectedProjectId === id) render();
  }
};

window._openTerminal = async (id) => {
  try {
    const resp = await fetch(`/projects/${id}/terminal`, { method: 'POST' });
    if (!resp.ok) showToast('Failed to open terminal', 'error');
  } catch (e) {
    showToast(`Terminal failed: ${e.message}`, 'error');
  }
};

window._refreshLogs = async (id) => {
  try {
    const logs = await fetch(`/projects/${id}/logs?n=200`).then(r => r.json());
    if (!panelData[id]) panelData[id] = {};
    panelData[id].logs = logs;
    if (selectedProjectId === id) {
      render();
      requestAnimationFrame(() => {
        const box = document.getElementById(`panel-log-box-${id}`);
        if (box) box.scrollTop = box.scrollHeight;
      });
    }
  } catch (_) {}
};

window._clearLogs = async (id) => {
  try {
    await fetch(`/projects/${id}/logs`, { method: 'DELETE' });
    if (panelData[id]) panelData[id].logs = [];
    if (selectedProjectId === id) render();
  } catch (e) {
    showToast(`Clear failed: ${e.message}`, 'error');
  }
};

// ─── Profiles ─────────────────────────────────────────────────────────────────

async function loadProfiles() {
  try {
    profiles = await fetch('/profiles').then(r => r.json());
    if (activeTab === 'profiles') render();
  } catch (_) {}
}

function profilesBoard() {
  let html = `
    <div class="profiles-header">
      <span class="section-label" style="padding:0">Profiles</span>
      <form class="add-profile-form" onsubmit="window._createProfile(event)">
        <input name="name" placeholder="Profile name…" required autocomplete="off">
        <button type="submit" class="set-btn">Create</button>
      </form>
    </div>`;

  if (profiles.length === 0) {
    html += `<div class="panel-muted" style="padding:24px 16px">No profiles yet. Create one to group projects together.</div>`;
    return html;
  }

  profiles.forEach(profile => { html += renderProfileCard(profile); });
  return html;
}

function renderProfileCard(profile) {
  const profileProjects = profile.project_ids
    .map(id => projects.find(p => p.id === id))
    .filter(Boolean);

  const allRunning = profileProjects.length > 0 && profileProjects.every(p => p.ports.length > 0);

  return `
    <div class="profile-card">
      <div class="profile-card-header">
        <span class="profile-name">${esc(profile.name)}</span>
        <div class="profile-header-actions">
          ${profileProjects.length > 0
            ? (allRunning
                ? `<button class="restart-btn" onclick="window._stopProfile(${profile.id})">Stop All</button>`
                : `<button class="open-btn" onclick="window._startProfile(${profile.id})">Start All</button>`)
            : ''
          }
          <button class="profile-delete-btn" onclick="window._deleteProfile(${profile.id})">Delete</button>
        </div>
      </div>
      <div class="profile-projects-list">
        ${profileProjects.length === 0
          ? `<div class="panel-muted" style="padding:6px 0">No projects in this profile.</div>`
          : profileProjects.map(p => `
            <div class="profile-project-row">
              <span class="profile-project-dot ${p.ports.length > 0 ? 'live' : 'dead'}"></span>
              <span class="profile-project-name">${esc(p.name)}</span>
              <span class="profile-project-ports">${p.ports.map(port => `:${port}`).join(' ')}</span>
              <button class="profile-remove-btn" onclick="window._removeFromProfile(${profile.id},${p.id})">✕</button>
            </div>`).join('')
        }
        <div class="profile-add-row">
          <select class="profile-add-select" id="profile-add-select-${profile.id}">
            <option value="">Add project…</option>
            ${projects
              .filter(p => !profile.project_ids.includes(p.id))
              .map(p => `<option value="${p.id}">${esc(p.name)}</option>`)
              .join('')}
          </select>
          <button class="open-btn" style="font-size:11px;padding:3px 8px" onclick="window._addToProfile(${profile.id})">Add</button>
        </div>
      </div>
    </div>`;
}

window._createProfile = async (event) => {
  event.preventDefault();
  const form = event.target;
  const name = form.name.value.trim();
  if (!name) return;
  const btn = form.querySelector('.set-btn');
  btn.disabled = true;
  try {
    const resp = await fetch('/profiles', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name }),
    });
    if (resp.ok) {
      form.name.value = '';
      await loadProfiles();
    } else {
      showToast('Failed to create profile', 'error');
    }
  } catch (e) {
    showToast(`Failed: ${e.message}`, 'error');
  } finally {
    btn.disabled = false;
  }
};

window._deleteProfile = async (id) => {
  if (!confirm('Delete this profile?')) return;
  try {
    const resp = await fetch(`/profiles/${id}`, { method: 'DELETE' });
    if (resp.ok) {
      await loadProfiles();
    } else {
      showToast('Failed to delete profile', 'error');
    }
  } catch (e) {
    showToast(`Failed: ${e.message}`, 'error');
  }
};

window._addToProfile = async (profileId) => {
  const sel = document.getElementById(`profile-add-select-${profileId}`);
  const projectId = parseInt(sel?.value);
  if (!projectId) return;
  try {
    const resp = await fetch(`/profiles/${profileId}/projects`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ project_id: projectId }),
    });
    if (resp.ok) {
      await loadProfiles();
    } else {
      showToast('Failed to add project', 'error');
    }
  } catch (e) {
    showToast(`Failed: ${e.message}`, 'error');
  }
};

window._removeFromProfile = async (profileId, projectId) => {
  try {
    const resp = await fetch(`/profiles/${profileId}/projects/${projectId}`, { method: 'DELETE' });
    if (resp.ok) {
      await loadProfiles();
    } else {
      showToast('Failed to remove project', 'error');
    }
  } catch (e) {
    showToast(`Failed: ${e.message}`, 'error');
  }
};

window._startProfile = async (id) => {
  try {
    const resp = await fetch(`/profiles/${id}/start`, { method: 'POST' });
    if (!resp.ok) showToast('Failed to start profile', 'error');
    else showToast('Profile started', 'success');
  } catch (e) {
    showToast(`Failed: ${e.message}`, 'error');
  }
};

window._stopProfile = async (id) => {
  try {
    const resp = await fetch(`/profiles/${id}/stop`, { method: 'POST' });
    if (!resp.ok) showToast('Failed to stop profile', 'error');
    else showToast('Profile stopped', 'success');
  } catch (e) {
    showToast(`Failed: ${e.message}`, 'error');
  }
};

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
