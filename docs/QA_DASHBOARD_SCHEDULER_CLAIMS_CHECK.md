# Dashboard Scheduler And Claims Check

Date: 2026-03-15

Scope:
- local dashboard instance at `http://127.0.0.1:3112/?project=dx-terminal`
- scheduler history rail
- active launch leases rail

Method:
- verified live DXOS scheduler state through the local API
- seeded one scheduler run and one launch-claim record in the local DXOS control-plane store for rendering coverage
- captured Playwright screenshots against the live dashboard and confirmed the expected rows rendered

Observed UI data:
- scheduler rail showed run id `health-check-run-1`
- scheduler rail showed action `launch_attempted`
- active claims rail showed session `health-check-claim`
- active claims rail showed claim id `health-check-claim-1`

Result:
- the `Recent scheduler ticks` section rendered populated data correctly
- the `Active launch leases` section rendered populated data correctly

Notes:
- this was a local verification pass only
- no product code was changed as part of the check
