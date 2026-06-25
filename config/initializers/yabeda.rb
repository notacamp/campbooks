# frozen_string_literal: true

# Application metrics collected by yabeda and exposed at the internal-only
# /metrics endpoint (see config/routes.rb) for Prometheus to scrape over the
# private network.
#
# yabeda-rails already registers HTTP RED metrics out of the box
# (rails_requests_total / rails_request_duration / rails_view_runtime /
# rails_db_runtime, tagged by controller#action#status). The metrics below add
# the domain-action and background-job signals that answer the operational
# question "are common actions succeeding, and how long do they take?".
#
# CARDINALITY DISCIPLINE: every tag here is bounded to a small, fixed set
# (registered event names, job class names, success/failure). NEVER tag a metric
# by workspace, user, email address, or any per-tenant value — Prometheus stores
# one time series per unique label combination, so an unbounded label will
# exhaust it. Use logs (Loki) or traces for per-tenant debugging instead.
Yabeda.configure do
  group :campbooks do
    counter :domain_events_total,
      comment: "Domain actions published through the Events bus (Events.publish).",
      tags: %i[event group]

    counter :job_executions_total,
      comment: "Background job runs by job class and outcome (success/failure).",
      tags: %i[job status]

    histogram :job_duration,
      comment: "Background job execution time in seconds.",
      unit: :seconds,
      buckets: [ 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60, 120, 300 ],
      tags: %i[job status]
  end
end
