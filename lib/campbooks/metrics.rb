# frozen_string_literal: true

module Campbooks
  # Prometheus application metrics (APM). Off by default — a no-op unless a
  # multi-process metrics dir is configured (the typical Prometheus setup) or it
  # is explicitly enabled, so the open-source / self-host build ships inert and
  # self-hosters running Prometheus can opt in with one ENV var.
  #
  # When on it provides:
  #   * HTTP RED metrics (rate/errors/duration by controller#action) — collected
  #     automatically by the yabeda-rails railtie once the gem is bundled.
  #   * custom campbooks_* background-job + domain-event metrics defined below.
  #   * a /metrics endpoint via yabeda-prometheus; multi-process safe via the
  #     Prometheus client's DirectFileStore (Puma + Solid Queue both fork).
  #
  # The hooks attach without touching unrelated code: an ActiveJob around_perform,
  # an Event after_create callback, and (in the Solid Queue worker, which has no
  # Puma to serve /metrics) a standalone metrics server.
  module Metrics
    module_function

    JOB_DURATION_BUCKETS = [ 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60, 120, 300 ].freeze

    # Default port for the Solid Queue worker's standalone metrics server.
    WORKER_METRICS_PORT = 9394

    # On when a multi-process dir is configured (the standard Prometheus setup, so
    # the hosted deployment switches it on for free) or explicitly enabled. ENV-only,
    # so this is safe to call without loading any of the heavy metrics deps.
    def enabled?
      present?(ENV["PROMETHEUS_MULTIPROC_DIR"]) || ENV["CAMPBOOKS_METRICS_ENABLED"].to_s == "1"
    end

    def install(app)
      configure_multiprocess
      define_metrics
      require "yabeda/prometheus/exporter"
      app.config.middleware.use Yabeda::Prometheus::Exporter
      install_job_metrics
      install_event_metrics(app)
      install_worker_metrics_server
    end

    # Every forked process (Puma workers, Solid Queue workers) writes to a shared
    # dir that the exporter aggregates on scrape. The deploy clears it on boot.
    def configure_multiprocess
      dir = ENV["PROMETHEUS_MULTIPROC_DIR"].to_s.strip
      return if dir.empty?

      require "fileutils"
      require "prometheus/client"
      require "prometheus/client/data_stores/direct_file_store"
      FileUtils.mkdir_p(dir)
      Prometheus::Client.config.data_store =
        Prometheus::Client::DataStores::DirectFileStore.new(dir: dir)
    end

    def define_metrics
      require "yabeda"
      Yabeda.configure do
        group :campbooks do
          counter :domain_events_total,
            comment: "Domain actions published through the Events bus.", tags: %i[event group]
          counter :job_executions_total,
            comment: "Background job runs by job class and outcome.", tags: %i[job status]
          histogram :job_duration,
            comment: "Background job execution time in seconds.",
            unit: :seconds, buckets: JOB_DURATION_BUCKETS, tags: %i[job status]
        end
      end
    end

    # Count + time every job, tagged by class and success/failure. Re-raises so
    # retry_on/discard_on still fire; the metric write can never break a job.
    def install_job_metrics
      ActiveSupport.on_load(:active_job) do
        around_perform do |job, block|
          started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
          status = "success"
          begin
            block.call
          rescue StandardError
            status = "failure"
            raise
          ensure
            begin
              labels = { job: job.class.name, status: status }
              Yabeda.campbooks.job_executions_total.increment(labels)
              Yabeda.campbooks.job_duration.measure(
                labels, Process.clock_gettime(Process::CLOCK_MONOTONIC) - started
              )
            rescue StandardError => e
              Rails.logger.error("[metrics] job metric failed: #{e.class}: #{e.message}")
            end
          end
        end
      end
    end

    # Count published domain events without touching Events::Publisher: hook the
    # Event record's creation (a publish always persists one). Bounded to the
    # registered event names (others bucket as "custom") so arbitrary names can't
    # blow up Prometheus cardinality. Idempotent across dev code reloads.
    def install_event_metrics(app)
      app.config.to_prepare do
        next unless defined?(::Event) && defined?(::Events::Registry)
        next if ::Event.respond_to?(:_campbooks_observed?) && ::Event._campbooks_observed?

        ::Event.after_create { |event| Campbooks::Metrics.record_event(event.name) }
        ::Event.define_singleton_method(:_campbooks_observed?) { true }
      end
    end

    def record_event(name)
      definition = ::Events::Registry.definition(name)
      Yabeda.campbooks.domain_events_total.increment(
        { event: definition ? name : "custom", group: (definition&.group || :custom).to_s }
      )
    rescue StandardError => e
      Rails.logger.error("[metrics] event metric failed: #{e.class}: #{e.message}") if defined?(::Rails)
    end

    # campbooks-worker runs Solid Queue (no Puma), so expose its own metrics
    # server from the supervisor; its forked workers write to the shared multiproc
    # dir above. The hook only fires where Solid Queue runs, so it never starts a
    # server inside the web process.
    def install_worker_metrics_server
      return unless defined?(::SolidQueue)

      ::SolidQueue.on_start do
        require "webrick" # standalone server backing the worker's /metrics
        require "yabeda/prometheus/exporter"
        Yabeda::Prometheus::Exporter.start_metrics_server!
      end
    end

    def present?(value)
      value.to_s.strip != ""
    end
  end
end
