# frozen_string_literal: true

module Campbooks
  module People
    # One counterpart in the People list: a person or an organization. Avatar,
    # name + time, a subtitle (the person's organization, or "Organization · N
    # people · N services"), then Scout's Ember-spark one-liner about where things
    # stand. Two shapes:
    #   * default — the left-pane row; the whole row opens the counterpart in the
    #     "people_detail" frame (and flips the mobile master-detail to the detail).
    #   * nested  — the organization page's "People at …" rows: same body, but an
    #     "Open" button on the right (the row isn't itself a link).
    class CounterpartRow < Campbooks::Base
      SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[13px] w-[13px]" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'

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
            "flex items-start gap-3 rounded-xl px-3 py-2.5 no-underline transition-colors",
            @selected ? "bg-secondary" : "hover:bg-secondary/60"
          )) do
          avatar
          body
        end
      end

      def nested_row
        div(class: "flex items-start gap-3 py-2.5") do
          avatar
          body
          a(href: href, data: { turbo_frame: "people_detail", turbo_action: "advance", action: "click->email-mobile#showDetail" },
            class: "mt-0.5 inline-flex h-[26px] flex-shrink-0 items-center rounded-lg border border-border px-2.5 text-[12px] font-medium text-foreground no-underline hover:bg-secondary") do
            plain(t(".open"))
          end
        end
      end

      def body
        div(class: "min-w-0 flex-1") do
          div(class: "flex items-baseline gap-2") do
            span(class: "min-w-0 flex-1 truncate text-[13.5px] font-semibold text-foreground") { @counterpart.name }
            if (time = @counterpart.last_activity)
              span(class: "flex-shrink-0 text-[11.5px] text-muted-foreground") { row_time(time) }
            end
          end
          if @counterpart.subtitle.present?
            div(class: "truncate text-[12px] text-muted-foreground") { @counterpart.subtitle }
          end
          standing_line
        end
      end

      def standing_line
        text = @counterpart.standing.text
        return if text.blank?

        div(class: "mt-1 flex items-start gap-1.5 text-[12.5px] leading-snug text-foreground/80") do
          span(class: "mt-[2px] flex-shrink-0", style: "color: var(--ember-solid)") { raw(safe(SPARK)) }
          span(class: "min-w-0") { text }
        end
      end

      def avatar
        if @counterpart.person?
          render(ContactAvatar.new(email: @counterpart.avatar_email.to_s, size: :lg, contact_id: nil, variant: :neutral))
        else
          div(class: "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-secondary text-[13px] font-semibold text-muted-foreground") do
            plain(@counterpart.avatar_initial.to_s)
          end
        end
      end

      def href
        @counterpart.person? ? helpers.person_page_path(@counterpart.id) : helpers.people_organization_path(@counterpart.id)
      end

      # Today → clock ("11:01 AM"); this year → "Jul 18"; older → full date.
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
