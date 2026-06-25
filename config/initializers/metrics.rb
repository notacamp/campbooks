# Prometheus application metrics (APM). See lib/campbooks/metrics.rb.
#
# Off unless PROMETHEUS_MULTIPROC_DIR is set (the standard multi-process
# Prometheus setup) or CAMPBOOKS_METRICS_ENABLED=1 — so this is inert for
# open-source / self-host builds that don't opt in, and a no-op in test/CI.
#
# Required explicitly (and Zeitwerk-ignored in config/application.rb) rather than
# autoloaded: it wires middleware during boot, before reloadable constants are
# safe to reference, and it's non-reloadable infrastructure.
require Rails.root.join("lib/campbooks/metrics").to_s

Campbooks::Metrics.install(Rails.application) if Campbooks::Metrics.enabled?
