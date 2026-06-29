module Campbooks
  module Calendar
    class ScheduledEmailRow < Campbooks::Base
      def initialize(scheduled_email:)
        @scheduled_email = scheduled_email
      end

      def view_template
        a(href: helpers.scheduled_email_path(@scheduled_email),
          class: "-mx-3 flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors hover:bg-muted/50 no-underline") do
          span(class: "w-16 shrink-0 text-xs text-muted-foreground tabular-nums") { time_label }
          span(class: "h-2 w-2 shrink-0 rounded-full", style: "background-color: #06b6d4")
          div(class: "min-w-0 flex-1") do
            span(class: "block truncate text-sm text-foreground") { @scheduled_email.subject }
            span(class: "block text-xs text-gray-400") { t(".to", address: helpers.truncate(@scheduled_email.to_address, length: 30)) }
          end
          raw safe(recurring_icon) if @scheduled_email.recurring?
        end
      end

      private

      def time_label
        l(@scheduled_email.display_time, format: :clock)
      end

      def recurring_icon
        %(<svg class="w-3.5 h-3.5 text-gray-300 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M17 2l4 4-4 4"/><path d="M3 11v-1a4 4 0 014-4h14"/><path d="M7 22l-4-4 4-4"/><path d="M21 13v1a4 4 0 01-4 4H3"/></svg>).html_safe
      end
    end
  end
end
