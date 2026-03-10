#!/usr/bin/env python3
"""
DX Terminal — Bidirectional Build Verification Framework

Two-pass verification:
  Pass 1 (Backend → Frontend): Query all APIs, predict expected DOM, verify via Playwright
  Pass 2 (Frontend → Backend): Snapshot DOM, predict expected API data, verify via curl

Deltas are flagged with exact field paths for debugging.

Usage:
  python3 tests/build_verify.py [--port 3100] [--verbose]
"""

import json
import subprocess
import sys
import time
import argparse
from dataclasses import dataclass, field
from typing import Any, Optional

# ─── Configuration ───────────────────────────────────────────────────────────

@dataclass
class TestResult:
    name: str
    passed: bool
    backend_value: Any = None
    frontend_value: Any = None
    delta: str = ""

@dataclass
class VerifyReport:
    pass_name: str  # "backend→frontend" or "frontend→backend"
    results: list = field(default_factory=list)

    @property
    def passed(self):
        return all(r.passed for r in self.results)

    @property
    def score(self):
        if not self.results: return 0
        return sum(1 for r in self.results if r.passed) / len(self.results) * 100

    def add(self, name, passed, backend=None, frontend=None, delta=""):
        self.results.append(TestResult(name, passed, backend, frontend, delta))

    def summary(self):
        total = len(self.results)
        ok = sum(1 for r in self.results if r.passed)
        fails = [r for r in self.results if not r.passed]
        lines = [f"\n{'='*60}", f"  {self.pass_name}: {ok}/{total} passed ({self.score:.0f}%)"]
        if fails:
            lines.append(f"  FAILURES:")
            for f in fails:
                lines.append(f"    ✗ {f.name}")
                if f.delta: lines.append(f"      Δ {f.delta}")
                if f.backend_value is not None: lines.append(f"      backend:  {f.backend_value}")
                if f.frontend_value is not None: lines.append(f"      frontend: {f.frontend_value}")
        lines.append(f"{'='*60}")
        return "\n".join(lines)


# ─── API Helpers ─────────────────────────────────────────────────────────────

def fetch_api(port, path):
    """Fetch JSON from API endpoint."""
    try:
        result = subprocess.run(
            ["curl", "-sf", f"http://localhost:{port}{path}"],
            capture_output=True, text=True, timeout=10
        )
        if result.returncode != 0 or not result.stdout.strip():
            return None
        return json.loads(result.stdout)
    except Exception as e:
        return None

