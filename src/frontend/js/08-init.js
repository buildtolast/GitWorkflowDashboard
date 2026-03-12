/* ═══════════════════════════════════════════════════════════════
   INITIALIZATION
   ═══════════════════════════════════════════════════════════════ */
(function init() {
  // Restore theme from localStorage, or detect system preference
  try {
    var savedTheme = localStorage.getItem('gwd_theme');
    if (savedTheme) {
      App.setTheme(savedTheme);
    } else if (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) {
      App.setTheme('light');
    }
  } catch(_) {}

  // Remove no-transition class after first paint to enable smooth theme switching
  requestAnimationFrame(function() {
    document.documentElement.classList.remove('no-transition');
  });

  // Listen for OS theme changes (only if user hasn't manually set a theme)
  try {
    window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', function(e) {
      if (!localStorage.getItem('gwd_theme')) {
        App.setTheme(e.matches ? 'light' : 'terminal');
      }
    });
  } catch(_) {}

  // Load Slack templates and config from localStorage
  loadSlackTemplates();
  loadSlackConfig();

  // Load custom workflow tabs from localStorage
  State.customWorkflowTabs = loadCustomWorkflowTabs();
  App.renderCustomTabButtons();

  // Load saved repos from localStorage (for restoration if server has none)
  State.savedReposToRestore = loadSavedReposFromStorage();

  // Restore last tab from localStorage
  try {
    const savedTab = localStorage.getItem('gwd_tab');
    var builtinTabs = ['active-runs', 'workflows', 'actions', 'history', 'whats-happening', 'analytics'];
    var isCustomTab = typeof savedTab === 'string' && savedTab.indexOf('custom-workflow-') === 0;
    var isValidCustomTab = isCustomTab && State.customWorkflowTabs.some(function(t) { return t.id === savedTab; });
    if (savedTab && (builtinTabs.indexOf(savedTab) >= 0 || isValidCustomTab)) {
      App.switchTab(savedTab);
    }
  } catch(_) {}

  // Log viewer panel resize handle
  (function() {
    var handle = document.getElementById('log-viewer-resize-handle');
    var panel = document.getElementById('log-viewer-panel');
    var startY, startH;
    function onMouseDown(e) {
      if (panel.classList.contains('collapsed')) return;
      e.preventDefault();
      startY = e.clientY;
      startH = panel.offsetHeight;
      handle.classList.add('dragging');
      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
      document.body.style.cursor = 'ns-resize';
      document.body.style.userSelect = 'none';
    }
    function onMouseMove(e) {
      var delta = startY - e.clientY;
      var newH = Math.max(80, Math.min(window.innerHeight * 0.7, startH + delta));
      panel.style.height = newH + 'px';
    }
    function onMouseUp() {
      handle.classList.remove('dragging');
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
    handle.addEventListener('mousedown', onMouseDown);
  })();

  // Connect WebSocket
  wsConnect();
})();
