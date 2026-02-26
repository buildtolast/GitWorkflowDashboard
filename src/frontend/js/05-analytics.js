/* ═══════════════════════════════════════════════════════════════
   ANALYTICS TAB
   ═══════════════════════════════════════════════════════════════ */

function renderAnalytics() {
  var container = document.getElementById('an-content');
  if (!container || !State.initialLoadDone) return;

  var allRuns = collectAllRunsInRange(State.analyticsRange, State.analyticsCustomFrom, State.analyticsCustomTo);
  var completed = allRuns.filter(function(r) { return r.status === 'completed' && r.conclusion; });

  if (allRuns.length === 0) {
    container.innerHTML = '<div class="empty-state"><div class="empty-state-icon">&#128202;</div>' +
      '<div class="empty-state-title">No data in this range</div>' +
      '<div class="empty-state-text">Try a wider time range to see analytics.</div></div>';
    return;
  }

  var html = '';

  // ── Section 1: Actor Leaderboard ──
  html += '<div class="an-section-title">Actor Leaderboard</div>';
  var actorMap = {};
  allRuns.forEach(function(r) {
    if (!r.actor || !r.actor.login) return;
    var login = r.actor.login;
    if (!actorMap[login]) actorMap[login] = { total: 0, success: 0, fail: 0, avatar: r.actor.avatar_url, lastActive: r.updated_at };
    actorMap[login].total++;
    if (r.conclusion === 'success') actorMap[login].success++;
    if (r.conclusion === 'failure') actorMap[login].fail++;
    if (r.updated_at > actorMap[login].lastActive) actorMap[login].lastActive = r.updated_at;
  });
  var actors = Object.keys(actorMap).map(function(login) {
    var d = actorMap[login];
    return { login: login, total: d.total, success: d.success, fail: d.fail, avatar: d.avatar,
             rate: d.total > 0 ? Math.round(d.success / d.total * 100) : 0, lastActive: d.lastActive };
  }).sort(function(a, b) { return b.total - a.total; }).slice(0, 10);

  if (actors.length > 0) {
    html += '<table class="an-table"><thead><tr><th>#</th><th>Actor</th><th>Runs</th><th>Success Rate</th><th>Failures</th><th>Last Active</th></tr></thead><tbody>';
    actors.forEach(function(a, i) {
      html += '<tr>';
      html += '<td>' + (i + 1) + '</td>';
      html += '<td>';
      if (a.avatar) html += '<img class="an-avatar" src="' + esc(a.avatar) + '&s=20" alt="" loading="lazy">';
      html += esc(a.login) + '</td>';
      html += '<td>' + a.total + '</td>';
      html += '<td><div class="an-inline-bar-container"><div class="an-inline-bar" style="width:' + a.rate + '%"></div></div> ' + a.rate + '%</td>';
      html += '<td style="color:' + (a.fail > 0 ? 'var(--status-failure)' : 'var(--text-muted)') + '">' + a.fail + '</td>';
      html += '<td>' + formatTime(a.lastActive) + '</td>';
      html += '</tr>';
    });
    html += '</tbody></table>';
  } else {
    html += '<div style="color:var(--text-muted);padding:8px">No actor information available.</div>';
  }

  // ── Section 2: Success Rate ──
  var successCount = completed.filter(function(r) { return r.conclusion === 'success'; }).length;
  var failCount = completed.filter(function(r) { return r.conclusion === 'failure'; }).length;
  var cancelCount = completed.filter(function(r) { return r.conclusion === 'cancelled'; }).length;
  var totalCompleted = completed.length;
  var overallRate = totalCompleted > 0 ? Math.round(successCount / totalCompleted * 100) : 0;
  var rateClass = overallRate >= 80 ? 'success' : (overallRate >= 50 ? 'warning' : 'danger');

  html += '<div class="an-section-title">Success Rate</div>';
  html += '<div class="an-rate-hero">';
  html += '<div class="an-rate-circle ' + rateClass + '">';
  html += '<div class="an-rate-pct">' + overallRate + '%</div>';
  html += '<div class="an-rate-label">Overall Success</div>';
  html += '<div class="an-rate-detail">' + successCount + ' of ' + totalCompleted + ' completed runs</div>';
  html += '</div></div>';

  // Per-repo success rates
  var repoMap = {};
  completed.forEach(function(r) {
    if (!repoMap[r.repo]) repoMap[r.repo] = { success: 0, total: 0 };
    repoMap[r.repo].total++;
    if (r.conclusion === 'success') repoMap[r.repo].success++;
  });
  var repoRates = Object.keys(repoMap).map(function(repo) {
    var d = repoMap[repo];
    return { name: repo, rate: Math.round(d.success / d.total * 100), success: d.success, total: d.total };
  }).sort(function(a, b) { return a.rate - b.rate; });

  if (repoRates.length > 1) {
    html += '<div class="an-subsection-title">By Repository</div>';
    html += '<div class="an-rate-list">';
    repoRates.forEach(function(r) {
      html += '<div class="an-rate-row">';
      html += '<div class="an-rate-row-name" title="' + esc(r.name) + '">' + esc(r.name) + '</div>';
      html += '<div class="an-rate-row-bar-container"><div class="an-rate-row-bar" style="width:' + r.rate + '%"></div></div>';
      html += '<div class="an-rate-row-pct">' + r.rate + '%</div>';
      html += '<div class="an-rate-row-count">(' + r.success + '/' + r.total + ')</div>';
      html += '</div>';
    });
    html += '</div>';
  }

  // Per-workflow success rates
  var wfMap = {};
  completed.forEach(function(r) {
    var name = r.name || r.display_title || 'Workflow';
    if (!wfMap[name]) wfMap[name] = { success: 0, total: 0 };
    wfMap[name].total++;
    if (r.conclusion === 'success') wfMap[name].success++;
  });
  var wfRates = Object.keys(wfMap).map(function(name) {
    var d = wfMap[name];
    return { name: name, rate: Math.round(d.success / d.total * 100), success: d.success, total: d.total };
  }).sort(function(a, b) { return a.rate - b.rate; });

  if (wfRates.length > 0) {
    html += '<div class="an-subsection-title">By Workflow</div>';
    html += '<div class="an-rate-list">';
    wfRates.forEach(function(r) {
      html += '<div class="an-rate-row">';
      html += '<div class="an-rate-row-name" title="' + esc(r.name) + '">' + esc(r.name) + '</div>';
      html += '<div class="an-rate-row-bar-container"><div class="an-rate-row-bar" style="width:' + r.rate + '%"></div></div>';
      html += '<div class="an-rate-row-pct">' + r.rate + '%</div>';
      html += '<div class="an-rate-row-count">(' + r.success + '/' + r.total + ')</div>';
      html += '</div>';
    });
    html += '</div>';
  }

  // ── Section 3: Duration Analysis ──
  html += '<div class="an-section-title">Duration Analysis</div>';

  var durations = completed.map(function(r) {
    var start = r.run_started_at || r.created_at;
    var end = r.updated_at;
    if (!start || !end) return null;
    var ms = new Date(end) - new Date(start);
    var sec = Math.round(ms / 1000);
    if (sec < 0 || sec > 86400) return null;
    return { sec: sec, run: r };
  }).filter(function(d) { return d !== null; });

  if (durations.length > 0) {
    durations.sort(function(a, b) { return a.sec - b.sec; });
    var totalSec = durations.reduce(function(s, d) { return s + d.sec; }, 0);
    var avgSec = Math.round(totalSec / durations.length);
    var medianSec = durations[Math.floor(durations.length / 2)].sec;

    // Trend: compare first half vs second half
    var trendHtml = '';
    if (durations.length >= 4) {
      var byTime = durations.slice().sort(function(a, b) { return new Date(a.run.created_at) - new Date(b.run.created_at); });
      var mid = Math.floor(byTime.length / 2);
      var firstAvg = byTime.slice(0, mid).reduce(function(s, d) { return s + d.sec; }, 0) / mid;
      var secondAvg = byTime.slice(mid).reduce(function(s, d) { return s + d.sec; }, 0) / (byTime.length - mid);
      var pctChange = Math.round((secondAvg - firstAvg) / firstAvg * 100);
      if (Math.abs(pctChange) > 10) {
        var trendIcon = pctChange > 0 ? '&#8593;' : '&#8595;';
        var trendCls = pctChange > 0 ? 'failure' : 'success';
        var trendLabel = pctChange > 0 ? 'Slower' : 'Faster';
        trendHtml = '<div class="wh-stat-card"><div class="wh-stat-value" style="color:var(--status-' + trendCls + ')">' +
          trendIcon + ' ' + Math.abs(pctChange) + '%</div><div class="wh-stat-label">' + trendLabel + '</div></div>';
      } else {
        trendHtml = '<div class="wh-stat-card"><div class="wh-stat-value">&#8596;</div><div class="wh-stat-label">Stable</div></div>';
      }
    }

    html += '<div class="an-duration-summary">';
    html += '<div class="wh-stat-card"><div class="wh-stat-value">' + formatDuration(avgSec) + '</div><div class="wh-stat-label">Average</div></div>';
    html += '<div class="wh-stat-card"><div class="wh-stat-value">' + formatDuration(medianSec) + '</div><div class="wh-stat-label">Median</div></div>';
    html += trendHtml;
    html += '</div>';

    // Per-workflow average duration
    var wfDurMap = {};
    durations.forEach(function(d) {
      var name = d.run.name || d.run.display_title || 'Workflow';
      if (!wfDurMap[name]) wfDurMap[name] = { total: 0, count: 0 };
      wfDurMap[name].total += d.sec;
      wfDurMap[name].count++;
    });
    var wfDurs = Object.keys(wfDurMap).map(function(name) {
      return { name: name, avg: Math.round(wfDurMap[name].total / wfDurMap[name].count) };
    }).sort(function(a, b) { return b.avg - a.avg; });
    var maxDur = wfDurs.length > 0 ? wfDurs[0].avg : 1;

    html += '<div class="an-subsection-title">Average by Workflow (slowest first)</div>';
    html += '<div class="an-duration-list">';
    wfDurs.forEach(function(w) {
      var pct = Math.round(w.avg / maxDur * 100);
      html += '<div class="an-duration-row">';
      html += '<div class="an-duration-row-name" title="' + esc(w.name) + '">' + esc(w.name) + '</div>';
      html += '<div class="an-duration-row-bar-container"><div class="an-duration-row-bar" style="width:' + pct + '%"></div></div>';
      html += '<div class="an-duration-row-time">' + formatDuration(w.avg) + '</div>';
      html += '</div>';
    });
    html += '</div>';

    // Slowest runs
    var slowest = durations.slice().sort(function(a, b) { return b.sec - a.sec; }).slice(0, 5);
    html += '<div class="an-subsection-title">Slowest Runs</div>';
    html += '<table class="an-table"><thead><tr><th>Workflow</th><th>Branch</th><th>Duration</th><th>Time</th></tr></thead><tbody>';
    slowest.forEach(function(d) {
      html += '<tr>';
      html += '<td>' + esc(d.run.name || d.run.display_title || 'Workflow') + '</td>';
      html += '<td>' + esc(d.run.head_branch || '--') + '</td>';
      html += '<td style="color:var(--accent-orange);font-weight:600">' + formatDuration(d.sec) + '</td>';
      html += '<td>' + formatTime(d.run.updated_at) + '</td>';
      html += '</tr>';
    });
    html += '</tbody></table>';
  } else {
    html += '<div style="color:var(--text-muted);padding:8px">No completed runs with duration data.</div>';
  }

  // ── Section 3: Triggers ──
  html += '<div class="an-section-title">Triggers</div>';
  var triggerMap = {};
  allRuns.forEach(function(r) {
    var evt = r.event || 'unknown';
    if (!triggerMap[evt]) triggerMap[evt] = { total: 0, success: 0 };
    triggerMap[evt].total++;
    if (r.conclusion === 'success') triggerMap[evt].success++;
  });
  var triggers = Object.keys(triggerMap).map(function(evt) {
    var d = triggerMap[evt];
    return { event: evt, total: d.total, success: d.success, pct: Math.round(d.total / allRuns.length * 100) };
  }).sort(function(a, b) { return b.total - a.total; });

  var triggerIcons = { push: '&#8593;', pull_request: '&#8644;', workflow_dispatch: '&#9654;', schedule: '&#9200;' };
  html += '<div class="an-trigger-grid">';
  triggers.forEach(function(t) {
    var icon = triggerIcons[t.event] || '&#9679;';
    var successRate = t.total > 0 ? Math.round(t.success / t.total * 100) : 0;
    html += '<div class="an-trigger-card">';
    html += '<div class="an-trigger-icon">' + icon + '</div>';
    html += '<div class="an-trigger-name">' + esc(t.event) + '</div>';
    html += '<div class="an-trigger-count">' + t.total + '</div>';
    html += '<div class="an-trigger-pct">' + t.pct + '% of all runs</div>';
    if (t.success > 0) html += '<div class="an-trigger-success">&#10003; ' + successRate + '% success</div>';
    html += '</div>';
  });
  html += '</div>';

  // ── Section 4: Branch Activity ──
  html += '<div class="an-section-title">Branch Activity</div>';
  var branchMap = {};
  allRuns.forEach(function(r) {
    var branch = r.head_branch || '(none)';
    if (!branchMap[branch]) branchMap[branch] = { total: 0, success: 0, fail: 0, cancel: 0 };
    branchMap[branch].total++;
    if (r.conclusion === 'success') branchMap[branch].success++;
    if (r.conclusion === 'failure') branchMap[branch].fail++;
    if (r.conclusion === 'cancelled') branchMap[branch].cancel++;
  });
  var branches = Object.keys(branchMap).map(function(b) {
    var d = branchMap[b];
    return { name: b, total: d.total, success: d.success, fail: d.fail, cancel: d.cancel };
  }).sort(function(a, b) { return b.total - a.total; }).slice(0, 15);

  if (branches.length > 0) {
    html += '<div class="an-branch-list">';
    branches.forEach(function(b) {
      var sPct = b.total > 0 ? Math.round(b.success / b.total * 100) : 0;
      var fPct = b.total > 0 ? Math.round(b.fail / b.total * 100) : 0;
      var cPct = b.total > 0 ? Math.round(b.cancel / b.total * 100) : 0;
      html += '<div class="an-branch-row">';
      html += '<div class="an-branch-name" title="' + esc(b.name) + '">' + esc(b.name) + '</div>';
      html += '<div class="an-rate-row-bar-container" style="display:flex;overflow:hidden">';
      if (sPct > 0) html += '<div class="wh-bar-success" style="width:' + sPct + '%;height:100%"></div>';
      if (fPct > 0) html += '<div class="wh-bar-failure" style="width:' + fPct + '%;height:100%"></div>';
      if (cPct > 0) html += '<div class="wh-bar-cancelled" style="width:' + cPct + '%;height:100%"></div>';
      html += '</div>';
      html += '<div class="an-branch-stats">';
      html += '<span>' + b.total + ' runs</span>';
      html += '<span style="color:var(--status-success)">' + b.success + ' ok</span>';
      if (b.fail > 0) html += '<span style="color:var(--status-failure)">' + b.fail + ' fail</span>';
      html += '</div>';
      html += '</div>';
    });
    html += '</div>';
  } else {
    html += '<div style="color:var(--text-muted);padding:8px">No branch data available.</div>';
  }

  // ── Section 5: Failure Analysis ──
  html += '<div class="an-section-title">Failure Analysis</div>';
  var failures = completed.filter(function(r) { return r.conclusion === 'failure'; });

  if (failures.length === 0) {
    html += '<div style="color:var(--status-success);padding:8px">&#10003; No failures in this time range!</div>';
  } else {
    // Most failing workflows
    var failWfMap = {};
    completed.forEach(function(r) {
      var name = r.name || r.display_title || 'Workflow';
      if (!failWfMap[name]) failWfMap[name] = { fail: 0, total: 0 };
      failWfMap[name].total++;
      if (r.conclusion === 'failure') failWfMap[name].fail++;
    });
    var failWfs = Object.keys(failWfMap).filter(function(n) { return failWfMap[n].fail > 0; })
      .map(function(n) { var d = failWfMap[n]; return { name: n, fail: d.fail, total: d.total, rate: Math.round(d.fail / d.total * 100) }; })
      .sort(function(a, b) { return b.fail - a.fail; }).slice(0, 5);

    html += '<div class="an-subsection-title">Most Failing Workflows</div>';
    html += '<div class="an-failure-list">';
    failWfs.forEach(function(w) {
      html += '<div class="an-failure-row">';
      html += '<div class="an-failure-name" title="' + esc(w.name) + '">' + esc(w.name) + '</div>';
      html += '<div class="an-rate-row-bar-container"><div class="an-rate-row-bar failure" style="width:' + w.rate + '%"></div></div>';
      html += '<div class="an-rate-row-pct" style="color:var(--status-failure)">' + w.fail + ' fail</div>';
      html += '<div class="an-rate-row-count">(' + w.rate + '% of ' + w.total + ')</div>';
      html += '</div>';
    });
    html += '</div>';

    // Failure streaks
    var wfStreaks = {};
    completed.forEach(function(r) {
      var name = r.name || r.display_title || 'Workflow';
      if (!wfStreaks[name]) wfStreaks[name] = [];
      wfStreaks[name].push(r);
    });
    var streaks = [];
    Object.keys(wfStreaks).forEach(function(name) {
      var runs = wfStreaks[name].sort(function(a, b) { return new Date(a.created_at) - new Date(b.created_at); });
      var maxStreak = 0, curStreak = 0, isActive = false;
      runs.forEach(function(r, i) {
        if (r.conclusion === 'failure') {
          curStreak++;
          if (curStreak > maxStreak) maxStreak = curStreak;
          if (i === runs.length - 1) isActive = true;
        } else {
          curStreak = 0;
          isActive = false;
        }
      });
      if (maxStreak >= 2) {
        streaks.push({ name: name, streak: maxStreak, active: isActive });
      }
    });
    streaks.sort(function(a, b) { return (b.active ? 1 : 0) - (a.active ? 1 : 0) || b.streak - a.streak; });

    if (streaks.length > 0) {
      html += '<div class="an-subsection-title">Failure Streaks</div>';
      html += '<div class="an-streak-list">';
      streaks.forEach(function(s) {
        html += '<div class="an-streak-row' + (s.active ? ' active' : '') + '">';
        html += '<div class="an-streak-indicator">' + (s.active ? '&#9888; ACTIVE' : s.streak + 'x') + '</div>';
        html += '<div class="an-streak-name">' + esc(s.name) + '</div>';
        html += '<div class="an-streak-detail">' + s.streak + ' consecutive failures' + (s.active ? ' (ongoing)' : '') + '</div>';
        html += '</div>';
      });
      html += '</div>';
    }

    // Recent failures table
    var recentFails = failures.sort(function(a, b) { return new Date(b.updated_at) - new Date(a.updated_at); }).slice(0, 10);
    html += '<div class="an-subsection-title">Recent Failures</div>';
    html += '<table class="an-table"><thead><tr><th>Workflow</th><th>Branch</th><th>Actor</th><th>Repo</th><th>Time</th></tr></thead><tbody>';
    recentFails.forEach(function(r) {
      html += '<tr>';
      html += '<td>' + esc(r.name || r.display_title || 'Workflow') + '</td>';
      html += '<td>' + esc(r.head_branch || '--') + '</td>';
      html += '<td>' + esc(r.actor ? r.actor.login : '--') + '</td>';
      html += '<td>' + esc(r.repo) + '</td>';
      html += '<td>' + formatTime(r.updated_at) + '</td>';
      html += '</tr>';
    });
    html += '</tbody></table>';
  }

  container.innerHTML = html;
}

