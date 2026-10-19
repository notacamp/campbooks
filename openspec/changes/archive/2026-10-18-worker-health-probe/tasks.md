## 1. Probe

- [x] 1.1 The worker loop beats after a successful `SELECT 1` every 10 s
- [x] 1.2 A worker-only process serves `GET /healthz` on `CAMPBOOKS_JOBS__HEALTH_ADDR`
- [x] 1.3 `campbooks-server healthcheck --worker`

## 2. Tests and docs

- [x] 2.1 Liveness, a running worker beats, the endpoint and the probe follow it
- [x] 2.2 The self-hosting guide's worker health check
