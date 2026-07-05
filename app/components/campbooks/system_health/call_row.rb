# frozen_string_literal: true

module Campbooks
  module SystemHealth
    # Dense list row for one ExternalServiceCall. Used in the call log section
    # of the system health dashboard.
    class CallRow < Campbooks::Base
      TIMEOUT_CLASSES = %w[
        Net::ReadTimeout Net::OpenTimeout Faraday::TimeoutError
        Errno::ETIMEDOUT Timeout::Error
      ].freeze

      def initialize(call:, show_workspace: true)
        @call           = call
        @show_workspace = show_workspace
      end

      def view_template
        div(class: "px-4 py-2.5") do
          line_one
          line_two if show_error_line?
        end
      end

      private

      def line_one
        div(class: "flex items-center gap-x-3 text-sm flex-wrap") do
          status_badge
          service_name
          operation_cell
          duration_cell
          timestamp_cell
        end
      end

      def status_badge
        if @call.status_success?
          render(Campbooks::Badge.new(variant: :success, size: :md)) do
            plain @call.http_status ? @call.http_status.to_s : "OK"
          end
        else
          label = timeout? ? "timeout" : (@call.http_status ? @call.http_status.to_s : "error")
          title_attr = @call.error_class.present? ? @call.error_class : nil
          render(Campbooks::Badge.new(variant: :danger, size: :md, title: title_attr)) { plain label }
        end
      end

      def service_name
        span(class: "shrink-0 font-medium text-xs") do
          plain t("system_health.services.#{@call.service}", default: @call.service.humanize)
        end
      end

      def operation_cell
        op = @call.operation.presence || t(".no_operation")
        span(
          class: "font-mono text-xs text-muted-foreground truncate flex-1 min-w-0 w-full order-last sm:w-auto sm:order-none",
          title: @call.operation.to_s
        ) { plain op }
      end

      def duration_cell
        span(class: "text-xs tabular-nums text-muted-foreground shrink-0") do
          plain @call.duration_ms ? "#{helpers.number_with_delimiter(@call.duration_ms)} ms" : "—"
        end
      end

      def timestamp_cell
        span(
          class: "text-xs text-muted-foreground shrink-0",
          title: l(@call.created_at, format: :at)
        ) { plain l(@call.created_at, format: :at_short) }
      end

      def line_two
        parts = []
        if @call.error_class.present? || @call.error_message.present?
          err_parts = []
          err_parts << @call.error_class   if @call.error_class.present?
          err_parts << @call.error_message if @call.error_message.present?
          parts << err_parts.join(": ")
        end
        parts << "· #{@call.workspace.name}" if @show_workspace && @call.workspace.present?

        full_text = parts.join(" ")
        p(
          class: "mt-1 pl-0 text-xs text-muted-foreground truncate",
          title: full_text
        ) { plain full_text }
      end

      def show_error_line?
        @call.status_error? && (
          @call.error_class.present? ||
          @call.error_message.present? ||
          (@show_workspace && @call.workspace.present?)
        )
      end

      def timeout?
        @call.error_class.present? && TIMEOUT_CLASSES.any? { |klass| @call.error_class.include?(klass) }
      end
    end
  end
end
