/* ═══════════════════════════════════════════════════════════════
   RENDER FUNCTIONS
   ═══════════════════════════════════════════════════════════════ */

function renderAll() {
  renderRateLimit();
  renderLastPoll();
  renderSidebar();
  renderActiveRuns();
  renderWorkflows();
  renderActions();
  renderHistory();
  renderWhatsHappening();
  renderAnalytics();
  // Re-render active custom tab if visible
  if (State.activeTab && State.activeTab.indexOf('custom-workflow-') === 0) {
    App.renderCustomWorkflowTab();
  }
}

/** Rate Limit Badge */
function renderRateLimit() {
  const badge = document.getElementById('rate-limit-badge');
  const text = document.getElementById('rate-limit-text');
  const rl = State.rateLimit;

  if (!rl || !rl.limit) {
    text.textContent = '--/--';
    badge.className = 'rate-limit-badge';
    return;
  }

  text.textContent = rl.remaining + '/' + rl.limit;
  const pct = rl.remaining / rl.limit;
  badge.className = 'rate-limit-badge';
  if (pct > 0.8) {
    badge.classList.add('rate-green');
  } else if (pct > 0.2) {
    badge.classList.add('rate-yellow');
  } else {
    badge.classList.add('rate-red');
  }
}

/** Last Poll Time */
function renderLastPoll() {
  const el = document.getElementById('last-poll-text');
  if (State.lastPoll) {
    el.textContent = 'Updated ' + timeAgo(State.lastPoll);
  } else {
    el.textContent = '';
  }
}

/** Sidebar Repos */
function renderSidebar() {
  const container = document.getElementById('repo-list');
  const countEl = document.getElementById('repo-count');

  var selectedCount = Object.keys(State.selectedSidebarRepos).length;
  if (selectedCount > 0) {
    countEl.textContent = selectedCount + '/' + State.repos.length;
  } else {
    countEl.textContent = State.repos.length;
  }

  if (State.repos.length === 0) {
    if (!State.initialLoadDone) {
      container.innerHTML = '<div style="padding:20px 16px;color:var(--text-muted);font-size:var(--font-size-xs);text-align:center;"><span class="spinner" style="width:14px;height:14px;display:inline-block;vertical-align:middle;margin-right:6px;"></span>Loading repositories...</div>';
    } else {
      container.innerHTML = '<div style="padding:20px 16px;color:var(--text-muted);font-size:var(--font-size-xs);text-align:center;">No repos monitored.<br>Add one below.</div>';
    }
    return;
  }

  let html = '';
  State.repos.forEach(function(repoName) {
    const repoState = State.repoStates.find(function(r) { return r.repo === repoName; });
    const activeCount = repoState ? repoState.active_runs.length : 0;
    const hasActive = activeCount > 0;
    const dotClass = hasActive ? 'has-active' : 'idle';
    const isSelected = !!State.selectedSidebarRepos[repoName];

    html += '<div class="repo-item' + (isSelected ? ' selected' : '') + '" data-repo="' + esc(repoName) + '">';
    html += '  <div class="repo-item-left" onclick="App.toggleSidebarRepo(\'' + escJs(repoName) + '\')">';
    html += '    <span class="repo-dot ' + dotClass + '"></span>';
    html += '    <span class="repo-name">' + esc(repoName) + '<span class="repo-name-tooltip">' + esc(repoName) + '</span></span>';
    if (hasActive) {
      html += '    <span class="repo-active-count">' + activeCount + '</span>';
    }
    html += '  </div>';
    html += '  <span class="repo-actions">';
    html += '    <button class="repo-action-btn" onclick="event.stopPropagation(); window.open(\'https://github.com/' + escJs(repoName) + '\', \'_blank\', \'noopener,noreferrer\')" title="Open on GitHub">';
    html += '      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>';
    html += '    </button>';
    html += '    <button class="repo-action-btn repo-copy-btn" onclick="event.stopPropagation(); App.copyRepoUrl(\'' + escJs(repoName) + '\', this)" title="Copy URL">';
    html += '      <svg width="13" height="13" viewBox="0 0 16 16" fill="currentColor"><path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 010 1.5h-1.5a.25.25 0 00-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 00.25-.25v-1.5a.75.75 0 011.5 0v1.5A1.75 1.75 0 019.25 16h-7.5A1.75 1.75 0 010 14.25zM5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0114.25 11h-7.5A1.75 1.75 0 015 9.25zm1.75-.25a.25.25 0 00-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 00.25-.25v-7.5a.25.25 0 00-.25-.25z"/></svg>';
    html += '    </button>';
    html += '  </span>';
    html += '  <button class="repo-remove-btn" onclick="App.removeRepo(\'' + escJs(repoName) + '\')" title="Remove">&times;</button>';
    html += '</div>';
  });

  container.innerHTML = html;

  // Auto-adjust sidebar width to fit longest repo name (only on data load, not manual resize)
  autoFitSidebar();
  // Mark truncated names so tooltip only shows when needed
  markTruncatedRepoNames();
}

