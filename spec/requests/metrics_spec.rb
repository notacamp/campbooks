require "rails_helper"

RSpec.describe "Prometheus metrics endpoint", type: :request do
  it "serves the metrics registry at /metrics without authentication" do
    get "/metrics"

    expect(response).to have_http_status(:ok)
    # Prometheus text exposition format, e.g. "text/plain; version=0.0.4".
    expect(response.content_type).to include("text/plain")
    # Declared at boot by yabeda-rails and config/initializers/yabeda.rb — their
    # presence proves the exporter middleware is wired and rendering the registry.
    expect(response.body).to include("campbooks_domain_events_total")
    expect(response.body).to include("campbooks_job_executions_total")
    # yabeda-rails' HTTP RED metrics (rails_requests_total, rails_request_duration_seconds,
    # …) only install under a real web server (puma), not under rspec — they're
    # verified by booting the server. See docs/observability.md.
  end
end
