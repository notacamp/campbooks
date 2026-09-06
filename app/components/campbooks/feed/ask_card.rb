# frozen_string_literal: true

module Campbooks
  module Feed
    # An ask on the home feed — a card of its own, shown only when the ask needs a
    # decision from the reader (a fresh suggestion whose email has no card, an
    # accepted ask with no date, or one due today / overdue). The only question it
    # asks is *when*: Hold Scout's free slot, Set a date, or Not now — except a due
    # ask, whose primary is simply Done.
    #
    # Borderless nudge shape like ReminderCard. `subject` is a Task; `framing`
    # ("suggested" / "undated" / "due") is stamped into the item's data by
    # Feed::Sources::Task, so the card never recomputes it.
    class AskCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            kicker
            title
            detail_line
            source_email_preview
            scout_note
            action_bar
          end
        end
      end

      private

      # ── Kicker ─────────────────────────────────────────────────────────────────
      def kicker
        div(class: "flex flex-wrap items-center gap-x-1.5 text-[12.5px]") do
          if framing == "suggested"
            span(class: "font-medium text-foreground") { t(".scout_found") }
            dot_sep
          end
          span(class: "font-medium text-foreground") { t(".ask") }
          dot_sep
          date_segment
          provenance_segment
        end
      end

      def date_segment
        if subject.due_at.nil?
          span(class: "font-medium text-muted-foreground") { t(".no_date") }
        elsif overdue?
          span(class: "font-medium text-red-600 dark:text-red-500") { t(".overdue") }
        elsif due_today?
          span(class: "font-medium text-muted-foreground") { t(".due_today") }
        else
          span(class: "font-medium text-muted-foreground") { t(".by", date: l(subject.due_at.to_date, format: :long)) }
        end
      end

      # from_email when there's a source email; else "Scout found it" for an AI ask
      # (unless the suggested lead already said so); else nothing.
      def provenance_segment
        if subject.source_email
          dot_sep
          span(class: "text-muted-foreground") { t(".from_email", name: Emails::SenderName.first_name(subject.source_email.from_address)) }
        elsif subject.ai_suggested? && framing != "suggested"
          dot_sep
          span(class: "text-muted-foreground") { t(".scout_found") }
        end
      end

      def dot_sep
        span(class: "text-muted-foreground/50") { "·" }
      end

      # ── Title + detail ─────────────────────────────────────────────────────────
      def title
        if subject.source_email
          a(href: helpers.email_message_path(subject.source_email),
            class: "mt-1 block truncate text-sm font-semibold leading-snug text-foreground hover:underline") { subject.title }
        else
          div(class: "mt-1 truncate text-sm font-semibold leading-snug text-foreground") { subject.title }
        end
      end

      def detail_line
        if subject.justification.present?
          p(class: "mt-0.5 line-clamp-1 text-[12.5px] italic text-muted-foreground") { subject.justification }
        elsif subject.description.present?
          p(class: "mt-0.5 line-clamp-1 text-[13px] text-muted-foreground") { subject.description }
        end
      end

      def source_email_preview
        return unless subject.source_email

        render Campbooks::Feed::ExpandablePreview.new(
          item: item, label: t("components.feed.expandable_preview.show_source"), class: "mt-1.5"
        )
      end

      # Compact Scout note only when there's something honest to say: a free slot to
      # hold. Otherwise omitted entirely (no filler).
      def scout_note
        return unless decision_framing? && slot

        render Campbooks::ScoutNote.new(
          message: t(".scout_slot", when: slot_long, minutes: Time::FocusHolder::DURATION_MINUTES),
          compact: true, lines: 2, class: "mt-2"
        )
      end

      # ── Actions ────────────────────────────────────────────────────────────────
      def action_bar
        div(class: "mt-2.5 flex flex-wrap items-center justify-end gap-2") do
          due_framing? ? due_actions : decision_actions
        end
      end

      # Due today / overdue: hide the card (the ask stays) · Open thread · Done.
      def due_actions
        dismiss_button(label: t(".not_now"), key: "x")
        if subject.source_email
          link_button(href: helpers.email_message_path(subject.source_email),
                      label: t(".open_thread"), variant: :outline, key: "o", primary: false)
        end
        act_button(tool: "complete", label: t(".complete"), variant: :primary, key: "c", primary: true)
      end

      # Suggested / undated: Not now (snooze) · Set a date · Hold the slot (primary),
      # or — when no slot is free — the Tomorrow preset as the primary.
      def decision_actions
        act_button(tool: "snooze_task", label: t(".not_now"), variant: :ghost, key: "x", dismiss: true)
        set_date_details
        if slot
          hold_button
        else
          act_button(tool: "schedule_task", args: { on: "tomorrow" }, label: t(".tomorrow_on_time"),
                     variant: :primary, key: "c", primary: true)
        end
      end

      # "Set a date": a details popover of preset chips + a no-JS date picker.
      def set_date_details
        details(class: "relative", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex cursor-pointer list-none items-center rounded-lg border border-border bg-transparent " \
                         "px-3 py-1.5 text-[13px] font-medium text-foreground hover:bg-muted [&::-webkit-details-marker]:hidden") { t(".set_a_date") }
          div(class: "absolute right-0 z-20 mt-1 w-56 rounded-lg border border-border bg-card p-2 shadow-lg") do
            div(class: "flex flex-wrap gap-1.5") do
              %w[today tomorrow friday next_week].each { |preset| schedule_chip(preset) }
            end
            date_pick_form
          end
        end
      end

      def schedule_chip(preset)
        act_button(tool: "schedule_task", args: { on: preset }, label: t(".#{preset}"), variant: :ghost, size: :xs)
      end

      def date_pick_form
        form(action: helpers.act_feed_item_path(item), method: :post, class: "mt-2 flex items-center gap-1.5", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "tool", value: "schedule_task")
          input(type: "date", name: "args[on]",
                class: "min-w-0 flex-1 rounded-md border border-input bg-background px-1.5 py-1 text-[13px] text-foreground")
          button(type: :submit,
                 class: "shrink-0 rounded-md bg-foreground px-2 py-1 text-[12px] font-medium text-background hover:opacity-90") { t(".pick") }
        end
      end

      # The primary "Hold <slot>" — Scout's suggestion, so it carries the Ember spark.
      def hold_button
        action_form(helpers.act_feed_item_path(item), fields: { tool: "hold_task" }) do
          render Campbooks::Button.new(
            variant: :primary, size: :sm, type: "submit", class: "gap-1.5",
            data: feed_action_attrs(key: "c", primary: true).merge(turbo_submits_with: t("components.feed.shared.working"))
          ) do
            span(class: "inline-flex") { raw safe(Campbooks::ScoutNote::SPARK) }
            plain t(".hold_at", when: slot_short)
            key_chip(key: "c", primary: true)
          end
        end
      end

      # ── Helpers ────────────────────────────────────────────────────────────────
      def framing = item.data["framing"].to_s
      def decision_framing? = %w[suggested undated].include?(framing)
      def due_framing? = framing == "due"
      def overdue? = subject.due_at.present? && subject.due_at < Time.current
      def due_today? = subject.due_at.present? && subject.due_at.to_date == Date.current

      # The slot Hold would take, request-cached via Time::BusyIntervals. Only for
      # decision framings, and only when a user is in context (nil in Lookbook).
      def slot
        return @slot if defined?(@slot)

        @slot = (decision_framing? && Current.user) ? Time::FocusHolder.preview(subject, user: Current.user) : nil
      end

      def slot_short = l(slot.in_time_zone(zone), format: "%a %H:%M")
      def slot_long  = l(slot.in_time_zone(zone), format: "%A %H:%M")
      def zone = Current.user&.effective_time_zone || Time.zone

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-ember/10 text-ember") do
          raw safe(check_icon)
        end
      end

      def check_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="h-4 w-4"><path stroke-linecap="round" stroke-linejoin="round" d="M9 11l3 3L20 6"/></svg>)
      end
    end
  end
end