/** Measure longest repo name and resize sidebar to fit */
function autoFitSidebar() {
  var sidebar = document.getElementById('sidebar');
  if (!sidebar || sidebar.dataset.userResized) return;

  // Measure the longest repo name using an offscreen element
  var measurer = document.createElement('span');
  measurer.style.cssText = 'position:absolute;visibility:hidden;white-space:nowrap;font-family:var(--font-mono);font-size:var(--font-size-sm);';
  document.body.appendChild(measurer);

  var maxW = 0;
  State.repos.forEach(function(name) {
    measurer.textContent = name;
    if (measurer.offsetWidth > maxW) maxW = measurer.offsetWidth;
  });
  document.body.removeChild(measurer);

  // Add padding for dot(6+8) + actions(~60) + remove btn(24) + item padding(32) + scrollbar(12)
  var idealWidth = maxW + 142;
  // Clamp between min and max
  if (idealWidth < 200) idealWidth = 200;
  if (idealWidth > 480) idealWidth = 480;

  sidebar.style.width = idealWidth + 'px';
}

/** Mark repo names as truncated if their text overflows */
function markTruncatedRepoNames() {
  document.querySelectorAll('.repo-name').forEach(function(el) {
    if (el.scrollWidth > el.clientWidth) {
      el.classList.add('truncated');
    } else {
      el.classList.remove('truncated');
    }
  });
}

/** Active Runs Tab */
function renderActiveRuns() {
  const container = document.getElementById('active-runs-content');

  if (!State.initialLoadDone) {
    return; // Keep showing loading
  }

  // Collect all active runs grouped by repo
  let totalActive = 0;
  const groups = [];

  State.repoStates.forEach(function(repoState) {
    if (repoState.active_runs && repoState.active_runs.length > 0) {
      groups.push({
        repo: repoState.repo,
        runs: repoState.active_runs
      });
      totalActive += repoState.active_runs.length;
    }
  });

  // Update badge
  const badge = document.getElementById('active-count-badge');
  if (totalActive > 0) {
    badge.textContent = totalActive;
    badge.style.display = '';
  } else {
    badge.style.display = 'none';
  }

  if (groups.length === 0) {
    container.innerHTML = '<div class="empty-state">' +
      '<div class="empty-state-icon">&#9656;</div>' +
      '<div class="empty-state-title">No active runs</div>' +
      '<div class="empty-state-text">All workflows are idle. Active and queued runs will appear here automatically.</div>' +
      '</div>';
    stopAllDurationTimers();
    return;
  }

  let html = '';
  groups.forEach(function(group) {
    html += '<div class="run-group">';
    html += '  <div class="run-group-header">';
    html += '    <span class="run-group-repo">' + esc(group.repo) + '</span>';
    html += '    <span class="run-group-count">' + group.runs.length + ' active</span>';
    html += '  </div>';
    html += '  <div class="run-cards">';

    group.runs.forEach(function(run) {
      html += renderRunCard(run);
    });

    html += '  </div>';
    html += '</div>';
  });

  container.innerHTML = html;

  // Start duration timers for in-progress runs
  startDurationTimers();
}

function renderRunCard(run) {
  const status = run.status || 'unknown';
  const conclusion = run.conclusion || '';
  const name = run.name || run.display_title || 'Workflow #' + run.workflow_id;
  const title = run.display_title || '';
  const branch = run.head_branch || '--';
  const sha = (run.head_sha || '').substring(0, 7);
  const event = run.event || '--';
  const actor = run.actor;
  const actorLogin = actor ? actor.login : '--';
  const avatarUrl = actor && actor.avatar_url ? actor.avatar_url + '&s=32' : '';
  const isExpanded = !!State.expandedRuns[run.id];
  const duration = runDuration(run);
  const isActive = status === 'in_progress' || status === 'queued';

  let cardClasses = 'run-card status-' + status;
  if (conclusion) cardClasses += ' conclusion-' + conclusion;

  let html = '<div class="' + cardClasses + '" data-run-id="' + run.id + '">';

  // Top row: name + status badge
  html += '<div class="run-card-top">';
  html += '  <div class="run-card-info">';
  html += '    <div class="run-card-name" title="' + esc(name) + '">' + esc(name) + '</div>';
  if (title && title !== name) {
    html += '    <div class="run-card-title" title="' + esc(title) + '">' + esc(title) + '</div>';
  }
  html += '  </div>';
  html += '  <div class="run-card-badges">' + statusBadge(status, conclusion) + '</div>';
  html += '</div>';

  // Meta row
  html += '<div class="run-card-meta">';

  // Branch
  html += '<span class="run-meta-item"><span class="meta-icon">&#9741;</span> ' + esc(branch) + '</span>';

  // Event
  html += '<span class="run-meta-item"><span class="meta-icon">&#9889;</span> ' + esc(event) + '</span>';

  // SHA
  if (sha) {
    html += '<span class="run-meta-item"><span class="meta-icon">#</span> ' + esc(sha) + '</span>';
  }

  // Duration
  html += '<span class="run-meta-item duration-display" data-run-id="' + run.id + '">';
  html += '<span class="meta-icon">&#9201;</span> ' + formatDuration(duration);
  html += '</span>';

  // Actor
  html += '<span class="run-card-actor">';
  if (avatarUrl) {
    html += '<img class="actor-avatar" src="' + esc(avatarUrl) + '" alt="" loading="lazy">';
  }
  html += esc(actorLogin);
  html += '</span>';

  // Run number
  html += '<span class="run-meta-item">#' + run.run_number + '</span>';

  html += '</div>';

  // Action buttons
  html += '<div class="run-card-actions">';
  html += '  <button class="run-action-btn" onclick="if(!State.expandedRuns[' + run.id + ']){this.disabled=true; this.innerHTML=\'<span class=spinner style=width:12px;height:12px;display:inline-block></span> Loading...\';} App.toggleRunDetail(' + run.id + ', \'' + escJs(run.repo) + '\', event)">' + (isExpanded ? 'Collapse' : 'Details') + '</button>';
  if (isActive) {
    html += '  <button class="run-action-btn cancel-btn" onclick="App.cancelRun(' + run.id + ', \'' + escJs(run.repo) + '\', event)">Cancel</button>';
  }
  html += '  <button class="run-action-btn rerun-btn" onclick="App.rerunRun(' + run.id + ', \'' + escJs(run.repo) + '\', event)">Re-run</button>';
  html += '  <a class="run-action-btn github-btn" href="' + esc(run.html_url) + '" target="_blank" rel="noopener" onclick="event.stopPropagation()">GitHub &#8599;</a>';
  html += '</div>';

  // Expanded detail
  if (isExpanded) {
    html += renderRunDetail(run.id, run.repo, name);
  }

  html += '</div>';
  return html;
}

