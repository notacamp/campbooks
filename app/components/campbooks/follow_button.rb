# frozen_string_literal: true

module Campbooks
  # Follow / Following toggle for an email's discussion thread. Following means
  # you get notified of new activity (and, for a teammate pulled in by a mention,
  # it's what grants thread access). Posts to ThreadFollowsController and is
  # swapped in place via Turbo Stream (target id "follow_toggle").
  class FollowButton < Campbooks::Base
    def initialize(email_message:, following:)
      @email_message = email_message
      @following = following
    end

    def view_template
      form(
        id: "follow_toggle",
        action: helpers.follow_email_message_path(@email_message),
        method: "post",
        class: "inline-flex"
      ) do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        input(type: "hidden", name: "_method", value: "delete") if @following

        button(
          type: "submit",
          class: button_classes,
          aria_label: @following ? t(".unfollow_aria") : t(".follow_aria")
        ) do
          raw(safe(@following ? CHECK_ICON : PLUS_ICON))
          span { @following ? t(".following") : t(".follow") }
        end
      end
    end

    private

    def button_classes
      base = "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-medium cursor-pointer transition-colors"
      if @following
        "#{base} border border-border text-foreground hover:bg-muted"
      else
        "#{base} bg-accent-100 text-accent-700 hover:bg-accent-200 dark:bg-accent-500/15 dark:text-accent-200 dark:hover:bg-accent-500/25"
      end
    end

    CHECK_ICON = %(<svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.2"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"/></svg>)
    PLUS_ICON  = %(<svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>)
  end
end
