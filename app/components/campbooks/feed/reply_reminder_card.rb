# frozen_string_literal: true

module Campbooks
  module Feed
    # A quiet nudge: mail you were expected to reply to but have sat on, or a
    # thread back from snooze. Lighter than the hero EmailActionCard (borderless,
    # muted, no Scout note, smaller subject), but it still *shows the email* —
    # sender + aging as the meta line, the subject as the spine, one line of the
    # message, so "reply or let it go" is a decision you can make at a glance
    # instead of a blind one. Reply opens the thread; Dismiss hides the nudge.
    class ReplyReminderCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            div(class: "text-[12.5px] text-muted-foreground") do
              span(class: "font-semibold text-foreground") { sender_name(subject) }
              plain " · "
              plain reminder_meta
            end
            div(class: "mt-1 line-clamp-2 text-sm font-semibold leading-snug text-foreground") { clean_subject(subject) }
            if (snippet = excerpt(subject, length: 140)).present?
              p(class: "mt-1 line-clamp-1 text-[13px] leading-relaxed text-muted-foreground") { snippet }
            end
            div(class: "mt-2.5 flex items-center justify-end gap-2") do
              dismiss_button(label: t(".dismiss"), variant: :ghost, key: "x")
              link_button(href: helpers.email_message_path(subject), label: t(".reply"), variant: :primary, key: "r")
            end
          end
        end
      end

      private

      def snooze_due? = item.data["reason"] == "snooze_due"

      # The aging line, now a tight meta phrase ("31 days, no reply") rather than a
      # full sentence with a dangling "this" — the subject below carries the content.
      def reminder_meta
        if snooze_due?
          t(".snoozed_back")
        else
          t(".aged", count: item.data["age_days"].to_i)
        end
      end

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground") do
          raw safe(snooze_due? ? alarm_icon : clock_icon)
        end
      end

      def clock_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-[17px] w-[17px]"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>)
      end

      def alarm_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-[17px] w-[17px]"><path d="M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16Z"/><path d="M12 9v4l2 2"/><path d="m5 3-2 2"/><path d="m21 5-2-2"/></svg>)
      end
    end
  end
end
