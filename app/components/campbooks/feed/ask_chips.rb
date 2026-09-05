# frozen_string_literal: true

module Campbooks
  module Feed
    # The ask chip row that rides an email's feed card (Campbooks::Feed::EmailActionCard):
    # while the email needs the reader, its ask is a chip under Scout's note, not a
    # card of its own. The chips POST to the email's feed act endpoint carrying the
    # ask's id (args[task_id]); the controller runs the ask action and replaces just
    # this row (id "ask_chips_<email id>") — the email card is never removed.
    #
    # Renders the live ask's current state: Hold Scout's slot + "Day on Time" (dated)
    # or Set a date (undated), collapsing to a resolved "Held …" chip once time is
    # held. The outer container renders even with no ask, so a later replace (e.g.
    # after a snooze drops the ask from the live set) leaves an empty, invisible row.
    class AskChips < Campbooks::Feed::Base
      def initialize(item:, subject:, **attrs)
        super
        @email = subject
        @user = Current.user
      end

      def view_template
        div(id: "ask_chips_#{@email.id}") do
          ask = live_ask
          chip_row(ask) if ask
        end
      end

      private

      def chip_row(ask)
        div(class: "mt-2.5 flex flex-wrap items-center gap-1.5") do
          block = ask.held_block
          if block
            held_chip(block)
          elsif hold_slot(ask)
            hold_chip(ask)
          end
          if ask.due_at
            on_time_chip(ask)
          elsif block.nil?
            set_date_details(ask)
          end
        end
      end

      # The most recent live ask sourced from any message in this email's thread.
      def live_ask
        return @live_ask if defined?(@live_ask)

        ids = @email.email_thread_id ? EmailMessage.where(email_thread_id: @email.email_thread_id).pluck(:id) : [ @email.id ]
        @live_ask = ::Task.accessible_to(@user).live
                          .where(source_type: "EmailMessage", source_id: ids)
                          .order(created_at: :desc).first
      end

      # ── Chips ──────────────────────────────────────────────────────────────────
      def hold_chip(ask)
        chip_act_form(tool: "hold_task", task: ask) do
          spark
          plain t("components.feed.email_action_card.hold_at", when: hold_when(ask))
        end
      end

      def held_chip(block)
        span(class: "inline-flex items-center gap-1.5 rounded-lg border border-green-600/40 px-2.5 py-1 " \
                    "text-[12px] font-medium text-green-700 dark:text-green-400") do
          raw safe(check_svg)
          plain t("components.feed.email_action_card.held", when: when_label(block.start_at))
        end
      end

      def on_time_chip(ask)
        a(href: helpers.time_path(date: ask.due_at.to_date.iso8601), data: { turbo_frame: "_top" },
          class: "inline-flex items-center gap-1.5 rounded-lg border border-border bg-transparent px-2.5 py-1 " \
                 "text-[12px] font-medium text-foreground no-underline hover:bg-muted") do
          raw safe(calendar_svg)
          plain t("components.feed.email_action_card.on_time", day: day_label(ask.due_at))
        end
      end

      def set_date_details(ask)
        details(class: "relative", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex cursor-pointer list-none items-center rounded-lg border border-border bg-transparent " \
                         "px-2.5 py-1 text-[12px] font-medium text-foreground hover:bg-muted [&::-webkit-details-marker]:hidden") do
            plain t("components.feed.email_action_card.set_a_date")
          end
          div(class: "absolute left-0 z-20 mt-1 w-56 rounded-lg border border-border bg-card p-2 shadow-lg") do
            div(class: "flex flex-wrap gap-1.5") do
              %w[today tomorrow friday next_week].each { |preset| preset_chip(ask, preset) }
            end
            date_pick_form(ask)
          end
        end
      end

      def preset_chip(ask, preset)
        chip_act_form(tool: "schedule_task", task: ask, extra: { "args[on]" => preset }) do
          plain t("components.feed.ask_card.#{preset}")
        end
      end

      def date_pick_form(ask)
        form(action: helpers.act_feed_item_path(item), method: :post, class: "mt-2 flex items-center gap-1.5", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "tool", value: "schedule_task")
          input(type: "hidden", name: "args[task_id]", value: ask.id)
          input(type: "date", name: "args[on]",
                class: "min-w-0 flex-1 rounded-md border border-input bg-background px-1.5 py-1 text-[13px] text-foreground")
          button(type: :submit,
                 class: "shrink-0 rounded-md bg-foreground px-2 py-1 text-[12px] font-medium text-background hover:opacity-90") { t("components.feed.ask_card.pick") }
        end
      end

      # A small ghost-chip act form on the EMAIL's feed item, carrying the ask id.
      def chip_act_form(tool:, task:, extra: {}, &block)
        fields = { "tool" => tool.to_s, "args[task_id]" => task.id }.merge(extra)
        form(action: helpers.act_feed_item_path(item), method: :post, class: "inline-flex", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          fields.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          render(Campbooks::Button.new(variant: :ghost, size: :xs, type: "submit", class: "gap-1.5")) { block.call }
        end
      end

      # ── Helpers ──────────────────────────────────────────────────────────────
      def hold_slot(ask)
        (@hold_slots ||= {})[ask.id] ||= @user ? Time::FocusHolder.preview(ask, user: @user) : nil
      end

      def hold_when(ask) = l(hold_slot(ask).in_time_zone(zone), format: "%a %H:%M")
      def when_label(time) = l(time.in_time_zone(zone), format: "%a %H:%M")
      def zone = @user&.effective_time_zone || Time.zone

      # "Friday" within the week, else a short date.
      def day_label(due_at)
        date = due_at.to_date
        today = Date.current
        date <= today + 6 && date >= today ? l(date, format: "%A") : l(date, format: :day_month)
      end

      def spark
        span(class: "inline-flex", style: "color: var(--ember-solid)") { raw safe(Campbooks::ScoutNote::SPARK) }
      end

      def check_svg
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="h-3 w-3"><path stroke-linecap="round" stroke-linejoin="round" d="M20 6 9 17l-5-5"/></svg>)
      end

      def calendar_svg
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="h-3 w-3"><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/></svg>)
      end
    end
  end
end
