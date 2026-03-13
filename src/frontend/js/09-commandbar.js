/* ═══════════════════════════════════════════════════════════════
   COMMAND BAR (Cmd+K natural language workflow dispatch)
   ═══════════════════════════════════════════════════════════════ */

var CommandBar = {
  _catalog: [],        // Array of { repo, workflowId, workflowName, nameTokens, repoTokens, inputs, inputTokenMap }
  _catalogDirty: true,
  _selectedIndex: 0,
  _visible: false,
  _overlayEl: null,
  _inputEl: null,
  _resultsEl: null,
  _lastResults: [],
  _prefetchPending: false,

  /* ── Tokenization ── */

  /** Split a name into lowercase tokens on [-_ .], camelCase, and digits */
  _tokenize: function(name) {
    if (!name) return [];
    // Insert space before uppercase letters in camelCase
    var spaced = name.replace(/([a-z])([A-Z])/g, '$1 $2')
                     .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2');
    return spaced.split(/[-_.\s\/]+/)
                 .map(function(t) { return t.toLowerCase().trim(); })
                 .filter(function(t) { return t.length > 0; });
  },

  /* ── Catalog Building ── */

  rebuildCatalog: function() {
    var catalog = [];
    var prefetchKeys = [];

    for (var i = 0; i < State.repoStates.length; i++) {
      var rs = State.repoStates[i];
      var repo = rs.repo;
      var repoTokens = CommandBar._tokenize(repo);
      var workflows = rs.workflows || [];

      for (var j = 0; j < workflows.length; j++) {
        var wf = workflows[j];
        if (wf.state !== 'active') continue;

        var nameTokens = CommandBar._tokenize(wf.name);
        var wfKey = repo + '::' + wf.id;
        var cached = State.workflowInputsByKey[wfKey];
        var inputs = [];
        var inputTokenMap = {};

        if (cached && !cached.loading && cached.inputs && cached.inputs.length > 0) {
          inputs = cached.inputs;
          for (var k = 0; k < inputs.length; k++) {
            var inp = inputs[k];
            if (inp.type === 'choice' && inp.options && inp.options.length > 0) {
              for (var m = 0; m < inp.options.length; m++) {
                var optVal = inp.options[m].toLowerCase();
                if (!inputTokenMap[optVal]) {
                  inputTokenMap[optVal] = { inputName: inp.name, value: inp.options[m] };
                }
              }
            } else if (inp.type === 'boolean') {
              inputTokenMap['true'] = { inputName: inp.name, value: 'true' };
              inputTokenMap['false'] = { inputName: inp.name, value: 'false' };
              inputTokenMap['enable'] = { inputName: inp.name, value: 'true' };
              inputTokenMap['disable'] = { inputName: inp.name, value: 'false' };
              inputTokenMap['yes'] = { inputName: inp.name, value: 'true' };
              inputTokenMap['no'] = { inputName: inp.name, value: 'false' };
            }
          }
        } else if (!cached || (!cached.loading && cached.has_dispatch == null)) {
          // Not cached yet — queue for prefetch
          prefetchKeys.push({ key: wfKey, repo: repo, workflowId: wf.id });
        }

        catalog.push({
          repo: repo,
          workflowId: wf.id,
          workflowName: wf.name,
          nameTokens: nameTokens,
          repoTokens: repoTokens,
          inputs: inputs,
          inputTokenMap: inputTokenMap,
          hasDispatch: cached ? cached.has_dispatch : null
        });
      }
    }

    CommandBar._catalog = catalog;
    CommandBar._catalogDirty = false;

    // Async-prefetch uncached inputs
    if (prefetchKeys.length > 0 && !CommandBar._prefetchPending) {
      CommandBar._prefetchPending = true;
      CommandBar._prefetchInputs(prefetchKeys);
    }
  },

  _prefetchInputs: async function(keys) {
    // Fetch up to 10 at a time to avoid hammering the API
    var batch = keys.slice(0, 10);
    var promises = batch.map(function(k) {
      return ensureWorkflowInputsCached(k.key, k.repo, k.workflowId);
    });
    await Promise.all(promises);
    CommandBar._prefetchPending = false;
    // Rebuild catalog now that inputs are cached
    CommandBar._catalogDirty = true;
    if (CommandBar._visible) {
      CommandBar.rebuildCatalog();
      // Re-run search with current input
      if (CommandBar._inputEl) {
        CommandBar._onInput();
      }
    }
  },

  /* ── Fuzzy Matching ── */

  /** Match a query token against a target token. Returns score: 3=exact, 2=prefix, 1=substring, 0=no match */
  _tokenScore: function(query, target) {
    if (query === target) return 3;
    if (target.indexOf(query) === 0) return 2;
    if (target.indexOf(query) >= 0) return 1;
    return 0;
  },

  search: function(query) {
    if (!query || !query.trim()) return [];
    var qTokens = CommandBar._tokenize(query);
    if (qTokens.length === 0) return [];

    var results = [];

    for (var i = 0; i < CommandBar._catalog.length; i++) {
      var entry = CommandBar._catalog[i];
      var nameScore = 0;
      var repoScore = 0;
      var inputScore = 0;
      var matchedNameTokens = {};  // nameToken index -> true
      var matchedInputs = {};      // inputName -> value
      var unmatchedQTokens = [];

      // Score query tokens against workflow name tokens
      for (var q = 0; q < qTokens.length; q++) {
        var qt = qTokens[q];
        var bestNameScore = 0;
        for (var n = 0; n < entry.nameTokens.length; n++) {
          if (matchedNameTokens[n]) continue;
          var s = CommandBar._tokenScore(qt, entry.nameTokens[n]);
          if (s > bestNameScore) {
            bestNameScore = s;
            if (s > 0) matchedNameTokens[n] = true;
          }
        }
        if (bestNameScore > 0) {
          nameScore += bestNameScore;
        } else {
          unmatchedQTokens.push(qt);
        }
      }

      // Score unmatched tokens against repo tokens (lower weight)
      var stillUnmatched = [];
      for (var u = 0; u < unmatchedQTokens.length; u++) {
        var uqt = unmatchedQTokens[u];
        var bestRepoScore = 0;
        for (var r = 0; r < entry.repoTokens.length; r++) {
          var rs = CommandBar._tokenScore(uqt, entry.repoTokens[r]);
          if (rs > bestRepoScore) bestRepoScore = rs;
        }
        if (bestRepoScore > 0) {
          repoScore += bestRepoScore * 0.5;
        } else {
          stillUnmatched.push(uqt);
        }
      }

      // Score remaining unmatched tokens against input option values
      for (var su = 0; su < stillUnmatched.length; su++) {
        var suqt = stillUnmatched[su];
        // Try exact match first in inputTokenMap
        if (entry.inputTokenMap[suqt]) {
          var mapping = entry.inputTokenMap[suqt];
          matchedInputs[mapping.inputName] = mapping.value;
          inputScore += 2;
        } else {
          // Try substring match against input options
          var keys = Object.keys(entry.inputTokenMap);
          for (var ik = 0; ik < keys.length; ik++) {
            if (keys[ik].indexOf(suqt) >= 0 && !matchedInputs[entry.inputTokenMap[keys[ik]].inputName]) {
              var im = entry.inputTokenMap[keys[ik]];
              matchedInputs[im.inputName] = im.value;
              inputScore += 1;
              break;
            }
          }
        }
      }

      var totalScore = nameScore + repoScore + inputScore;
      if (totalScore > 0) {
        results.push({
          entry: entry,
          score: totalScore,
          nameScore: nameScore,
          matchedInputs: matchedInputs,
          matchedNameTokens: matchedNameTokens
        });
      }
    }

    // Sort by score descending, then by name alphabetically
    results.sort(function(a, b) {
      if (b.score !== a.score) return b.score - a.score;
      return a.entry.workflowName.localeCompare(b.entry.workflowName);
    });

    return results.slice(0, 15);
  },

  /* ── UI ── */

  _ensureDOM: function() {
    if (CommandBar._overlayEl) return;

    var overlay = document.createElement('div');
    overlay.className = 'commandbar-overlay';
    overlay.id = 'commandbar-overlay';

    overlay.innerHTML =
      '<div class="commandbar-container">' +
        '<input class="commandbar-input" placeholder="Type a command... (e.g. deploy to staging)" autocomplete="off" spellcheck="false">' +
        '<div class="commandbar-results"></div>' +
        '<div class="commandbar-hint">' +
          '<span><kbd>&#8593;&#8595;</kbd> navigate</span>' +
          '<span><kbd>&#9166;</kbd> select</span>' +
          '<span><kbd>esc</kbd> close</span>' +
        '</div>' +
      '</div>';

    // Close on backdrop click
    overlay.addEventListener('click', function(e) {
      if (e.target === overlay) CommandBar.hide();
    });

    var input = overlay.querySelector('.commandbar-input');
    input.addEventListener('input', function() { CommandBar._onInput(); });
    input.addEventListener('keydown', function(e) { CommandBar._onKeydown(e); });

    CommandBar._overlayEl = overlay;
    CommandBar._inputEl = input;
    CommandBar._resultsEl = overlay.querySelector('.commandbar-results');

    document.body.appendChild(overlay);
  },

  show: function() {
    CommandBar._ensureDOM();
    if (CommandBar._catalogDirty) CommandBar.rebuildCatalog();
    CommandBar._overlayEl.classList.add('visible');
    CommandBar._inputEl.value = '';
    CommandBar._resultsEl.innerHTML = '';
    CommandBar._selectedIndex = 0;
    CommandBar._lastResults = [];
    CommandBar._visible = true;
    // Defer focus so the overlay transition completes
    setTimeout(function() { CommandBar._inputEl.focus(); }, 10);
  },

  hide: function() {
    if (!CommandBar._overlayEl) return;
    CommandBar._overlayEl.classList.remove('visible');
    CommandBar._visible = false;
    CommandBar._lastResults = [];
  },

  toggle: function() {
    if (CommandBar._visible) CommandBar.hide();
    else CommandBar.show();
  },

  _onInput: function() {
    var query = CommandBar._inputEl.value;
    var results = CommandBar.search(query);
    CommandBar._lastResults = results;
    CommandBar._selectedIndex = 0;
    CommandBar._renderResults(results, query);
  },

  _onKeydown: function(e) {
    var len = CommandBar._lastResults.length;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (len > 0) {
        CommandBar._selectedIndex = (CommandBar._selectedIndex + 1) % len;
        CommandBar._updateSelection();
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (len > 0) {
        CommandBar._selectedIndex = (CommandBar._selectedIndex - 1 + len) % len;
        CommandBar._updateSelection();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (len > 0) {
        CommandBar._selectResult(CommandBar._selectedIndex);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      CommandBar.hide();
    }
  },

  _renderResults: function(results, query) {
    if (!results || results.length === 0) {
      var q = (query || '').trim();
      if (q.length === 0) {
        CommandBar._resultsEl.innerHTML = '';
      } else {
        CommandBar._resultsEl.innerHTML = '<div class="commandbar-empty">No matching workflows</div>';
      }
      return;
    }

    var html = '';
    for (var i = 0; i < results.length; i++) {
      var r = results[i];
      var entry = r.entry;
      var selectedClass = i === CommandBar._selectedIndex ? ' selected' : '';

      // Highlight matched tokens in the workflow name
      var nameHtml = CommandBar._highlightName(entry.workflowName, entry.nameTokens, r.matchedNameTokens);

      // Build input chips
      var chipsHtml = '';
      var mKeys = Object.keys(r.matchedInputs);
      for (var c = 0; c < mKeys.length; c++) {
        chipsHtml += '<span class="commandbar-input-chip">' + esc(mKeys[c]) + '=' + esc(r.matchedInputs[mKeys[c]]) + '</span>';
      }

      // No-dispatch indicator
      var noDispatchHtml = '';
      if (entry.hasDispatch === false) {
        noDispatchHtml = '<span class="commandbar-input-chip" style="color:var(--text-muted);background:var(--tint-muted);border-color:var(--border);">no dispatch</span>';
      }

      html += '<div class="commandbar-result-item' + selectedClass + '" data-index="' + i + '">';
      html += '<div class="commandbar-result-name">' + nameHtml + '</div>';
      html += '<div class="commandbar-result-meta">';
      html += chipsHtml;
      html += noDispatchHtml;
      html += '<span class="commandbar-repo-badge">' + esc(entry.repo) + '</span>';
      html += '</div>';
      html += '</div>';
    }

    CommandBar._resultsEl.innerHTML = html;

    // Attach click handlers
    var items = CommandBar._resultsEl.querySelectorAll('.commandbar-result-item');
    for (var j = 0; j < items.length; j++) {
      items[j].addEventListener('click', (function(idx) {
        return function() { CommandBar._selectResult(idx); };
      })(j));
    }
  },

  /** Highlight matched tokens in the workflow name */
  _highlightName: function(name, nameTokens, matchedIndexes) {
    if (!matchedIndexes || Object.keys(matchedIndexes).length === 0) return esc(name);

    // Build set of matched token strings
    var matchedSet = {};
    for (var key in matchedIndexes) {
      if (matchedIndexes[key] && nameTokens[key]) {
        matchedSet[nameTokens[key]] = true;
      }
    }

    // Split name into tokens preserving separators
    var parts = name.split(/(\s+|[-_.])/);
    var html = '';
    for (var i = 0; i < parts.length; i++) {
      var p = parts[i];
      if (matchedSet[p.toLowerCase()]) {
        html += '<span class="commandbar-match-hl">' + esc(p) + '</span>';
      } else {
        html += esc(p);
      }
    }
    return html;
  },

  _updateSelection: function() {
    var items = CommandBar._resultsEl.querySelectorAll('.commandbar-result-item');
    for (var i = 0; i < items.length; i++) {
      items[i].classList.toggle('selected', i === CommandBar._selectedIndex);
    }
    // Scroll selected into view
    if (items[CommandBar._selectedIndex]) {
      items[CommandBar._selectedIndex].scrollIntoView({ block: 'nearest' });
    }
  },

  _selectResult: function(index) {
    var result = CommandBar._lastResults[index];
    if (!result) return;
    var entry = result.entry;
    var matchedInputs = result.matchedInputs;
    CommandBar.hide();
    // Open dispatch modal pre-filled
    App.openDispatchModal(entry.repo, entry.workflowId, entry.workflowName, matchedInputs);
  }
};
