/* ═══════════════════════════════════════════════════════════════
   APPLICATION ACTIONS (App namespace)
   ═══════════════════════════════════════════════════════════════ */

const App = {

  /** Switch active tab */
  switchTab: function(tabId) {
    State.activeTab = tabId;
    try { localStorage.setItem('gwd_tab', tabId); } catch(_) {}

    // Close the log viewer panel when switching tabs
    App.clearLogViewer();

    var isCustomTab = typeof tabId === 'string' && tabId.indexOf('custom-workflow-') === 0;

    // Update tab buttons (including custom tabs)
    document.querySelectorAll('.tab-btn').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.tab === tabId);
    });

    // Update panels
    document.querySelectorAll('.tab-panel').forEach(function(panel) {
      if (isCustomTab) {
        // For custom tabs, show the shared custom-workflow-tab panel
        panel.classList.toggle('active', panel.id === 'panel-custom-workflow-tab');
      } else {
        panel.classList.toggle('active', panel.id === 'panel-' + tabId);
      }
    });

    // Render custom tab content if switching to one
    if (isCustomTab) {
      App.renderCustomWorkflowTab();
    }
  },

  /** Toggle sidebar */
  toggleSidebar: function() {
    State.sidebarCollapsed = !State.sidebarCollapsed;
    document.getElementById('sidebar').classList.toggle('collapsed', State.sidebarCollapsed);
  },

  /** Switch add-repo mode between search and manual */
  /** Toggle collapse/expand of the add repo section in sidebar */
  toggleAddRepoSection: function() {
    var section = document.querySelector('.sidebar-add-section');
    if (section) section.classList.toggle('collapsed');
  },

  switchAddMode: function(mode) {
    State.addRepoMode = mode;
    document.querySelectorAll('.add-repo-tab').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.mode === mode);
    });
    document.getElementById('add-repo-search-mode').style.display = mode === 'search' ? 'block' : 'none';
    document.getElementById('add-repo-manual-mode').style.display = mode === 'manual' ? 'block' : 'none';
  },

  /** Debounced search input handler */
  onRepoSearchInput: function() {
    if (State.repoSearchTimer) clearTimeout(State.repoSearchTimer);
    var query = document.getElementById('repo-search-input').value.trim();
    if (!query) {
      document.getElementById('repo-search-results').style.display = 'none';
      document.getElementById('repo-search-spinner').style.display = 'none';
      document.getElementById('add-selected-btn').style.display = 'none';
      State.repoSearchResults = [];
      State.selectedSearchRepos = {};
      return;
    }
    document.getElementById('repo-search-spinner').style.display = 'block';
    State.repoSearchTimer = setTimeout(function() {
      App._doRepoSearch(query);
    }, 400);
  },

  /** Execute repo search */
  _doRepoSearch: async function(query) {
    try {
      var data = await apiFetch('/api/repos/search?q=' + encodeURIComponent(query));
      State.repoSearchResults = (data.results || []).slice(0, 50);
      // Auto-select all results (up to 50)
      State.selectedSearchRepos = {};
      State.repoSearchResults.forEach(function(r) {
        State.selectedSearchRepos[r.full_name] = true;
      });
      App._renderSearchResults();
    } catch (err) {
      document.getElementById('repo-search-results').innerHTML =
        '<div class="repo-search-empty">Search failed: ' + esc(err.message) + '</div>';
      document.getElementById('repo-search-results').style.display = 'block';
    } finally {
      document.getElementById('repo-search-spinner').style.display = 'none';
    }
  },

  /** Render search results dropdown */
  _renderSearchResults: function() {
    var container = document.getElementById('repo-search-results');
    var results = State.repoSearchResults;

    if (results.length === 0) {
      container.innerHTML = '<div class="repo-search-empty">No repos found</div>';
      container.style.display = 'block';
      document.getElementById('add-selected-btn').style.display = 'none';
      return;
    }

    var html = '';
    results.forEach(function(repo) {
      var checked = State.selectedSearchRepos[repo.full_name] ? ' checked' : '';
      var selectedClass = State.selectedSearchRepos[repo.full_name] ? ' selected' : '';
      html += '<div class="repo-search-item' + selectedClass + '" onclick="App.toggleSearchRepo(\'' + escJs(repo.full_name) + '\')">';
      html += '<input type="checkbox"' + checked + ' onclick="event.stopPropagation(); App.toggleSearchRepo(\'' + escJs(repo.full_name) + '\')">';
      html += '<div class="repo-search-info">';
      html += '<div class="repo-search-name">' + esc(repo.full_name) + '</div>';
      if (repo.description) {
        html += '<div class="repo-search-desc" title="' + esc(repo.description) + '">' + esc(repo.description) + '</div>';
      }
      html += '<div class="repo-search-meta">';
      if (repo.private) html += '<span class="private-badge">private</span>';
      if (repo.language) html += '<span class="lang-badge">' + esc(repo.language) + '</span>';
      if (repo.stargazers_count) html += '<span>&#9733; ' + repo.stargazers_count + '</span>';
      html += '</div>';
      html += '</div>';
      html += '</div>';
    });

    container.innerHTML = html;
    container.style.display = 'block';
    App._updateSelectedCount();
  },

  /** Toggle a repo selection in search results */
  toggleSearchRepo: function(fullName) {
    if (State.selectedSearchRepos[fullName]) {
      delete State.selectedSearchRepos[fullName];
    } else {
      State.selectedSearchRepos[fullName] = true;
    }
    App._renderSearchResults();
  },

  /** Toggle a repo in sidebar selection (for filtering Branches tab) */
  toggleSidebarRepo: function(repoName) {
    if (State.selectedSidebarRepos[repoName]) {
      delete State.selectedSidebarRepos[repoName];
    } else {
      State.selectedSidebarRepos[repoName] = true;
    }
    renderSidebar();
  },

  /** Update the selected count badge */
  _updateSelectedCount: function() {
    var count = Object.keys(State.selectedSearchRepos).length;
    document.getElementById('selected-count').textContent = count;
    document.getElementById('add-selected-btn').style.display = count > 0 ? 'block' : 'none';
  },

  /** Bulk add all selected repos */
  addSelectedRepos: async function() {
    var selected = Object.keys(State.selectedSearchRepos);
    if (selected.length === 0) return;

    var btn = document.getElementById('add-selected-btn');
    btn.disabled = true;
    btn.textContent = 'Adding...';

    try {
      await apiFetch('/api/repos', {
        method: 'POST',
        body: JSON.stringify({ repos: selected })
      });
      App.showToast('Added ' + selected.length + ' repo' + (selected.length > 1 ? 's' : '') + ' — saved to ~/.gwd/repos.json', 'success');
      State.selectedSearchRepos = {};
      State.repoSearchResults = [];
      document.getElementById('repo-search-input').value = '';
      document.getElementById('repo-search-results').style.display = 'none';
      document.getElementById('add-selected-btn').style.display = 'none';
    } catch (err) {
      App.showToast('Failed to add repos: ' + err.message, 'error');
    } finally {
      btn.disabled = false;
      btn.innerHTML = 'Add Selected (<span id="selected-count">0</span>)';
    }
  },

  /** Add a repository */
  addRepo: async function(e) {
    e.preventDefault();
    const input = document.getElementById('add-repo-input');
    const btn = document.getElementById('add-repo-btn');
    const raw = input.value.trim();

    const validated = validateRepoFormat(raw);
    if (!validated) {
      App.showToast('Invalid format. Use owner/repo', 'error');
      input.focus();
      return;
    }

    // Check duplicate
    if (State.repos.indexOf(validated) >= 0) {
      App.showToast(validated + ' is already monitored', 'info');
      input.value = '';
      return;
    }

    btn.disabled = true;
    try {
      await apiFetch('/api/repos', {
        method: 'POST',
        body: JSON.stringify({ repos: [validated] })
      });
      input.value = '';
      App.showToast('Added ' + validated + ' — saved to ~/.gwd/repos.json', 'success');
    } catch (err) {
      App.showToast('Failed to add: ' + err.message, 'error');
    } finally {
      btn.disabled = false;
    }
  },

  /** Remove a repository */
  removeRepo: async function(repoName) {
    try {
      await apiFetch('/api/repos', {
        method: 'DELETE',
        body: JSON.stringify({ repos: [repoName] })
      });
      App.showToast('Removed ' + repoName + ' — saved to ~/.gwd/repos.json', 'success');
    } catch (err) {
      App.showToast('Failed to remove: ' + err.message, 'error');
    }
  },

  /** Copy repo GitHub URL to clipboard */
  copyRepoUrl: function(repoName, btn) {
    var url = 'https://github.com/' + repoName;
    navigator.clipboard.writeText(url).then(function() {
      btn.classList.add('copied');
      btn.title = 'Copied!';
      setTimeout(function() {
        btn.classList.remove('copied');
        btn.title = 'Copy URL';
      }, 1500);
    }).catch(function() {
      // Fallback
      var ta = document.createElement('textarea');
      ta.value = url;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
      btn.classList.add('copied');
      setTimeout(function() { btn.classList.remove('copied'); }, 1500);
    });
  },

  /** Toggle run detail expansion */
  toggleRunDetail: async function(runId, repo, event) {
    if (event) event.stopPropagation();

    if (State.expandedRuns[runId]) {
      delete State.expandedRuns[runId];
      // Clear auto-refresh timer for this run
      if (State.expandedRunRefreshTimers[runId]) {
        clearInterval(State.expandedRunRefreshTimers[runId]);
        delete State.expandedRunRefreshTimers[runId];
      }
      renderActiveRuns();
      renderHistory();
      return;
    }

    // Start loading
    State.expandedRuns[runId] = { loading: true, jobs: [], error: null };
    renderActiveRuns();
    renderHistory();

    const parts = repo.split('/');
    const owner = parts[0];
    const repoName = parts[1];

    try {
      const data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repoName) + '/' + runId);
      State.expandedRuns[runId] = { loading: false, jobs: data.jobs || [], error: null };
    } catch (err) {
      State.expandedRuns[runId] = { loading: false, jobs: [], error: err.message };
    }

    renderActiveRuns();
    renderHistory();

    // Auto-refresh expanded in-progress runs every 5 seconds
    App._startRunDetailRefresh(runId, repo);
  },

  /** Start auto-refresh for an expanded in-progress run */
  _startRunDetailRefresh: function(runId, repo) {
    // Don't start if already refreshing or run is not in-progress
    if (State.expandedRunRefreshTimers[runId]) return;
    var isActive = false;
    for (var i = 0; i < State.repoStates.length; i++) {
      var runs = State.repoStates[i].active_runs || [];
      for (var j = 0; j < runs.length; j++) {
        if (runs[j].id === runId && (runs[j].status === 'in_progress' || runs[j].status === 'queued')) {
          isActive = true;
          break;
        }
      }
      if (isActive) break;
    }
    if (!isActive) return;

    var parts = repo.split('/');
    var owner = parts[0];
    var repoName = parts[1];

    State.expandedRunRefreshTimers[runId] = setInterval(async function() {
      // Stop if collapsed or run no longer in-progress
      if (!State.expandedRuns[runId]) {
        clearInterval(State.expandedRunRefreshTimers[runId]);
        delete State.expandedRunRefreshTimers[runId];
        return;
      }
      try {
        var data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repoName) + '/' + runId);
        if (State.expandedRuns[runId]) {
          State.expandedRuns[runId] = { loading: false, jobs: data.jobs || [], error: null };
          renderActiveRuns();
          renderHistory();
        }
      } catch (err) {
        // Silently ignore refresh errors
      }
      // Check if run completed — stop refreshing
      var stillActive = false;
      for (var i = 0; i < State.repoStates.length; i++) {
        var runs = State.repoStates[i].active_runs || [];
        for (var j = 0; j < runs.length; j++) {
          if (runs[j].id === runId) { stillActive = true; break; }
        }
        if (stillActive) break;
      }
      if (!stillActive) {
        clearInterval(State.expandedRunRefreshTimers[runId]);
        delete State.expandedRunRefreshTimers[runId];
      }
    }, 5000);
  },

  /** Toggle job steps expansion */
  toggleJob: function(jobId, runId) {
    if (State.expandedJobs[jobId]) {
      delete State.expandedJobs[jobId];
    } else {
      State.expandedJobs[jobId] = true;
    }
    renderActiveRuns();
    renderHistory();
  },

  /** Toggle collapse/expand of a workflow repo group */
  toggleWorkflowGroup: function(repo) {
    if (State.collapsedWorkflowGroups[repo]) {
      delete State.collapsedWorkflowGroups[repo];
    } else {
      State.collapsedWorkflowGroups[repo] = true;
    }
    renderWorkflows();
  },

  /** Toggle all workflow groups between expanded and collapsed */
  toggleAllWorkflowGroups: function() {
    var allCollapsed = State.repoStates.every(function(rs) {
      return !rs.workflows || rs.workflows.length === 0 || !!State.collapsedWorkflowGroups[rs.repo];
    });
    if (allCollapsed) {
      State.collapsedWorkflowGroups = {};
    } else {
      State.repoStates.forEach(function(rs) {
        if (rs.workflows && rs.workflows.length > 0) {
          State.collapsedWorkflowGroups[rs.repo] = true;
        }
      });
    }
    renderWorkflows();
  },

  /** Fetch and display job logs in the bottom log viewer panel */
  fetchJobLog: async function(jobId, owner, repo, jobName, workflowName) {
    // Clear any pending auto-retry and stop any existing live tail
    if (State.logRetryTimer) { clearTimeout(State.logRetryTimer); State.logRetryTimer = null; }
    App.stopLiveTail();

    var panel = document.getElementById('log-viewer-panel');
    var content = document.getElementById('log-viewer-content');
    var nameEl = document.getElementById('log-viewer-job-name');

    // Show panel with loading state, uncollapse if collapsed
    panel.style.display = '';
    panel.classList.remove('collapsed');

    // Build informative header: "jobName — workflowName · owner/repo · time"
    var now = new Date().toLocaleTimeString();
    var headerParts = [jobName || 'Job #' + jobId];
    if (workflowName && workflowName !== jobName) headerParts.push(workflowName);
    var headerText = headerParts.join(' \u2014 ');
    headerText += ' \u00B7 ' + owner + '/' + repo + ' \u00B7 ' + now;
    nameEl.textContent = headerText;
    content.innerHTML = '<div class="run-detail-loading">' + BASKETBALL_LOADER_HTML + ' Loading logs...</div>';

    // Track which job is currently shown and update button states
    State.logViewerJobId = jobId;
    State.logViewerMeta = { owner: owner, repo: repo, jobName: jobName, jobId: jobId, workflowName: workflowName, time: now };
    renderActiveRuns();
    renderHistory();

    // Scroll panel into view so user sees the loading state
    setTimeout(function() { panel.scrollIntoView({ behavior: 'smooth', block: 'nearest' }); }, 100);

    // Determine if job is in-progress (check active runs)
    var isJobLive = App._isJobInProgress(jobId);

    try {
      var data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repo) + '/jobs/' + jobId + '/logs');

      // Only update if still showing this job (user may have clicked another)
      if (State.logViewerJobId !== jobId) return;

      App._renderLogContent(content, data);

      // If job is in-progress, start live tailing
      if (isJobLive) {
        App._startLiveTail(jobId, owner, repo, jobName, workflowName, data);
      }
    } catch (err) {
      if (State.logViewerJobId !== jobId) return;
      var msg = err.message || 'Unknown error';
      var isNotReady = msg.indexOf('not available yet') !== -1 || msg.indexOf('in progress') !== -1;
      if (isNotReady) {
        // Show step-level progress instead of blank "waiting"
        App._showStepProgress(jobId, owner, repo, content);
        App._startStepPolling(jobId, owner, repo, jobName, workflowName);
      } else {
        content.innerHTML = '<div class="job-log-error">Error: ' + esc(msg) + '</div>';
      }
    }
  },

  /** Check if a job is currently in-progress by scanning active runs */
  _isJobInProgress: function(jobId) {
    for (var i = 0; i < State.repoStates.length; i++) {
      var runs = State.repoStates[i].active_runs || [];
      for (var j = 0; j < runs.length; j++) {
        var run = runs[j];
        if (run.jobs) {
          for (var k = 0; k < run.jobs.length; k++) {
            if (run.jobs[k].id === jobId) return true;
          }
        }
        // Also check expanded run details
        var detail = State.expandedRuns[run.id];
        if (detail && detail.jobs) {
          for (var k = 0; k < detail.jobs.length; k++) {
            if (detail.jobs[k].id === jobId) return true;
          }
        }
      }
    }
    return false;
  },

  /** Render log steps into the content element */
  _renderLogContent: function(content, data, highlightFrom) {
    var html = '';
    if (data.steps && data.steps.length > 0) {
      html += '<div class="job-log-container">';
      data.steps.forEach(function(step, stepIdx) {
        html += '<div class="job-log-step">';
        html += '<div class="job-log-step-header">' + esc(step.name) + '</div>';
        var lines = step.log.split('\n');
        var pre = '<pre class="job-log-pre">';
        lines.forEach(function(line, lineIdx) {
          var escaped = esc(line);
          // Mark new lines if we have a previous line count to compare
          if (highlightFrom && stepIdx === data.steps.length - 1 && lineIdx >= highlightFrom) {
            pre += '<span class="log-line-new">' + escaped + '</span>\n';
          } else {
            pre += escaped + '\n';
          }
        });
        pre += '</pre>';
        html += pre;
        html += '</div>';
      });
      if (data.truncated) {
        html += '<div class="job-log-truncated">Log truncated (exceeded 2 MB)</div>';
      }
      html += '</div>';
    } else {
      html += '<div class="run-detail-loading">No log content available</div>';
    }
    content.innerHTML = html;
  },

  /** Serialize log data for diffing */
  _serializeLogData: function(data) {
    if (!data || !data.steps) return '';
    var text = '';
    data.steps.forEach(function(step) { text += step.name + ':' + step.log.length + ';'; });
    return text;
  },

  /** Count total lines in log data */
  _countLogLines: function(data) {
    if (!data || !data.steps) return 0;
    var count = 0;
    data.steps.forEach(function(step) { count += step.log.split('\n').length; });
    return count;
  },

  /** Start live log tailing for an in-progress job */
  _startLiveTail: function(jobId, owner, repo, jobName, workflowName, initialData) {
    State.liveLogJobId = jobId;
    State.liveLogAutoScroll = true;
    State.liveLogInterval = 3000;
    State.liveLogStaleCount = 0;
    State.liveLogPrevText = initialData ? App._serializeLogData(initialData) : '';
    State._liveLogPrevLineCount = initialData ? App._countLogLines(initialData) : 0;

    // Show LIVE indicator and auto-scroll button
    var liveEl = document.getElementById('live-log-indicator');
    var scrollBtn = document.getElementById('auto-scroll-btn');
    if (liveEl) liveEl.style.display = '';
    if (scrollBtn) scrollBtn.style.display = '';
    App._updateAutoScrollBtn();

    // Attach scroll listener for auto-scroll detection
    var contentEl = document.getElementById('log-viewer-content');
    contentEl._liveScrollHandler = function() {
      var atBottom = (contentEl.scrollTop + contentEl.clientHeight >= contentEl.scrollHeight - 30);
      State.liveLogAutoScroll = atBottom;
      App._updateAutoScrollBtn();
    };
    contentEl.addEventListener('scroll', contentEl._liveScrollHandler);

    // Start polling loop
    State.liveLogTimer = setInterval(function() {
      if (State.logViewerJobId !== jobId || State.liveLogJobId !== jobId) {
        App.stopLiveTail();
        return;
      }
      App._pollLiveLog(jobId, owner, repo, jobName, workflowName);
    }, State.liveLogInterval);
  },

  /** Poll for updated logs (called by setInterval) */
  _pollLiveLog: async function(jobId, owner, repo, jobName, workflowName) {
    // Rate limit guard
    if (State.rateLimit && State.rateLimit.remaining < 30) {
      App.stopLiveTail();
      App.showToast('Live tailing paused \u2014 rate limit low (' + State.rateLimit.remaining + ' remaining)', 'warning');
      return;
    }
    if (State.rateLimit && State.rateLimit.remaining < 100) {
      // Slow down polling
      if (State.liveLogInterval < 10000) {
        State.liveLogInterval = 10000;
        clearInterval(State.liveLogTimer);
        State.liveLogTimer = setInterval(function() {
          if (State.logViewerJobId !== jobId || State.liveLogJobId !== jobId) { App.stopLiveTail(); return; }
          App._pollLiveLog(jobId, owner, repo, jobName, workflowName);
        }, State.liveLogInterval);
        var warnEl = document.getElementById('live-rate-warning');
        if (warnEl) warnEl.style.display = '';
      }
    }

    try {
      var data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repo) + '/jobs/' + jobId + '/logs');
      if (State.logViewerJobId !== jobId || State.liveLogJobId !== jobId) return;

      var newText = App._serializeLogData(data);
      if (newText !== State.liveLogPrevText) {
        // Content changed — render with new-line highlighting
        State.liveLogStaleCount = 0;
        var prevLineCount = State._liveLogPrevLineCount || 0;
        var newLineCount = App._countLogLines(data);
        var highlightFrom = prevLineCount > 0 ? prevLineCount : null;

        var content = document.getElementById('log-viewer-content');
        App._renderLogContent(content, data, (data.steps.length > 0 && highlightFrom) ? highlightFrom : null);

        State.liveLogPrevText = newText;
        State._liveLogPrevLineCount = newLineCount;

        // Auto-scroll to bottom
        if (State.liveLogAutoScroll) {
          content.scrollTop = content.scrollHeight;
        }

        // Fade out new-line highlights after 2 seconds
        setTimeout(function() {
          var newLines = content.querySelectorAll('.log-line-new');
          for (var i = 0; i < newLines.length; i++) {
            newLines[i].classList.add('faded');
          }
        }, 2000);
      } else {
        // No change — adaptive polling (slow down after 5 unchanged polls)
        State.liveLogStaleCount++;
        if (State.liveLogStaleCount >= 5 && State.liveLogInterval < 10000) {
          State.liveLogInterval = 10000;
          clearInterval(State.liveLogTimer);
          State.liveLogTimer = setInterval(function() {
            if (State.logViewerJobId !== jobId || State.liveLogJobId !== jobId) { App.stopLiveTail(); return; }
            App._pollLiveLog(jobId, owner, repo, jobName, workflowName);
          }, State.liveLogInterval);
        }
      }

      // Check if job is still in-progress
      if (!App._isJobInProgress(jobId)) {
        // Job completed — do one final render and stop
        App.stopLiveTail();
      }
    } catch (err) {
      if (State.logViewerJobId !== jobId || State.liveLogJobId !== jobId) return;
      var msg = err.message || '';
      var isNotReady = msg.indexOf('not available yet') !== -1 || msg.indexOf('in progress') !== -1;
      if (isNotReady) {
        // Logs not yet available — keep polling, show waiting message if first time
        var content = document.getElementById('log-viewer-content');
        if (!content.querySelector('.job-log-container')) {
          content.innerHTML = '<div class="run-detail-loading">' +
            BASKETBALL_LOADER_HTML + ' Waiting for logs \u2014 job is still in progress...' +
            '</div>';
        }
      }
      // Other errors: silently ignore, keep polling
    }
  },

  /** Stop live log tailing and clean up */
  stopLiveTail: function() {
    if (State.liveLogTimer) {
      clearInterval(State.liveLogTimer);
      State.liveLogTimer = null;
    }
    State.liveLogJobId = null;
    State.liveLogPrevText = '';
    State.liveLogStaleCount = 0;
    State._liveLogPrevLineCount = 0;

    // Hide LIVE indicator and auto-scroll button
    var liveEl = document.getElementById('live-log-indicator');
    var scrollBtn = document.getElementById('auto-scroll-btn');
    var warnEl = document.getElementById('live-rate-warning');
    if (liveEl) liveEl.style.display = 'none';
    if (scrollBtn) scrollBtn.style.display = 'none';
    if (warnEl) warnEl.style.display = 'none';

    // Remove scroll listener
    var contentEl = document.getElementById('log-viewer-content');
    if (contentEl && contentEl._liveScrollHandler) {
      contentEl.removeEventListener('scroll', contentEl._liveScrollHandler);
      contentEl._liveScrollHandler = null;
    }
  },

  // ═══════════════════════════════════════════════════════
  //  STEP PROGRESS (shown while waiting for in-progress job logs)
  // ═══════════════════════════════════════════════════════

  /** Fetch and render step-level progress for an in-progress job */
  _showStepProgress: async function(jobId, owner, repo, contentEl) {
    try {
      var data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repo) + '/jobs/' + jobId + '/steps');
      if (State.logViewerJobId !== jobId) return;
      App._renderStepProgress(contentEl, data);
    } catch (err) {
      // Fallback: show generic waiting message if steps endpoint also fails
      contentEl.innerHTML = '<div class="run-detail-loading">' +
        BASKETBALL_LOADER_HTML + ' Waiting for logs \u2014 job is still in progress...' +
        '</div>';
    }
  },

  /** Render step progress timeline into the content element */
  _renderStepProgress: function(contentEl, data) {
    var html = '<div class="step-progress">';
    html += '<div class="step-progress-header">' +
      '<span class="step-progress-title">' + esc(data.job_name) + '</span>' +
      '<span class="step-progress-badge live">' + BASKETBALL_LOADER_HTML + ' LIVE</span>' +
      '</div>';

    if (data.steps && data.steps.length > 0) {
      data.steps.forEach(function(step) {
        var statusClass = step.conclusion || step.status || 'queued';
        var icon = '&#9675;'; // ○ queued
        if (step.status === 'completed') {
          icon = step.conclusion === 'success' ? '&#10003;' : (step.conclusion === 'failure' ? '&#10007;' : '&#8212;');
        } else if (step.status === 'in_progress') {
          icon = '&#9679;'; // ● filled circle
        }

        var duration = '';
        if (step.started_at) {
          var start = new Date(step.started_at);
          if (step.completed_at) {
            var end = new Date(step.completed_at);
            duration = App._formatDuration(Math.round((end - start) / 1000));
          } else {
            // In-progress: show elapsed time
            duration = App._formatDuration(Math.round((Date.now() - start) / 1000)) + '\u2026';
          }
        }

        html += '<div class="step-progress-item ' + esc(statusClass) + '" data-step-number="' + step.number + '">';
        html += '<span class="step-icon">' + icon + '</span>';
        html += '<span class="step-name">' + esc(step.name) + '</span>';
        html += '<span class="step-duration">' + duration + '</span>';
        html += '</div>';
      });
    } else {
      html += '<div class="step-progress-item queued"><span class="step-icon">&#9679;</span><span class="step-name">Waiting for steps...</span></div>';
    }

    html += '</div>';
    contentEl.innerHTML = html;
  },

  /** Format seconds into a human-readable duration */
  _formatDuration: function(secs) {
    if (secs < 60) return secs + 's';
    var m = Math.floor(secs / 60);
    var s = secs % 60;
    if (m < 60) return m + 'm ' + s + 's';
    var h = Math.floor(m / 60);
    m = m % 60;
    return h + 'h ' + m + 'm';
  },

  /** Start polling for step progress updates */
  _startStepPolling: function(jobId, owner, repo, jobName, workflowName) {
    // Stop any existing live tail
    App.stopLiveTail();

    // Show LIVE indicator
    var liveEl = document.getElementById('live-log-indicator');
    if (liveEl) liveEl.style.display = '';

    State.liveLogJobId = jobId;
    State.liveLogInterval = 3000;
    State.liveLogStaleCount = 0;

    State.liveLogTimer = setInterval(function() {
      if (State.logViewerJobId !== jobId || State.liveLogJobId !== jobId) {
        App.stopLiveTail();
        return;
      }
      App._pollStepProgress(jobId, owner, repo, jobName, workflowName);
    }, 3000);
  },

  /** Poll step progress and transition to logs when job completes */
  _pollStepProgress: async function(jobId, owner, repo, jobName, workflowName) {
    // Rate limit guard
    if (State.rateLimit && State.rateLimit.remaining < 30) {
      App.stopLiveTail();
      App.showToast('Step polling paused \u2014 rate limit low', 'warning');
      return;
    }

    try {
      var data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repo) + '/jobs/' + jobId + '/steps');
      if (State.logViewerJobId !== jobId) return;

      var contentEl = document.getElementById('log-viewer-content');
      App._renderStepProgress(contentEl, data);

      // Check if job completed — transition to full logs
      if (data.status === 'completed') {
        App.stopLiveTail();
        // Brief transition message
        contentEl.innerHTML = '<div class="run-detail-loading">' +
          BASKETBALL_LOADER_HTML + ' Job completed \u2014 loading logs...' +
          '</div>';
        // Fetch full logs
        try {
          var logData = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repo) + '/jobs/' + jobId + '/logs');
          if (State.logViewerJobId !== jobId) return;
          App._renderLogContent(contentEl, logData);
        } catch (logErr) {
          // Logs might not be available yet even after completion — retry after short delay
          setTimeout(function() {
            if (State.logViewerJobId === jobId) {
              App.fetchJobLog(jobId, owner, repo, jobName, workflowName);
            }
          }, 2000);
        }
      }
    } catch (err) {
      // Silently continue polling — endpoint may be temporarily unavailable
    }
  },

  /** Toggle auto-scroll for live log tailing */
  toggleAutoScroll: function() {
    State.liveLogAutoScroll = !State.liveLogAutoScroll;
    App._updateAutoScrollBtn();
    if (State.liveLogAutoScroll) {
      var content = document.getElementById('log-viewer-content');
      if (content) content.scrollTop = content.scrollHeight;
    }
  },

  /** Update auto-scroll button appearance */
  _updateAutoScrollBtn: function() {
    var btn = document.getElementById('auto-scroll-btn');
    if (!btn) return;
    if (State.liveLogAutoScroll) {
      btn.classList.add('active');
      btn.innerHTML = '&#8595; Auto-scroll';
    } else {
      btn.classList.remove('active');
      btn.innerHTML = '&#8595; Auto-scroll off';
    }
  },

  /** Close the log viewer panel */
  clearLogViewer: function() {
    if (State.logRetryTimer) { clearTimeout(State.logRetryTimer); State.logRetryTimer = null; }
    App.stopLiveTail();
    document.getElementById('log-viewer-panel').style.display = 'none';
    document.getElementById('log-viewer-content').innerHTML = '';
    State.logViewerJobId = null;
    renderActiveRuns();
    renderHistory();
  },

  /** Toggle collapse/expand of the log viewer panel */
  toggleLogViewerCollapse: function(event) {
    if (event) event.stopPropagation();
    var panel = document.getElementById('log-viewer-panel');
    panel.classList.toggle('collapsed');
  },

  /** Cancel a run */
  cancelRun: async function(runId, repo, event) {
    if (event) event.stopPropagation();
    const parts = repo.split('/');
    const owner = parts[0];
    const repoName = parts[1];

    try {
      const data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repoName) + '/' + runId + '/cancel', { method: 'POST' });
      App.showToast(data.message || 'Run cancelled', 'success');
    } catch (err) {
      App.showToast('Cancel failed: ' + err.message, 'error');
    }
  },

  /** Re-run a workflow */
  rerunRun: async function(runId, repo, event) {
    if (event) event.stopPropagation();
    const parts = repo.split('/');
    const owner = parts[0];
    const repoName = parts[1];

    try {
      const data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repoName) + '/' + runId + '/rerun', { method: 'POST' });
      App.showToast(data.message || 'Workflow re-triggered', 'success');
      // Switch to Active Runs tab and trigger refresh so the new run appears
      App.switchTab('active-runs');
      setTimeout(function() { App.manualRefresh(); }, 1500);
    } catch (err) {
      App.showToast('Re-run failed: ' + err.message, 'error');
    }
  },

  /** Filter workflows by search query */
  filterWorkflows: function() {
    State.workflowSearchQuery = document.getElementById('workflow-search-input').value;
    renderWorkflows();
  },

  /** Filter actions by search query */
  filterActions: function() {
    State.actionsSearchQuery = document.getElementById('actions-search-input').value;
    renderActions();
  },

  /** Filter history by search query */
  filterHistory: function() {
    State.historySearchQuery = document.getElementById('history-search-input').value;
    renderHistory();
  },

  /** Set history time range filter */
  setHistoryTimeRange: function(range) {
    State.historyTimeRange = range;
    document.querySelectorAll('.history-time-filter .time-filter-btn').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.range === range);
    });
    if (range !== 'custom') {
      document.getElementById('history-custom-range').style.display = 'none';
    }
    renderHistory();
  },

  /** Toggle custom time range picker for history */
  toggleHistoryCustomRange: function() {
    var el = document.getElementById('history-custom-range');
    var isVisible = el.style.display !== 'none';
    el.style.display = isVisible ? 'none' : 'flex';
    State.historyTimeRange = 'custom';
    document.querySelectorAll('.history-time-filter .time-filter-btn').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.range === 'custom');
    });
  },

  /** Apply custom date range for history */
  applyHistoryCustomRange: function() {
    State.historyCustomFrom = document.getElementById('history-range-from').value;
    State.historyCustomTo = document.getElementById('history-range-to').value;
    renderHistory();
  },

  /** Set What's Happening time range */
  setWhatsHappeningRange: function(range) {
    State.whatsHappeningRange = range;
    document.querySelectorAll('.wh-time-filter .time-filter-btn').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.range === range);
    });
    if (range !== 'custom') {
      document.getElementById('wh-custom-range').style.display = 'none';
    }
    renderWhatsHappening();
  },

  /** Toggle custom time range picker for What's Happening */
  toggleWhatsHappeningCustomRange: function() {
    var el = document.getElementById('wh-custom-range');
    var isVisible = el.style.display !== 'none';
    el.style.display = isVisible ? 'none' : 'flex';
    State.whatsHappeningRange = 'custom';
    document.querySelectorAll('.wh-time-filter .time-filter-btn').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.range === 'custom');
    });
  },

  /** Apply custom date range for What's Happening */
  applyWhatsHappeningCustomRange: function() {
    State.whatsHappeningCustomFrom = document.getElementById('wh-range-from').value;
    State.whatsHappeningCustomTo = document.getElementById('wh-range-to').value;
    renderWhatsHappening();
  },

  /** Set Analytics time range */
  setAnalyticsRange: function(range) {
    State.analyticsRange = range;
    document.querySelectorAll('.an-time-filter .time-filter-btn').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.range === range);
    });
    if (range !== 'custom') {
      document.getElementById('an-custom-range').style.display = 'none';
    }
    renderAnalytics();
  },

  /** Toggle custom range picker for Analytics */
  toggleAnalyticsCustomRange: function() {
    var el = document.getElementById('an-custom-range');
    var isVisible = el.style.display !== 'none';
    el.style.display = isVisible ? 'none' : 'flex';
    State.analyticsRange = 'custom';
    document.querySelectorAll('.an-time-filter .time-filter-btn').forEach(function(btn) {
      btn.classList.toggle('active', btn.dataset.range === 'custom');
    });
  },

  /** Apply custom date range for Analytics */
  applyAnalyticsCustomRange: function() {
    State.analyticsCustomFrom = document.getElementById('an-range-from').value;
    State.analyticsCustomTo = document.getElementById('an-range-to').value;
    renderAnalytics();
  },

  /** Quick-run a workflow on its default branch (main or master).
   *  If workflow has required inputs, opens the full modal instead. */
  quickRunAction: async function(repo, workflowId, workflowName) {
    var parts = repo.split('/');

    // Check for required inputs first
    try {
      var inputsData = await apiFetch('/api/workflows/' + encodeURIComponent(parts[0]) + '/' + encodeURIComponent(parts[1]) + '/' + workflowId + '/inputs');
      if (inputsData && !inputsData.has_dispatch) {
        App.showToast('This workflow has no workflow_dispatch trigger', 'error');
        return;
      }
      // If there are required inputs, open the full modal
      if (inputsData && inputsData.inputs) {
        var hasRequired = inputsData.inputs.some(function(inp) { return inp.required; });
        if (hasRequired || inputsData.inputs.length > 0) {
          App.showToast('This workflow has inputs — opening configure dialog', 'info');
          App.openDispatchModal(repo, workflowId, workflowName);
          return;
        }
      }
    } catch (_) {
      // If we can't fetch inputs, try dispatching anyway
    }

    // No required inputs — quick dispatch
    var defaultBranch = 'main';
    try {
      var branches = await apiFetch('/api/branches/' + encodeURIComponent(parts[0]) + '/' + encodeURIComponent(parts[1]));
      if (Array.isArray(branches) && branches.length > 0) {
        var mainBranch = branches.find(function(b) { return b.name === 'main'; }) ||
                         branches.find(function(b) { return b.name === 'master'; }) ||
                         branches[0];
        if (mainBranch) defaultBranch = mainBranch.name;
      }
    } catch (_) {}

    App.showToast('Running "' + workflowName + '" on ' + defaultBranch + '...', 'info');

    try {
      var body = {
        repo: repo,
        workflow_id: workflowId,
        ref: defaultBranch,
      };
      var data = await apiFetch('/api/workflows/dispatch', {
        method: 'POST',
        body: JSON.stringify(body)
      });
      App.showToast(data.message || 'Workflow dispatched on ' + defaultBranch, 'success');
    } catch (err) {
      App.showToast('Dispatch failed: ' + err.message, 'error');
    }
  },

  /** Branch autocomplete active index (-1 = none) */
  _branchActiveIdx: -1,

  /** Show branch dropdown with filtered results */
  _showBranchDropdown: function(query) {
    var dropdown = document.getElementById('branch-dropdown');
    var branches = State.dispatchContext ? State.dispatchContext.branches || [] : [];
    var q = (query || '').toLowerCase();
    var priorityNames = ['development', 'integration', 'main'];

    var filtered = branches.filter(function(b) {
      if (!q) return true;
      return b.name.toLowerCase().indexOf(q) >= 0;
    });

    if (filtered.length === 0) {
      dropdown.innerHTML = '<div class="branch-option" style="color:var(--text-muted);cursor:default;">' +
        (q ? 'No branches match "' + esc(query) + '"' : 'No branches available') + '</div>';
      dropdown.classList.add('visible');
      App._branchActiveIdx = -1;
      return;
    }

    var html = '';
    filtered.forEach(function(b, i) {
      var name = b.name;
      var isPriority = priorityNames.indexOf(name) >= 0;
      // Highlight matching text
      var display = name;
      if (q) {
        var idx = name.toLowerCase().indexOf(q);
        if (idx >= 0) {
          display = esc(name.substring(0, idx)) +
            '<span class="branch-match">' + esc(name.substring(idx, idx + q.length)) + '</span>' +
            esc(name.substring(idx + q.length));
        } else {
          display = esc(name);
        }
      } else {
        display = esc(name);
      }
      html += '<div class="branch-option' + (i === App._branchActiveIdx ? ' active' : '') + '" data-index="' + i + '" data-value="' + esc(name) + '"' +
        ' onmousedown="App.selectBranch(\'' + escJs(name) + '\')" onmouseenter="App._branchActiveIdx=' + i + ';App._highlightBranchOption()">' +
        display + (isPriority ? '<span class="branch-priority">★</span>' : '') +
        '</div>';
    });
    dropdown.innerHTML = html;
    dropdown.classList.add('visible');
  },

  /** Hide branch dropdown */
  _hideBranchDropdown: function() {
    var dropdown = document.getElementById('branch-dropdown');
    if (dropdown) {
      dropdown.classList.remove('visible');
      App._branchActiveIdx = -1;
    }
  },

  /** Highlight the active branch option */
  _highlightBranchOption: function() {
    var dropdown = document.getElementById('branch-dropdown');
    var options = dropdown.querySelectorAll('.branch-option[data-value]');
    options.forEach(function(opt, i) {
      opt.classList.toggle('active', i === App._branchActiveIdx);
    });
  },

  /** Select a branch from autocomplete */
  selectBranch: function(name) {
    document.getElementById('dispatch-branch').value = name;
    App._hideBranchDropdown();
  },

  /** Handle branch input typing */
  onBranchInput: function() {
    var query = document.getElementById('dispatch-branch').value;
    App._branchActiveIdx = -1;
    App._showBranchDropdown(query);
  },

  /** Handle branch input focus — show all options */
  onBranchFocus: function() {
    var query = document.getElementById('dispatch-branch').value;
    App._showBranchDropdown(query);
  },

  /** Handle branch input keydown for keyboard navigation */
  onBranchKeydown: function(e) {
    var dropdown = document.getElementById('branch-dropdown');
    var options = dropdown.querySelectorAll('.branch-option[data-value]');
    var count = options.length;
    if (!count) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      App._branchActiveIdx = Math.min(App._branchActiveIdx + 1, count - 1);
      App._highlightBranchOption();
      if (options[App._branchActiveIdx]) options[App._branchActiveIdx].scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      App._branchActiveIdx = Math.max(App._branchActiveIdx - 1, 0);
      App._highlightBranchOption();
      if (options[App._branchActiveIdx]) options[App._branchActiveIdx].scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (App._branchActiveIdx >= 0 && options[App._branchActiveIdx]) {
        App.selectBranch(options[App._branchActiveIdx].dataset.value);
      }
    } else if (e.key === 'Escape') {
      App._hideBranchDropdown();
    }
  },

  /** Open dispatch modal. prefillInputs is an optional { inputName: value } map from the command bar. */
  openDispatchModal: async function(repo, workflowId, workflowName, prefillInputs) {
    State.dispatchContext = { repo: repo, workflowId: workflowId, workflowName: workflowName, workflowInputs: [], branches: [], prefillInputs: prefillInputs || null };

    document.getElementById('dispatch-repo').value = repo;
    document.getElementById('dispatch-workflow-name').value = workflowName;

    // Reset branch autocomplete
    document.getElementById('dispatch-branch').value = '';
    document.getElementById('dispatch-branch').placeholder = 'Loading branches...';
    App._hideBranchDropdown();

    // Reset inputs
    var inputsList = document.getElementById('dispatch-inputs-list');
    inputsList.innerHTML = '';
    var customSection = document.getElementById('dispatch-custom-inputs');
    customSection.style.display = 'none';
    document.getElementById('dispatch-custom-inputs-list').innerHTML = '';
    var inputsLoading = document.getElementById('dispatch-inputs-loading');
    var noDispatch = document.getElementById('dispatch-no-dispatch');
    inputsLoading.style.display = 'flex';
    noDispatch.style.display = 'none';

    // Enable run button
    var runBtn = document.getElementById('dispatch-run-btn');
    runBtn.disabled = false;
    runBtn.textContent = 'Run Workflow';

    // Show modal
    document.getElementById('dispatch-modal').classList.add('visible');

    var parts = repo.split('/');

    // Fetch branches and inputs concurrently
    var branchesPromise = apiFetch('/api/branches/' + encodeURIComponent(parts[0]) + '/' + encodeURIComponent(parts[1])).catch(function() { return null; });
    var inputsPromise = apiFetch('/api/workflows/' + encodeURIComponent(parts[0]) + '/' + encodeURIComponent(parts[1]) + '/' + workflowId + '/inputs').catch(function() { return null; });

    var results = await Promise.all([branchesPromise, inputsPromise]);
    var branches = results[0];
    var inputsData = results[1];

    // Store branches (pre-sorted from backend) and set default
    if (Array.isArray(branches) && branches.length > 0) {
      State.dispatchContext.branches = branches;
    } else {
      State.dispatchContext.branches = [{ name: 'main', protected: false }];
    }
    var branchInput = document.getElementById('dispatch-branch');
    branchInput.placeholder = 'Type to search branches...';
    branchInput.value = State.dispatchContext.branches[0].name;

    // Populate inputs
    inputsLoading.style.display = 'none';

    if (inputsData && !inputsData.has_dispatch) {
      noDispatch.style.display = 'block';
      runBtn.disabled = true;
      return;
    }

    if (inputsData && inputsData.inputs && inputsData.inputs.length > 0) {
      State.dispatchContext.workflowInputs = inputsData.inputs;
      App._renderWorkflowInputs(inputsData.inputs, inputsList);
      // Pre-fill inputs from command bar (if provided)
      App._applyPrefillInputs(inputsList, State.dispatchContext.prefillInputs);
    } else {
      inputsList.innerHTML = '<div style="font-size:var(--font-size-xs);color:var(--text-muted);padding:4px 0;">No defined inputs for this workflow.</div>';
    }

    // Show custom inputs section
    customSection.style.display = 'block';
  },

  /** Apply pre-fill values from the command bar to rendered input fields */
  _applyPrefillInputs: function(container, prefillInputs) {
    if (!prefillInputs || typeof prefillInputs !== 'object') return;
    var keys = Object.keys(prefillInputs);
    for (var i = 0; i < keys.length; i++) {
      var inputName = keys[i];
      var value = prefillInputs[inputName];
      var field = container.querySelector('.wf-input-field[data-input-name="' + inputName + '"]');
      if (!field) continue;
      var el = field.querySelector('.wf-input-value');
      if (!el) continue;
      var inputType = el.dataset.inputType;
      if (inputType === 'boolean') {
        el.checked = (value === 'true' || value === true);
      } else {
        el.value = value;
      }
    }
  },

  /** Render typed workflow input fields */
  _renderWorkflowInputs: function(inputs, container) {
    var html = '';
    inputs.forEach(function(inp) {
      html += '<div class="wf-input-field" data-input-name="' + esc(inp.name) + '">';
      html += '<div class="wf-input-field-header">';
      html += '<span class="wf-input-name">' + esc(inp.name) + '</span>';
      if (inp.required) html += '<span class="wf-input-required">*required</span>';
      html += '<span class="wf-input-type">(' + esc(inp.type || 'string') + ')</span>';
      html += '</div>';
      if (inp.description) {
        html += '<div class="wf-input-desc">' + esc(inp.description) + '</div>';
      }

      var inputType = inp.type || 'string';
      var defaultVal = inp.default != null ? inp.default : '';

      if (inputType === 'boolean') {
        var checked = defaultVal === 'true' ? ' checked' : '';
        html += '<div class="wf-input-checkbox-wrap">';
        html += '<input type="checkbox" class="wf-input-value" data-input-type="boolean"' + checked + '>';
        html += '<span style="font-size:var(--font-size-xs);color:var(--text-muted);">' + (defaultVal ? 'default: ' + esc(defaultVal) : '') + '</span>';
        html += '</div>';
      } else if (inputType === 'choice' && inp.options && inp.options.length > 0) {
        html += '<select class="form-select wf-input-value" data-input-type="choice">';
        inp.options.forEach(function(opt) {
          var selected = opt === defaultVal ? ' selected' : '';
          html += '<option value="' + esc(opt) + '"' + selected + '>' + esc(opt) + '</option>';
        });
        html += '</select>';
      } else if (inputType === 'number') {
        html += '<input type="number" class="form-input wf-input-value" data-input-type="number" value="' + esc(defaultVal) + '" placeholder="Enter number">';
      } else {
        // string / environment / unknown
        html += '<input type="text" class="form-input wf-input-value" data-input-type="string" value="' + esc(defaultVal) + '" placeholder="Enter value">';
      }

      html += '</div>';
    });
    container.innerHTML = html;
  },

  /** Close dispatch modal */
  closeDispatchModal: function() {
    document.getElementById('dispatch-modal').classList.remove('visible');
    State.dispatchContext = null;
  },

  /** Add custom input key-value pair in dispatch modal */
  addDispatchInput: function() {
    var list = document.getElementById('dispatch-custom-inputs-list');
    var pair = document.createElement('div');
    pair.className = 'input-pair';
    pair.innerHTML = '<input type="text" class="form-input dispatch-input-key" placeholder="key">' +
      '<input type="text" class="form-input dispatch-input-val" placeholder="value">' +
      '<button type="button" class="repo-remove-btn" style="opacity:1" onclick="this.parentElement.remove()">&times;</button>';
    list.appendChild(pair);
    pair.querySelector('.dispatch-input-key').focus();
  },

  /** Dispatch (trigger) a workflow */
  dispatchWorkflow: async function() {
    var ctx = State.dispatchContext;
    if (!ctx) return;

    var branch = document.getElementById('dispatch-branch').value.trim();
    if (!branch) {
      App.showToast('Please enter a branch name', 'error');
      return;
    }

    // Collect typed workflow inputs
    var inputs = {};
    var hasInputs = false;
    document.querySelectorAll('#dispatch-inputs-list .wf-input-field').forEach(function(field) {
      var name = field.dataset.inputName;
      var el = field.querySelector('.wf-input-value');
      if (!el || !name) return;
      var inputType = el.dataset.inputType;
      var value;
      if (inputType === 'boolean') {
        value = el.checked ? 'true' : 'false';
      } else {
        value = el.value;
      }
      if (value !== '' || (inputType === 'boolean')) {
        inputs[name] = value;
        hasInputs = true;
      }
    });

    // Collect custom key-value inputs
    document.querySelectorAll('#dispatch-custom-inputs-list .input-pair').forEach(function(pair) {
      var key = pair.querySelector('.dispatch-input-key').value.trim();
      var val = pair.querySelector('.dispatch-input-val').value.trim();
      if (key) {
        inputs[key] = val;
        hasInputs = true;
      }
    });

    // Validate required fields
    var missingRequired = [];
    if (ctx.workflowInputs) {
      ctx.workflowInputs.forEach(function(inp) {
        if (inp.required && (!inputs[inp.name] || inputs[inp.name] === '')) {
          missingRequired.push(inp.name);
        }
      });
    }
    if (missingRequired.length > 0) {
      App.showToast('Missing required inputs: ' + missingRequired.join(', '), 'error');
      return;
    }

    var body = {
      repo: ctx.repo,
      workflow_id: ctx.workflowId,
      ref: branch,
    };
    if (hasInputs) body.inputs = inputs;

    var btn = document.getElementById('dispatch-run-btn');
    btn.disabled = true;
    btn.textContent = 'Dispatching...';

    try {
      var data = await apiFetch('/api/workflows/dispatch', {
        method: 'POST',
        body: JSON.stringify(body)
      });
      App.showToast(data.message || 'Workflow dispatched', 'success');
      App.closeDispatchModal();
      // Switch to Active Runs tab and trigger refresh so the new run appears
      App.switchTab('active-runs');
      setTimeout(function() { App.manualRefresh(); }, 1500);
    } catch (err) {
      App.showToast('Dispatch failed: ' + err.message, 'error');
    } finally {
      btn.disabled = false;
      btn.textContent = 'Run Workflow';
    }
  },

  /* ═══════════════════════════════════════════════════════════════
     STEP-LEVEL LOG VIEWER
     ═══════════════════════════════════════════════════════════════ */

  toggleStepLogs: async function(runId, repo, jobId, stepNumber, event) {
    if (event) event.stopPropagation();

    var info = State.expandedRuns[runId];
    if (!info || info.loading) return;

    if (!info.jobLogsById) info.jobLogsById = {};
    if (!info.jobLogsTruncatedById) info.jobLogsTruncatedById = {};
    if (!info.jobLogsLoadingById) info.jobLogsLoadingById = {};
    if (!info.jobLogsErrorById) info.jobLogsErrorById = {};
    if (!info.expandedStepLogs) info.expandedStepLogs = {};
    if (!info.stepLogsByKey) info.stepLogsByKey = {};

    var stepKey = runId + '::' + jobId + '::' + stepNumber;
    if (info.expandedStepLogs[stepKey]) {
      delete info.expandedStepLogs[stepKey];
      renderActiveRuns();
      return;
    }
    info.expandedStepLogs[stepKey] = true;

    var job = (info.jobs || []).find(function(j) { return j.id === jobId; });
    var step = job && Array.isArray(job.steps)
      ? job.steps.find(function(s) { return Number(s.number) === Number(stepNumber); })
      : null;
    if (!step) {
      info.stepLogsByKey[stepKey] = 'Step metadata not available.';
      renderActiveRuns();
      return;
    }

    if (info.stepLogsByKey[stepKey]) {
      renderActiveRuns();
      return;
    }

    if (info.jobLogsById[jobId]) {
      info.stepLogsByKey[stepKey] = extractStepLogFromJobLogs(info.jobLogsById[jobId], step);
      renderActiveRuns();
      return;
    }

    if (info.jobLogsLoadingById[jobId]) {
      renderActiveRuns();
      return;
    }

    info.jobLogsLoadingById[jobId] = true;
    delete info.jobLogsErrorById[jobId];
    renderActiveRuns();

    var parts = String(repo || '').split('/');
    var owner = parts[0];
    var repoName = parts[1];
    if (!owner || !repoName) {
      info.jobLogsLoadingById[jobId] = false;
      info.jobLogsErrorById[jobId] = 'Invalid repository identifier';
      renderActiveRuns();
      return;
    }

    try {
      var data = await apiFetch('/api/runs/' + encodeURIComponent(owner) + '/' + encodeURIComponent(repoName) + '/jobs/' + jobId + '/logs');
      var updatedInfo = State.expandedRuns[runId];
      if (!updatedInfo) return;

      if (!updatedInfo.jobLogsById) updatedInfo.jobLogsById = {};
      if (!updatedInfo.jobLogsTruncatedById) updatedInfo.jobLogsTruncatedById = {};
      if (!updatedInfo.jobLogsLoadingById) updatedInfo.jobLogsLoadingById = {};
      if (!updatedInfo.stepLogsByKey) updatedInfo.stepLogsByKey = {};
      if (!updatedInfo.expandedStepLogs) updatedInfo.expandedStepLogs = {};

      updatedInfo.jobLogsById[jobId] = data && data.logs ? String(data.logs) : '';
      updatedInfo.jobLogsTruncatedById[jobId] = !!(data && data.truncated);
      updatedInfo.jobLogsLoadingById[jobId] = false;

      var updatedJob = (updatedInfo.jobs || []).find(function(j) { return j.id === jobId; });
      var updatedSteps = updatedJob && Array.isArray(updatedJob.steps) ? updatedJob.steps : [];
      updatedSteps.forEach(function(s) {
        var key = runId + '::' + jobId + '::' + s.number;
        if (updatedInfo.expandedStepLogs[key] && !updatedInfo.stepLogsByKey[key]) {
          updatedInfo.stepLogsByKey[key] = extractStepLogFromJobLogs(updatedInfo.jobLogsById[jobId], s);
        }
      });
    } catch (err) {
      var failedInfo = State.expandedRuns[runId];
      if (!failedInfo) return;
      if (!failedInfo.jobLogsLoadingById) failedInfo.jobLogsLoadingById = {};
      if (!failedInfo.jobLogsErrorById) failedInfo.jobLogsErrorById = {};
      failedInfo.jobLogsLoadingById[jobId] = false;
      failedInfo.jobLogsErrorById[jobId] = err.message || 'Failed to fetch logs';
    }

    renderActiveRuns();
  },

  openStepLogViewer: function(runId, jobId, stepNumber, event) {
    if (event) event.stopPropagation();
    var info = State.expandedRuns[runId];
    if (!info) {
      App.showToast('Run details are not loaded', 'error');
      return;
    }

    var key = runId + '::' + jobId + '::' + stepNumber;
    var logText = (info.stepLogsByKey && Object.prototype.hasOwnProperty.call(info.stepLogsByKey, key))
      ? String(info.stepLogsByKey[key] || '')
      : '';
    if (!logText) {
      App.showToast('Load step logs first', 'info');
      return;
    }

    var job = (info.jobs || []).find(function(j) { return j.id === jobId; });
    var step = job && Array.isArray(job.steps)
      ? job.steps.find(function(s) { return Number(s.number) === Number(stepNumber); })
      : null;
    var subtitleParts = ['Run #' + runId];
    if (job) subtitleParts.push('Job: ' + job.name);
    if (step) subtitleParts.push('Step ' + step.number + ': ' + step.name);

    var subtitleEl = document.getElementById('log-viewer-subtitle');
    var contentEl = document.getElementById('log-viewer-content');
    var noteEl = document.getElementById('log-viewer-note');

    if (subtitleEl) subtitleEl.textContent = subtitleParts.join(' | ');
    if (contentEl) contentEl.textContent = logText;
    if (noteEl) {
      var truncated = !!(info.jobLogsTruncatedById && info.jobLogsTruncatedById[jobId]);
      noteEl.style.display = truncated ? 'block' : 'none';
    }

    document.getElementById('log-viewer-modal').classList.add('visible');
  },

  closeStepLogViewer: function() {
    var modal = document.getElementById('log-viewer-modal');
    if (!modal) return;
    modal.classList.remove('visible');
  },

  /* ═══════════════════════════════════════════════════════════════
     WORKFLOW SELECTION & CUSTOM TABS
     ═══════════════════════════════════════════════════════════════ */

  toggleWorkflowSelection: function(wfKey, wfName) {
    if (State.selectedWorkflowKeys[wfKey]) {
      delete State.selectedWorkflowKeys[wfKey];
    } else {
      State.selectedWorkflowKeys[wfKey] = wfName || true;
    }
    renderWorkflows();
  },

  clearWorkflowSelection: function() {
    State.selectedWorkflowKeys = {};
    renderWorkflows();
  },

  createCustomTab: function() {
    var keys = Object.keys(State.selectedWorkflowKeys);
    if (keys.length === 0) return;

    var nameInput = document.getElementById('custom-tab-name-input');
    var name = (nameInput && nameInput.value || '').trim() || ('Tab ' + (State.customWorkflowTabs.length + 1));

    var tab = {
      id: 'custom-workflow-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
      name: name,
      workflowKeys: keys.slice(),
      stageByKey: {},
      pipelineConfigsByKey: {},
      manualApprovalAfterStage: {},
      slackNotificationByStage: {},
      agentConfig: { enabled: false, frequencyMinutes: 30 },
    };

    // Default all to stage 1
    keys.forEach(function(k) { tab.stageByKey[k] = 1; });

    State.customWorkflowTabs.push(tab);
    saveCustomWorkflowTabs();
    State.selectedWorkflowKeys = {};
    if (nameInput) nameInput.value = '';

    App.renderCustomTabButtons();
    App.switchTab(tab.id);
    App.showToast('Created tab: ' + name, 'success');
  },

  addToExistingCustomTab: function() {
    var keys = Object.keys(State.selectedWorkflowKeys);
    if (keys.length === 0 || State.customWorkflowTabs.length === 0) return;

    // Add to the last tab for simplicity (could prompt for which tab)
    var tab = State.customWorkflowTabs[State.customWorkflowTabs.length - 1];
    keys.forEach(function(k) {
      if (tab.workflowKeys.indexOf(k) < 0) {
        tab.workflowKeys.push(k);
        tab.stageByKey[k] = 1;
      }
    });

    saveCustomWorkflowTabs();
    State.selectedWorkflowKeys = {};
    App.renderCustomTabButtons();
    App.showToast('Added ' + keys.length + ' workflow(s) to ' + tab.name, 'success');
    renderWorkflows();
  },

  renderCustomTabButtons: function() {
    var container = document.getElementById('custom-tab-buttons');
    if (!container) return;

    if (!State.customWorkflowTabs || State.customWorkflowTabs.length === 0) {
      container.innerHTML = '';
      return;
    }

    var html = '';
    State.customWorkflowTabs.forEach(function(tab) {
      var isActive = State.activeTab === tab.id;
      html += '<button class="tab-btn custom-tab-btn' + (isActive ? ' active' : '') + '" data-tab="' + esc(tab.id) + '" onclick="App.switchTab(\'' + esc(tab.id) + '\')" title="' + esc(tab.name) + ' (drag to reorder)" draggable="true" ondragstart="App.onCustomTabDragStart(event, \'' + esc(tab.id) + '\')" ondragover="App.onCustomTabDragOver(event, \'' + esc(tab.id) + '\')" ondrop="App.onCustomTabDrop(event, \'' + esc(tab.id) + '\')" ondragend="App.onCustomTabDragEnd()">';
      html += esc(tab.name);
      html += '</button>';
    });
    container.innerHTML = html;
  },

  onCustomTabDragStart: function(event, tabId) {
    State.draggingCustomTabId = tabId;
    event.dataTransfer.effectAllowed = 'move';
  },

  onCustomTabDragOver: function(event, tabId) {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  },

  onCustomTabDrop: function(event, targetTabId) {
    event.preventDefault();
    var srcId = State.draggingCustomTabId;
    if (!srcId || srcId === targetTabId) return;

    var tabs = State.customWorkflowTabs;
    var srcIdx = tabs.findIndex(function(t) { return t.id === srcId; });
    var tgtIdx = tabs.findIndex(function(t) { return t.id === targetTabId; });
    if (srcIdx < 0 || tgtIdx < 0) return;

    var item = tabs.splice(srcIdx, 1)[0];
    tabs.splice(tgtIdx, 0, item);
    saveCustomWorkflowTabs();
    App.renderCustomTabButtons();
  },

  onCustomTabDragEnd: function() {
    State.draggingCustomTabId = null;
  },

  deleteCustomTab: function(tabId) {
    State.customWorkflowTabs = State.customWorkflowTabs.filter(function(t) { return t.id !== tabId; });
    delete State.customTabRuntime[tabId];
    saveCustomWorkflowTabs();
    App.renderCustomTabButtons();
    if (State.activeTab === tabId) {
      App.switchTab('workflows');
    }
    App.showToast('Tab deleted', 'info');
  },

  renameCustomTab: function(tabId) {
    var input = document.getElementById('custom-tab-rename-input');
    var newName = (input && input.value || '').trim();
    if (!newName) return;
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === tabId; });
    if (tab) {
      tab.name = newName;
      saveCustomWorkflowTabs();
      App.renderCustomTabButtons();
      App.renderCustomWorkflowTab();
    }
  },

  setWorkflowStage: function(tabId, wfKey, stage) {
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === tabId; });
    if (!tab) return;
    tab.stageByKey[wfKey] = Math.max(1, Number(stage) || 1);
    saveCustomWorkflowTabs();
    App.renderCustomWorkflowTab();
  },

  removeWorkflowFromTab: function(tabId, wfKey) {
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === tabId; });
    if (!tab) return;
    tab.workflowKeys = tab.workflowKeys.filter(function(k) { return k !== wfKey; });
    delete tab.stageByKey[wfKey];
    delete tab.pipelineConfigsByKey[wfKey];
    saveCustomWorkflowTabs();
    App.renderCustomWorkflowTab();
  },

  moveCustomTabLeft: function(tabId) {
    var idx = State.customWorkflowTabs.findIndex(function(t) { return t.id === tabId; });
    if (idx <= 0) return;
    var tmp = State.customWorkflowTabs[idx - 1];
    State.customWorkflowTabs[idx - 1] = State.customWorkflowTabs[idx];
    State.customWorkflowTabs[idx] = tmp;
    saveCustomWorkflowTabs();
    App.renderCustomTabButtons();
    App.renderCustomWorkflowTab();
  },

  moveCustomTabRight: function(tabId) {
    var idx = State.customWorkflowTabs.findIndex(function(t) { return t.id === tabId; });
    if (idx < 0 || idx >= State.customWorkflowTabs.length - 1) return;
    var tmp = State.customWorkflowTabs[idx + 1];
    State.customWorkflowTabs[idx + 1] = State.customWorkflowTabs[idx];
    State.customWorkflowTabs[idx] = tmp;
    saveCustomWorkflowTabs();
    App.renderCustomTabButtons();
    App.renderCustomWorkflowTab();
  },

  renderCustomWorkflowTab: function() {
    var container = document.getElementById('custom-workflow-tab-content');
    if (!container) return;

    var tabId = State.activeTab;
    if (!tabId || tabId.indexOf('custom-workflow-') !== 0) return;

    var tab = State.customWorkflowTabs.find(function(t) { return t.id === tabId; });
    if (!tab) {
      container.innerHTML = '<div class="empty-state"><div class="empty-state-title">Tab not found</div></div>';
      return;
    }

    var tabIndex = State.customWorkflowTabs.findIndex(function(t) { return t.id === tab.id; });
    var canMoveLeft = tabIndex > 0;
    var canMoveRight = tabIndex >= 0 && tabIndex < State.customWorkflowTabs.length - 1;

    // Resolve workflow entries
    var entries = [];
    (tab.workflowKeys || []).forEach(function(key) {
      var parsed = parseWorkflowKey(key);
      if (!parsed) return;
      var repoState = State.repoStates.find(function(rs) { return rs.repo === parsed.repo; });
      var wf = repoState && repoState.workflows
        ? repoState.workflows.find(function(w) { return w.id === parsed.workflowId; })
        : null;
      entries.push({
        key: key,
        repo: parsed.repo,
        workflowId: parsed.workflowId,
        name: wf ? wf.name : (parsed.name || 'Unknown workflow'),
        stage: tab.stageByKey[key] || 1,
        config: tab.pipelineConfigsByKey[key] || { branch: '', inputsByName: {} },
      });
    });

    var runtime = State.customTabRuntime[tab.id] || {
      running: false,
      stageLabel: '',
      lastMessage: '',
      results: [],
      awaitingManual: false,
      awaitingStageNo: null,
      nextStageNo: null,
      agentSilenced: false,
    };
    State.customTabRuntime[tab.id] = runtime;

    var html = '';

    // Header
    html += '<div class="custom-workflow-tab-header">';
    html += '<div>';
    html += '<div class="custom-workflow-tab-title">' + esc(tab.name) + '</div>';
    html += '<div class="custom-workflow-tab-subtitle">' + entries.length + ' workflow' + (entries.length === 1 ? '' : 's') + ' in this tab. Same stage runs in parallel.</div>';
    html += '</div>';
    html += '<div class="custom-workflow-tab-actions">';
    html += '<input type="text" class="search-input" id="custom-tab-rename-input" value="' + esc(tab.name) + '">';
    html += '<button class="custom-workflow-tab-btn" onclick="App.renameCustomTab(\'' + esc(tab.id) + '\')">Rename</button>';

    var planStartLabel = runtime.running ? 'Running...' : 'Run Plan';
    var planStartAction = 'App.runActiveWorkflowTabPlan(\'' + esc(tab.id) + '\')';
    html += '<button class="custom-workflow-tab-btn run-plan" ' + (runtime.running ? 'disabled' : '') + ' onclick="' + planStartAction + '">' + planStartLabel + '</button>';

    if (canMoveLeft) html += '<button class="custom-workflow-tab-btn" onclick="App.moveCustomTabLeft(\'' + esc(tab.id) + '\')">&larr;</button>';
    if (canMoveRight) html += '<button class="custom-workflow-tab-btn" onclick="App.moveCustomTabRight(\'' + esc(tab.id) + '\')">&rarr;</button>';

    html += '<button class="custom-workflow-tab-btn" onclick="App.exportCustomTab(\'' + esc(tab.id) + '\')">Export</button>';
    html += '<button class="custom-workflow-tab-btn delete" onclick="if(confirm(\'Delete this tab?\')) App.deleteCustomTab(\'' + esc(tab.id) + '\')">Delete</button>';
    html += '</div>';
    html += '</div>';

    // Status banner
    if (runtime.lastMessage) {
      html += '<div class="custom-plan-status">' + esc(runtime.lastMessage) + '</div>';
    }

    // Layout: main + variables sidebar
    html += '<div class="custom-workflow-layout">';
    html += '<div class="custom-workflow-main">';

    // Build stage groups
    ensureTabSlackNotifications(tab);
    ensureTabManualApprovals(tab);
    var stagesMap = {};
    entries.forEach(function(e) {
      var stageNo = e.stage || 1;
      if (!stagesMap[stageNo]) stagesMap[stageNo] = [];
      stagesMap[stageNo].push(e);
    });
    var stageNums = Object.keys(stagesMap).map(Number).sort(function(a, b) { return a - b; });
    var stageGroups = stageNums.map(function(sn) { return { stageNo: sn, items: stagesMap[sn] }; });
    var firstStageNo = stageNums.length > 0 ? stageNums[0] : null;
    var lastStageNo = stageNums.length > 0 ? stageNums[stageNums.length - 1] : null;

    if (entries.length === 0) {
      html += '<div class="empty-state">';
      html += '<div class="empty-state-icon">&#9881;</div>';
      html += '<div class="empty-state-title">No matching workflows available</div>';
      html += '<div class="empty-state-text">The selected workflows may have been removed from monitored repos.</div>';
      html += '</div>';
    } else {
      html += '<div class="workflow-table-wrap"><table class="workflow-table">';
      html += '<thead><tr><th>Stage</th><th>Workflow</th><th>Repo</th><th></th></tr></thead><tbody>';

      // Before first stage boundary
      if (firstStageNo != null) {
        var beforeBoundaryKey = 'boundary-before:' + firstStageNo;
        var beforeBoundaryOpen = State.openInterStageConfigKey === beforeBoundaryKey;
        var beforeFirstTemplateId = tab.slackNotificationByStage ? (tab.slackNotificationByStage['before:' + firstStageNo] || '') : '';
        var beforeFirstTemplateName = getSlackTemplateNameById(beforeFirstTemplateId);

        html += '<tr class="inter-stage-row"><td colspan="4">';
        html += '<div class="inter-stage-wrap">';
        html += '<div class="inter-stage-label">Before stage ' + firstStageNo + '</div>';
        html += '<div class="inter-stage-tags">';
        if (beforeFirstTemplateName) {
          html += '<span class="inter-stage-tag slack">Slack Before ' + firstStageNo + ': ' + esc(beforeFirstTemplateName) + '<button class="inter-stage-tag-remove" onclick="App.removeSlackNotificationStep(\'before\',' + firstStageNo + ', event)" title="Remove Slack step">&times;</button></span>';
        } else {
          html += '<span class="pipeline-input-note">No pre-stage actions</span>';
        }
        html += '</div>';
        html += '<button class="inter-stage-add-btn" onclick="App.toggleBoundaryStageConfig(\'before\',' + firstStageNo + ')">&#43; Add Step</button>';
        html += '</div>';
        if (beforeBoundaryOpen) {
          html += '<div class="inter-stage-config"><div class="inter-stage-config-grid"><div>';
          html += '<div class="pipeline-input-note">Slack before stage ' + firstStageNo + '</div>';
          html += '<select class="search-input pipeline-config-branch" onchange="App.setSlackNotificationTemplate(\'before\',' + firstStageNo + ', this.value)">';
          html += renderSlackTemplateOptions(beforeFirstTemplateId, 'No Slack step');
          html += '</select></div></div></div>';
        }
        html += '</td></tr>';
      }

      // Stage rows with inter-stage gaps
      stageGroups.forEach(function(group, groupIdx) {
        group.items.forEach(function(item) {
          html += '<tr class="wf-child-row">';
          html += '<td><input class="stage-input" type="number" min="1" value="' + item.stage + '" onchange="App.setWorkflowStage(\'' + esc(tab.id) + '\', \'' + escJs(item.key) + '\', this.value)"></td>';
          html += '<td class="wf-name">' + esc(item.name) + '</td>';
          html += '<td class="wf-repo">' + esc(item.repo) + '</td>';
          html += '<td><button class="custom-workflow-tab-btn delete" style="padding:4px 8px;font-size:11px" onclick="App.removeWorkflowFromTab(\'' + esc(tab.id) + '\', \'' + escJs(item.key) + '\')">Remove</button></td>';
          html += '</tr>';
        });

        // Inter-stage row between groups
        if (groupIdx < stageGroups.length - 1) {
          var stageNo = group.stageNo;
          var nextStageNo = stageGroups[groupIdx + 1].stageNo;
          var gapKey = stageNo + '->' + nextStageNo;
          var panelOpen = State.openInterStageConfigKey === gapKey;

          var hasManual = !!(tab.manualApprovalAfterStage && tab.manualApprovalAfterStage[String(stageNo)]);
          var afterTemplateId = tab.slackNotificationByStage ? (tab.slackNotificationByStage['after:' + stageNo] || '') : '';
          var beforeTemplateId = tab.slackNotificationByStage ? (tab.slackNotificationByStage['before:' + nextStageNo] || '') : '';
          var afterTemplateName = getSlackTemplateNameById(afterTemplateId);
          var beforeTemplateName = getSlackTemplateNameById(beforeTemplateId);
          var hasAny = hasManual || !!afterTemplateName || !!beforeTemplateName;

          html += '<tr class="inter-stage-row"><td colspan="4">';
          html += '<div class="inter-stage-wrap">';
          html += '<div class="inter-stage-label">Between stage ' + stageNo + ' and stage ' + nextStageNo + '</div>';
          html += '<div class="inter-stage-tags">';
          if (hasManual) {
            html += '<span class="inter-stage-tag manual">Manual Approval<button class="inter-stage-tag-remove" onclick="App.removeManualInterventionAfterStage(' + stageNo + ', event)" title="Remove manual step">&times;</button></span>';
          }
          if (afterTemplateName) {
            html += '<span class="inter-stage-tag slack">Slack After ' + stageNo + ': ' + esc(afterTemplateName) + '<button class="inter-stage-tag-remove" onclick="App.removeSlackNotificationStep(\'after\',' + stageNo + ', event)" title="Remove Slack step">&times;</button></span>';
          }
          if (beforeTemplateName) {
            html += '<span class="inter-stage-tag slack">Slack Before ' + nextStageNo + ': ' + esc(beforeTemplateName) + '<button class="inter-stage-tag-remove" onclick="App.removeSlackNotificationStep(\'before\',' + nextStageNo + ', event)" title="Remove Slack step">&times;</button></span>';
          }
          if (!hasAny) {
            html += '<span class="pipeline-input-note">No inter-stage actions</span>';
          }
          html += '</div>';
          html += '<button class="inter-stage-add-btn" onclick="App.toggleInterStageConfig(' + stageNo + ', ' + nextStageNo + ')">&#43; Add Step</button>';
          html += '</div>';

          if (panelOpen) {
            html += '<div class="inter-stage-config">';
            html += '<div style="display:flex;align-items:center;justify-content:space-between;gap:8px;flex-wrap:wrap;">';
            html += '<span class="pipeline-input-note" style="margin:0;">Manual intervention gate</span>';
            html += '<button class="workflow-add-to-tab-btn" onclick="App.toggleManualInterventionAfterStage(' + stageNo + ')">' + (hasManual ? 'Remove Manual Step' : 'Add Manual Step') + '</button>';
            html += '</div>';
            html += '<div class="inter-stage-config-grid"><div>';
            html += '<div class="pipeline-input-note">Slack after stage ' + stageNo + '</div>';
            html += '<select class="search-input pipeline-config-branch" onchange="App.setSlackNotificationTemplate(\'after\',' + stageNo + ', this.value)">';
            html += renderSlackTemplateOptions(afterTemplateId, 'No Slack step');
            html += '</select></div><div>';
            html += '<div class="pipeline-input-note">Slack before stage ' + nextStageNo + '</div>';
            html += '<select class="search-input pipeline-config-branch" onchange="App.setSlackNotificationTemplate(\'before\',' + nextStageNo + ', this.value)">';
            html += renderSlackTemplateOptions(beforeTemplateId, 'No Slack step');
            html += '</select></div></div></div>';
          }
          html += '</td></tr>';
        }
      });

      // After last stage boundary
      if (lastStageNo != null) {
        var afterBoundaryKey = 'boundary-after:' + lastStageNo;
        var afterBoundaryOpen = State.openInterStageConfigKey === afterBoundaryKey;
        var afterLastTemplateId = tab.slackNotificationByStage ? (tab.slackNotificationByStage['after:' + lastStageNo] || '') : '';
        var afterLastTemplateName = getSlackTemplateNameById(afterLastTemplateId);

        html += '<tr class="inter-stage-row"><td colspan="4">';
        html += '<div class="inter-stage-wrap">';
        html += '<div class="inter-stage-label">After stage ' + lastStageNo + '</div>';
        html += '<div class="inter-stage-tags">';
        if (afterLastTemplateName) {
          html += '<span class="inter-stage-tag slack">Slack After ' + lastStageNo + ': ' + esc(afterLastTemplateName) + '<button class="inter-stage-tag-remove" onclick="App.removeSlackNotificationStep(\'after\',' + lastStageNo + ', event)" title="Remove Slack step">&times;</button></span>';
        } else {
          html += '<span class="pipeline-input-note">No post-stage actions</span>';
        }
        html += '</div>';
        html += '<button class="inter-stage-add-btn" onclick="App.toggleBoundaryStageConfig(\'after\',' + lastStageNo + ')">&#43; Add Step</button>';
        html += '</div>';
        if (afterBoundaryOpen) {
          html += '<div class="inter-stage-config"><div class="inter-stage-config-grid"><div>';
          html += '<div class="pipeline-input-note">Slack after stage ' + lastStageNo + '</div>';
          html += '<select class="search-input pipeline-config-branch" onchange="App.setSlackNotificationTemplate(\'after\',' + lastStageNo + ', this.value)">';
          html += renderSlackTemplateOptions(afterLastTemplateId, 'No Slack step');
          html += '</select></div></div></div>';
        }
        html += '</td></tr>';
      }

      html += '</tbody></table>';
    }

    // Results
    if (runtime.results && runtime.results.length > 0) {
      html += '<div style="margin-top:12px">';
      html += '<div class="custom-workflow-tab-subtitle" style="margin-bottom:8px">Last Run Results</div>';
      runtime.results.forEach(function(r) {
        var color = r.success ? 'var(--status-success)' : 'var(--status-failure)';
        html += '<div style="font-size:var(--font-size-xs);color:' + color + ';margin-bottom:4px">';
        html += (r.success ? '&#10003;' : '&#10007;') + ' ' + esc(r.name) + ' (' + esc(r.repo) + ')';
        if (r.error) html += ' &mdash; ' + esc(r.error);
        html += '</div>';
      });
      html += '</div>';
    }

    html += '</div>'; // .custom-workflow-main

    // Variables sidebar
    html += '<div class="variables-sidebar">';
    html += '<div class="variables-sidebar-title">Pipeline Variables</div>';
    html += '<div class="variables-sidebar-subtitle">Configure branch and inputs per workflow.</div>';

    // Slack Template Library
    html += '<div class="pipeline-config-card">';
    html += '<div class="pipeline-config-title">Inter-Stage Steps</div>';
    html += '<div class="pipeline-input-note">Use &#43; Add Step between stages (and before stage 1 / after last stage) to add manual approvals and Slack notifications.</div>';
    html += '<div class="pipeline-config-title" style="margin-top:8px;">Slack Template Library</div>';
    html += '<div class="pipeline-input-note">Template variables: {{tab_name}}, {{stage_no}}, {{stage_position}}, {{status}}, {{total_stages}}, {{pipeline_count}}, {{timestamp}}.</div>';
    html += '<div class="pipeline-config-label">Slack Webhook URLs</div>';
    html += '<textarea class="search-input pipeline-config-branch" rows="3" placeholder="One URL per line (or comma-separated)&#10;https://hooks.slack.com/services/...&#10;https://hooks.slack.com/services/..." onchange="App.setSlackWebhookUrls(this.value)">' + esc(slackWebhookUrlsText(State.slackConfig && State.slackConfig.webhookUrls)) + '</textarea>';
    html += '<div class="pipeline-config-label">Template Editor</div>';
    html += '<select class="search-input pipeline-config-branch" id="slack-template-editor-select" onchange="App.loadSlackTemplateIntoEditor(this.value)">';
    html += renderSlackTemplateOptions('', 'New template...');
    html += '</select>';
    html += '<input class="search-input pipeline-config-branch" id="slack-template-name-input" type="text" placeholder="Template name">';
    html += '<textarea class="search-input pipeline-config-branch" id="slack-template-body-input" rows="4" placeholder="Deploy started for {{tab_name}} stage {{stage_no}}"></textarea>';
    html += '<div style="display:flex;gap:6px;">';
    html += '<button class="workflow-add-to-tab-btn" onclick="App.startNewSlackTemplate()">New</button>';
    html += '<button class="workflow-add-to-tab-btn" onclick="App.saveSlackTemplateFromEditor()">Save / Update</button>';
    html += '<button class="workflow-clear-selection-btn" onclick="App.deleteSlackTemplateFromEditor()">Delete Selected</button>';
    html += '</div>';
    html += '<div class="template-library-list">';
    if (!State.slackTemplates || State.slackTemplates.length === 0) {
      html += '<div class="pipeline-input-note">No templates saved yet.</div>';
    } else {
      State.slackTemplates.forEach(function(tpl) {
        var tplIdJson = JSON.stringify(tpl.id);
        html += '<div class="template-library-row">';
        html += '<div class="template-library-name" title="' + esc(tpl.name) + '">' + esc(tpl.name) + '</div>';
        html += '<div class="template-library-actions">';
        html += '<button class="workflow-add-to-tab-btn" onclick=\'App.selectSlackTemplateForEditing(' + tplIdJson + ')\'>Edit</button>';
        html += '<button class="workflow-clear-selection-btn" onclick=\'App.deleteSlackTemplateById(' + tplIdJson + ')\'>Delete</button>';
        html += '</div>';
        html += '</div>';
      });
    }
    html += '</div>';
    html += '</div>';

    // Per-workflow pipeline configs with workflow_dispatch inputs
    entries.forEach(function(entry) {
      App.ensureWorkflowInputsLoaded(entry.key, entry.repo, entry.workflowId);
      var cfg = entry.config;
      var inputMeta = State.workflowInputsByKey[entry.key];
      html += '<div class="pipeline-config-card">';
      html += '<div class="pipeline-config-title">' + esc(entry.name) + '</div>';
      html += '<div class="pipeline-config-repo">' + esc(entry.repo) + ' - stage ' + entry.stage + '</div>';
      html += '<div class="pipeline-config-label">Branch (optional; blank uses default)</div>';
      html += '<input class="search-input pipeline-config-branch" type="text" value="' + esc(cfg.branch || '') + '" placeholder="main" onchange="App.setPipelineBranch(\'' + esc(entry.key) + '\', this.value)">';

      html += '<div class="pipeline-config-label">Workflow Inputs</div>';
      if (!inputMeta || inputMeta.loading) {
        html += '<div class="pipeline-input-note">Loading workflow inputs...</div>';
      } else if (inputMeta.error) {
        html += '<div class="pipeline-input-note">Could not load inputs: ' + esc(inputMeta.error) + '</div>';
      } else if (!inputMeta.has_dispatch) {
        html += '<div class="pipeline-input-note">This workflow has no workflow_dispatch trigger.</div>';
      } else if (!inputMeta.inputs || inputMeta.inputs.length === 0) {
        html += '<div class="pipeline-input-note">No defined inputs for this workflow.</div>';
      } else {
        inputMeta.inputs.forEach(function(inp) {
          var inputType = inp.type || inp.input_type || 'string';
          var saved = (cfg.inputsByName && Object.prototype.hasOwnProperty.call(cfg.inputsByName, inp.name))
            ? cfg.inputsByName[inp.name] : null;
          var baseValue = saved != null ? String(saved) : (inp.default != null ? String(inp.default) : '');
          var inputNameJson = JSON.stringify(inp.name);
          var keyJson = JSON.stringify(entry.key);

          html += '<div class="pipeline-config-label">' + esc(inp.name) + (inp.required ? ' *' : '') + '</div>';
          if (inp.description) {
            html += '<div class="pipeline-input-note">' + esc(inp.description) + '</div>';
          }

          if (inputType === 'boolean') {
            var checked = (baseValue === 'true') ? 'checked' : '';
            html += '<label class="pipeline-input-note" style="display:flex;align-items:center;gap:8px;margin-bottom:6px;">';
            html += '<input type="checkbox" ' + checked + ' onchange="App.setPipelineWorkflowInput(' + keyJson + ', ' + inputNameJson + ', \'boolean\', this)">';
            html += esc(inp.name);
            html += '</label>';
          } else if (inputType === 'choice' && Array.isArray(inp.options) && inp.options.length > 0) {
            html += '<select class="search-input pipeline-config-branch" onchange="App.setPipelineWorkflowInput(' + keyJson + ', ' + inputNameJson + ', \'choice\', this)">';
            inp.options.forEach(function(opt) {
              var sel = (String(opt) === baseValue) ? ' selected' : '';
              html += '<option value="' + esc(opt) + '"' + sel + '>' + esc(opt) + '</option>';
            });
            html += '</select>';
          } else if (inputType === 'environment') {
            html += '<input class="search-input pipeline-config-branch" type="text" value="' + esc(baseValue) + '" placeholder="environment name" onchange="App.setPipelineWorkflowInput(' + keyJson + ', ' + inputNameJson + ', \'environment\', this)">';
          } else {
            html += '<input class="search-input pipeline-config-branch" type="text" value="' + esc(baseValue) + '" placeholder="' + esc(inp.default || '') + '" onchange="App.setPipelineWorkflowInput(' + keyJson + ', ' + inputNameJson + ', \'string\', this)">';
          }
        });
      }

      html += '</div>';
    });

    html += '</div>'; // .variables-sidebar
    html += '</div>'; // .custom-workflow-layout

    container.innerHTML = html;
  },

  /* Pipeline variable config helpers */
  setPipelineBranch: function(workflowKeyValue, value) {
    if (!State.activeTab || State.activeTab.indexOf('custom-workflow-') !== 0) return;
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === State.activeTab; });
    if (!tab) return;
    if (!tab.pipelineConfigsByKey) tab.pipelineConfigsByKey = {};
    if (!tab.pipelineConfigsByKey[workflowKeyValue]) {
      tab.pipelineConfigsByKey[workflowKeyValue] = { branch: '', inputsByName: {} };
    }
    tab.pipelineConfigsByKey[workflowKeyValue].branch = value != null ? String(value) : '';
    saveCustomWorkflowTabs();
  },

  ensureWorkflowInputsLoaded: async function(workflowKeyValue, repo, workflowId) {
    await ensureWorkflowInputsCached(workflowKeyValue, repo, workflowId, false);
  },

  setPipelineWorkflowInput: function(workflowKeyValue, inputName, inputType, el) {
    if (!State.activeTab || State.activeTab.indexOf('custom-workflow-') !== 0) return;
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === State.activeTab; });
    if (!tab) return;
    if (!tab.pipelineConfigsByKey) tab.pipelineConfigsByKey = {};
    if (!tab.pipelineConfigsByKey[workflowKeyValue]) {
      tab.pipelineConfigsByKey[workflowKeyValue] = { branch: '', inputsByName: {} };
    }
    var cfg = tab.pipelineConfigsByKey[workflowKeyValue];
    if (!cfg.inputsByName || typeof cfg.inputsByName !== 'object') {
      cfg.inputsByName = {};
    }
    var value = '';
    if (inputType === 'boolean') {
      value = el && el.checked ? 'true' : 'false';
    } else {
      value = el && el.value != null ? String(el.value) : '';
    }
    if (value === '' || value == null) {
      delete cfg.inputsByName[inputName];
    } else {
      cfg.inputsByName[inputName] = value;
    }
    saveCustomWorkflowTabs();
    if (inputType === 'boolean') {
      App.renderCustomWorkflowTab();
    }
  },

  /* Inter-stage gates */
  setManualApprovalAfterStage: function(stageNo, checked) {
    if (!State.activeTab || State.activeTab.indexOf('custom-workflow-') !== 0) return;
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === State.activeTab; });
    if (!tab) return;
    ensureTabManualApprovals(tab);
    var normalizedStage = Math.floor(Number(stageNo));
    if (!normalizedStage || normalizedStage < 1) return;
    var stages = getTabStageNumbers(tab);
    if (stages.length < 2) return;
    var maxStage = stages[stages.length - 1];
    if (normalizedStage >= maxStage) {
      delete tab.manualApprovalAfterStage[String(normalizedStage)];
      saveCustomWorkflowTabs();
      App.renderCustomWorkflowTab();
      return;
    }
    if (checked) {
      tab.manualApprovalAfterStage[String(normalizedStage)] = true;
    } else {
      delete tab.manualApprovalAfterStage[String(normalizedStage)];
    }
    saveCustomWorkflowTabs();
    App.renderCustomWorkflowTab();
  },

  toggleInterStageConfig: function(stageNo, nextStageNo) {
    if (!State.activeTab || State.activeTab.indexOf('custom-workflow-') !== 0) return;
    var key = String(stageNo) + '->' + String(nextStageNo);
    if (State.openInterStageConfigKey === key) {
      State.openInterStageConfigKey = '';
    } else {
      State.openInterStageConfigKey = key;
    }
    App.renderCustomWorkflowTab();
  },

  toggleBoundaryStageConfig: function(position, stageNo) {
    if (!State.activeTab || State.activeTab.indexOf('custom-workflow-') !== 0) return;
    var normalizedPosition = String(position || '').trim();
    if (normalizedPosition !== 'before' && normalizedPosition !== 'after') return;
    var normalizedStage = Math.floor(Number(stageNo));
    if (!normalizedStage || normalizedStage < 1) return;
    var key = 'boundary-' + normalizedPosition + ':' + String(normalizedStage);
    if (State.openInterStageConfigKey === key) {
      State.openInterStageConfigKey = '';
    } else {
      State.openInterStageConfigKey = key;
    }
    App.renderCustomWorkflowTab();
  },

  toggleManualInterventionAfterStage: function(stageNo) {
    if (!State.activeTab || State.activeTab.indexOf('custom-workflow-') !== 0) return;
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === State.activeTab; });
    if (!tab) return;
    ensureTabManualApprovals(tab);
    var key = String(Math.floor(Number(stageNo)));
    if (!key || key === 'NaN') return;
    var checked = !(tab.manualApprovalAfterStage && tab.manualApprovalAfterStage[key]);
    App.setManualApprovalAfterStage(Number(key), checked);
  },

  removeManualInterventionAfterStage: function(stageNo, event) {
    if (event) event.stopPropagation();
    App.setManualApprovalAfterStage(stageNo, false);
  },

  setSlackNotificationTemplate: function(position, stageNo, templateId) {
    if (!State.activeTab || State.activeTab.indexOf('custom-workflow-') !== 0) return;
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === State.activeTab; });
    if (!tab) return;
    var normalizedPosition = String(position || '').trim();
    if (normalizedPosition !== 'before' && normalizedPosition !== 'after') return;
    var normalizedStage = Math.floor(Number(stageNo));
    if (!normalizedStage || normalizedStage < 1) return;
    ensureTabSlackNotifications(tab);
    var key = normalizedPosition + ':' + normalizedStage;
    var selectedTemplateId = templateId != null ? String(templateId).trim() : '';
    if (!selectedTemplateId) {
      delete tab.slackNotificationByStage[key];
    } else {
      tab.slackNotificationByStage[key] = selectedTemplateId;
    }
    saveCustomWorkflowTabs();
    App.renderCustomWorkflowTab();
  },

  removeSlackNotificationStep: function(position, stageNo, event) {
    if (event) event.stopPropagation();
    App.setSlackNotificationTemplate(position, stageNo, '');
  },

  setSlackWebhookUrls: function(value) {
    if (!State.slackConfig || typeof State.slackConfig !== 'object') {
      State.slackConfig = { webhookUrls: [] };
    }
    State.slackConfig.webhookUrls = normalizeSlackWebhookUrls(value);
    saveSlackConfig();
  },

  /* Slack template editor */
  startNewSlackTemplate: function() {
    var select = document.getElementById('slack-template-editor-select');
    if (select) select.value = '';
    App.loadSlackTemplateIntoEditor('');
  },

  selectSlackTemplateForEditing: function(templateId) {
    var id = templateId != null ? String(templateId).trim() : '';
    var select = document.getElementById('slack-template-editor-select');
    if (select) select.value = id;
    App.loadSlackTemplateIntoEditor(id);
  },

  loadSlackTemplateIntoEditor: function(templateId) {
    var nameInput = document.getElementById('slack-template-name-input');
    var bodyInput = document.getElementById('slack-template-body-input');
    if (!nameInput || !bodyInput) return;
    var id = templateId != null ? String(templateId).trim() : '';
    if (!id) { nameInput.value = ''; bodyInput.value = ''; return; }
    var template = (State.slackTemplates || []).find(function(t) { return t.id === id; });
    if (!template) { nameInput.value = ''; bodyInput.value = ''; return; }
    nameInput.value = template.name || '';
    bodyInput.value = template.body || '';
  },

  saveSlackTemplateFromEditor: function() {
    var select = document.getElementById('slack-template-editor-select');
    var nameInput = document.getElementById('slack-template-name-input');
    var bodyInput = document.getElementById('slack-template-body-input');
    if (!nameInput || !bodyInput) return;
    var selectedId = select ? String(select.value || '').trim() : '';
    var name = String(nameInput.value || '').trim();
    var body = String(bodyInput.value || '');
    if (!name) { App.showToast('Template name is required', 'error'); return; }
    if (!body.trim()) { App.showToast('Template body is required', 'error'); return; }
    if (!Array.isArray(State.slackTemplates)) { State.slackTemplates = []; }
    var nextId = selectedId;
    var existing = selectedId ? State.slackTemplates.find(function(t) { return t.id === selectedId; }) : null;
    var isUpdate = !!existing;
    if (existing) { existing.name = name; existing.body = body; }
    else {
      nextId = 'slack-template-' + Date.now() + '-' + Math.floor(Math.random() * 100000);
      State.slackTemplates.push({ id: nextId, name: name, body: body });
    }
    saveSlackTemplates();
    App.renderCustomWorkflowTab();
    setTimeout(function() {
      var selectEl = document.getElementById('slack-template-editor-select');
      if (!selectEl) return;
      selectEl.value = nextId;
      App.loadSlackTemplateIntoEditor(nextId);
    }, 0);
    App.showToast(isUpdate ? 'Slack template updated' : 'Slack template saved', 'success');
  },

  deleteSlackTemplateFromEditor: function() {
    var select = document.getElementById('slack-template-editor-select');
    var selectedId = select ? String(select.value || '').trim() : '';
    if (!selectedId) { App.showToast('Select a template to delete', 'error'); return; }
    App.deleteSlackTemplateById(selectedId);
  },

  deleteSlackTemplateById: function(templateId) {
    var selectedId = templateId != null ? String(templateId).trim() : '';
    if (!selectedId) return;
    var select = document.getElementById('slack-template-editor-select');
    var currentlySelected = select ? String(select.value || '').trim() : '';
    var beforeCount = (State.slackTemplates || []).length;
    State.slackTemplates = (State.slackTemplates || []).filter(function(t) { return t.id !== selectedId; });
    if (State.slackTemplates.length === beforeCount) { App.showToast('Template not found', 'error'); return; }
    (State.customWorkflowTabs || []).forEach(function(tab) {
      ensureTabSlackNotifications(tab);
      Object.keys(tab.slackNotificationByStage || {}).forEach(function(key) {
        if (tab.slackNotificationByStage[key] === selectedId) {
          delete tab.slackNotificationByStage[key];
        }
      });
    });
    saveSlackTemplates();
    saveCustomWorkflowTabs();
    App.renderCustomWorkflowTab();
    if (currentlySelected === selectedId) {
      setTimeout(function() { App.startNewSlackTemplate(); }, 0);
    }
    App.showToast('Slack template deleted', 'info');
  },

  _sendSlackWebhookMessage: async function(webhookUrl, text) {
    await apiFetch('/api/integrations/slack/message', {
      method: 'POST',
      body: JSON.stringify({ webhook_url: webhookUrl, text: text })
    });
  },

  /* ═══════════════════════════════════════════════════════════════
     PIPELINE PLAN EXECUTION
     ═══════════════════════════════════════════════════════════════ */

  _runSlackNotificationStep: async function(tab, stageNo, stagePosition, stageIndex, totalStages, runtime) {
    ensureTabSlackNotifications(tab);
    var key = String(stagePosition) + ':' + String(stageNo);
    var templateId = tab.slackNotificationByStage ? tab.slackNotificationByStage[key] : '';
    if (!templateId) return;

    var template = (State.slackTemplates || []).find(function(t) { return t.id === templateId; });
    if (!template) {
      throw new Error('Slack template not found for ' + key);
    }

    var webhookUrls = getConfiguredSlackWebhookUrls();
    if (webhookUrls.length === 0) {
      throw new Error('Slack webhook URL(s) are not configured');
    }

    var stageItems = [];
    (tab.workflowKeys || []).forEach(function(wk) {
      if (Number(tab.stageByKey[wk]) === stageNo) stageItems.push(wk);
    });

    var message = renderSlackTemplateBody(template.body, {
      tab_name: tab.name || 'Workflow Plan',
      stage_no: stageNo,
      stage_position: stagePosition,
      status: stagePosition === 'before' ? 'before_stage' : 'after_stage',
      total_stages: totalStages,
      stage_index: stageIndex + 1,
      pipeline_count: stageItems.length,
      timestamp: new Date().toISOString()
    });

    if (!message.trim()) {
      throw new Error('Slack template rendered an empty message');
    }

    runtime.stageLabel = 'Slack notification';
    runtime.lastMessage = 'Posting "' + template.name + '" ' + stagePosition + ' stage ' + stageNo + ' to ' + webhookUrls.length + ' channel' + (webhookUrls.length === 1 ? '' : 's');
    App.renderCustomWorkflowTab();

    var results = await Promise.allSettled(webhookUrls.map(function(url) {
      return App._sendSlackWebhookMessage(url, message);
    }));
    var failed = results.filter(function(r) { return r.status === 'rejected'; });
    if (failed.length > 0) {
      var firstErr = failed[0] && failed[0].reason && failed[0].reason.message
        ? failed[0].reason.message : 'unknown error';
      throw new Error('Failed to post Slack message to ' + failed.length + ' of ' + webhookUrls.length + ' webhook(s): ' + firstErr);
    }
  },

  runActiveWorkflowTabPlan: async function(tabId) {
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === tabId; });
    if (!tab) return;

    var runtime = State.customTabRuntime[tabId] || {};
    if (runtime.running) return;

    runtime.running = true;
    runtime.results = [];
    runtime.lastMessage = 'Starting plan execution...';
    State.customTabRuntime[tabId] = runtime;
    ensureTabSlackNotifications(tab);
    ensureTabManualApprovals(tab);
    App.renderCustomWorkflowTab();

    // Group entries by stage
    var stages = {};
    (tab.workflowKeys || []).forEach(function(key) {
      var stage = tab.stageByKey[key] || 1;
      if (!stages[stage]) stages[stage] = [];
      var parsed = parseWorkflowKey(key);
      if (parsed) {
        stages[stage].push({
          key: key,
          repo: parsed.repo,
          workflowId: parsed.workflowId,
          config: tab.pipelineConfigsByKey[key] || { branch: '', inputsByName: {} },
          name: parsed.name || key,
        });
      }
    });

    var stageNums = Object.keys(stages).map(Number).sort(function(a, b) { return a - b; });
    var totalStages = stageNums.length;

    for (var si = 0; si < stageNums.length; si++) {
      var stageNo = stageNums[si];
      var stageEntries = stages[stageNo];

      // Slack "before" notification for this stage
      try {
        await App._runSlackNotificationStep(tab, stageNo, 'before', si, totalStages, runtime);
      } catch (err) {
        runtime.lastMessage = 'Slack before-stage notification failed: ' + err.message;
        App.renderCustomWorkflowTab();
      }

      runtime.stageLabel = 'Stage ' + stageNo;
      runtime.lastMessage = 'Running Stage ' + stageNo + ' (' + stageEntries.length + ' workflow' + (stageEntries.length === 1 ? '' : 's') + ')...';
      App.renderCustomWorkflowTab();

      // Dispatch all workflows in this stage in parallel
      var dispatches = stageEntries.map(function(entry) {
        return dispatchWorkflowWithConfig(entry.repo, entry.workflowId, entry.config)
          .then(function(data) {
            runtime.results.push({ name: entry.name || entry.key, repo: entry.repo, success: true });
            return { success: true, entry: entry };
          })
          .catch(function(err) {
            runtime.results.push({ name: entry.name || entry.key, repo: entry.repo, success: false, error: err.message });
            return { success: false, entry: entry, error: err.message };
          });
      });

      await Promise.all(dispatches);

      // Slack "after" notification for this stage
      try {
        await App._runSlackNotificationStep(tab, stageNo, 'after', si, totalStages, runtime);
      } catch (err) {
        runtime.lastMessage = 'Slack after-stage notification failed: ' + err.message;
        App.renderCustomWorkflowTab();
      }

      // Check inter-stage gates (manual approval between stages)
      if (si < stageNums.length - 1) {
        var nextStage = stageNums[si + 1];

        // Manual approval gate
        if (tab.manualApprovalAfterStage && tab.manualApprovalAfterStage[String(stageNo)]) {
          runtime.awaitingManual = true;
          runtime.awaitingStageNo = stageNo;
          runtime.nextStageNo = nextStage;
          runtime.lastMessage = 'Awaiting manual approval before Stage ' + nextStage + '...';
          App.renderCustomWorkflowTab();

          await new Promise(function(resolve) {
            State.customTabGateResolvers[tabId] = resolve;
          });

          runtime.awaitingManual = false;
          delete State.customTabGateResolvers[tabId];
        }
      }
    }

    runtime.running = false;
    runtime.stageLabel = '';
    runtime.lastMessage = 'Plan execution complete. ' + runtime.results.length + ' workflow(s) dispatched.';
    App.renderCustomWorkflowTab();
    App.showToast('Plan execution complete', 'success');
  },

  continueWorkflowTabPlan: function() {
    var tabId = State.activeTab;
    if (!tabId) return;
    var resolver = State.customTabGateResolvers[tabId];
    if (resolver) {
      resolver();
      App.showToast('Approved — continuing plan', 'success');
    }
  },

  exportCustomTab: function(tabId) {
    var tab = State.customWorkflowTabs.find(function(t) { return t.id === tabId; });
    if (!tab) return;
    var json = JSON.stringify(tab, null, 2);
    var blob = new Blob([json], { type: 'application/json' });
    var url = URL.createObjectURL(blob);
    var a = document.createElement('a');
    a.href = url;
    a.download = (tab.name || 'tab') + '.json';
    a.click();
    URL.revokeObjectURL(url);
    App.showToast('Tab exported', 'success');
  },

  /* ═══════════════════════════════════════════════════════════════
     MANUAL REFRESH
     ═══════════════════════════════════════════════════════════════ */

  manualRefresh: async function() {
    var btn = document.getElementById('refresh-btn');
    if (btn.disabled) return;
    btn.disabled = true;
    btn.classList.add('spinning');
    try {
      await apiFetch('/api/refresh', { method: 'POST' });
      App.showToast('Refresh complete', 'success');
    } catch (err) {
      App.showToast('Refresh failed: ' + err.message, 'error');
    } finally {
      btn.disabled = false;
      btn.classList.remove('spinning');
    }
  },

  /** Set and persist theme */
  setTheme: function(themeId) {
    State.theme = themeId;
    if (themeId === 'terminal') {
      document.documentElement.removeAttribute('data-theme');
    } else {
      document.documentElement.setAttribute('data-theme', themeId);
    }
    try { localStorage.setItem('gwd_theme', themeId); } catch(_) {}
    var sel = document.getElementById('theme-select');
    if (sel) sel.value = themeId;
  },

  /* ═══════════════════════════════════════════════════════════════
     TOAST NOTIFICATION SYSTEM
     ═══════════════════════════════════════════════════════════════ */

  showToast: function(message, type) {
    type = type || 'info';
    const container = document.getElementById('toast-container');

    const icons = {
      success: '&#10003;',
      error: '&#10007;',
      info: '&#9432;'
    };

    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;
    toast.innerHTML = '<span class="toast-icon">' + (icons[type] || icons.info) + '</span>' +
      '<span class="toast-message">' + esc(message) + '</span>' +
      '<button class="toast-close" onclick="App.dismissToast(this.parentElement)">&times;</button>';

    container.appendChild(toast);

    // Auto-dismiss after 5 seconds
    setTimeout(function() {
      App.dismissToast(toast);
    }, 5000);
  },

  dismissToast: function(toastEl) {
    if (!toastEl || !toastEl.parentElement) return;
    toastEl.classList.add('toast-out');
    setTimeout(function() {
      if (toastEl.parentElement) toastEl.parentElement.removeChild(toastEl);
    }, 250);
  },

  /* ═══════════════════════════════════════════════════════════════
     SCANLINE TOGGLE
     ═══════════════════════════════════════════════════════════════ */

  toggleScanlines: function() {
    document.getElementById('scanline-overlay').classList.toggle('active');
  },

  /* ═══════════════════════════════════════════════════════════════
     AUTH TOKEN MANAGEMENT
     ═══════════════════════════════════════════════════════════════ */

  submitAuthToken: function() {
    var input = document.getElementById('auth-token-input');
    var token = (input.value || '').trim();
    if (!token) {
      document.getElementById('auth-error').textContent = 'Please enter a token';
      document.getElementById('auth-error').style.display = 'block';
      return;
    }
    setAuthToken(token);
    document.getElementById('auth-modal').style.display = 'none';
    document.getElementById('auth-error').style.display = 'none';
    input.value = '';
    // Reconnect WebSocket with new token
    if (State.ws) {
      State.ws.close();
    }
    wsConnect();
    // Show success
    App.showToast('Token saved — reconnecting...', 'success');
  }
};
