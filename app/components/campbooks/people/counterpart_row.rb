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
    #
    # The default shape includes a hover action cluster (Reply / Done / Snooze / Star /
    # More) absolutely positioned over the right meta, revealed on group-hover. Only
    # shown for person rows with the matching data flags. Wrap in Campbooks::Swipeable
    # for touch devices. The outer wrapper carries id="people_row_<id>" and
    # data-people-row for live broadcasts and keyboard navigation.
    class CounterpartRow < Campbooks::Base
      SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[12px] w-[12px]" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'
      PAPERCLIP = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-[11px] w-[11px]" aria-hidden="true"><path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/></svg>'

      # Icon SVG inner paths for the action cluster.
      ICON_REPLY   = '<path stroke-linecap="round" stroke-linejoin="round" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/>'
      ICON_DONE    = '<path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7"/>'
      ICON_SNOOZE  = '<path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/>'
      ICON_STAR_OUTLINE = '<path stroke-linecap="round" stroke-linejoin="round" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z"/>'
      ICON_STAR_FILLED = '<path fill="currentColor" stroke-linecap="round" stroke-linejoin="round" d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>'
      ICON_MORE    = '<path stroke-linecap="round" stroke-linejoin="round" d="M5 12h.01M12 12h.01M19 12h.01"/>'

      ACTION_BTN = "inline-flex h-[28px] w-[28px] items-center justify-center rounded-lg text-muted-foreground " \
                   "hover:bg-secondary hover:text-foreground transition-colors"

      # @param counterpart [People::Counterpart]
      # @param selected [Boolean] lit (left-pane selection)
      # @param nested [Boolean] organization-page row shape (trailing "Open" button)
      # @param variant [Symbol] :lane (the verb lanes: wait at the right) or :latest
      #   (the inbox list: the date at the right). The two lists can hold the same
      #   person, so the DOM id carries the variant.
      def initialize(counterpart:, selected: false, nested: false, variant: :lane)
        @counterpart = counterpart
        @selected = selected
        @nested = nested
        @variant = variant
      end

      def view_template
        @nested ? nested_row : list_row
      end

      private

      # The list-row wraps the <a> and the action cluster in a group div so the
      # cluster can be positioned absolutely without the <a> needing to be relative.
      def list_row
        div(
          id: row_dom_id,
          data: { people_row: true },
          class: "group relative"
        ) do
          a(href: href,
            data: { turbo_frame: "people_detail", turbo_action: "advance", action: "click->email-mobile#showDetail" },
            class: class_names(
              "flex items-start gap-2.5 rounded-xl px-3 py-[7px] no-underline transition-colors",
              @selected ? "bg-secondary" : "hover:bg-secondary/60"
            )) do
            avatar_with_dot
            body
          end
          # Action cluster rendered outside the <a> so clicks don't navigate.
          action_cluster if show_action_cluster?
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
        if @counterpart.needs_you? && @variant == :lane
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

      # Scout's read with the spark when there is one; otherwise the newest
      # message's first line, muted — the row then reads like an inbox row.
      def standing_line
        text = standing.text
        if text.present?
          div(class: "mt-0.5 flex items-start gap-1 text-[12px] leading-snug text-foreground/70 line-clamp-1") do
            span(class: "mt-[2px] flex-shrink-0", style: "color: var(--ember-solid)") { raw(safe(SPARK)) }
            span(class: "min-w-0 line-clamp-1") { text }
          end
        elsif (snippet = data["snippet"]).present?
          div(class: "mt-0.5 text-[12px] leading-snug text-muted-foreground line-clamp-1") { snippet }
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

      # ── Action cluster (hover, person rows only) ────────────────────────────

      def show_action_cluster?
        @counterpart.person? && !@nested
      end

      def action_cluster
        div(class: "absolute inset-y-0 right-0 hidden items-center gap-0.5 pr-2 group-hover:flex group-focus-within:flex") do
          div(class: "flex items-center gap-0.5 rounded-xl bg-card/90 px-1 py-1 shadow-sm backdrop-blur") do
            reply_button   if can_reply?
            done_button    if can_done?
            snooze_button  if needs_you?
            star_button
            more_menu
          end
        end
      end

      def reply_button
        if (msg_id = standing.email_message_id)
          form(action: helpers.compose_email_message_path(msg_id, mode: :reply),
               method: "post", class: "contents") do
            input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
            button(type: "submit", class: ACTION_BTN,
                   title: t(".actions.reply_hint"),
                   data: { people_reply: true }) do
              action_icon(ICON_REPLY)
            end
          end
        end
      end

      def done_button
        form(action: helpers.people_action_path(@counterpart.id, :done),
             method: :post, class: "contents",
             data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit", class: ACTION_BTN, title: t(".actions.done_hint"),
                 data: { people_done: true }) do
            action_icon(ICON_DONE)
          end
        end
      end

      def snooze_button
        form(action: helpers.people_action_path(@counterpart.id, :snooze),
             method: :post, class: "contents",
             data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "until", value: "tomorrow")
          button(type: "submit", class: ACTION_BTN, title: t(".actions.snooze_hint"),
                 data: { people_snooze: true }) do
            action_icon(ICON_SNOOZE)
          end
        end
      end

      def star_button
        kind  = starred? ? :unstar : :star
        icon  = starred? ? ICON_STAR_FILLED : ICON_STAR_OUTLINE
        title = starred? ? t(".actions.unstar_hint") : t(".actions.star_hint")
        form(action: helpers.people_action_path(@counterpart.id, kind),
             method: :post, class: "contents",
             data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
                 class: class_names(ACTION_BTN, starred? ? "text-amber-500" : nil),
                 title: title) do
            action_icon(icon)
          end
        end
      end

      def more_menu
        details(class: "relative") do
          summary(class: class_names(ACTION_BTN, "list-none cursor-pointer"),
                  title: t(".actions.more_hint"),
                  data: { people_more: true }) do
            action_icon(ICON_MORE)
          end
          div(class: "absolute right-0 top-full z-50 mt-1 w-48 rounded-xl border border-border bg-card py-1 shadow-lg") do
            # Archive thread
            if standing.email_message_id
              form(action: helpers.people_action_path(@counterpart.id, :archive),
                   method: :post, class: "block w-full",
                   data: { turbo_stream: true }) do
                input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
                button(type: "submit",
                       class: "w-full px-4 py-2 text-left text-[13px] hover:bg-secondary") do
                  plain(t(".actions.archive"))
                end
              end
            end
            if (contact_id = data["contact_id"])
              # Details (opens the rail/sheet for this person)
              a(href: helpers.people_details_path(@counterpart.id), data: { turbo_frame: "people_details" },
                class: "block px-4 py-2 text-[13px] no-underline hover:bg-secondary") do
                plain(t(".actions.details"))
              end
              # Block sender (POST set_state, like the conversation kebab)
              form(action: helpers.set_state_contact_path(contact_id, state: :block),
                   method: "post", class: "block w-full", data: { turbo: false }) do
                input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
                button(type: "submit", class: "w-full px-4 py-2 text-left text-[13px] hover:bg-secondary") do
                  plain(t(".actions.block"))
                end
              end
              # Open classic profile
              a(href: helpers.contact_path(contact_id), data: { turbo_frame: "_top" },
                class: "block px-4 py-2 text-[13px] text-muted-foreground no-underline hover:bg-secondary") do
                plain(t(".actions.open_profile"))
              end
            end
          end
        end
      end

      def action_icon(inner_path)
        svg(class: "h-4 w-4", fill: "none", stroke: "currentColor",
            stroke_width: "2", viewBox: "0 0 24 24", aria_hidden: "true") do
          raw(safe(inner_path))
        end
      end

      # ── Helpers ─────────────────────────────────────────────────────────────

      def href
        @counterpart.person? ? helpers.person_page_path(@counterpart.id) : helpers.people_organization_path(@counterpart.id)
      end

      def row_dom_id
        @variant == :latest ? "people_row_latest_#{@counterpart.id}" : "people_row_#{@counterpart.id}"
      end

      def standing    = @counterpart.standing
      def data        = @counterpart.data || {}

      def new_sender?   = data["new"] == true
      def unread?       = data["unread"] == true
      def has_attachment? = data["has_attachment"] == true
      def starred?      = data["starred"] == true
      def can_reply?    = data["can_reply"] == true
      def can_done?     = data["can_done"] == true
      def needs_you?    = @counterpart.needs_you?

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
