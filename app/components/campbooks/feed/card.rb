# frozen_string_literal: true

module Campbooks
  module Feed
    # Dispatcher: renders the right card for a feed item, wrapped in the element
    # Feed::ItemsController targets for inline removal (id "feed_item_<id>").
    # Adding a kind to the feed = a Feed::Source + a card component + one entry here.
    #
    # The wrapper is the single hook for acting on a card three ways from ONE set of
    # tools (kept in #swipe_sides): the on-card buttons (a tap), the keyboard
    # (feed_keyboard_controller — → primary, ← escape, letters, on the focused card),
    # and mobile swipe (the Swipeable left/right panels). Desktop shows key chips on
    # the focused card; touch shows a swipe hint instead — see #focus_hints.
    class Card < Campbooks::Base
      CARDS = {
        "calendar_event"  => Campbooks::Feed::CalendarEventCard,
        "starred_email"   => Campbooks::Feed::StarredEmailCard,
        "email_action"    => Campbooks::Feed::EmailActionCard,
        "reply_reminder"  => Campbooks::Feed::ReplyReminderCard,
        "follow_up"       => Campbooks::Feed::FollowUpCard,
        "tag_suggestion"  => Campbooks::Feed::TagSuggestionCard,
        "reminder"        => Campbooks::Feed::ReminderCard,
        "task"            => Campbooks::Feed::TaskCard,
        "digest_issue"    => Campbooks::Feed::DigestCard
      }.freeze

      def initialize(item:, subject:)
        @item = item
        @subject = subject
      end

      def view_template
        component = CARDS[@item.kind]
        return unless component && @subject

        sides = swipe_sides

        # The Swipeable WRAPPER carries the id + data-feed-* hooks: id matches
        # Feed::ItemsController's dom_id(@item) so dismiss can turbo_stream.remove
        # this exact node (the whole slot + divider); data-feed-card is a stop for
        # keyboard navigation; data-feed-focus-unit is the unit the scroll-focus
        # dims/sharpens (and the keyboard cursor + chip anchor); data-feed-dismiss-url
        # is the ← fallback for any card with no escape button. left = swipe-left
        # escape (mirrors the ← button), right = swipe-right primary (mirrors →).
        render Campbooks::Swipeable.new(
          id: "feed_item_#{@item.id}",
          class: "animate-fade-in scroll-mt-24 py-9",
          data: { feed_card: true, feed_focus_unit: true, feed_dismiss_url: helpers.dismiss_feed_item_path(@item) },
          surface: "var(--color-background)",
          left: sides[:left],
          right: sides[:right]
        ) do
          render component.new(item: @item, subject: @subject)
          focus_hints(sides)
        end
      end

      private

      # The escape (swipe-left) + primary (swipe-right) actions per kind, as
      # Swipeable stages. They mirror the card's ← and → buttons exactly — same
      # endpoint, same params — so a swipe and a keypress do the identical thing.
      # A nil/[] side is inert (no panel, no commit). Navigation primaries (Open /
      # Reply / View) have no swipe-right: on touch you tap the card to open.
      def swipe_sides
        case @item.kind
        when "email_action"
          { left: [ archive_stage ], right: [ file_stage ].compact }
        when "starred_email"
          { left: [ archive_stage ], right: [] }
        when "reminder"
          { left: [ reminder_dismiss_stage ], right: [ reminder_confirm_stage ] }
        when "task"
          { left: [ feed_dismiss_stage ], right: [ task_complete_stage ] }
        when "tag_suggestion"
          { left: [ feed_dismiss_stage(t("components.feed.tag_suggestion_card.not_now")) ], right: [ tag_file_stage ] }
        when "follow_up"
          # Swipe-left retires the follow-up on the thread (not just this card);
          # the primary "Draft follow-up" is navigation (open the thread), so no swipe-right.
          { left: [ follow_up_dismiss_stage ], right: [] }
        else # reply_reminder, calendar_event, and any future read-only nudge
          { left: [ feed_dismiss_stage ], right: [] }
        end
      end

      def act_endpoint = helpers.act_feed_item_path(@item)

      def archive_stage
        { key: "archive", label: t("components.feed.email_action_card.archive"), icon: :archive,
          color: "neutral", endpoint: act_endpoint, params: { "tool" => "archive" } }
      end

      def file_stage
        tag = suggested_tag_name
        return nil unless tag

        { key: "file", label: t("components.feed.email_action_card.file_as", tag: tag), icon: :approve,
          color: "green", endpoint: act_endpoint, params: { "tool" => "add_tag", "args[tag_name]" => tag } }
      end

      def suggested_tag_name
        action = Array(@subject.try(:ai_suggested_actions)).find { |a| a["tool"] == "add_tag" }
        action&.dig("args", "tag_name").to_s.presence
      end

      def reminder_confirm_stage
        { key: "confirm", label: t("components.feed.reminder_card.confirm"), icon: :approve,
          color: "green", endpoint: act_endpoint, params: { "tool" => "confirm" } }
      end

      def reminder_dismiss_stage
        { key: "dismiss_reminder", label: t("components.feed.reminder_card.dismiss"), icon: :dismiss,
          color: "neutral", endpoint: act_endpoint, params: { "tool" => "dismiss_reminder" } }
      end

      def task_complete_stage
        { key: "complete", label: t("components.feed.task_card.complete"), icon: :approve,
          color: "green", endpoint: act_endpoint, params: { "tool" => "complete" } }
      end

      def tag_file_stage
        { key: "file", label: t("components.feed.tag_suggestion_card.file_it"), icon: :approve,
          color: "green", endpoint: act_endpoint,
          params: { "tool" => "add_tag", "args[tag_name]" => @item.data["tag_name"].to_s } }
      end

      def feed_dismiss_stage(label = t("components.swipeable.dismiss"))
        { key: "dismiss", label: label, icon: :dismiss, color: "neutral",
          endpoint: helpers.dismiss_feed_item_path(@item), params: {} }
      end

      # Follow-up dismiss goes through act (not the generic feed dismiss) so it
      # clears the thread's follow_up flag and can't regenerate next cycle.
      def follow_up_dismiss_stage
        { key: "dismiss_follow_up", label: t("components.feed.follow_up_card.dismiss"), icon: :dismiss,
          color: "neutral", endpoint: act_endpoint, params: { "tool" => "dismiss_follow_up" } }
      end

      # Hints shown only on the highlighted card (CSS-gated by pointer type, so they
      # take no space until then): a ↑↓ nav reminder for keyboards, and a swipe
      # direction hint for touch — where the chips don't apply and the gesture would
      # otherwise be invisible. The hint reuses each side's real action label
      # (Archive / File / Confirm…) so it matches what the swipe actually does; the
      # right half only shows when a swipe-right exists.
      def focus_hints(sides)
        div(data: { feed_keyhint: true }, class: "mt-3 items-center gap-1.5 px-0.5 text-[11px] text-muted-foreground") do
          kbd(class: "rounded border border-border px-1 font-mono text-[10px] leading-[1.4]") { "↑↓" }
          span { t("components.feed.shared.kbd_navigate") }
        end

        left_label = sides[:left].first&.dig(:label)
        right_label = sides[:right].first&.dig(:label)
        div(data: { feed_swipe_hint: true }, class: "mt-3 items-center gap-3 px-0.5 text-[11px] text-muted-foreground") do
          if left_label
            span(class: "inline-flex items-center gap-1") { plain "←"; plain left_label }
          end
          if right_label
            span(class: "inline-flex items-center gap-1") { plain right_label; plain "→" }
          end
        end
      end
    end
  end
end
