(function(){const t=document.createElement("link").relList;if(t&&t.supports&&t.supports("modulepreload"))return;for(const a of document.querySelectorAll('link[rel="modulepreload"]'))i(a);new MutationObserver(a=>{for(const n of a)if(n.type==="childList")for(const r of n.addedNodes)r.tagName==="LINK"&&r.rel==="modulepreload"&&i(r)}).observe(document,{childList:!0,subtree:!0});function s(a){const n={};return a.integrity&&(n.integrity=a.integrity),a.referrerPolicy&&(n.referrerPolicy=a.referrerPolicy),a.crossOrigin==="use-credentials"?n.credentials="include":a.crossOrigin==="anonymous"?n.credentials="omit":n.credentials="same-origin",n}function i(a){if(a.ep)return;a.ep=!0;const n=s(a);fetch(a.href,n)}})();function E(e){return console.debug("[ksu.mock] exec:",e),{errno:0,stdout:"",stderr:""}}function k(e){console.debug("[ksu.mock] toast:",e)}function L(){return{id:"anetd",name:"Anetd",version:"v0.1.0",versionCode:1}}var d=globalThis.ksu??{exec:E,toast:k,moduleInfo:L,fullScreen:()=>{}},c="/data/adb/modules/anetd",v=`${c}/log/anetd.pid`,f=`${c}/rules`,u="/data/adb/anetd/config.toml",I=`${c}/log/dns_off`;function l(e){return d.exec(e)}function R(){const e=l(`cat "${v}" 2>/dev/null`).stdout.trim(),t=e?parseInt(e,10):null;let s=!1,i="—";t&&!isNaN(t)&&(s=l(`ps -p ${t} -o pid= 2>/dev/null`).stdout.trim()===String(t),s&&(i=l(`ps -p ${t} -o etime= 2>/dev/null`).stdout.trim()||"running"));const a=l(`test -f "${I}" && echo 1 || echo 0`).stdout.trim();return{running:s,pid:s?t:null,dnsFilterEnabled:a!=="1",uptime:i}}function w(){const e=l(`ls "${f}" 2>/dev/null`).stdout.trim();if(!e)return[];const t=e.split(`
`).filter(Boolean),s=[];for(const i of t){const a=l(`cat "${`${f}/${i}`}" 2>/dev/null`).stdout.split(`
`);for(const n of a){const r=n.trim();if(!r){s.push({raw:n,type:"blank",original:n});continue}if(r.startsWith("!")){s.push({raw:n,type:"comment",original:n});continue}if(r.startsWith("[")){s.push({raw:n,type:"header",original:n});continue}const g=n.indexOf("!");if(g>0&&n[g-1]===" "){s.push({raw:n,type:"inline-comment",original:n});continue}if(r.startsWith("@@||")){s.push({raw:n,type:"allow",original:n});continue}if(r.startsWith("||")){s.push({raw:n,type:"block",original:n});continue}s.push({raw:n,type:"comment",original:n})}}return s}function p(){const e=l(`cat "${v}" 2>/dev/null`).stdout.trim(),t=parseInt(e,10);return!t||isNaN(t)?!1:l(`kill -HUP ${t} 2>/dev/null`).errno===0}function S(){return l(`sh "${c}/action.sh"`).errno===0}function h(){return l(`cat "${u}" 2>/dev/null`).stdout}function B(e){const t=`${u}.tmp`;return l(`printf '%s' '${e.replace(/\\/g,"\\\\").replace(/'/g,"'\\''")}' > "${t}" && mv "${t}" "${u}"`).errno!==0?!1:p()}function x(e=100){const t=l(`tail -n ${e} "${`${c}/log/anetd.log`}" 2>/dev/null`).stdout;return t?t.split(`
`):[]}function N(){const e=R(),t=e.running?'<span class="badge on">RUNNING</span>':'<span class="badge off">STOPPED</span>',s=e.dnsFilterEnabled?'<span class="badge on">ACTIVE</span>':'<span class="badge off">PAUSED</span>';return`
    <div class="page" id="page-dashboard">
      <h2>Dashboard</h2>

      <div class="card">
        <div class="card-title">Daemon Status</div>
        <div class="card-body">
          <div class="stat-row">
            <span class="label">Status</span>
            <span>${t}</span>
          </div>
          <div class="stat-row">
            <span class="label">PID</span>
            <span>${e.pid??"—"}</span>
          </div>
          <div class="stat-row">
            <span class="label">Uptime</span>
            <span>${e.uptime}</span>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">DNS Filter</div>
        <div class="card-body">
          <div class="stat-row">
            <span class="label">Adblock Filter</span>
            <span>${s}</span>
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
  `}function b(e){switch(e){case"block":return"rule-block";case"allow":return"rule-allow";case"comment":return"rule-comment";case"header":return"rule-header";case"inline-comment":return"rule-inline-comment";case"blank":return"rule-blank"}}function m(e){switch(e){case"block":return"BLOCK";case"allow":return"ALLOW";case"comment":return"#";case"header":return"HDR";case"inline-comment":return"+#";case"blank":return""}}function D(){const e=w();return`
    <div class="page" id="page-rules">
      <h2>Rules</h2>
      <p class="subtitle">${e.filter(t=>t.type==="block").length} block rules, ${e.filter(t=>t.type==="allow").length} allow rules loaded</p>
      <button class="btn" id="btn-reload-rules2">Refresh</button>
      <div class="rule-table-wrap">
        <table class="rule-table">
          <thead>
            <tr><th></th><th>Rule</th></tr>
          </thead>
          <tbody>${e.map(t=>`
      <tr class="${b(t.type)}">
        <td class="rule-badge">${m(t.type)?`<span class="mini-badge ${b(t.type)}">${m(t.type)}</span>`:""}</td>
        <td class="rule-text">${O(t.raw||" ")}</td>
      </tr>`).join("")||'<tr><td colspan="2" class="empty">No rules loaded</td></tr>'}</tbody>
        </table>
      </div>
    </div>
  `}function O(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}function F(){return`
    <div class="page" id="page-settings">
      <h2>Settings</h2>
      <p class="subtitle">Edit <code>/data/adb/anetd/config.toml</code></p>

      <div class="card">
        <div class="card-title">Configuration</div>
        <div class="card-body">
          <textarea id="config-editor" rows="12" spellcheck="false">${T(h())}</textarea>
          <div class="actions" style="margin-top:12px">
            <button class="btn" id="btn-save-config">Save & Reload</button>
            <button class="btn btn-secondary" id="btn-reset-config">Reset</button>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">Config Reference</div>
        <div class="card-body">
          <pre class="ref-block"># anetd config.toml
rules = "/data/adb/anetd/rules"
standalone = false      # daemon mode
multi_thread = true     # tokio multi-thread
dns_server = false      # built-in DNS server
dns_port = 53
dns_upstream = "8.8.8.8:53"
battery_saver = false</pre>
        </div>
      </div>
    </div>
  `}function T(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}function A(){return`
    <div class="page" id="page-logs">
      <h2>Logs</h2>
      <p class="subtitle">Recent log entries from anetd daemon</p>
      <button class="btn" id="btn-refresh-logs">Refresh</button>
      <button class="btn btn-secondary" id="btn-clear-logs" style="margin-left:8px">Clear Logs</button>
      <div class="log-viewer">
        ${x(200).map(e=>`<div class="log-line">${C(e)}</div>`).join("")||'<div class="empty">No log entries</div>'}
      </div>
    </div>
  `}function C(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}var y={dashboard:N,rules:D,settings:F,logs:A},P=document.getElementById("view"),$=document.getElementById("tabs");function o(e){P.innerHTML=y[e](),H(),_(e)}function _(e){$.querySelectorAll(".tab").forEach(t=>{const s=t;s.classList.toggle("active",s.dataset.page===e)})}function H(){document.getElementById("btn-toggle-filter")?.addEventListener("click",()=>{const e=S();d.toast(e?"Filter toggled":"Toggle failed"),o("dashboard")}),document.getElementById("btn-reload-rules")?.addEventListener("click",()=>{const e=p();d.toast(e?"Rules reloaded":"Reload failed"),o("dashboard")}),document.getElementById("btn-restart")?.addEventListener("click",()=>{const e=d.exec("kill -TERM $(cat /data/adb/modules/anetd/log/anetd.pid) 2>/dev/null; sleep 1; sh /data/adb/modules/anetd/post-fs-data.sh");d.toast(e.errno===0?"Restarted":"Restart failed"),setTimeout(()=>o("dashboard"),1500)}),document.getElementById("btn-reload-rules2")?.addEventListener("click",()=>{p(),o("rules")}),document.getElementById("btn-save-config")?.addEventListener("click",()=>{const e=document.getElementById("config-editor");if(!e)return;const t=B(e.value);d.toast(t?"Config saved & reloaded":"Save failed")}),document.getElementById("btn-reset-config")?.addEventListener("click",()=>{const e=document.getElementById("config-editor");e&&(e.value=h())}),document.getElementById("btn-refresh-logs")?.addEventListener("click",()=>{o("logs")}),document.getElementById("btn-clear-logs")?.addEventListener("click",()=>{d.exec(": > /data/adb/modules/anetd/log/anetd.log"),o("logs")})}$.addEventListener("click",e=>{const t=e.target.closest(".tab");if(!t)return;const s=t.dataset.page;s&&s in y&&o(s)});o("dashboard");
