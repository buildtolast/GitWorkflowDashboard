/* ═══════════════════════════════════════════════════════════════
   WEBSOCKET CONNECTION
   ═══════════════════════════════════════════════════════════════ */

function wsConnect() {
  if (State.ws && (State.ws.readyState === WebSocket.CONNECTING || State.ws.readyState === WebSocket.OPEN)) {
    return;
  }

  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  let url = protocol + '//' + location.host + '/ws/live';
  // Add auth token as query param (WebSocket API doesn't support custom headers)
  const wsToken = getAuthToken();
  if (wsToken) {
    url += '?token=' + encodeURIComponent(wsToken);
  }

  updateWsStatus('connecting');

  try {
    State.ws = new WebSocket(url);
  } catch (e) {
    console.error('WebSocket creation failed:', e);
    wsScheduleReconnect();
    return;
  }

  State.ws.onopen = function() {
    State.wsConnected = true;
    State.wsReconnectDelay = 1000;
    updateWsStatus('connected');
    console.log('[WS] Connected');
  };

  State.ws.onmessage = function(evt) {
    try {
      const msg = JSON.parse(evt.data);
      handleWsMessage(msg);
    } catch (e) {
      console.error('[WS] Parse error:', e);
    }
  };

  State.ws.onclose = function(evt) {
    State.wsConnected = false;
    updateWsStatus('disconnected');
    console.log('[WS] Closed:', evt.code, evt.reason);
    wsScheduleReconnect();
  };

  State.ws.onerror = function(err) {
    console.error('[WS] Error:', err);
  };
}

function wsScheduleReconnect() {
  if (State.wsReconnectTimer) return;
  const delay = State.wsReconnectDelay;
  console.log('[WS] Reconnecting in ' + delay + 'ms');
  updateWsStatus('connecting');
  State.wsReconnectTimer = setTimeout(function() {
    State.wsReconnectTimer = null;
    // Exponential backoff
    State.wsReconnectDelay = Math.min(State.wsReconnectDelay * 2, State.wsMaxReconnectDelay);
    wsConnect();
  }, delay);
}

function updateWsStatus(status) {
  const dot = document.getElementById('ws-dot');
  const text = document.getElementById('ws-status-text');
  dot.className = 'ws-dot';
  if (status === 'connected') {
    dot.classList.add('connected');
    text.textContent = 'live';
    // Update init overlay — mark connect done, activate repos step
    updateInitStep('connect', 'done');
    updateInitStep('repos', 'active');
    updateInitStatus('Loading repositories...');
  } else if (status === 'connecting') {
    dot.classList.add('connecting');
    text.textContent = 'connecting';
    updateInitStatus('Connecting to server...');
  } else {
    text.textContent = 'disconnected';
  }
}

/** Update a step in the init overlay */
function updateInitStep(step, state) {
  var el = document.getElementById('init-step-' + step);
  if (!el) return;
  el.className = 'init-step';
  if (state) el.classList.add(state);
  var icon = el.querySelector('.init-step-icon');
  if (icon) {
    if (state === 'done') icon.innerHTML = '&#10003;';
    else if (state === 'active') icon.innerHTML = '&#9679;';
  }
  // Drive progress bar: connect=33%, repos=66%, ready=100%
  var bar = document.getElementById('init-bar');
  if (bar) {
    if (step === 'connect' && state === 'done') bar.style.width = '33%';
    else if (step === 'repos' && state === 'active') bar.style.width = '50%';
    else if (step === 'repos' && state === 'done') bar.style.width = '75%';
    else if (step === 'ready' && state === 'done') bar.style.width = '100%';
  }
}

/** Update the init overlay status text */
function updateInitStatus(msg) {
  var el = document.getElementById('init-status');
  if (el) el.textContent = msg;
  // When status is set explicitly, pause message rotation
  _initRotatePaused = true;
  clearTimeout(_initRotateTimer);
}

/** Dismiss the init overlay with fade-out */
function dismissInitOverlay() {
  State.initOverlayDismissed = true;
  clearInterval(_initElapsedInterval);
  clearTimeout(_initSkipTimer);
  clearTimeout(_initRotateTimer);
  clearTimeout(State.initEmptyTimer);
  updateInitStep('repos', 'done');
  updateInitStep('ready', 'done');
  // Set status directly without pausing (overlay is going away)
  var statusEl = document.getElementById('init-status');
  if (statusEl) statusEl.textContent = 'Ready';
  var overlay = document.getElementById('init-overlay');
  if (!overlay) return;
  setTimeout(function() {
    overlay.classList.add('fade-out');
    setTimeout(function() { overlay.remove(); }, 700);
  }, 500);
}

/* ── Init overlay: elapsed timer, rotating messages, skip button ── */
var _initStartTime = Date.now();
var _initElapsedInterval = null;
var _initSkipTimer = null;
var _initRotateTimer = null;
var _initRotatePaused = false;
var _initMsgIndex = 0;
var _initMessages = [
  'Working hard for you!',
  'Fetching your repositories...',
  'Almost there...',
  'Crunching the latest data...',
  'Polling GitHub for updates...',
  'Hang tight, good things take time...',
  'Warming up the engines...'
];