function renderRunDetail(runId, repo, workflowName) {
  const info = State.expandedRuns[runId];
  if (!info) return '';

  let html = '<div class="run-detail">';

  if (info.loading) {
    html += '<div class="run-detail-loading">' + BASKETBALL_LOADER_HTML + ' Loading jobs...</div>';
  } else if (info.error) {
    html += '<div class="run-detail-loading" style="color:var(--accent-red)">Error: ' + esc(info.error) + '</div>';
  } else if (!info.jobs || info.jobs.length === 0) {
    html += '<div class="run-detail-loading">No jobs found</div>';
  } else {
    var repoParts = (repo || '').split('/');
    var repoOwner = repoParts[0] || '';
    var repoName = repoParts[1] || '';

    info.jobs.forEach(function(job) {
      const isJobExpanded = !!State.expandedJobs[job.id];
      const jobDuration = jobDurationCalc(job);
      html += '<div class="job-item">';
      html += '  <div class="job-header" onclick="App.toggleJob(' + job.id + ', ' + runId + ')">';
      html += '    <div class="job-header-left">';
      html += '      ' + statusIcon(job.status, job.conclusion);
      html += '      <span class="job-name">' + esc(job.name) + '</span>';
      html += '    </div>';
      html += '    <div class="job-header-right">';
      if (job.runner_name) {
        html += '      <span>' + esc(job.runner_name) + '</span>';
      }
      html += '      <span>' + formatDuration(jobDuration) + '</span>';
      html += '      <span>' + (isJobExpanded ? '&#9660;' : '&#9654;') + '</span>';
      html += '    </div>';
      html += '  </div>';

      // Steps (with per-step log viewer)
      if (isJobExpanded && job.steps && job.steps.length > 0) {
        html += '  <div class="job-steps expanded">';
        job.steps.forEach(function(step) {
          const stepDur = stepDurationCalc(step);
          var stepKey = runId + '::' + job.id + '::' + step.number;
          var stepExpanded = !!(info.expandedStepLogs && info.expandedStepLogs[stepKey]);
          var logsLoading = !!(info.jobLogsLoadingById && info.jobLogsLoadingById[job.id] && !info.stepLogsByKey[stepKey]);
          var logsError = info.jobLogsErrorById && info.jobLogsErrorById[job.id];
          var stepLogs = info.stepLogsByKey && info.stepLogsByKey[stepKey];

          html += '<div class="step-item">';
          html += stepIcon(step.conclusion);
          html += '<span class="step-name" title="' + esc(step.name) + '">' + esc(step.name) + '</span>';
          html += '<span class="step-duration">' + formatDuration(stepDur) + '</span>';
          html += '<button class="step-log-btn" onclick="App.toggleStepLogs(' + runId + ', \'' + escJs(repo) + '\', ' + job.id + ', ' + step.number + ', event)" ' + (logsLoading ? 'disabled' : '') + '>' + (stepExpanded ? 'Hide Logs' : (logsLoading ? 'Loading...' : 'View Logs')) + '</button>';
          html += '</div>';

          if (stepExpanded) {
            html += '<div class="step-log-panel">';
            if (logsLoading) {
              html += '<div class="step-log-meta"><span class="spinner"></span> Loading logs...</div>';
            } else if (logsError) {
              html += '<div class="step-log-meta" style="color:var(--accent-red)">Error: ' + esc(logsError) + '</div>';
            } else {
              html += '<div class="step-log-actions"><button class="step-log-open-btn" onclick="App.openStepLogViewer(' + runId + ', ' + job.id + ', ' + step.number + ', event)">Open Expanded</button></div>';
              if (info.jobLogsTruncatedById && info.jobLogsTruncatedById[job.id]) {
                html += '<div class="step-log-meta">Showing tail of job logs (truncated for performance).</div>';
              }
              html += '<pre class="step-log-text">' + esc(stepLogs || 'No logs found for this step in the available job output.') + '</pre>';
            }
            html += '</div>';
          }
        });
        html += '  </div>';
      }

      // Log actions (only when job is expanded)
      if (isJobExpanded) {
        var isViewing = State.logViewerJobId === job.id;
        html += '<div class="job-log-actions">';
        if (isViewing) {
          html += '<button class="job-log-btn log-viewer-viewing-btn" disabled>&#9638; Viewing Logs</button>';
        } else {
          html += '<button class="job-log-btn" id="log-btn-' + job.id + '" onclick="event.stopPropagation(); this.disabled=true; this.innerHTML=\'<span class=spinner style=width:12px;height:12px;display:inline-block></span> Loading...\'; App.fetchJobLog(' + job.id + ', \'' + escJs(repoOwner) + '\', \'' + escJs(repoName) + '\', \'' + escJs(job.name) + '\', \'' + escJs(workflowName || '') + '\')">View Logs</button>';
        }
        html += '</div>';
      }

      html += '</div>';
    });
  }

  html += '</div>';
  return html;
}

