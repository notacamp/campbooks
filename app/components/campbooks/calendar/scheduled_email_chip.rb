module Campbooks
  module Calendar
    class ScheduledEmailChip < Campbooks::Base
      COLOR = "#06b6d4".freeze

      def initialize(scheduled_email:, variant: :chip)
        @scheduled_email = scheduled_email
        @variant = variant
      end

      def view_template
        @variant == :row ? row : chip
      end

      private

      def row
        render Campbooks::Calendar::ScheduledEmailRow.new(scheduled_email: @scheduled_email)
      end

      def chip
        a(href: helpers.scheduled_email_path(@scheduled_email),
          title: @scheduled_email.subject,
          class: "flex items-center gap-1 truncate rounded px-1.5 py-0.5 text-[10px] leading-tight no-underline sm:text-[11px]",
          style: "background-color: #{COLOR}; color: #{contrast_on(COLOR)}") do
          span(class: "truncate") { "#{l(@scheduled_email.display_time, format: :clock)} #{@scheduled_email.subject}" }
        end
      end
    end
  end
end
