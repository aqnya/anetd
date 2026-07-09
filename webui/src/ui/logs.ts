import { loadLogs } from "../api/anetd";

export function renderLogs(): string {
  const lines = loadLogs(200);

  const rows = lines
    .map((l) => `<div class="log-line">${escHtml(l)}</div>`)
    .join("");

  return `
    <div class="page" id="page-logs">
      <h2>Logs</h2>
      <p class="subtitle">Recent log entries from anetd daemon</p>
      <button class="btn" id="btn-refresh-logs">Refresh</button>
      <button class="btn btn-secondary" id="btn-clear-logs" style="margin-left:8px">Clear Logs</button>
      <div class="log-viewer">
        ${rows || '<div class="empty">No log entries</div>'}
      </div>
    </div>
  `;
}

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
