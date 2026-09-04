# frozen_string_literal: true

module Campbooks
  module People
    # One counterpart in the People list: a person or organization, inbox density.
    #
    # Line 1: avatar (30 px, unread dot) | name ["New" chip] [middot] subject (muted,
    #         truncate) | wait (Need-you) or date (Recent)
    # Line 2: spark + Scout text (only when standing.text present), one line.
    #
    # Two shapes:
    #   * default  — the left-pane row; the whole row opens the counterpart.
    #   * nested   — the organization page's "People at ..." rows: same body, but
    #                an "Open" button on the right (the row is not itself a link).
    class CounterpartRow < Campbooks::Base
      SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[12px] w-[12px]" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'
      PAPERCLIP = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-[11px] w-[11px]" aria-hidden="true"><path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/></svg>'

      # @param counterpart [People::Counterpart]
      # @param selected [Boolean] lit (left-pane selection)
      # @param nested [Boolean] organization-page row shape (trailing "Open" button)
      def initialize(counterpart:, selected: false, nested: false)
        @counterpart = counterpart
        @selected = selected
        @nested = nested
      end

      def view_template
        @nested ? nested_row : list_row
      end

      private

      def list_row
        a(href: href,
          data: { turbo_frame: "people_detail", turbo_action: "advance", action: "click->email-mobile#showDetail" },
          class: class_names(
            "flex items-start gap-2.5 rounded-xl px-3 py-[7px] no-underline transition-colors",
            @selected ? "bg-secondary" : "hover:bg-secondary/60"
          )) do
          avatar_with_dot
          body
        end
      end

      def nested_row
        div(class: "flex items-start gap-2.5 py-[7px]") do
          avatar_with_dot
          body
          a(href: href, data: { turbo_frame: "people_detail", turbo_action: "advance", action: "click->email-mobile#showDetail" },
            class: "mt-0.5 inline-flex h-[26px] flex-shrink-0 items-center rounded-lg border border-border px-2.5 text-[12px] font-medium text-foreground no-underline hover:bg-secondary") do
            plain(t(".open"))
          end
        end
      end

      def body
        div(class: "min-w-0 flex-1") do
          # Line 1: name [chip] [dot] subject | wait/date
          div(class: "flex items-baseline gap-1.5") do
            div(class: "min-w-0 flex-1 flex items-baseline gap-1 truncate") do
              span(class: "flex-shrink-0 text-[13.5px] font-semibold text-foreground") { @counterpart.name }
              if new_sender?
                span(class: "flex-shrink-0 rounded bg-ember/15 px-1 py-px text-[9.5px] font-semibold uppercase tracking-wide text-ember") { t(".new") }
              end
              if (subj = standing.subject).present?
                span(class: "flex-shrink-0 text-muted-foreground/50 text-[12px]") { "·" }
                span(class: "min-w-0 truncate text-[12.5px] text-muted-foreground") { subj }
              end
            end
            right_meta
          end

          # Line 2: spark text (when present)
          standing_line
        end
      end

      def right_meta
        if @counterpart.needs_you?
          wait = standing.wait_days.to_i
          urgent = wait >= 7
          div(class: "flex flex-shrink-0 items-center gap-0.5") do
            if has_attachment?
              span(class: "text-muted-foreground/60") { raw(safe(PAPERCLIP)) }
            end
            span(class: class_names("text-[11.5px] font-semibold tabular-nums",
                                    urgent ? "text-ember" : "text-muted-foreground")) do
              t(".wait_days", count: wait)
            end
          end
        elsif (time = @counterpart.last_activity)
          span(class: "flex-shrink-0 text-[11.5px] text-muted-foreground") { row_time(time) }
        end
      end

      def standing_line
        text = standing.text
        return if text.blank?

        div(class: "mt-0.5 flex items-start gap-1 text-[12px] leading-snug text-foreground/70 line-clamp-1") do
          span(class: "mt-[2px] flex-shrink-0", style: "color: var(--ember-solid)") { raw(safe(SPARK)) }
          span(class: "min-w-0 line-clamp-1") { text }
        end
      end

      def avatar_with_dot
        div(class: "relative flex-shrink-0") do
          if @counterpart.person?
            render(ContactAvatar.new(email: @counterpart.avatar_email.to_s, size: :sm,
                                     contact_id: nil, variant: :neutral))
          else
            div(class: "flex h-[30px] w-[30px] items-center justify-center rounded-full bg-secondary text-[12px] font-semibold text-muted-foreground") do
              plain(@counterpart.avatar_initial.to_s)
            end
          end
          if unread?
            span(class: "absolute bottom-0 right-0 h-2 w-2 rounded-full border border-background",
                 style: "background-color: var(--ember-solid)") { }
          end
        end
      end

      def href
        @counterpart.person? ? helpers.person_page_path(@counterpart.id) : helpers.people_organization_path(@counterpart.id)
      end

      def standing = @counterpart.standing
      def data     = @counterpart.data || {}

      def new_sender?   = data["new"] == true
      def unread?       = data["unread"] == true
      def has_attachment? = data["has_attachment"] == true

      # Today -> clock ("11:01 AM"); this year -> "Jul 18"; older -> full date.
      def row_time(time)
        if time.to_date == Date.current
          l(time, format: :clock)
        elsif time.year == Date.current.year
          l(time.to_date, format: :day_month)
        else
          l(time.to_date, format: :date)
        end
      end
    end
  end
end