function _startInitTimers() {
  // Elapsed timer — updates every second
  _initElapsedInterval = setInterval(function() {
    var el = document.getElementById('init-elapsed');
    if (!el) { clearInterval(_initElapsedInterval); return; }
    var secs = Math.floor((Date.now() - _initStartTime) / 1000);
    el.textContent = secs + 's elapsed';
  }, 1000);

  // Show skip button after 10 seconds
  _initSkipTimer = setTimeout(function() {
    var skip = document.getElementById('init-skip');
    if (skip) skip.classList.add('visible');
  }, 10000);

  // Rotate status messages every 3 seconds
  function rotateMessage() {
    if (State.initOverlayDismissed) return;
    if (!_initRotatePaused) {
      var statusEl = document.getElementById('init-status');
      if (statusEl) {
        statusEl.textContent = _initMessages[_initMsgIndex % _initMessages.length];
        _initMsgIndex++;
      }
    }
    _initRotatePaused = false;
    _initRotateTimer = setTimeout(rotateMessage, 3000);
  }
  // Start rotating after the first explicit status message has shown for 3s
  _initRotateTimer = setTimeout(rotateMessage, 3000);
}
_startInitTimers();

/* ═══════════════════════════════════════════════════════════════
   WEBSOCKET MESSAGE HANDLERS
   ═══════════════════════════════════════════════════════════════ */

function handleWsMessage(msg) {
  switch (msg.type) {
    case 'state':
      handleStateUpdate(msg.data);
      break;
    case 'run_update':
      handleRunUpdate(msg.repo, msg.run);
      break;
    case 'rate_limit':
      handleRateLimitUpdate(msg.data);
      break;
    case 'repos_changed':
      handleReposChanged(msg.repos);
      break;
    default:
      console.log('[WS] Unknown message type:', msg.type);
  }
}

function handleStateUpdate(data) {
  State.repoStates = data.repos || [];
  State.repos = State.repoStates.map(function(r) { return r.repo; });
  if (typeof CommandBar !== 'undefined') CommandBar._catalogDirty = true;
  State.rateLimit = data.rate_limit || State.rateLimit;
  State.lastPoll = data.last_poll;
  State.initialLoadDone = true;

  // Persist repos to localStorage
  if (State.repos.length > 0) {
    saveReposToStorage(State.repos);
  }

  renderAll();

  // Dismiss init overlay when repos are loaded, or after a short timeout
  // if no repos are configured (avoids being stuck forever).
  if (!State.initOverlayDismissed) {
    if (State.repoStates.length > 0 || data.last_poll) {
      // Repos loaded or poller completed — wait for sidebar to render
      // before dismissing so the left panel is visually populated first
      clearTimeout(State.initEmptyTimer);
      State.initOverlayDismissed = true;
      updateInitStatus('Loaded ' + State.repoStates.length + ' repositories from ~/.gwd/repos.json');
      updateInitStep('repos', 'done');
      requestAnimationFrame(function() {
        requestAnimationFrame(function() {
          dismissInitOverlay();
        });
      });
    } else if (!State.initEmptyTimer) {
      // First empty state (initial snapshot before poll) — give poller
      // 3 seconds to deliver data; if nothing arrives, dismiss with message
      State.initEmptyTimer = setTimeout(function() {
        if (!State.initOverlayDismissed) {
          State.initOverlayDismissed = true;
          updateInitStatus('No repositories found — add repos to get started');
          updateInitStep('repos', 'done');
          dismissInitOverlay();
        }
      }, 3000);
    }
  }
}

function handleRunUpdate(repo, run) {
  // Ensure repo field is set on the run
  run.repo = run.repo || repo;

  // Find the repo state
  const repoState = State.repoStates.find(function(r) { return r.repo === repo; });
  if (!repoState) return;

  const isActive = run.status === 'in_progress' || run.status === 'queued';

  // Update active runs
  const activeIdx = repoState.active_runs.findIndex(function(r) { return r.id === run.id; });
  if (isActive) {
    if (activeIdx >= 0) {
      repoState.active_runs[activeIdx] = run;
    } else {
      repoState.active_runs.push(run);
    }
  } else {
    if (activeIdx >= 0) {
      repoState.active_runs.splice(activeIdx, 1);
    }
  }

  // Update recent runs
  if (run.status === 'completed') {
    const recentIdx = repoState.recent_runs.findIndex(function(r) { return r.id === run.id; });
    if (recentIdx >= 0) {
      repoState.recent_runs[recentIdx] = run;
    } else {
      repoState.recent_runs.unshift(run);
      // Keep only last 20
      if (repoState.recent_runs.length > 20) {
        repoState.recent_runs.pop();
      }
    }
  }

  // Stop live log tailing if the job being tailed belongs to a completed run
  if (!isActive && State.liveLogJobId) {
    // Check if our live-tailed job was in this run
    if (!App._isJobInProgress(State.liveLogJobId)) {
      // Do a final log fetch then stop tailing
      var meta = State.logViewerMeta;
      if (meta) {
        App._pollLiveLog(meta.jobId, meta.owner, meta.repo, meta.jobName, meta.workflowName);
      }
      App.stopLiveTail();
    }
  }

  // Stop auto-refresh for expanded run details if run completed
  if (!isActive && State.expandedRunRefreshTimers[run.id]) {
    clearInterval(State.expandedRunRefreshTimers[run.id]);
    delete State.expandedRunRefreshTimers[run.id];
  }

  renderAll();
}

function handleRateLimitUpdate(data) {
  State.rateLimit = data;
  renderRateLimit();
}

function handleReposChanged(repos) {
  State.repos = repos;
  saveReposToStorage(repos);
  // Remove states for repos no longer tracked
  State.repoStates = State.repoStates.filter(function(r) {
    return repos.indexOf(r.repo) >= 0;
  });
  renderSidebar();
  renderActiveRuns();
  renderWorkflows();
  renderActions();
  renderHistory();
  renderWhatsHappening();
  renderAnalytics();
}

