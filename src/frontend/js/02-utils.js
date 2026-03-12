/* ═══════════════════════════════════════════════════════════════
   UTILITY FUNCTIONS
   ═══════════════════════════════════════════════════════════════ */

/** Reusable basketball loader HTML for JS-generated loading states */
var BASKETBALL_LOADER_HTML = '<div class="basketball-loader"><div class="ball"></div><div class="hoop"></div><div class="player"><div class="head"></div><div class="body"></div><div class="arm"></div></div></div>';

var SAVED_REPOS_STORAGE_KEY = 'gwd_saved_repos';
var CUSTOM_WORKFLOW_TAB_STORAGE_KEY = 'gwd_custom_workflow_tabs';
var SLACK_TEMPLATES_STORAGE_KEY = 'gwd_slack_templates';
var SLACK_CONFIG_STORAGE_KEY = 'gwd_slack_config';

/** Escape HTML to prevent XSS */
function esc(str) {
  if (str == null) return '';
  const d = document.createElement('div');
  d.textContent = String(str);
  return d.innerHTML;
}

/** Escape a string for safe embedding in JavaScript string literals (onclick handlers etc.) */
function escJs(str) {
  if (str == null) return '';
  return String(str)
    .replace(/\\/g, '\\\\')
    .replace(/'/g, "\\'")
    .replace(/"/g, '\\"')
    .replace(/\n/g, '\\n')
    .replace(/\r/g, '\\r')
    .replace(/</g, '\\x3c')
    .replace(/>/g, '\\x3e');
}

/** Validate a URL starts with expected GitHub origin before opening */
function safeOpenUrl(url) {
  if (!url || typeof url !== 'string') return;
  try {
    const parsed = new URL(url);
    if (parsed.protocol === 'https:' && parsed.hostname === 'github.com') {
      window.open(url, '_blank', 'noopener,noreferrer');
    } else {
      console.warn('Blocked opening non-GitHub URL:', url);
    }
  } catch (e) {
    console.warn('Invalid URL blocked:', url);
  }
}

/** Get stored auth token from sessionStorage */
function getAuthToken() {
  try {
    return sessionStorage.getItem('gwd_auth_token') || '';
  } catch (_) {
    return '';
  }
}

/** Store auth token in sessionStorage */
function setAuthToken(token) {
  try {
    sessionStorage.setItem('gwd_auth_token', token);
  } catch (_) {}
}

/** Show the auth token dialog */
function showAuthDialog() {
  var modal = document.getElementById('auth-modal');
  if (modal) {
    modal.style.display = 'flex';
    var input = document.getElementById('auth-token-input');
    if (input) {
      input.value = '';
      setTimeout(function() { input.focus(); }, 100);
    }
  }
}

/** Format a duration in seconds to human-readable */
function formatDuration(seconds) {
  if (seconds == null || seconds < 0) return '--';
  const s = Math.floor(seconds);
  if (s < 60) return s + 's';
  const m = Math.floor(s / 60);
  const rem = s % 60;
  if (m < 60) return m + 'm ' + rem + 's';
  const h = Math.floor(m / 60);
  const remM = m % 60;
  return h + 'h ' + remM + 'm';
}

/** Calculate elapsed seconds from a date string to now */
function elapsedSince(dateStr) {
  if (!dateStr) return null;
  const start = new Date(dateStr).getTime();
  const now = Date.now();
  return Math.max(0, Math.floor((now - start) / 1000));
}

/** Format a date string to a short local string */
function formatTime(dateStr) {
  if (!dateStr) return '--';
  const d = new Date(dateStr);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) {
    return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) +
    ' ' + d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

/** Format relative time ago */
function timeAgo(dateStr) {
  if (!dateStr) return '';
  const seconds = elapsedSince(dateStr);
  if (seconds == null) return '';
  if (seconds < 60) return 'just now';
  const m = Math.floor(seconds / 60);
  if (m < 60) return m + 'm ago';
  const h = Math.floor(m / 60);
  if (h < 24) return h + 'h ago';
  const d = Math.floor(h / 24);
  return d + 'd ago';
}

/** Get status icon */
function statusIcon(status, conclusion) {
  if (status === 'queued') return '<span style="color:var(--status-queued)">&#9679;</span>';
  if (status === 'in_progress') return '<span style="color:var(--status-in-progress)">&#9679;</span>';
  if (status === 'completed') {
    if (conclusion === 'success') return '<span style="color:var(--status-success)">&#10003;</span>';
    if (conclusion === 'failure') return '<span style="color:var(--status-failure)">&#10007;</span>';
    if (conclusion === 'cancelled') return '<span style="color:var(--status-cancelled)">&#9632;</span>';
    return '<span style="color:var(--text-muted)">&#9679;</span>';
  }
  return '<span style="color:var(--text-muted)">&#9679;</span>';
}

/** Get step conclusion icon */
function stepIcon(conclusion) {
  if (conclusion === 'success') return '<span class="step-icon" style="color:var(--status-success)">&#10003;</span>';
  if (conclusion === 'failure') return '<span class="step-icon" style="color:var(--status-failure)">&#10007;</span>';
  if (conclusion === 'skipped') return '<span class="step-icon" style="color:var(--text-muted)">&#8722;</span>';
  return '<span class="step-icon" style="color:var(--status-in-progress)">&#9679;</span>';
}

/** Get status badge HTML */
function statusBadge(status, conclusion) {
  let label = status || 'unknown';
  let cls = status || 'unknown';
  if (status === 'completed') {
    label = conclusion || 'completed';
    cls = conclusion || 'completed';
  }
  return '<span class="status-badge ' + esc(cls) + '">' + statusIcon(status, conclusion) + ' ' + esc(label) + '</span>';
}

/** Calculate run duration */
function runDuration(run) {
  const startAt = run.run_started_at || run.created_at;
  if (!startAt) return null;
  const start = new Date(startAt).getTime();
  if (run.status === 'completed') {
    const end = new Date(run.updated_at).getTime();
    return Math.max(0, Math.floor((end - start) / 1000));
  }
  // In progress — calculate from now
  return Math.max(0, Math.floor((Date.now() - start) / 1000));
}

/** Validate owner/repo format */
function validateRepoFormat(str) {
  const trimmed = (str || '').trim();
  if (!trimmed) return null;
  const parts = trimmed.split('/');
  if (parts.length !== 2) return null;
  const owner = parts[0].trim();
  const repo = parts[1].trim();
  if (!owner || !repo) return null;
  if (!/^[a-zA-Z0-9._-]+$/.test(owner)) return null;
  if (!/^[a-zA-Z0-9._-]+$/.test(repo)) return null;
  return owner + '/' + repo;
}

/* ── Repo Persistence (localStorage) ── */
function saveReposToStorage(repos) {
  try {
    localStorage.setItem(SAVED_REPOS_STORAGE_KEY, JSON.stringify((repos || []).slice().sort()));
  } catch (_) {}
}

function loadSavedReposFromStorage() {
  try {
    var raw = localStorage.getItem(SAVED_REPOS_STORAGE_KEY);
    if (!raw) return [];
    var parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return Array.from(new Set(parsed
      .map(function(r) { return typeof r === 'string' ? validateRepoFormat(r) : null; })
      .filter(function(r) { return !!r; })));
  } catch (_) {
    return [];
  }
}

function inferReposFromStateArtifacts() {
  var raw = [];
  function pushRepoFromKeyish(key) {
    var text = key != null ? String(key) : '';
    var prefix = text.split('::')[0] || text;
    var validated = validateRepoFormat(prefix);
    if (validated) {
      raw.push(validated);
    } else {
      var m = text.match(/([a-zA-Z0-9._-]+\/[a-zA-Z0-9._-]+)/);
      if (m && m[1]) {
        var validatedMatch = validateRepoFormat(m[1]);
        if (validatedMatch) raw.push(validatedMatch);
      }
    }
  }
  (State.customWorkflowTabs || []).forEach(function(tab) {
    (tab.workflowKeys || []).forEach(function(key) { pushRepoFromKeyish(key); });
    Object.keys(tab.stageByKey || {}).forEach(function(key) { pushRepoFromKeyish(key); });
  });
  return Array.from(new Set(raw));
}

/* ── Custom Workflow Tab Persistence ── */
function saveCustomWorkflowTabs() {
  try {
    localStorage.setItem(CUSTOM_WORKFLOW_TAB_STORAGE_KEY, JSON.stringify(State.customWorkflowTabs));
  } catch (_) {}
}

function loadCustomWorkflowTabs() {
  try {
    var raw = localStorage.getItem(CUSTOM_WORKFLOW_TAB_STORAGE_KEY);
    if (!raw) return [];
    var parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.map(function(tab) {
      return {
        id: tab.id || ('custom-workflow-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8)),
        name: tab.name || 'Untitled',
        workflowKeys: Array.isArray(tab.workflowKeys) ? tab.workflowKeys : [],
        stageByKey: tab.stageByKey || {},
        pipelineConfigsByKey: tab.pipelineConfigsByKey || {},
        manualApprovalAfterStage: tab.manualApprovalAfterStage || {},
        slackNotificationByStage: tab.slackNotificationByStage || {},
        agentConfig: tab.agentConfig || { enabled: false, frequencyMinutes: 30 },
      };
    });
  } catch (_) {
    return [];
  }
}

/* ── Workflow Key Helpers ── */
function makeWorkflowKey(repo, workflowId) {
  return repo + '::' + workflowId;
}

function parseWorkflowKey(key) {
  var parts = String(key || '').split('::');
  if (parts.length < 2) return null;
  return { repo: parts[0], workflowId: Number(parts[1]) || 0, name: parts[2] || '' };
}

/* ── Branch Caching ── */
async function getDefaultBranchForRepo(repo) {
  if (State.branchCache[repo]) {
    return State.branchCache[repo];
  }
  var parts = repo.split('/');
  var owner = parts[0];
  var repoName = parts[1];
  var defaultBranch = 'main';
  try {
    var branches = await apiFetch('/api/branches/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repoName));
    if (Array.isArray(branches) && branches.length > 0) {
      var preferred = branches.find(function(b) { return b.name === 'main'; }) ||
        branches.find(function(b) { return b.name === 'master'; }) ||
        branches[0];
      if (preferred && preferred.name) {
        defaultBranch = preferred.name;
      }
    }
  } catch (_) {}
  State.branchCache[repo] = defaultBranch;
  return defaultBranch;
}

/* ── Slack Template Helpers ── */
function normalizeSlackTemplate(template, idx) {
  if (!template || typeof template !== 'object') return null;
  var name = template.name != null ? String(template.name).trim() : '';
  var body = template.body != null ? String(template.body) : '';
  if (!name || !body.trim()) return null;
  var id = template.id != null ? String(template.id).trim() : '';
  if (!id) {
    id = 'slack-template-' + Date.now() + '-' + idx + '-' + Math.floor(Math.random() * 100000);
  }
  return { id: id, name: name, body: body };
}

function normalizeSlackWebhookUrls(raw) {
  var values = [];
  if (Array.isArray(raw)) { values = raw.slice(); }
  else if (raw != null) { values = [raw]; }
  var seen = {};
  var normalized = [];
  values.forEach(function(v) {
    var text = v != null ? String(v) : '';
    text.split(/\r?\n|,/).forEach(function(part) {
      var url = String(part || '').trim();
      if (!url || seen[url]) return;
      seen[url] = true;
      normalized.push(url);
    });
  });
  return normalized;
}

function slackWebhookUrlsText(urls) {
  return (normalizeSlackWebhookUrls(urls) || []).join('\n');
}

function getConfiguredSlackWebhookUrls() {
  var urls = normalizeSlackWebhookUrls(State.slackConfig && State.slackConfig.webhookUrls);
  if (urls.length === 0 && State.slackConfig && State.slackConfig.webhookUrl != null) {
    urls = normalizeSlackWebhookUrls(State.slackConfig.webhookUrl);
  }
  return urls;
}

function saveSlackTemplates() {
  try {
    localStorage.setItem(SLACK_TEMPLATES_STORAGE_KEY, JSON.stringify(State.slackTemplates || []));
  } catch (_) {}
}

function loadSlackTemplates() {
  try {
    var raw = localStorage.getItem(SLACK_TEMPLATES_STORAGE_KEY);
    if (!raw) return;
    var parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return;
    State.slackTemplates = parsed
      .map(function(t, idx) { return normalizeSlackTemplate(t, idx); })
      .filter(function(t) { return !!t; });
  } catch (_) {
    State.slackTemplates = [];
  }
}

function saveSlackConfig() {
  try {
    var urls = normalizeSlackWebhookUrls(State.slackConfig && State.slackConfig.webhookUrls);
    localStorage.setItem(SLACK_CONFIG_STORAGE_KEY, JSON.stringify({
      webhookUrls: urls,
      webhookUrl: urls.length > 0 ? urls[0] : ''
    }));
  } catch (_) {}
}

function loadSlackConfig() {
  try {
    var raw = localStorage.getItem(SLACK_CONFIG_STORAGE_KEY);
    if (!raw) return;
    var parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object') return;
    var urls = normalizeSlackWebhookUrls(parsed.webhookUrls);
    if (urls.length === 0 && parsed.webhookUrl != null) {
      urls = normalizeSlackWebhookUrls(parsed.webhookUrl);
    }
    State.slackConfig = { webhookUrls: urls };
  } catch (_) {
    State.slackConfig = { webhookUrls: [] };
  }
}

function getTabStageNumbers(tab) {
  var unique = {};
  (tab.workflowKeys || []).forEach(function(key) {
    var stage = Number(tab.stageByKey && tab.stageByKey[key]);
    if (!stage || stage < 1) return;
    unique[String(Math.floor(stage))] = true;
  });
  return Object.keys(unique)
    .map(function(n) { return Number(n); })
    .sort(function(a, b) { return a - b; });
}

function ensureTabSlackNotifications(tab) {
  if (!tab.slackNotificationByStage || typeof tab.slackNotificationByStage !== 'object') {
    tab.slackNotificationByStage = {};
  }
  var stages = getTabStageNumbers(tab);
  var valid = {};
  stages.forEach(function(stageNo) {
    valid['before:' + stageNo] = true;
    valid['after:' + stageNo] = true;
  });
  var cleaned = {};
  Object.keys(tab.slackNotificationByStage).forEach(function(key) {
    if (!valid[key]) return;
    var templateId = tab.slackNotificationByStage[key];
    if (templateId == null) return;
    var normalizedTemplateId = String(templateId).trim();
    if (!normalizedTemplateId) return;
    cleaned[key] = normalizedTemplateId;
  });
  tab.slackNotificationByStage = cleaned;
}

function ensureTabManualApprovals(tab) {
  if (!tab.manualApprovalAfterStage || typeof tab.manualApprovalAfterStage !== 'object') {
    tab.manualApprovalAfterStage = {};
  }
  var stages = getTabStageNumbers(tab);
  var valid = {};
  for (var i = 0; i < stages.length - 1; i++) {
    valid[String(stages[i])] = true;
  }
  var cleaned = {};
  Object.keys(tab.manualApprovalAfterStage).forEach(function(key) {
    var stageNo = Math.floor(Number(key));
    if (!stageNo || !valid[String(stageNo)]) return;
    if (tab.manualApprovalAfterStage[key]) {
      cleaned[String(stageNo)] = true;
    }
  });
  tab.manualApprovalAfterStage = cleaned;
}

function renderSlackTemplateOptions(selectedId, includeNoneLabel) {
  var html = '';
  if (includeNoneLabel) {
    html += '<option value="">' + esc(includeNoneLabel) + '</option>';
  }
  (State.slackTemplates || []).forEach(function(tpl) {
    var selected = tpl.id === selectedId ? ' selected' : '';
    html += '<option value="' + esc(tpl.id) + '"' + selected + '>' + esc(tpl.name) + '</option>';
  });
  return html;
}

function getSlackTemplateById(templateId) {
  var id = templateId != null ? String(templateId).trim() : '';
  if (!id) return null;
  return (State.slackTemplates || []).find(function(tpl) { return tpl.id === id; }) || null;
}

function getSlackTemplateNameById(templateId) {
  var tpl = getSlackTemplateById(templateId);
  return tpl ? tpl.name : '';
}

function renderSlackTemplateBody(templateBody, context) {
  var raw = String(templateBody || '');
  var vars = context || {};
  return raw.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, function(_, key) {
    if (!Object.prototype.hasOwnProperty.call(vars, key)) return '';
    var value = vars[key];
    return value == null ? '' : String(value);
  });
}

function escapeSlackText(text) {
  return String(text == null ? '' : text)
    .replace(/`/g, "'")
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function sleepMs(ms) {
  return new Promise(function(resolve) { setTimeout(resolve, ms); });
}

async function ensureWorkflowInputsCached(workflowKeyValue, repo, workflowId, force) {
  var existing = State.workflowInputsByKey[workflowKeyValue];
  if (!force && existing && (existing.loading || existing.has_dispatch != null)) {
    return existing;
  }
  var parts = String(repo || '').split('/');
  var owner = parts[0];
  var repoName = parts[1];
  var wfId = workflowId || 0;
  if (!owner || !repoName || !wfId) return null;

  State.workflowInputsByKey[workflowKeyValue] = { loading: true, error: null, has_dispatch: null, inputs: [] };
  try {
    var data = await apiFetch('/api/workflows/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repoName) + '/' + wfId + '/inputs');
    State.workflowInputsByKey[workflowKeyValue] = {
      loading: false,
      error: null,
      has_dispatch: !!(data && data.has_dispatch),
      inputs: (data && Array.isArray(data.inputs)) ? data.inputs : []
    };
  } catch (err) {
    State.workflowInputsByKey[workflowKeyValue] = {
      loading: false,
      error: err && err.message ? err.message : 'Failed to load inputs',
      has_dispatch: null,
      inputs: []
    };
  }
  return State.workflowInputsByKey[workflowKeyValue];
}

/* ── Dispatch with Config (for pipeline orchestration) ── */
async function dispatchWorkflowWithConfig(repo, workflowId, config) {
  var cfg = config || { branch: '', inputsByName: {} };
  var branch = (cfg.branch || '').trim();
  if (!branch) {
    branch = await getDefaultBranchForRepo(repo);
  }
  var inputs = {};
  if (cfg.inputsByName && typeof cfg.inputsByName === 'object') {
    Object.keys(cfg.inputsByName).forEach(function(key) {
      var normalizedKey = (key || '').trim();
      if (!normalizedKey) return;
      var value = cfg.inputsByName[key];
      if (value == null) return;
      var normalizedValue = String(value);
      if (normalizedValue === '') return;
      inputs[normalizedKey] = normalizedValue;
    });
  }
  var body = {
    repo: repo,
    workflow_id: workflowId,
    ref: branch,
    inputs: inputs,
  };
  return apiFetch('/api/workflows/dispatch', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

/* ── Step Log Extraction ── */
function extractStepLogFromJobLogs(rawLogs, step) {
  var logs = String(rawLogs || '');
  if (!logs) return '';
  if (!step || !step.started_at) return logs;

  var startMs = new Date(step.started_at).getTime();
  if (!isFinite(startMs)) return logs;
  var endMs = step.completed_at ? new Date(step.completed_at).getTime() : (startMs + 15 * 60 * 1000);
  if (!isFinite(endMs) || endMs < startMs) {
    endMs = startMs + 15 * 60 * 1000;
  }
  var fromMs = startMs - 1000;
  var toMs = endMs + 1000;

  var lines = logs.split('\n');
  var tsPattern = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)\s?(.*)$/;
  var inWindow = false;
  var selected = [];

  lines.forEach(function(line) {
    var match = tsPattern.exec(line);
    if (match) {
      var ts = Date.parse(match[1]);
      if (isFinite(ts) && ts >= fromMs && ts <= toMs) {
        inWindow = true;
        selected.push(line);
      } else {
        inWindow = false;
      }
      return;
    }
    if (inWindow) {
      selected.push(line);
    }
  });

  var byTimeWindow = selected.join('\n').trim();
  if (byTimeWindow) return byTimeWindow;

  // Fallback: substring match on step name
  var stepName = (step.name || '').trim();
  if (stepName) {
    var lowerLogs = logs.toLowerCase();
    var lowerName = stepName.toLowerCase();
    var idx = lowerLogs.indexOf(lowerName);
    if (idx >= 0) {
      var start = Math.max(0, idx - 3000);
      var end = Math.min(logs.length, idx + lowerName.length + 12000);
      return logs.slice(start, end).trim();
    }
  }

  return logs;
}

/** Simple API fetch wrapper with auth & CSRF headers */
async function apiFetch(path, options) {
  const opts = options || {};
  const headers = Object.assign({
    'Content-Type': 'application/json',
    'X-Requested-With': 'XMLHttpRequest',
  }, opts.headers || {});
  // Add auth token if available
  const token = getAuthToken();
  if (token) {
    headers['Authorization'] = 'Bearer ' + token;
  }
  opts.headers = headers;
  const res = await fetch(path, opts);
  // If 401 or 403, prompt for token
  if (res.status === 401 || res.status === 403) {
    showAuthDialog();
    throw new Error('Authentication required');
  }
  if (!res.ok) {
    let errorMsg = 'Request failed: ' + res.status;
    try {
      const body = await res.json();
      if (body.error) errorMsg = body.error;
      else if (body.message) errorMsg = body.message;
    } catch (_) {}
    throw new Error(errorMsg);
  }
  // Some endpoints return 204 No Content
  if (res.status === 204) return {};
  return res.json();
}

