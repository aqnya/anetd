import { loadRules, type RuleEntry } from "../api/anetd";

function ruleClass(t: RuleEntry["type"]): string {
  switch (t) {
    case "block": return "rule-block";
    case "allow": return "rule-allow";
    case "comment": return "rule-comment";
    case "header": return "rule-header";
    case "inline-comment": return "rule-inline-comment";
    case "blank": return "rule-blank";
  }
}

function ruleLabel(t: RuleEntry["type"]): string {
  switch (t) {
    case "block": return "BLOCK";
    case "allow": return "ALLOW";
    case "comment": return "#";
    case "header": return "HDR";
    case "inline-comment": return "+#";
    case "blank": return "";
  }
}

export function renderRules(): string {
  const entries = loadRules();
  const blockCount = entries.filter((e) => e.type === "block").length;
  const allowCount = entries.filter((e) => e.type === "allow").length;

  const rows = entries
    .map(
      (e) => `
      <tr class="${ruleClass(e.type)}">
        <td class="rule-badge">${ruleLabel(e.type) ? `<span class="mini-badge ${ruleClass(e.type)}">${ruleLabel(e.type)}</span>` : ""}</td>
        <td class="rule-text">${escHtml(e.raw || " ")}</td>
      </tr>`
    )
    .join("");

  return `
    <div class="page" id="page-rules">
      <h2>Rules</h2>
      <p class="subtitle">${blockCount} block rules, ${allowCount} allow rules loaded</p>
      <button class="btn" id="btn-reload-rules2">Refresh</button>
      <div class="rule-table-wrap">
        <table class="rule-table">
          <thead>
            <tr><th></th><th>Rule</th></tr>
          </thead>
          <tbody>${rows || '<tr><td colspan="2" class="empty">No rules loaded</td></tr>'}</tbody>
        </table>
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
