module Campbooks
  # Inline links back to where a reminder was extracted from — the email, or the
  # document plus the email it was attached to. Shared by the /reminders page row
  # and the home-feed card so the evidence is always one tap away.
  class ReminderSourceLinks < Campbooks::Base
    def initialize(reminder:, css: nil)
      @reminder = reminder
      @css = css
    end

    def view_template
      links = helpers.reminder_source_links(@reminder)
      return if links.empty?

      div(class: class_names("flex flex-wrap items-center gap-x-3 gap-y-1 text-xs", @css)) do
        links.each { |link| chip(link) }
      end
    end

    private

    def chip(link)
      a(href: link[:path], class: "inline-flex items-center gap-1 text-accent-700 hover:underline") do
        raw safe(icon(link[:kind]))
        span { link[:label] }
      end
    end

    def icon(kind)
      if kind == :email
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5"><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/></svg>)
      else
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5"><rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h3"/></svg>)
      end
    end
  end
end
