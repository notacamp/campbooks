# frozen_string_literal: true

module Campbooks
  module SystemHealth
    # Card representing one service's 24-hour health summary. Clicking the card
    # navigates to the call log filtered to that service.
    #
    # entry is a ::SystemHealth::Snapshot::ServiceStat struct.
    class ServiceCard < Campbooks::Base
      BADGE_VARIANTS = {
        healthy:  :success,
        degraded: :warning,
        failing:  :danger,
        idle:     :neutral
      }.freeze

      def initialize(entry:)
        @entry = entry
      end

      def view_template
        a(
          href: "#{helpers.admin_system_health_path(service: @entry.service)}#call-log",
          class: "block focus-visible:ring-2 focus-visible:ring-ring rounded-xl"
        ) do
          render(Campbooks::Card.new(padding: :md)) do
            service_name_and_badge
            sparkline_row
            stats_row
            last_error_row if @entry.last_error
          end
        end
      end

      private

      def service_name_and_badge
        div(class: "flex items-center justify-between gap-2") do
          span(class: "text-sm font-semibold") do
            plain t("system_health.services.#{@entry.service}", default: @entry.service.humanize)
          end
          render(Campbooks::Badge.new(variant: BADGE_VARIANTS.fetch(@entry.state, :neutral), size: :md)) do
            t(".state.#{@entry.state}")
          end
        end
      end

      def sparkline_row
        div(class: "mt-3") do
          render(Campbooks::Sparkline.new(points: sparkline_points, height: 32, label: sparkline_aria_label))
        end
      end

      def stats_row
        div(class: "mt-2 text-xs text-muted-foreground tabular-nums") do
          plain stats_line
        end
      end

      def last_error_row
        err = @entry.last_error
        div(class: "mt-3 pt-3 border-t border-border") do
          p(class: "text-xs text-muted-foreground") do
            meta = [ t(".last_error_ago", time: helpers.time_ago_in_words(err.at)) ]
            meta << (err.http_status ? err.http_status.to_s : err.error_class.to_s) if err.http_status || err.error_class
            plain meta.join(" · ")
          end
          if err.error_message.present?
            p(class: "mt-0.5 text-xs text-foreground/80 truncate", title: err.error_message) do
              plain err.error_message
            end
          end
        end
      end

      def sparkline_points
        @entry.buckets.map do |bucket|
          {
            value:    bucket.total,
            emphasis: bucket.errors,
            label:    t(".bucket_label",
                        time:   l(bucket.starts_at.in_time_zone, format: :clock),
                        calls:  t(".bucket_calls", count: bucket.total),
                        errors: t(".bucket_errors", count: bucket.errors))
          }
        end
      end

      def sparkline_aria_label
        t(".sparkline_label",
          total:  helpers.number_with_delimiter(@entry.total),
          errors: helpers.number_with_delimiter(@entry.errors))
      end

      def stats_line
        parts = [ t(".calls", count: @entry.total, formatted: helpers.number_with_delimiter(@entry.total)) ]
        parts << t(".error_rate", rate: "%.1f" % (@entry.error_rate * 100))
        parts << t(".avg_duration", ms: helpers.number_with_delimiter(@entry.avg_duration_ms.to_i)) if @entry.avg_duration_ms
        parts.join(" · ")
      end
    end
  end
end
