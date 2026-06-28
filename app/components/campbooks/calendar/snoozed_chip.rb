module Campbooks
  module Calendar
    class SnoozedChip < Campbooks::Base
      COLOR = "#8b5cf6".freeze

      def initialize(thread:, variant: :chip)
        @thread = thread
        @variant = variant
      end

      def view_template
        @variant == :row ? row : chip
      end

      private

      def row
        render Campbooks::Calendar::SnoozedRow.new(thread: @thread)
      end

      def chip
        a(href: helpers.email_thread_path(@thread),
          title: @thread.display_subject,
          class: "flex items-center gap-1 truncate rounded px-1.5 py-0.5 text-[10px] leading-tight no-underline sm:text-[11px]",
          style: "background-color: #{COLOR}; color: #{contrast_on(COLOR)}") do
          span(class: "truncate") { "#{l(@thread.snoozed_until, format: :clock)} #{@thread.display_subject}" }
        end
      end
    end
  end
end
