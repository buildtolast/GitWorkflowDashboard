/* ═══════════════════════════════════════════════════════════════
   APPLICATION STATE
   ═══════════════════════════════════════════════════════════════ */
const State = {
  repos: [],
  repoStates: [],       // Array of { repo, workflows, active_runs, recent_runs }
  rateLimit: { limit: 0, remaining: 0, used: 0, reset: null },
  lastPoll: null,
  activeTab: 'active-runs',
  expandedRuns: {},      // { runId: { jobs: [...], loading: bool } }
  expandedJobs: {},      // { jobId: true }
  ws: null,
  wsReconnectTimer: null,
  wsReconnectDelay: 1000,
  wsMaxReconnectDelay: 30000,
  wsConnected: false,
  durationTimers: new Map(),  // runId -> intervalId
  sidebarCollapsed: false,
  dispatchContext: null,  // { repo, workflowId, workflowName }
  initialLoadDone: false,
  initOverlayDismissed: false,
  initEmptyTimer: null,
  workflowSearchQuery: '',
  actionsSearchQuery: '',
  historySearchQuery: '',
  historyTimeRange: 'all',       // 'all', '1h', '6h', '12h', '3d', '7d', 'custom'
  historyCustomFrom: null,
  historyCustomTo: null,
  whatsHappeningRange: '1h',     // '1h', '6h', '12h', '3d', '7d', 'custom'
  whatsHappeningCustomFrom: null,
  whatsHappeningCustomTo: null,
  analyticsRange: '7d',
  analyticsCustomFrom: null,
  analyticsCustomTo: null,
  repoSearchTimer: null,
  repoSearchResults: [],       // cached search results
  selectedSearchRepos: {},     // { full_name: true }
  addRepoMode: 'search',      // 'search' or 'manual'
  logViewerJobId: null,        // Currently displayed job in log viewer panel
  logRetryTimer: null,         // Auto-retry timer for in-progress job logs
  liveLogJobId: null,          // Job ID currently being live-tailed (null = not tailing)
  liveLogTimer: null,          // setInterval ID for live log polling
  liveLogPrevText: '',         // Full serialized text from last fetch (for diffing)
  liveLogAutoScroll: true,     // Auto-scroll to bottom on new content
  liveLogInterval: 3000,       // Current polling interval (ms) — adapts to rate limit
  liveLogStaleCount: 0,        // Consecutive polls with no change (for adaptive interval)
  expandedRunRefreshTimers: {},  // { runId: intervalId } for auto-refreshing in-progress run details
  theme: 'terminal',           // Current theme ID
  selectedSidebarRepos: {},      // { "owner/repo": true } — empty means all
  collapsedWorkflowGroups: {},   // { "owner/repo": true } — collapsed repo groups in workflows tab
  workflowGroupsInitialized: false, // true after first render sets all groups collapsed
  branchCache: {},               // { "owner/repo": "main" } — default branch cache
  selectedWorkflowKeys: {},      // { "owner/repo::workflow_id": true } — checkboxes in workflows tab
  customWorkflowTabs: [],        // [{ id, name, workflowKeys, stageByKey, pipelineConfigsByKey, manualApprovalAfterStage, slackNotificationByStage, agentConfig }]
  customTabRuntime: {},           // { [tabId]: { running, stageLabel, lastMessage, results, awaitingManual, awaitingStageNo, nextStageNo } }
  savedReposToRestore: [],       // repos loaded from localStorage on startup
  draggingCustomTabId: null,     // tab being dragged for reorder
  customTabGateResolvers: {},    // { [tabId]: () => void } — manual approval gate resolvers
  openInterStageConfigKey: null, // stage key whose inter-stage config panel is open
  globalPipelinePresets: {},     // { "preset_name": { branchByKey, inputsByKey } }
  slackTemplates: [],            // [{ name, webhookUrl }]
  slackConfig: {},               // { defaultWebhookUrl }
  workflowInputsByKey: {},       // { "owner/repo::wf_id": [...input_defs] } — cached workflow inputs
};

