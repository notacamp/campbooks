# frozen_string_literal: true

module Campbooks
  module TimePage
    # One row of the bold Time agenda — a Time::AgendaItem, whatever it came from
    # (event / deadline / ask / focus), in one grammar: time · colour dot · title ·
    # muted provenance meta · the row's action(s). Asks add three ways out — Set a
    # date, Hold Scout's slot, Not now — plus Done. Mobile stacks the time above the
    # title and lets the buttons wrap; nothing overflows at 375px.
    class AgendaRow < Campbooks::Base
      # @param item [::Time::AgendaItem]
      # @param zone [ActiveSupport::TimeZone] for the HH:MM time column
      # @param move_slots [Array<Time>] the Move popover's alternatives (focus rows)
      # @param hold_slot [Time, nil] the slot "Hold" would take for an ask row
      def initialize(item:, zone:, move_slots: [], hold_slot: nil)
        @item = item
        @zone = zone
        @move_slots = Array(move_slots)
        @hold_slot = hold_slot
      end

      def view_template
        div(id: "agenda_#{@item.kind}_#{@item.record.id}",
            class: "flex flex-col gap-1.5 border-b border-border py-3 last:border-b-0 " \
                   "sm:grid sm:grid-cols-[64px_minmax(0,1fr)_auto] sm:items-center sm:gap-3") do
          time_cell
          body_cell
          actions_cell
        end
      end

      private

      def time_cell
        span(class: "font-mono text-xs tabular-nums text-muted-foreground") { time_text }
      end

      def time_text
        return t(".no_date") if @item.undated?
        return t(".all_day") if @item.all_day

        @item.at.in_time_zone(@zone).strftime("%H:%M")
      end

      def body_cell
        div(class: "min-w-0") do
          dot
          span(class: "text-sm font-semibold text-foreground") { @item.title }
          overdue_badge if @item.overdue
          meta_suffix
        end
      end

      # A 10px rounded square inline before the title, colour-coded by kind: the
      # calendar colour (event), the Ember gradient (a Scout deadline), a dashed
      # ring (a proposed focus block) or an ink outline (an ask).
      def dot
        base = "mr-2 inline-block h-2.5 w-2.5 shrink-0 rounded-[3px] align-[-1px]"
        case @item.kind
        when :event
          span(class: base, style: "background-color: #{@item.color}")
        when :deadline
          span(class: class_names(base, "bg-ember-gradient"))
        when :focus
          span(class: class_names(base, "border border-dashed border-muted-foreground"))
        else # :task
          span(class: class_names(base, "border border-foreground/50"))
        end
      end

      def meta_suffix
        text = [ lead_text, @item.source_label ].compact.reject(&:blank?).join(" · ")
        return if text.blank?

        span(class: "text-[12.5px] text-muted-foreground") { " · #{text}" }
      end

      def overdue_badge
        span(class: "ml-1.5 rounded bg-red-100 px-1.5 py-0.5 text-[11px] font-medium text-red-700 " \
                    "dark:bg-red-950 dark:text-red-300") { t(".overdue") }
      end

      # The bit before the source label: a duration for timed events/focus, "deadline"
      # for a (non-overdue) deadline, "ask" for an ask.
      def lead_text
        case @item.kind
        when :event, :focus
          @item.all_day ? nil : duration_label(@item.duration_minutes)
        when :deadline
          @item.overdue ? nil : t(".deadline")
        when :task
          t(".ask")
        end
      end

      def duration_label(mins)
        if mins < 60
          t(".minutes", count: mins)
        elsif (mins % 60).zero?
          t(".hours", count: mins / 60)
        else
          t(".hours_minutes", hours: mins / 60, minutes: mins % 60)
        end
      end

      # ── Actions ──────────────────────────────────────────────────────────────
      def actions_cell
        div(class: "flex flex-wrap items-center gap-2 sm:justify-end") do
          meet_button        if @item.action?(:meet)
          open_button        if @item.action?(:open_thread) || @item.action?(:open_document)
          schedule_popover   if @item.action?(:schedule)
          hold_button        if hold_as_button?
          done_form(@item.record)      if @item.action?(:done)
          done_form(@item.record.task) if @item.action?(:done_ask)
          move_popover       if @item.action?(:move)
          keep_form          if @item.action?(:keep)
          kebab              if kebab?
        end
      end

      def meet_button
        render Campbooks::Button.new(
          variant: :ghost, size: :xs,
          href: @item.record.join_url, target: "_blank", rel: "noopener noreferrer",
          class: "bg-secondary text-muted-foreground hover:bg-secondary/70"
        ) { t(".meet") }
      end

      def open_button
        label = @item.action?(:open_document) ? t(".open") : t(".open_thread")
        render Campbooks::Button.new(
          variant: :outline, size: :xs, href: @item.source_path, data: { turbo_frame: "_top" }
        ) { label }
      end

      # Done posts to the ask (a focus row's :done_ask completes its backing ask).
      def done_form(record)
        post_form(helpers.done_ask_path(record), method: :patch) do
          render(Campbooks::Button.new(variant: :outline, size: :xs, type: "submit")) { t(".done") }
        end
      end

      def keep_form
        post_form(helpers.keep_focus_block_path(@item.record)) do
          render(Campbooks::Button.new(variant: :primary, size: :xs, type: "submit")) { t(".keep") }
        end
      end

      # ── Ask: Set a date ────────────────────────────────────────────────────────
      # A details popover (undated rows). The kebab's "Change date" reuses #date_menu.
      def schedule_popover
        details(class: "relative", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex h-[26px] cursor-pointer list-none items-center rounded border border-input " \
                         "bg-background px-2 text-xs font-medium text-foreground shadow-sm hover:bg-accent " \
                         "[&::-webkit-details-marker]:hidden") { t(".set_a_date") }
          date_menu
        end
      end

      def date_menu
        div(class: "absolute right-0 z-20 mt-1 w-56 rounded-lg border border-border bg-card p-1 shadow-lg") do
          p(class: "px-2 py-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground") { t(".set_a_date") }
          %w[today tomorrow friday next_week].each { |preset| preset_form(preset) }
          date_pick_form
        end
      end

      def preset_form(preset)
        post_form(helpers.schedule_ask_path(@item.record), method: :patch, class: "block") do
          input(type: "hidden", name: "on", value: preset)
          menu_submit(t(".#{preset}"))
        end
      end

      # No-JS date picker: a native date input + a "Pick" submit, same form pattern
      # (the date input is named `on`, which the controller parses as an ISO date).
      def date_pick_form
        post_form(helpers.schedule_ask_path(@item.record), method: :patch,
                  class: "mt-1 flex items-center gap-1.5 border-t border-border px-2 py-1.5") do
          input(type: "date", name: "on",
                class: "min-w-0 flex-1 rounded-md border border-input bg-background px-1.5 py-1 text-[13px] text-foreground")
          button(type: :submit,
                 class: "shrink-0 rounded-md bg-foreground px-2 py-1 text-[12px] font-medium text-background hover:opacity-90") { t(".pick") }
        end
      end

      # ── Ask: Hold Scout's slot ─────────────────────────────────────────────────
      def hold_as_button? = @item.action?(:hold) && @item.undated?
      def hold_in_kebab?   = @item.action?(:hold) && !@item.undated?

      def hold_button
        post_form(helpers.hold_ask_path(@item.record)) do
          render(Campbooks::Button.new(variant: :outline, size: :xs, type: "submit", class: "gap-1.5")) do
            spark
            plain hold_label
          end
        end
      end

      def hold_label
        @hold_slot ? t(".hold_at", when: hold_when(@hold_slot)) : t(".hold")
      end

      def hold_when(slot)
        l(slot.in_time_zone(@zone), format: "%a %H:%M")
      end

      def spark
        span(class: "inline-flex", style: "color: var(--ember-solid)") { raw safe(Campbooks::ScoutNote::SPARK) }
      end

      # ── Move (focus rows) ──────────────────────────────────────────────────────
      def move_popover
        return if @move_slots.empty?

        details(class: "relative", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex h-[26px] cursor-pointer list-none items-center rounded border border-input " \
                         "bg-background px-2 text-xs font-medium text-foreground shadow-sm hover:bg-accent " \
                         "[&::-webkit-details-marker]:hidden") { t(".move") }
          div(class: "absolute right-0 z-20 mt-1 w-52 rounded-lg border border-border bg-card p-1 shadow-lg") do
            p(class: "px-2 py-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground") { t(".move_to") }
            @move_slots.each { |slot| move_slot_form(slot) }
          end
        end
      end

      def move_slot_form(slot)
        post_form(helpers.move_focus_block_path(@item.record), class: "block") do
          input(type: "hidden", name: "start_at", value: slot.iso8601)
          button(type: :submit,
                 class: "block w-full rounded-md px-2 py-1.5 text-left text-[13px] text-foreground hover:bg-muted") do
            slot_label(slot)
          end
        end
      end

      # ── Kebab ──────────────────────────────────────────────────────────────────
      def kebab?
        @item.action?(:change_date) || hold_in_kebab? || @item.action?(:snooze) ||
          @item.action?(:dismiss_ask) || @item.action?(:add_to_calendar) || @item.action?(:dismiss_focus)
      end

      def kebab
        details(class: "relative", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex h-[26px] w-[26px] cursor-pointer list-none items-center justify-center rounded " \
                         "text-muted-foreground hover:bg-muted [&::-webkit-details-marker]:hidden") do
            raw(safe(kebab_icon))
          end
          div(class: "absolute right-0 z-20 mt-1 w-52 rounded-lg border border-border bg-card p-1 shadow-lg") do
            change_date_item     if @item.action?(:change_date)
            hold_item            if hold_in_kebab?
            snooze_item          if @item.action?(:snooze)
            dismiss_ask_item     if @item.action?(:dismiss_ask)
            add_to_calendar_item if @item.action?(:add_to_calendar)
            dismiss_focus_item   if @item.action?(:dismiss_focus)
          end
        end
      end

      # "Change date" — the same date menu, as a nested popover inside the kebab.
      def change_date_item
        details(class: "relative", data: { controller: "dropdown-close" }) do
          summary(class: "block w-full cursor-pointer list-none rounded-md px-2 py-1.5 text-left text-[13px] " \
                         "text-foreground hover:bg-muted [&::-webkit-details-marker]:hidden") { t(".change_date") }
          date_menu
        end
      end

      def hold_item
        post_form(helpers.hold_ask_path(@item.record), class: "block") do
          button(type: :submit,
                 class: "flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-[13px] text-foreground hover:bg-muted") do
            spark
            plain hold_label
          end
        end
      end

      def snooze_item
        post_form(helpers.snooze_ask_path(@item.record), class: "block") { menu_submit(t(".not_now")) }
      end

      def dismiss_ask_item
        post_form(helpers.dismiss_ask_path(@item.record), class: "block") { menu_submit(t(".dismiss")) }
      end

      def add_to_calendar_item
        post_form(helpers.confirm_reminder_path(@item.record), class: "block") do
          menu_submit(t(".add_to_calendar"))
        end
      end

      def dismiss_focus_item
        post_form(helpers.dismiss_focus_block_path(@item.record), class: "block") do
          menu_submit(t(".dismiss"))
        end
      end

      def menu_submit(label)
        button(type: :submit, class: "block w-full rounded-md px-2 py-1.5 text-left text-[13px] text-foreground hover:bg-muted") { label }
      end

      # Shared Turbo POST/PATCH form (CSRF token + optional method override), matching
      # Campbooks::ReminderRow's pattern.
      def post_form(action, method: :post, **attrs, &block)
        form(action:, method: :post, **attrs) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "_method", value: method.to_s) unless method == :post
          block.call
        end
      end

      def slot_label(slot)
        local = slot.in_time_zone(@zone)
        "#{relative_day(local.to_date)} #{local.strftime('%H:%M')}"
      end

      def relative_day(date)
        today = ::Time.current.in_time_zone(@zone).to_date
        case date
        when today then t(".today")
        when today + 1 then t(".tomorrow")
        else l(date, format: :day_month)
        end
      end

      def kebab_icon
        %(<svg viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4"><circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/></svg>)
      end
    end
  end
end
