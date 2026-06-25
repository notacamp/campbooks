# frozen_string_literal: true

# Multi-process metrics. In production Campbooks runs across several processes:
# Puma forks WEB_CONCURRENCY workers (campbooks-web) and Solid Queue forks job
# workers in a separate container (campbooks-worker). Each fork has its own
# memory, so the default in-process metric store would make /metrics return only
# the responding process's numbers — counters would jump between workers and
# rate() would be wrong. The Prometheus client's DirectFileStore has every
# process write to a shared directory that the exporter aggregates at scrape
# time.
#
# No-op unless PROMETHEUS_MULTIPROC_DIR is set, so dev and test keep the simple
# in-memory store (existing specs are unaffected). bin/docker-entrypoint clears
# the directory on boot so stale per-pid files aren't double-counted.
if (multiproc_dir = ENV["PROMETHEUS_MULTIPROC_DIR"].presence)
  require "fileutils"
  require "prometheus/client"
  require "prometheus/client/data_stores/direct_file_store"

  FileUtils.mkdir_p(multiproc_dir)
  Prometheus::Client.config.data_store =
    Prometheus::Client::DataStores::DirectFileStore.new(dir: multiproc_dir)

  # campbooks-worker runs `rails solid_queue:start` — a separate container with
  # no Puma, so it has no /metrics route. Start yabeda's standalone metrics
  # server (defaults to 0.0.0.0:9394) from the Solid Queue supervisor so the job
  # metrics its forked workers write to the shared dir above are scrapable at
  # campbooks-worker:9394/metrics. The hook only fires where Solid Queue is the
  # process being run, so it never starts a server inside campbooks-web.
  if defined?(SolidQueue)
    SolidQueue.on_start do
      require "yabeda/prometheus/exporter"
      Yabeda::Prometheus::Exporter.start_metrics_server!
    end
  end
end
