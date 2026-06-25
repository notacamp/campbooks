# frozen_string_literal: true

# OpenTelemetry distributed tracing. Auto-instruments HTTP requests, Active
# Record queries, outbound HTTP, Active Job, etc., and exports the spans via OTLP
# to a tracing backend (Grafana Tempo in the hosted deployment).
#
# Gated on OTEL_EXPORTER_OTLP_ENDPOINT so it's a complete no-op in dev, test, and
# self-hosted installs that don't run a tracing backend — only a deployment that
# points it at a collector/Tempo turns it on. The OTLP exporter reads the
# endpoint (and optional headers/protocol) from the standard OTEL_* env vars.
if ENV["OTEL_EXPORTER_OTLP_ENDPOINT"].present?
  require "opentelemetry/sdk"
  require "opentelemetry/instrumentation/all"
  require "opentelemetry/exporter/otlp"

  OpenTelemetry::SDK.configure do |c|
    c.service_name = ENV.fetch("OTEL_SERVICE_NAME", "campbooks")
    # Value must be a plain String (Rails.env is a StringInquirer, which the SDK's
    # attribute validation rejects).
    c.resource = OpenTelemetry::SDK::Resources::Resource.create(
      "deployment.environment" => Rails.env.to_s
    )
    # Enable every instrumentation whose underlying library is loaded; each is a
    # no-op if its gem isn't present, so this stays correct as dependencies change.
    c.use_all
  end
end
