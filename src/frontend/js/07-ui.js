
/* ═══════════════════════════════════════════════════════════════
   SIDEBAR RESIZE HANDLER
   ═══════════════════════════════════════════════════════════════ */
(function() {
  const handle = document.getElementById('sidebar-resize');
  const sidebar = document.getElementById('sidebar');
  let isResizing = false;
  let startX = 0;
  let startWidth = 0;

  handle.addEventListener('mousedown', function(e) {
    isResizing = true;
    startX = e.clientX;
    startWidth = sidebar.offsetWidth;
    handle.classList.add('dragging');
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
    e.preventDefault();
  });

  document.addEventListener('mousemove', function(e) {
    if (!isResizing) return;
    const diff = e.clientX - startX;
    let newWidth = startWidth + diff;
    if (newWidth < 200) newWidth = 200;
    if (newWidth > 480) newWidth = 480;
    sidebar.style.width = newWidth + 'px';
    markTruncatedRepoNames();
  });

  document.addEventListener('mouseup', function() {
    if (!isResizing) return;
    isResizing = false;
    sidebar.dataset.userResized = 'true';
    handle.classList.remove('dragging');
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  });
})();

/* ═══════════════════════════════════════════════════════════════
   MODAL CLOSE ON BACKDROP CLICK + ESCAPE KEY
   ═══════════════════════════════════════════════════════════════ */
document.getElementById('dispatch-modal').addEventListener('click', function(e) {
  if (e.target === this) App.closeDispatchModal();
});

document.addEventListener('keydown', function(e) {
  // Cmd+K / Ctrl+K — toggle command bar
  if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
    e.preventDefault();
    if (typeof CommandBar !== 'undefined') CommandBar.toggle();
    return;
  }
  if (e.key === 'Escape') {
    if (typeof CommandBar !== 'undefined' && CommandBar._visible) {
      CommandBar.hide();
      return;
    }
    App.closeDispatchModal();
  }
  if (e.key === 'Enter' && document.getElementById('auth-modal').style.display !== 'none') {
    App.submitAuthToken();
  }
});

/* ═══════════════════════════════════════════════════════════════
   PERIODIC LAST-POLL UPDATE
   ═══════════════════════════════════════════════════════════════ */
setInterval(function() {
  renderLastPoll();
}, 30000);

