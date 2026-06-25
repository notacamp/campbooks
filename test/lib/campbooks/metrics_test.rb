# frozen_string_literal: true

require "test_helper"

# Guards the ENV gate that decides whether application metrics activate. The
# module is loaded by config/initializers/metrics.rb at boot; #enabled? only reads
# ENV (the heavy yabeda/prometheus deps are required lazily inside #install), so
# these run without exercising the exporter.
class Campbooks::MetricsTest < ActiveSupport::TestCase
  test "enabled? is true when a multiprocess dir is configured" do
    with_env("PROMETHEUS_MULTIPROC_DIR" => "/tmp/cb-metrics", "CAMPBOOKS_METRICS_ENABLED" => nil) do
      assert Campbooks::Metrics.enabled?
    end
  end

  test "enabled? is true when explicitly flagged on" do
    with_env("PROMETHEUS_MULTIPROC_DIR" => nil, "CAMPBOOKS_METRICS_ENABLED" => "1") do
      assert Campbooks::Metrics.enabled?
    end
  end

  test "enabled? is false by default (no-op for open-source / self-host / test)" do
    with_env("PROMETHEUS_MULTIPROC_DIR" => nil, "CAMPBOOKS_METRICS_ENABLED" => nil) do
      assert_not Campbooks::Metrics.enabled?
    end
  end

  test "blank ENV values do not enable" do
    with_env("PROMETHEUS_MULTIPROC_DIR" => "  ", "CAMPBOOKS_METRICS_ENABLED" => "0") do
      assert_not Campbooks::Metrics.enabled?
    end
  end
end