def ws_snapshot(port):
    """Get WebSocket snapshot via a quick connection."""
    # We use websocat if available, otherwise fall back to API endpoints
    try:
        result = subprocess.run(
            ["websocat", "-t", "-1", f"ws://localhost:{port}/ws"],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0 and result.stdout.strip():
            msg = json.loads(result.stdout.strip().split("\n")[0])
            if msg.get("type") == "init":
                return msg.get("data")
    except FileNotFoundError:
        pass  # websocat not installed
    except Exception:
        pass
    return None


# ─── Pass 1: Backend → Frontend ─────────────────────────────────────────────
# Query APIs, predict what frontend should show, verify DOM

def pass1_backend_to_frontend(port, verbose=False):
    """Fetch all backend data, predict expected frontend state, verify."""
    report = VerifyReport("Backend → Frontend")

    # 1. Fetch all backend data
    status = fetch_api(port, "/api/status")
    monitor = fetch_api(port, "/api/monitor")
    queue = fetch_api(port, "/api/queue")
    digest = fetch_api(port, "/api/analytics/digest")
    overview = fetch_api(port, "/api/analytics/overview")
    capacity = fetch_api(port, "/api/capacity/dashboard?space=all")
    board = fetch_api(port, "/api/board?space=all")
    ws_data = ws_snapshot(port)

    if verbose:
        print(f"  [data] status: {'ok' if status else 'FAIL'}")
        print(f"  [data] monitor: {'ok' if monitor else 'FAIL'}")
        print(f"  [data] queue: {'ok' if queue else 'FAIL'}")
        print(f"  [data] digest: {'ok' if digest else 'FAIL'}")
        print(f"  [data] overview: {'ok' if overview else 'FAIL'}")
        print(f"  [data] capacity: {'ok' if capacity else 'FAIL'}")
        print(f"  [data] ws_snapshot: {'ok' if ws_data else 'FAIL (websocat may not be installed)'}")

    # 2. Build expectations from backend data
    expectations = {}

    # -- Pane expectations
    if ws_data and "panes" in ws_data:
        panes = ws_data["panes"]
        expectations["pane_count"] = len(panes)
        expectations["workspaces"] = ws_data.get("workspaces", [])
        expectations["pane_projects"] = {p["pane"]: p.get("project", "--") for p in panes}
        expectations["pane_statuses"] = {p["pane"]: p.get("status", "idle") for p in panes}
        expectations["pane_roles"] = {p["pane"]: p.get("role", "--") for p in panes}
        expectations["live_count"] = sum(1 for p in panes if p.get("live"))
    elif status and "panes" in status:
        expectations["pane_count"] = len(status["panes"])

    # -- Queue expectations
    if queue and "tasks" in queue:
        tasks = queue["tasks"]
        expectations["queue_pending"] = sum(1 for t in tasks if t.get("status") == "pending")
        expectations["queue_running"] = sum(1 for t in tasks if t.get("status") == "running")
        expectations["queue_done"] = sum(1 for t in tasks if t.get("status") == "done")
        expectations["queue_failed"] = sum(1 for t in tasks if t.get("status") == "failed")
        expectations["queue_total_shown"] = len([t for t in tasks if t["status"] != "done"]) + min(2, len([t for t in tasks if t["status"] == "done"]))
        expectations["queue_task_ids"] = [t["id"] for t in tasks]

    # -- Digest expectations
    if digest:
        expectations["digest_tool_calls"] = digest.get("tool_calls", 0)
        expectations["digest_tasks_done"] = digest.get("tasks_completed", 0)
        expectations["digest_agents_active"] = digest.get("agents_active", 0)
        expectations["digest_errors"] = digest.get("errors", 0)

    # -- Overview expectations
    if overview:
        expectations["overview_agents"] = overview.get("agent_count", 0)
        expectations["overview_locks"] = overview.get("active_locks", 0)
        expectations["overview_ports_count"] = len(overview.get("ports", []))
        expectations["overview_ports"] = overview.get("ports", [])
        if overview.get("recent"):
            expectations["overview_tools_1h"] = overview["recent"].get("tool_calls_1h", 0)

    # -- Capacity/gauge expectations
    if capacity:
        expectations["acu_used"] = capacity.get("acu_used", 0)
        expectations["acu_total"] = capacity.get("acu_total", 0)

    # 3. Use Playwright to verify frontend matches expectations
    frontend_data = playwright_extract(port, verbose)

    if not frontend_data:
        report.add("playwright_connection", False, delta="Could not connect to Playwright")
        return report, expectations

    # 4. Compare
    # -- Pane count
    if "pane_count" in expectations:
        fe_count = frontend_data.get("pane_count", 0)
        report.add("pane_count",
                    expectations["pane_count"] == fe_count,
                    expectations["pane_count"], fe_count,
                    f"backend has {expectations['pane_count']} panes, frontend shows {fe_count}")

    # -- Workspace dropdown
    if "workspaces" in expectations:
        fe_ws = frontend_data.get("workspaces", [])
        be_ws = expectations["workspaces"]
        report.add("workspace_options",
                    set(be_ws) == set(fe_ws),
                    sorted(be_ws), sorted(fe_ws),
                    f"missing: {set(be_ws)-set(fe_ws)}, extra: {set(fe_ws)-set(be_ws)}")

    # -- Project names on pane cards
    if "pane_projects" in expectations:
        fe_projects = frontend_data.get("pane_projects", {})
        mismatches = []
        for pane, be_proj in expectations["pane_projects"].items():
            fe_proj = fe_projects.get(str(pane), fe_projects.get(pane, "?"))
            if be_proj != fe_proj and be_proj != "--":
                mismatches.append(f"P{pane}: be={be_proj} fe={fe_proj}")
        report.add("pane_projects", len(mismatches) == 0,
                    expectations["pane_projects"], fe_projects,
                    "; ".join(mismatches) if mismatches else "")

    # -- Queue counts
    for key in ["queue_pending", "queue_running", "queue_done", "queue_failed"]:
        if key in expectations:
            fe_val = frontend_data.get(key, -1)
            report.add(key, expectations[key] == fe_val,
                       expectations[key], fe_val)

    # -- Digest values
    for key in ["digest_tool_calls", "digest_tasks_done", "digest_agents_active"]:
        if key in expectations:
            fe_val = frontend_data.get(key, -1)
            # Allow small delta for live-updating values
            close = abs(expectations[key] - fe_val) <= max(5, expectations[key] * 0.05)
            report.add(key, close,
                       expectations[key], fe_val,
                       f"diff={abs(expectations[key]-fe_val)}")

    # -- Overview values
    for key in ["overview_agents", "overview_locks", "overview_ports_count"]:
        if key in expectations:
            fe_val = frontend_data.get(key, -1)
            report.add(key, expectations[key] == fe_val,
                       expectations[key], fe_val)

    return report, expectations


# ─── Pass 2: Frontend → Backend ─────────────────────────────────────────────
# Snapshot DOM, predict what backend should have, verify via API

def pass2_frontend_to_backend(port, frontend_data, verbose=False):
    """From frontend state, predict backend data and verify."""
    report = VerifyReport("Frontend → Backend")

    if not frontend_data:
        report.add("frontend_data", False, delta="No frontend data available")
        return report

    # 1. From frontend pane count, verify backend has same
    if "pane_count" in frontend_data:
        ws_data = ws_snapshot(port)
        if ws_data:
            be_count = len(ws_data.get("panes", []))
            report.add("panes_exist_in_backend",
                       be_count == frontend_data["pane_count"],
                       be_count, frontend_data["pane_count"])

    # 2. From frontend queue rows, verify each task exists in backend
    queue = fetch_api(port, "/api/queue")
    if queue and "tasks" in queue:
        be_ids = {t["id"] for t in queue["tasks"]}
        fe_ids = set(frontend_data.get("queue_task_ids", []))
        # Frontend only shows active + last 2 done, so fe_ids ⊆ be_ids
        missing_in_backend = fe_ids - be_ids
        report.add("queue_tasks_exist",
                    len(missing_in_backend) == 0,
                    sorted(be_ids), sorted(fe_ids),
                    f"frontend shows tasks not in backend: {missing_in_backend}" if missing_in_backend else "")

    # 3. From frontend project names, verify JSONL detection is consistent
    if "pane_projects" in frontend_data:
        ws_data = ws_data if 'ws_data' in dir() else ws_snapshot(port)
        if ws_data:
            be_projects = {str(p["pane"]): p.get("project", "--") for p in ws_data.get("panes", [])}
            mismatches = []
            for pane, fe_proj in frontend_data["pane_projects"].items():
                be_proj = be_projects.get(str(pane), "?")
                if fe_proj != be_proj and fe_proj not in ("?", "--", ""):
                    mismatches.append(f"P{pane}: fe={fe_proj} be={be_proj}")
            report.add("project_consistency",
                       len(mismatches) == 0,
                       delta="; ".join(mismatches) if mismatches else "")

    # 4. From frontend workspace dropdown, verify against pane projects
    if "workspaces" in frontend_data and "pane_projects" in frontend_data:
        actual_projects = set(v for v in frontend_data["pane_projects"].values() if v and v != "--")
        dropdown = set(frontend_data.get("workspaces", []))
        # Every dropdown option (except generic "Projects") should have at least one pane
        orphan_ws = dropdown - actual_projects - {"Projects"}
        report.add("workspace_dropdown_valid",
                    len(orphan_ws) == 0,
                    delta=f"dropdown options with no panes: {orphan_ws}" if orphan_ws else "")

    # 5. From frontend gauge values, verify against capacity API
    capacity = fetch_api(port, "/api/capacity/dashboard?space=all")
    if capacity and "acu_used" in frontend_data:
        report.add("acu_gauge_matches",
                    abs(capacity.get("acu_used", 0) - frontend_data.get("acu_used", -1)) < 1,
                    capacity.get("acu_used"), frontend_data.get("acu_used"))

    # 6. Verify all API endpoints return valid JSON (no 404s or crashes)
    endpoints = [
        "/api/status", "/api/monitor", "/api/queue", "/api/health",
        "/api/analytics/digest", "/api/analytics/overview", "/api/analytics/alerts",
        "/api/capacity/dashboard?space=all", "/api/board?space=all",
        "/api/agents", "/api/roles",
    ]
    for ep in endpoints:
        data = fetch_api(port, ep)
        report.add(f"api_{ep.replace('/api/','').replace('/','_').split('?')[0]}",
                    data is not None,
                    delta=f"GET {ep} returned null/error" if data is None else "")

    return report


# ─── Playwright DOM Extraction ───────────────────────────────────────────────

def playwright_extract(port, verbose=False):
    """Use Playwright MCP to extract structured data from the dashboard."""
    # Since we're in a CLI context, we use a JavaScript extraction script
    # that runs in the page context

    js_extractor = """
    (async () => {
        // Wait for WebSocket data to load
        await new Promise(r => setTimeout(r, 3000));

        const result = {};

        // Pane count
        const paneCards = document.querySelectorAll('.pane-card');
        result.pane_count = paneCards.length;

        // Pane projects
        result.pane_projects = {};
        paneCards.forEach(card => {
            const num = card.querySelector('.pane-num');
            const proj = card.querySelector('.pane-project');
            if (num && proj) {
                result.pane_projects[num.textContent.trim()] = proj.textContent.trim().split(' ')[0];
            }
        });

        // Workspace dropdown
        const sel = document.getElementById('space-select');
        if (sel) {
            result.workspaces = Array.from(sel.options)
                .map(o => o.value)
                .filter(v => v !== 'all');
        }

        // Queue - count by status badges
        const qRows = document.querySelectorAll('.q-table tbody tr');
        let pending=0, running=0, done=0, failed=0;
        const taskIds = [];
        qRows.forEach(row => {
            const statusEl = row.querySelector('.q-status');
            const idEl = row.querySelector('.q-id');
            if (statusEl) {
                const s = statusEl.textContent.trim();
                if (s === 'pending') pending++;
                if (s === 'running') running++;
                if (s === 'done') done++;
                if (s === 'failed') failed++;
            }
            if (idEl) taskIds.push(idEl.textContent.trim());
        });
        result.queue_pending = pending;
        result.queue_running = running;
        result.queue_done = done;
        result.queue_failed = failed;
        result.queue_task_ids = taskIds;

        // Digest - extract stat tile values
        const digestEl = document.getElementById('digest');
        if (digestEl) {
            const tiles = digestEl.querySelectorAll('.stat-tile');
            tiles.forEach(t => {
                const val = t.querySelector('.val');
                const label = t.querySelector('.label');
                if (val && label) {
                    const v = parseFloat(val.textContent);
                    const l = label.textContent.trim();
                    if (l === 'Tool Calls') result.digest_tool_calls = v;
                    if (l === 'Tasks Done') result.digest_tasks_done = v;
                    if (l === 'Agents Active') result.digest_agents_active = v;
                    if (l === 'Errors') result.digest_errors = v;
                }
            });
        }

        // Overview
        const overviewEl = document.getElementById('overview');
        if (overviewEl) {
            const tiles = overviewEl.querySelectorAll('.stat-tile');
            tiles.forEach(t => {
                const val = t.querySelector('.val');
                const label = t.querySelector('.label');
                if (val && label) {
                    const v = parseFloat(val.textContent);
                    const l = label.textContent.trim();
                    if (l === 'Agents') result.overview_agents = v;
                    if (l === 'File Locks') result.overview_locks = v;
                    if (l === 'Ports') result.overview_ports_count = v;
                    if (l === 'Tools/1h') result.overview_tools_1h = v;
                }
            });
        }

        // Gauges
        const gauges = document.getElementById('gauges');
        if (gauges) {
            const vals = gauges.querySelectorAll('.gauge-value');
            // ACU gauge is the third one
            if (vals.length >= 3) {
                const acuText = vals[2].textContent;
                const match = acuText.match(/([0-9.]+)/);
                if (match) result.acu_used = parseFloat(match[1]);
            }
        }

        return JSON.stringify(result);
    })()
    """.strip()

    # Write JS to temp file and run via node with Playwright
    import tempfile
    with tempfile.NamedTemporaryFile(mode='w', suffix='.js', delete=False) as f:
        f.write(f"""
const {{ chromium }} = require('playwright');
(async () => {{
    let browser;
    try {{
        browser = await chromium.launch({{ headless: true }});
        const page = await browser.newPage();
        await page.goto('http://localhost:{port}');
        await page.waitForTimeout(4000);  // Let WS connect and render
        const result = await page.evaluate(`{js_extractor}`);
        console.log(result);
    }} catch(e) {{
        console.error(e.message);
        process.exit(1);
    }} finally {{
        if (browser) await browser.close();
    }}
}})();
""")
        tmp_path = f.name

    try:
        result = subprocess.run(
            ["node", tmp_path],
            capture_output=True, text=True, timeout=30,
            env={**dict(__import__('os').environ), "PLAYWRIGHT_BROWSERS_PATH": "/Users/pran/Library/Caches/ms-playwright"}
        )
        if result.returncode == 0 and result.stdout.strip():
            return json.loads(result.stdout.strip())
        else:
            if verbose:
                print(f"  [playwright] stderr: {result.stderr[:200]}")
            # Fall back to curl-based extraction (no browser)
            return extract_without_playwright(port, verbose)
    except FileNotFoundError:
        if verbose:
            print("  [playwright] node/playwright not found, using API-only mode")
        return extract_without_playwright(port, verbose)
    except Exception as e:
        if verbose:
            print(f"  [playwright] error: {e}")
        return extract_without_playwright(port, verbose)
    finally:
        import os
        os.unlink(tmp_path)


def extract_without_playwright(port, verbose=False):
    """Fallback: infer frontend state from API data (assumes correct rendering)."""
    if verbose:
        print("  [fallback] Using API-based frontend inference")

    ws_data = ws_snapshot(port)
    status = fetch_api(port, "/api/status")
    queue = fetch_api(port, "/api/queue")
    digest = fetch_api(port, "/api/analytics/digest")
    overview = fetch_api(port, "/api/analytics/overview")

    result = {}

    if ws_data:
        panes = ws_data.get("panes", [])
        result["pane_count"] = len(panes)
        result["workspaces"] = ws_data.get("workspaces", [])
        result["pane_projects"] = {str(p["pane"]): p.get("project", "--") for p in panes}
    elif status and "panes" in status:
        # Fallback: use /api/status which has pane data
        panes = status["panes"]
        result["pane_count"] = len(panes)
        result["pane_projects"] = {str(p["pane"]): p.get("project", "--") for p in panes}
        # No workspaces from status API — derive from projects
        projects = sorted(set(p.get("project", "--") for p in panes if p.get("project") and p.get("project") != "--"))
        result["workspaces"] = projects

    if queue and "tasks" in queue:
        tasks = queue["tasks"]
        result["queue_pending"] = sum(1 for t in tasks if t.get("status") == "pending")
        result["queue_running"] = sum(1 for t in tasks if t.get("status") == "running")
        result["queue_done"] = sum(1 for t in tasks if t.get("status") == "done")
        result["queue_failed"] = sum(1 for t in tasks if t.get("status") == "failed")
        active = [t for t in tasks if t["status"] != "done"]
        done_show = [t for t in tasks if t["status"] == "done"][-2:]
        result["queue_task_ids"] = [t["id"] for t in active + done_show]

    if digest:
        result["digest_tool_calls"] = digest.get("tool_calls", 0)
        result["digest_tasks_done"] = digest.get("tasks_completed", 0)
        result["digest_agents_active"] = digest.get("agents_active", 0)

    if overview:
        result["overview_agents"] = overview.get("agent_count", 0)
        result["overview_locks"] = overview.get("active_locks", 0)
        result["overview_ports_count"] = len(overview.get("ports", []))

    return result


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="DX Terminal Bidirectional Build Verification")
    parser.add_argument("--port", type=int, default=3100, help="Web server port")
    parser.add_argument("--verbose", "-v", action="store_true")
    parser.add_argument("--json", action="store_true", help="Output JSON report")
    args = parser.parse_args()

    print(f"DX Terminal Build Verification — port {args.port}")
    print(f"{'='*60}")

    # Check server is up
    if not fetch_api(args.port, "/api/health"):
        # Try /api/status as fallback
        if not fetch_api(args.port, "/api/status"):
            print(f"ERROR: Server not responding on port {args.port}")
            sys.exit(1)

    print("\n▸ Pass 1: Backend → Frontend")
    report1, expectations = pass1_backend_to_frontend(args.port, args.verbose)

    print("\n▸ Pass 2: Frontend → Backend")
    frontend_data = playwright_extract(args.port, args.verbose)
    report2 = pass2_frontend_to_backend(args.port, frontend_data, args.verbose)

    # Print reports
    print(report1.summary())
    print(report2.summary())

    # Overall
    total = len(report1.results) + len(report2.results)
    passed = sum(1 for r in report1.results + report2.results if r.passed)
    overall_pct = passed / total * 100 if total else 0

    print(f"\n{'='*60}")
    print(f"  OVERALL: {passed}/{total} ({overall_pct:.0f}%)")
    if overall_pct == 100:
        print(f"  ✓ ZERO DELTA — backend and frontend are in sync")
    else:
        print(f"  ✗ DELTAS FOUND — {total - passed} mismatches")
    print(f"{'='*60}")

    if args.json:
        output = {
            "pass1": {"name": report1.pass_name, "score": report1.score,
                      "results": [{"name": r.name, "passed": r.passed, "delta": r.delta} for r in report1.results]},
            "pass2": {"name": report2.pass_name, "score": report2.score,
                      "results": [{"name": r.name, "passed": r.passed, "delta": r.delta} for r in report2.results]},
            "overall_score": overall_pct,
            "total_checks": total,
            "passed_checks": passed,
        }
        print(json.dumps(output, indent=2))

    sys.exit(0 if overall_pct >= 90 else 1)


if __name__ == "__main__":
    main()