function jobDurationCalc(job) {
  if (!job.started_at) return null;
  const start = new Date(job.started_at).getTime();
  if (job.completed_at) {
    return Math.max(0, Math.floor((new Date(job.completed_at).getTime() - start) / 1000));
  }
  return Math.max(0, Math.floor((Date.now() - start) / 1000));
}

function stepDurationCalc(step) {
  if (!step.started_at) return null;
  const start = new Date(step.started_at).getTime();
  if (step.completed_at) {
    return Math.max(0, Math.floor((new Date(step.completed_at).getTime() - start) / 1000));
  }
  return Math.max(0, Math.floor((Date.now() - start) / 1000));
}

/** Workflows Tab */
function renderWorkflows() {
  const container = document.getElementById('workflows-content');
  const query = State.workflowSearchQuery.toLowerCase();

  // Collect all workflows grouped by repo
  const allGroups = [];
  State.repoStates.forEach(function(repoState) {
    if (!repoState.workflows || repoState.workflows.length === 0) return;
    const filtered = repoState.workflows.filter(function(wf) {
      if (!query) return true;
      return (wf.name || '').toLowerCase().indexOf(query) >= 0 ||
             (wf.path || '').toLowerCase().indexOf(query) >= 0;
    });
    if (filtered.length > 0) {
      allGroups.push({ repo: repoState.repo, workflows: filtered });
    }
  });

  if (!State.initialLoadDone) return;

  if (allGroups.length === 0) {
    container.innerHTML = '<div class="empty-state">' +
      '<div class="empty-state-icon">&#9881;</div>' +
      '<div class="empty-state-title">No workflows found</div>' +
      '<div class="empty-state-text">' + (query ? 'No workflows match your search.' : 'Add repositories to see their workflows.') + '</div>' +
      '</div>';
    return;
  }

  // On first data load, default all groups to collapsed
  if (!State.workflowGroupsInitialized) {
    allGroups.forEach(function(g) { State.collapsedWorkflowGroups[g.repo] = true; });
    State.workflowGroupsInitialized = true;
  }

  // Selected workflow count for custom tab creation
  var selectedCount = Object.keys(State.selectedWorkflowKeys).length;
  let html = '<div class="workflow-controls-shell">';
  html += '<span style="font-size:var(--font-size-xs);color:var(--text-muted)">' + selectedCount + ' workflow' + (selectedCount === 1 ? '' : 's') + ' selected</span> ';
  html += '<input type="text" class="search-input" id="custom-tab-name-input" placeholder="Tab name..." style="width:160px;display:inline-block;margin-left:8px">';
  html += ' <button class="custom-workflow-tab-btn" onclick="App.createCustomTab()" ' + (selectedCount === 0 ? 'disabled' : '') + '>Create Tab</button>';
  html += ' <button class="custom-workflow-tab-btn" onclick="App.addToExistingCustomTab()" ' + (selectedCount === 0 || (State.customWorkflowTabs || []).length === 0 ? 'disabled' : '') + '>Add to Tab</button>';
  html += ' <button class="custom-workflow-tab-btn" style="margin-left:8px" onclick="App.clearWorkflowSelection()">Clear Selection</button>';
  html += '</div>';

  html += '<div class="workflow-table-wrap"><table class="workflow-table">';
  html += '<thead><tr>';
  html += '<th class="wf-select-col"></th><th>Workflow</th><th>File</th><th>State</th><th>Repo</th><th></th>';
  html += '</tr></thead><tbody>';

  allGroups.forEach(function(group, groupIdx) {
    // When searching, force-expand all matching groups; otherwise respect collapsed state
    var isCollapsed = query ? false : !!State.collapsedWorkflowGroups[group.repo];
    var parity = groupIdx % 2 === 0 ? 'wf-group-odd' : 'wf-group-even';
    html += '<tr class="workflow-group-header ' + parity + (isCollapsed ? ' collapsed' : '') + '" onclick="App.toggleWorkflowGroup(\'' + escJs(group.repo) + '\')">';
    html += '<td colspan="6"><span class="wf-toggle">&#9660;</span>' + esc(group.repo) + ' (' + group.workflows.length + ')</td></tr>';
    group.workflows.forEach(function(wf) {
      const stateClass = (wf.state || '').replace(/ /g, '_').toLowerCase();
      var wfKey = makeWorkflowKey(group.repo, wf.id);
      var isChecked = !!State.selectedWorkflowKeys[wfKey];
      html += '<tr class="wf-child-row ' + parity + (isCollapsed ? ' wf-hidden' : '') + '">';
      html += '<td class="wf-select-cell"><input type="checkbox" class="wf-select-checkbox" ' + (isChecked ? 'checked' : '') + ' onclick="event.stopPropagation(); App.toggleWorkflowSelection(\'' + escJs(wfKey) + '\', \'' + escJs(wf.name) + '\')"></td>';
      html += '<td class="wf-name">' + esc(wf.name) + '</td>';
      html += '<td class="wf-path">' + esc(wf.path) + '</td>';
      html += '<td><span class="wf-state ' + esc(stateClass) + '">' + esc(wf.state) + '</span></td>';
      html += '<td class="wf-repo">' + esc(group.repo) + '</td>';
      html += '<td><button class="trigger-btn" onclick="App.openDispatchModal(\'' + escJs(group.repo) + '\', ' + wf.id + ', \'' + escJs(wf.name) + '\')">&#9654; Trigger</button></td>';
      html += '</tr>';
    });
  });

  html += '</tbody></table></div>';
  container.innerHTML = html;

  // Update toggle-all button label
  var toggleBtn = document.getElementById('wf-toggle-all-btn');
  if (toggleBtn) {
    var allCollapsed = allGroups.every(function(g) {
      return !!State.collapsedWorkflowGroups[g.repo];
    });
    toggleBtn.innerHTML = allCollapsed ? '&#9660; Expand All' : '&#9654; Collapse All';
    toggleBtn.title = allCollapsed ? 'Expand all groups' : 'Collapse all groups';
  }
}

