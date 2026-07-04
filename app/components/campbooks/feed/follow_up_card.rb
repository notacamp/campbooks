# frozen_string_literal: true

module Campbooks
  module Feed
    # A conversation you replied to and are still waiting to hear back on — the AI
    # judged a follow-up worth sending and its time has come. Mirrors
    # ReplyReminderCard's quiet, borderless layout, but the framing is "they haven't
    # answered YOU" and the primary action is an AI-drafted nudge: it opens the
    # thread, where Scout leads with "Draft follow-up" into the preview→approve→send
    # composer. Dismiss retires the follow-up on the thread (not just this card).
    class FollowUpCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            div(class: "text-[12.5px] text-muted-foreground") do
              span(class: "font-semibold text-foreground") { sender_name(subject) }
              plain " · "
              plain meta
            end
            div(class: "mt-1 line-clamp-2 text-sm font-semibold leading-snug text-foreground") { clean_subject(subject) }
            if reason.present?
              p(class: "mt-1 line-clamp-2 text-[13px] leading-relaxed text-muted-foreground") { reason }
            end
            # "Nudge or let it rest" is judged on what was last said — the collapsed
            # preview reopens the conversation without leaving the feed.
            render Campbooks::Feed::ExpandablePreview.new(item: item, class: "mt-1.5")
            div(class: "mt-2.5 flex items-center justify-end gap-2") do
              act_button(tool: "dismiss_follow_up", label: t(".dismiss"), variant: :ghost, key: "x", dismiss: true)
              link_button(href: helpers.email_message_path(subject, compose: "follow_up"), label: t(".draft_follow_up"), variant: :primary, key: "d")
            end
          end
        end
      end

      private

      def reason = safe_text(item.data["reason"]).presence

      # "You replied 4 days ago · no reply yet" — the aging is the spine of the nudge.
      def meta
        days = item.data["age_days"].to_i
        days.positive? ? t(".aged", count: days) : t(".waiting")
      end

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground") do
          raw safe(send_icon)
        end
      end

      # A paper-plane — the nudge is something you send, distinct from the reply
      # reminder's clock.
      def send_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-[16px] w-[16px]"><path d="M22 2 11 13"/><path d="M22 2 15 22l-4-9-9-4 20-7z"/></svg>)
      end
    end
  end
end
