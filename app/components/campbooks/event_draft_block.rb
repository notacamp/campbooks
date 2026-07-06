# frozen_string_literal: true

module Campbooks
  # Inline drafted event block shown on the email show page when Scout detects
  # a scheduling proposal in the email body (via Ai::EventExtractor). Presents
  # the extracted event title and time range with two actions: Edit (opens the
  # full calendar form pre-filled) and Add to calendar (one-tap POST that calls
  # the same Tools::CreateCalendarEvent path the Cmd+K palette and Scout use).
  #
  # Scout drafts the event — the USER taps Add. Never auto-adds.
  #
  # Three visual states:
  #   :draft     — extraction result; shows dot, title, time range, Edit + Add.
  #   :confirmed — after a successful Add; checkmark, localized time, View link.
  #   :error     — create failed; shows message + retry button.
  #
  # @param state         [:draft, :confirmed, :error]
  # @param title         [String]  event title (draft only)
  # @param start_at      [Time]    extracted start time (draft + confirmed)
  # @param end_at        [Time]    extracted end time (draft only, optional)
  # @param edit_url      [String]  link to new_calendar_event_path with prefill
  # @param add_url       [String]  POST path for create action
  # @param event         [CalendarEvent] created event (confirmed only)
  # @param error_message [String]  user-facing error copy (error only)
  class EventDraftBlock < Campbooks::Base
    def initialize(
      state: :draft,
      title: nil,
      start_at: nil,
      end_at: nil,
      edit_url: nil,
      add_url: nil,
      event: nil,
      error_message: nil
    )
      @state         = state.to_sym
      @title         = title
      @start_at      = start_at
      @end_at        = end_at
      @edit_url      = edit_url
      @add_url       = add_url
      @event         = event
      @error_message = error_message
    end

    def view_template
      case @state
      when :draft     then draft_state
      when :confirmed then confirmed_state
      when :error     then error_state
      end
    end

    private

    # ── Draft ────────────────────────────────────────────────────────────────

    def draft_state
      div(class: "mx-5 my-2.5 flex flex-wrap items-center gap-3 rounded-[14px] border border-border bg-card px-4 py-3") do
        event_dot
        event_info
        event_actions
      end
    end

    def event_dot
      # Calendar-blue square dot matching the prototype spec
      span(
        class: "h-[10px] w-[10px] flex-shrink-0 rounded-[3px]",
        style: "background: oklch(48% 0.14 248); background: var(--blue, oklch(48% 0.14 248))"
      )
    end

    def event_info
      div(class: "min-w-0 flex-1") do
        p(class: "text-[13.5px] font-semibold leading-snug text-foreground") { @title.to_s }
        p(class: "mt-0.5 text-[12px] tabular-nums text-muted-foreground") do
          plain(time_range_label)
          plain(" · ") # middle dot
          plain(t(".drafted_from"))
        end
      end
    end

    def event_actions
      div(class: "flex flex-shrink-0 items-center gap-2") do
        # Edit — ghost link opens the full calendar form in the event modal frame
        a(
          href: @edit_url,
          class: "rounded-[9px] border border-border bg-transparent px-[11px] py-[6px] text-[12px] font-[550] text-muted-foreground no-underline transition-colors hover:bg-muted hover:text-foreground",
          data: { turbo_frame: "calendar_event_modal" }
        ) { t(".edit") }

        # Add to calendar — POST via a minimal form; Turbo submits it in-frame
        if @add_url
          add_form
        end
      end
    end

    def add_form
      form(action: @add_url, method: "post", data: { turbo: "true" }) do
        input(
          type: "hidden",
          name: "authenticity_token",
          value: helpers.form_authenticity_token,
          autocomplete: "off"
        )
        button(
          type: "submit",
          class: "cursor-pointer rounded-[9px] border-0 bg-foreground px-[11px] py-[6px] text-[12px] font-semibold text-background transition-opacity hover:opacity-90"
        ) { t(".add") }
      end
    end

    # ── Confirmed ────────────────────────────────────────────────────────────

    def confirmed_state
      div(class: "mx-5 my-2.5 flex flex-wrap items-center gap-2 rounded-[14px] border border-border bg-card px-4 py-3") do
        span(class: "flex-shrink-0 text-[12px] font-semibold", style: "color: oklch(45% 0.13 152); color: var(--green, oklch(45% 0.13 152))") { "✓" }

        span(class: "text-[13px] font-medium text-foreground") do
          if @event&.start_at
            plain("#{t('.added')} · #{l(@event.start_at, format: :at_short)}")
          else
            plain(t(".added"))
          end
        end

        if @event
          a(
            href: helpers.calendar_event_path(@event),
            class: "ml-auto text-[12px] font-semibold text-accent-600 no-underline hover:underline dark:text-accent-300"
          ) { t(".view") }
        end
      end
    end

    # ── Error ────────────────────────────────────────────────────────────────

    def error_state
      div(class: "mx-5 my-2.5 flex flex-wrap items-center gap-2 rounded-[14px] border border-border bg-card px-4 py-3") do
        span(class: "min-w-0 flex-1 text-[13px] text-muted-foreground") { @error_message.to_s }

        if @add_url
          form(action: @add_url, method: "post", data: { turbo: "true" }) do
            input(
              type: "hidden",
              name: "authenticity_token",
              value: helpers.form_authenticity_token,
              autocomplete: "off"
            )
            button(
              type: "submit",
              class: "cursor-pointer border-0 bg-transparent text-[12px] font-[550] text-accent-600 hover:underline dark:text-accent-300"
            ) { t(".retry") }
          end
        end
      end
    end

    # ── Helpers ──────────────────────────────────────────────────────────────

    def time_range_label
      return "" unless @start_at

      start_str = l(@start_at, format: :at_short)
      if @end_at && @end_at != @start_at
        "#{start_str} – #{l(@end_at, format: :clock)}"
      else
        start_str
      end
    end
  end
end