/** Actions Tab — unified view for triggering and monitoring workflow runs */
function renderActions() {
  var container = document.getElementById('actions-content');

  if (!State.initialLoadDone) return;

  var query = State.actionsSearchQuery.toLowerCase();

  // Build a list of all workflows with their latest run
  var allActions = [];
  State.repoStates.forEach(function(repoState) {
    if (!repoState.workflows || repoState.workflows.length === 0) return;
    repoState.workflows.forEach(function(wf) {
      // Apply search filter
      if (query) {
        var match = (wf.name || '').toLowerCase().indexOf(query) >= 0 ||
                    (wf.path || '').toLowerCase().indexOf(query) >= 0 ||
                    (repoState.repo || '').toLowerCase().indexOf(query) >= 0;
        if (!match) return;
      }
      // Find latest run for this workflow
      var latestRun = null;
      var allRuns = (repoState.active_runs || []).concat(repoState.recent_runs || []);
      allRuns.forEach(function(run) {
        if (run.workflow_id === wf.id) {
          if (!latestRun || new Date(run.updated_at) > new Date(latestRun.updated_at)) {
            latestRun = run;
          }
        }
      });
      allActions.push({ workflow: wf, repo: repoState.repo, latestRun: latestRun });
    });
  });

  if (allActions.length === 0) {
    container.innerHTML = '<div class="empty-state">' +
      '<div class="empty-state-icon">&#9654;</div>' +
      '<div class="empty-state-title">No actions available</div>' +
      '<div class="empty-state-text">Add repositories to see their GitHub Actions workflows here.</div>' +
      '</div>';
    return;
  }

  // Group by repo
  var groups = {};
  allActions.forEach(function(a) {
    if (!groups[a.repo]) groups[a.repo] = [];
    groups[a.repo].push(a);
  });

  var html = '';
  Object.keys(groups).forEach(function(repo) {
    html += '<div class="action-group-header">' + esc(repo) + ' (' + groups[repo].length + ' workflows)</div>';
    html += '<div class="actions-grid">';

    groups[repo].forEach(function(item) {
      var wf = item.workflow;
      var run = item.latestRun;
      var stateClass = (wf.state || '').replace(/ /g, '_').toLowerCase();

      html += '<div class="action-card">';

      // Top: name + state badge
      html += '<div class="action-card-top">';
      html += '  <div class="action-card-info">';
      html += '    <div class="action-card-name" title="' + esc(wf.name) + '">' + esc(wf.name) + '</div>';
      html += '    <div class="action-card-path" title="' + esc(wf.path) + '">' + esc(wf.path) + '</div>';
      html += '  </div>';
      html += '  <span class="action-card-state ' + esc(stateClass) + '">' + esc(wf.state) + '</span>';
      html += '</div>';

      // Latest run info
      if (run) {
        var runStatus = run.status || 'unknown';
        var runConclusion = run.conclusion || '';
        var runName = run.display_title || run.name || '';
        var runBranch = run.head_branch || '';
        var runTime = timeAgo(run.updated_at);

        html += '<div class="action-card-latest">';
        html += '  <span class="action-card-latest-label">Latest:</span>';
        html += '  <div class="action-card-latest-info">';
        html += '    ' + statusIcon(runStatus, runConclusion);
        if (runName) {
          html += '    <span class="action-card-latest-name" title="' + esc(runName) + '">' + esc(runName) + '</span>';
        }
        if (runBranch) {
          html += '    <span class="action-card-latest-branch">' + esc(runBranch) + '</span>';
        }
        html += '  </div>';
        html += '  <span class="action-card-latest-time">' + esc(runTime) + '</span>';
        html += '</div>';
      } else {
        html += '<div class="action-card-latest">';
        html += '  <span class="action-card-latest-label" style="color:var(--text-muted)">No runs yet</span>';
        html += '</div>';
      }

      // Action buttons
      html += '<div class="action-card-actions">';
      html += '  <button class="action-run-btn" onclick="App.quickRunAction(\'' + escJs(item.repo) + '\', ' + wf.id + ', \'' + escJs(wf.name) + '\')">&#9654; Run</button>';
      html += '  <button class="action-trigger-btn" onclick="App.openDispatchModal(\'' + escJs(item.repo) + '\', ' + wf.id + ', \'' + escJs(wf.name) + '\')">&#9881; Configure &amp; Run</button>';
      // GitHub link
      var ghUrl = 'https://github.com/' + item.repo + '/actions/workflows/' + (wf.path || '').replace(/^\.github\/workflows\//, '');
      html += '  <a class="action-github-link" href="' + esc(ghUrl) + '" target="_blank" rel="noopener">GitHub &#8599;</a>';
      html += '</div>';

      html += '</div>';
    });

    html += '</div>';
  });

  container.innerHTML = html;
}

/** History Tab */
function renderHistory() {
  const container = document.getElementById('history-content');

  if (!State.initialLoadDone) return;

  var query = State.historySearchQuery.toLowerCase();
  var cutoff = getTimeRangeCutoff(State.historyTimeRange, State.historyCustomFrom, State.historyCustomTo);

  // Collect all recent completed runs, sort by updated_at desc
  let allRecent = [];
  State.repoStates.forEach(function(repoState) {
    if (repoState.recent_runs) {
      repoState.recent_runs.forEach(function(run) {
        const r = Object.assign({}, run);
        r.repo = r.repo || repoState.repo;

        // Apply time range filter
        if (cutoff) {
          var runTime = new Date(r.updated_at).getTime();
          if (cutoff.from && runTime < cutoff.from) return;
          if (cutoff.to && runTime > cutoff.to) return;
        }

        // Apply search filter
        if (query) {
          var name = (r.name || r.display_title || '').toLowerCase();
          var branch = (r.head_branch || '').toLowerCase();
          var repo = (r.repo || '').toLowerCase();
          var event = (r.event || '').toLowerCase();
          var actor = (r.actor && r.actor.login || '').toLowerCase();
          var conclusion = (r.conclusion || '').toLowerCase();
          var match = name.indexOf(query) >= 0 || branch.indexOf(query) >= 0 ||
                      repo.indexOf(query) >= 0 || event.indexOf(query) >= 0 ||
                      actor.indexOf(query) >= 0 || conclusion.indexOf(query) >= 0;
          if (!match) return;
        }

        allRecent.push(r);
      });
    }
  });

  allRecent.sort(function(a, b) {
    return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
  });

  if (allRecent.length === 0) {
    container.innerHTML = '<div class="empty-state">' +
      '<div class="empty-state-icon">&#9716;</div>' +
      '<div class="empty-state-title">No matching runs</div>' +
      '<div class="empty-state-text">No workflow runs match your search or time range filters.</div>' +
      '</div>';
    return;
  }

  let html = '<table class="history-table">';
  html += '<thead><tr>';
  html += '<th></th><th>Workflow</th><th>Branch</th><th>Event</th><th>Actor</th><th>Duration</th><th>Completed</th><th>Repo</th><th>Actions</th>';
  html += '</tr></thead><tbody>';

  allRecent.forEach(function(run) {
    const conclusion = run.conclusion || '';
    const name = run.name || run.display_title || 'Workflow';
    const branch = run.head_branch || '--';
    const actor = run.actor;
    const actorLogin = actor ? actor.login : '--';
    const avatarUrl = actor && actor.avatar_url ? actor.avatar_url + '&s=20' : '';
    const dur = runDuration(run);

    var rowCls = 'conclusion-' + esc(conclusion) + ' history-clickable';
    if (State.expandedRuns[run.id]) rowCls += ' history-expanded';
    html += '<tr class="' + rowCls + '" onclick="App.toggleRunDetail(' + run.id + ', \'' + escJs(run.repo) + '\', event)">';
    html += '<td>' + statusIcon('completed', conclusion) + '</td>';
    html += '<td>' + esc(name) + '</td>';
    html += '<td>' + esc(branch) + '</td>';
    html += '<td>' + esc(run.event) + '</td>';
    html += '<td><span class="history-actor">';
    if (avatarUrl) {
      html += '<img class="actor-avatar" src="' + esc(avatarUrl) + '" alt="" loading="lazy">';
    }
    html += esc(actorLogin) + '</span></td>';
    html += '<td>' + formatDuration(dur) + '</td>';
    html += '<td title="' + esc(run.updated_at) + '">' + formatTime(run.updated_at) + '</td>';
    html += '<td class="history-repo">' + esc(run.repo) + '</td>';
    html += '<td class="history-actions-cell">';
    html += '<button class="history-rerun-btn" onclick="event.stopPropagation(); App.toggleRunDetail(' + run.id + ', \'' + escJs(run.repo) + '\', event)">' + (State.expandedRuns[run.id] ? '&#9660; Collapse' : '&#128269; Details') + '</button>';
    html += '<button class="history-rerun-btn" onclick="event.stopPropagation(); App.openDispatchModal(\'' + escJs(run.repo) + '\', ' + run.workflow_id + ', \'' + escJs(name) + '\')">&#9654; Re-run</button>';
    html += '<a class="history-rerun-btn github-link-btn" href="' + esc(run.html_url) + '" target="_blank" rel="noopener" onclick="event.stopPropagation()">GitHub &#8599;</a>';
    html += '</td>';
    html += '</tr>';

    // Expanded detail row (inline logs)
    if (State.expandedRuns[run.id]) {
      html += '<tr class="history-detail-row"><td colspan="9">' + renderRunDetail(run.id, run.repo, name) + '</td></tr>';
    }
  });

  html += '</tbody></table>';
  container.innerHTML = html;
}


/** Parse time range into { from, to } timestamps */
function getTimeRangeCutoff(range, customFrom, customTo) {
  if (range === 'all') return null;
  if (range === 'custom') {
    return {
      from: customFrom ? new Date(customFrom).getTime() : null,
      to: customTo ? new Date(customTo).getTime() : null,
    };
  }
  var now = Date.now();
  var ms = { '1h': 3600000, '6h': 21600000, '12h': 43200000, '1d': 86400000, '3d': 259200000, '7d': 604800000,
             '30d': 2592000000, '3m': 7776000000, '6m': 15552000000, '1y': 31536000000 };
  return { from: now - (ms[range] || 3600000), to: null };
}

/** Collect all runs (active + recent) filtered by time range */
function collectAllRunsInRange(range, customFrom, customTo) {
  var cutoff = getTimeRangeCutoff(range, customFrom, customTo);
  var runs = [];
  State.repoStates.forEach(function(repoState) {
    var allRuns = (repoState.active_runs || []).concat(repoState.recent_runs || []);
    allRuns.forEach(function(run) {
      var r = Object.assign({}, run);
      r.repo = r.repo || repoState.repo;
      if (cutoff) {
        var runTime = new Date(r.updated_at).getTime();
        if (cutoff.from && runTime < cutoff.from) return;
        if (cutoff.to && runTime > cutoff.to) return;
      }
      runs.push(r);
    });
  });
  runs.sort(function(a, b) {
    return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
  });
  return runs;
}

/** What's Happening Tab */
function renderWhatsHappening() {
  var container = document.getElementById('wh-content');
  if (!State.initialLoadDone) return;

  var runs = collectAllRunsInRange(State.whatsHappeningRange, State.whatsHappeningCustomFrom, State.whatsHappeningCustomTo);

  // Calculate summary stats
  var stats = { total: runs.length, success: 0, failure: 0, cancelled: 0, in_progress: 0, queued: 0 };
  var repoStats = {};   // repo -> { success, failure, cancelled, total }
  var wfStats = {};     // workflowName -> { success, failure, cancelled, total }

  runs.forEach(function(run) {
    var conclusion = run.conclusion || '';
    var status = run.status || '';
    if (conclusion === 'success') stats.success++;
    else if (conclusion === 'failure') stats.failure++;
    else if (conclusion === 'cancelled') stats.cancelled++;
    else if (status === 'in_progress') stats.in_progress++;
    else if (status === 'queued') stats.queued++;

    // Per-repo
    if (!repoStats[run.repo]) repoStats[run.repo] = { success: 0, failure: 0, cancelled: 0, total: 0 };
    repoStats[run.repo].total++;
    if (conclusion === 'success') repoStats[run.repo].success++;
    else if (conclusion === 'failure') repoStats[run.repo].failure++;
    else if (conclusion === 'cancelled') repoStats[run.repo].cancelled++;

    // Per-workflow
    var wfName = run.name || run.display_title || 'Unknown';
    if (!wfStats[wfName]) wfStats[wfName] = { success: 0, failure: 0, cancelled: 0, total: 0 };
    wfStats[wfName].total++;
    if (conclusion === 'success') wfStats[wfName].success++;
    else if (conclusion === 'failure') wfStats[wfName].failure++;
    else if (conclusion === 'cancelled') wfStats[wfName].cancelled++;
  });

  if (runs.length === 0) {
    container.innerHTML = '<div class="empty-state">' +
      '<div class="empty-state-icon">&#128196;</div>' +
      '<div class="empty-state-title">No activity</div>' +
      '<div class="empty-state-text">No workflow runs found in this time range.</div>' +
      '</div>';
    return;
  }

  var rangeLabel = State.whatsHappeningRange === 'custom' ? 'custom range' : 'the last ' + State.whatsHappeningRange;
  var successRate = stats.total > 0 ? Math.round((stats.success / stats.total) * 100) : 0;

  var html = '';

  // Narrative summary
  html += '<div class="wh-narrative">';
  html += 'In <strong>' + esc(rangeLabel) + '</strong>, there were <strong>' + stats.total + '</strong> workflow runs across <strong>' + Object.keys(repoStats).length + '</strong> repos. ';
  if (stats.success > 0) html += '<span style="color:var(--status-success)">' + stats.success + ' succeeded</span>';
  if (stats.failure > 0) html += (stats.success > 0 ? ', ' : '') + '<span style="color:var(--status-failure)">' + stats.failure + ' failed</span>';
  if (stats.cancelled > 0) html += ((stats.success > 0 || stats.failure > 0) ? ', ' : '') + '<span style="color:var(--status-cancelled)">' + stats.cancelled + ' cancelled</span>';
  if (stats.in_progress > 0) html += ((stats.success > 0 || stats.failure > 0 || stats.cancelled > 0) ? ', ' : '') + '<span style="color:var(--status-in-progress)">' + stats.in_progress + ' in progress</span>';
  html += '. Success rate: <strong>' + successRate + '%</strong>.';
  html += '</div>';

  // Summary stat cards
  html += '<div class="wh-summary">';
  html += '<div class="wh-stat-card"><div class="wh-stat-value">' + stats.total + '</div><div class="wh-stat-label">Total Runs</div></div>';
  html += '<div class="wh-stat-card success"><div class="wh-stat-value">' + stats.success + '</div><div class="wh-stat-label">Succeeded</div></div>';
  html += '<div class="wh-stat-card failure"><div class="wh-stat-value">' + stats.failure + '</div><div class="wh-stat-label">Failed</div></div>';
  html += '<div class="wh-stat-card cancelled"><div class="wh-stat-value">' + stats.cancelled + '</div><div class="wh-stat-label">Cancelled</div></div>';
  if (stats.in_progress > 0) {
    html += '<div class="wh-stat-card in-progress"><div class="wh-stat-value">' + stats.in_progress + '</div><div class="wh-stat-label">In Progress</div></div>';
  }
  html += '<div class="wh-stat-card"><div class="wh-stat-value">' + successRate + '%</div><div class="wh-stat-label">Success Rate</div></div>';
  html += '</div>';

  // Per-repo breakdown
  html += '<div class="wh-section-title">By Repository</div>';
  html += '<div class="wh-breakdown-grid">';
  Object.keys(repoStats).forEach(function(repo) {
    var rs = repoStats[repo];
    html += '<div class="wh-breakdown-card">';
    html += '<div class="wh-breakdown-name">' + esc(repo) + '</div>';
    html += '<div class="wh-breakdown-stats">';
    html += '<span style="color:var(--status-success)">&#10003; ' + rs.success + '</span>';
    html += '<span style="color:var(--status-failure)">&#10007; ' + rs.failure + '</span>';
    html += '<span style="color:var(--status-cancelled)">&#9673; ' + rs.cancelled + '</span>';
    html += '<span>' + rs.total + ' total</span>';
    html += '</div>';
    if (rs.total > 0) {
      html += '<div class="wh-breakdown-bar">';
      html += '<div class="wh-bar-success" style="width:' + (rs.success / rs.total * 100) + '%"></div>';
      html += '<div class="wh-bar-failure" style="width:' + (rs.failure / rs.total * 100) + '%"></div>';
      html += '<div class="wh-bar-cancelled" style="width:' + (rs.cancelled / rs.total * 100) + '%"></div>';
      html += '</div>';
    }
    html += '</div>';
  });
  html += '</div>';

  // Per-workflow breakdown
  html += '<div class="wh-section-title">By Workflow</div>';
  html += '<div class="wh-breakdown-grid">';
  Object.keys(wfStats).forEach(function(wfName) {
    var ws = wfStats[wfName];
    html += '<div class="wh-breakdown-card">';
    html += '<div class="wh-breakdown-name">' + esc(wfName) + '</div>';
    html += '<div class="wh-breakdown-stats">';
    html += '<span style="color:var(--status-success)">&#10003; ' + ws.success + '</span>';
    html += '<span style="color:var(--status-failure)">&#10007; ' + ws.failure + '</span>';
    html += '<span style="color:var(--status-cancelled)">&#9673; ' + ws.cancelled + '</span>';
    html += '<span>' + ws.total + ' total</span>';
    html += '</div>';
    if (ws.total > 0) {
      html += '<div class="wh-breakdown-bar">';
      html += '<div class="wh-bar-success" style="width:' + (ws.success / ws.total * 100) + '%"></div>';
      html += '<div class="wh-bar-failure" style="width:' + (ws.failure / ws.total * 100) + '%"></div>';
      html += '<div class="wh-bar-cancelled" style="width:' + (ws.cancelled / ws.total * 100) + '%"></div>';
      html += '</div>';
    }
    html += '</div>';
  });
  html += '</div>';

  // Timeline of recent events
  html += '<div class="wh-section-title">Activity Timeline</div>';
  html += '<div class="wh-timeline">';
  var timelineRuns = runs.slice(0, 50);
  timelineRuns.forEach(function(run) {
    var conclusion = run.conclusion || '';
    var status = run.status || '';
    var eventClass = conclusion || status;
    var name = run.name || run.display_title || 'Workflow';
    var branch = run.head_branch || '';
    var actor = run.actor ? run.actor.login : '';
    var timeStr = formatTime(run.updated_at);

    html += '<div class="wh-event ' + esc(eventClass) + '">';
    html += '<div class="wh-event-header">';
    html += '<span class="wh-event-time">' + esc(timeStr) + '</span>';
    html += statusIcon(status === 'in_progress' || status === 'queued' ? status : 'completed', conclusion);
    html += '<span class="wh-event-title">' + esc(name) + '</span>';
    html += '</div>';
    html += '<div class="wh-event-detail">';
    html += '<span class="wh-event-repo">' + esc(run.repo) + '</span>';
    if (branch) html += ' &middot; <span class="wh-event-branch">' + esc(branch) + '</span>';
    if (actor) html += ' &middot; ' + esc(actor);
    html += '</div>';
    html += '</div>';
  });
  html += '</div>';

  container.innerHTML = html;
}

/* ═══════════════════════════════════════════════════════════════
   DURATION TIMERS (live ticking for in-progress runs)
   ═══════════════════════════════════════════════════════════════ */

function stopAllDurationTimers() {
  State.durationTimers.forEach(function(id) {
    clearInterval(id);
  });
  State.durationTimers.clear();
}

function startDurationTimers() {
  stopAllDurationTimers();

  // Find all in-progress runs and start an interval for each
  State.repoStates.forEach(function(repoState) {
    if (!repoState.active_runs) return;
    repoState.active_runs.forEach(function(run) {
      if (run.status !== 'in_progress') return;
      const startAt = run.run_started_at || run.created_at;
      if (!startAt) return;

      const runId = run.id;
      const timerId = setInterval(function() {
        const el = document.querySelector('.duration-display[data-run-id="' + runId + '"]');
        if (!el) {
          clearInterval(timerId);
          State.durationTimers.delete(runId);
          return;
        }
        const elapsed = elapsedSince(startAt);
        el.innerHTML = '<span class="meta-icon">&#9201;</span> ' + formatDuration(elapsed);
      }, 1000);

      State.durationTimers.set(runId, timerId);
    });
  });
}

