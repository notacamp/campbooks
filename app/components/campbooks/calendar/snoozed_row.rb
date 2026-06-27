module Campbooks
  module Calendar
    class SnoozedRow < Campbooks::Base
      def initialize(thread:)
        @thread = thread
      end

      def view_template
        a(href: helpers.email_thread_path(@thread),
          class: "-mx-3 flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors hover:bg-muted/50 no-underline") do
          span(class: "w-16 shrink-0 text-xs text-muted-foreground tabular-nums") { time_label }
          span(class: "h-2 w-2 shrink-0 rounded-full", style: "background-color: #8b5cf6")
          div(class: "min-w-0 flex-1") do
            span(class: "block truncate text-sm text-foreground") { @thread.display_subject.presence || t(".no_subject") }
            span(class: "block text-xs text-gray-400") { t(".snoozed_from", account: @thread.email_account.display_name) }
          end
          raw safe(snooze_icon)
        end
      end

      private

      def time_label
        l(@thread.snoozed_until, format: :clock)
      end

      def snooze_icon
        %(<svg class="w-3.5 h-3.5 shrink-0 text-purple-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>).html_safe
      end
    end
  end
end
