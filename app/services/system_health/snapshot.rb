# frozen_string_literal: true

# Query object for the System Health dashboard. Runs at most 4 SQL queries and
# materializes per-service statistics + hourly bucket charts for a rolling window.
#
# Usage:
#   snapshot = SystemHealth::Snapshot.new(window: 24.hours)
#   snapshot.services   # => Array of ServiceStat structs, failing first
#   snapshot.totals     # => { total:, errors:, services_failing:, services_degraded: }
class SystemHealth::Snapshot
  # Named structs so Lookbook previews and component tests can build them
  # directly without a database.
  ServiceStat = Struct.new(
    :service, :group, :state, :total, :errors, :error_rate,
    :avg_duration_ms, :last_error, :last_success_at, :buckets,
    keyword_init: true
  )

  LastError = Struct.new(
    :at, :error_class, :error_message, :http_status, :operation,
    keyword_init: true
  )

  Bucket = Struct.new(:starts_at, :total, :errors, keyword_init: true)

  # State thresholds.
  FAILING_MIN_ERRORS      = 3
  FAILING_MIN_ERRORS_RATE = 5
  FAILING_RATE            = 0.5
  DEGRADED_RATE           = 0.05
  DEGRADED_MIN_ERRORS     = 3

  def initialize(window: 24.hours)
    @window = window
    @since  = window.ago
  end

  # Returns an Array of ServiceStat, sorted: failing first, then degraded,
  # then healthy, then idle; within each group sorted by total desc.
  def services
    @services ||= build_services
  end

  # Summary strip totals.
  def totals
    @totals ||= begin
      list = services
      {
        total:              list.sum(&:total),
        errors:             list.sum(&:errors),
        services_failing:   list.count { |s| s.state == :failing },
        services_degraded:  list.count { |s| s.state == :degraded }
      }
    end
  end

  private

  def build_services
    # 4 SQL queries total.
    aggregates       = load_aggregates         # Query 1
    buckets_by_svc   = load_buckets            # Query 2
    last_errors      = load_last_errors        # Query 3
    last_successes   = load_last_successes     # Query 4

    # All known services + any services seen in the window not in the registry.
    all_keys = (SystemHealth::SERVICES.keys + aggregates.keys).uniq

    entries = all_keys.map do |svc|
      agg      = aggregates[svc] || { total: 0, errors: 0, avg_duration_ms: nil }
      total    = agg[:total]
      errors   = agg[:errors]
      successes = total - errors
      error_rate = total.positive? ? errors.to_f / total : 0.0

      ServiceStat.new(
        service:        svc,
        group:          SystemHealth.group_for(svc),
        state:          compute_state(total, errors, successes, error_rate),
        total:          total,
        errors:         errors,
        error_rate:     error_rate,
        avg_duration_ms: agg[:avg_duration_ms],
        last_error:     last_errors[svc],
        last_success_at: last_successes[svc],
        buckets:        build_buckets_for(buckets_by_svc[svc] || [])
      )
    end

    entries.sort_by { |e| [ state_priority(e.state), -e.total ] }
  end

  STATE_PRIORITY = { failing: 0, degraded: 1, healthy: 2, idle: 3 }.freeze

  def state_priority(state)
    STATE_PRIORITY.fetch(state, 4)
  end

  def compute_state(total, errors, successes, error_rate)
    return :idle     if total.zero?
    return :failing  if errors >= FAILING_MIN_ERRORS && successes.zero?
    return :failing  if error_rate >= FAILING_RATE && errors >= FAILING_MIN_ERRORS_RATE
    return :degraded if error_rate >= DEGRADED_RATE && errors >= DEGRADED_MIN_ERRORS

    :healthy
  end

  # Query 1 — grouped aggregate per service.
  def load_aggregates
    ExternalServiceCall
      .since(@since)
      .group(:service)
      .select(
        :service,
        "COUNT(*) AS total",
        "SUM(CASE WHEN status = 1 THEN 1 ELSE 0 END) AS error_count",
        "ROUND(AVG(duration_ms))::int AS avg_dur"
      )
      .map { |r| [ r.service, { total: r.total.to_i, errors: r.error_count.to_i, avg_duration_ms: r.avg_dur } ] }
      .to_h
  end

  # Query 2 — hourly bucket counts per service.
  def load_buckets
    rows = ExternalServiceCall
      .since(@since)
      .group(:service, Arel.sql("date_trunc('hour', created_at AT TIME ZONE 'UTC')"))
      .select(
        :service,
        "date_trunc('hour', created_at AT TIME ZONE 'UTC') AS hour",
        "COUNT(*) AS total",
        "SUM(CASE WHEN status = 1 THEN 1 ELSE 0 END) AS error_count"
      )

    result = Hash.new { |h, k| h[k] = [] }
    rows.each do |r|
      result[r.service] << { starts_at: r.hour.utc, total: r.total.to_i, errors: r.error_count.to_i }
    end
    result
  end

  # Query 3 — last error per service using DISTINCT ON.
  def load_last_errors
    ExternalServiceCall
      .since(@since)
      .where(status: :error)
      .select("DISTINCT ON (service) service, created_at, error_class, error_message, http_status, operation")
      .order(Arel.sql("service, created_at DESC"))
      .each_with_object({}) do |r, h|
        h[r.service] = LastError.new(
          at:            r.created_at,
          error_class:   r.error_class,
          error_message: r.error_message,
          http_status:   r.http_status,
          operation:     r.operation
        )
      end
  end

  # Query 4 — last success timestamp per service using DISTINCT ON.
  def load_last_successes
    ExternalServiceCall
      .since(@since)
      .where(status: :success)
      .select("DISTINCT ON (service) service, created_at")
      .order(Arel.sql("service, created_at DESC"))
      .each_with_object({}) do |r, h|
        h[r.service] = r.created_at
      end
  end

  # Generates exactly 24 hourly Bucket structs covering the window, oldest
  # first, zero-filled for hours with no data.
  def build_buckets_for(raw_buckets)
    now_hour = Time.current.utc.beginning_of_hour

    slots = 24.times.map do |i|
      starts_at = now_hour - (23 - i) * 1.hour
      Bucket.new(starts_at: starts_at, total: 0, errors: 0)
    end

    raw_by_hour = raw_buckets.index_by { |b| b[:starts_at].utc.beginning_of_hour }

    slots.each do |slot|
      bucket = raw_by_hour[slot.starts_at]
      next unless bucket

      slot.total  = bucket[:total]
      slot.errors = bucket[:errors]
    end

    slots
  end
end
