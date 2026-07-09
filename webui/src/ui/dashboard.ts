import { getStatus, type AnetdStatus } from "../api/anetd";

export function renderDashboard(): string {
  const s: AnetdStatus = getStatus();

  const statusBadge = s.running
    ? '<span class="badge on">RUNNING</span>'
    : '<span class="badge off">STOPPED</span>';

  const filterBadge = s.dnsFilterEnabled
    ? '<span class="badge on">ACTIVE</span>'
    : '<span class="badge off">PAUSED</span>';

  return `
    <div class="page" id="page-dashboard">
      <h2>Dashboard</h2>

      <div class="card">
        <div class="card-title">Daemon Status</div>
        <div class="card-body">
          <div class="stat-row">
            <span class="label">Status</span>
            <span>${statusBadge}</span>
          </div>
          <div class="stat-row">
            <span class="label">PID</span>
            <span>${s.pid ?? "—"}</span>
          </div>
          <div class="stat-row">
            <span class="label">Uptime</span>
            <span>${s.uptime}</span>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">DNS Filter</div>
        <div class="card-body">
          <div class="stat-row">
            <span class="label">Adblock Filter</span>
            <span>${filterBadge}</span>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">Quick Actions</div>
        <div class="card-body actions">
          <button class="btn" id="btn-toggle-filter">Toggle Filter</button>
          <button class="btn" id="btn-reload-rules">Reload Rules</button>
          <button class="btn btn-danger" id="btn-restart">Restart Daemon</button>
        </div>
      </div>
    </div>
  `;
}
