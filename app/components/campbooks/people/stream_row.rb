# frozen_string_literal: true

module Campbooks
  module People
    # One stream row — a service's mail grouped by kind. Used in two places:
    #   * the Streams tab's left pane (workspace inbox groups): a kind icon, the
    #     name, "N threads · last <date>", and Scout's line; the whole row opens
    #     the stream in the "people_detail" frame.
    #   * the organization page's "Streams from …" section: same shape with a
    #     trailing action button ("Open stream" / "Money") and an optional warn
    #     badge (e.g. "Late").
    #
    # Kept generic so both callers share the layout; the caller supplies the icon,
    # copy, link and any actions.
    class StreamRow < Campbooks::Base
      SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[13px] w-[13px]" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'

      # @param icon [Symbol] :mail | :bell | :file | :users
      # @param title [String]
      # @param meta [String] the second line ("N threads · last …" / "Service · N messages")
      # @param note [String, nil] Scout's line
      # @param href [String, nil] whole-row link target (left-pane); nil in the org page
      # @param selected [Boolean]
      # @param badge [String, nil] a warn chip (e.g. "Late")
      # @param actions [Array<Hash>] {label:, href:, frame:} buttons (org page)
      def initialize(icon:, title:, meta:, note: nil, href: nil, selected: false, badge: nil, actions: [])
        @icon = icon.to_sym
        @title = title
        @meta = meta
        @note = note
        @href = href
        @selected = selected
        @badge = badge
        @actions = actions
      end

      def view_template
        if @href && @actions.empty?
          a(href: @href,
            data: { turbo_frame: "people_detail", turbo_action: "advance", action: "click->email-mobile#showDetail" },
            class: class_names(
              "flex items-start gap-3 rounded-xl px-3 py-2.5 no-underline transition-colors",
              @selected ? "bg-secondary" : "hover:bg-secondary/60"
            )) { inner }
        else
          div(class: "flex items-start gap-3 py-2.5") { inner }
        end
      end

      private

      def inner
        icon_tile
        body
        badge_chip if @badge
        actions if @actions.any?
      end

      def icon_tile
        render(StreamIcon.new(kind: @icon))
      end

      def body
        div(class: "min-w-0 flex-1") do
          div(class: "flex items-baseline gap-2") do
            span(class: "min-w-0 flex-1 truncate text-[13.5px] font-semibold text-foreground") { @title }
          end
          div(class: "truncate text-[12px] text-muted-foreground") { @meta }
          if @note.present?
            div(class: "mt-1 flex items-start gap-1.5 text-[12.5px] leading-snug text-foreground/80") do
              span(class: "mt-[2px] flex-shrink-0", style: "color: var(--ember-solid)") { raw(safe(SPARK)) }
              span(class: "min-w-0") { @note }
            end
          end
        end
      end

      def badge_chip
        span(class: "mt-0.5 inline-flex h-[22px] flex-shrink-0 items-center gap-1 self-center rounded-md bg-amber-100 px-2 text-[11.5px] font-medium text-amber-700 dark:bg-amber-500/15 dark:text-amber-300") do
          plain(@badge)
        end
      end

      def actions
        div(class: "flex flex-shrink-0 items-center gap-1.5 self-center") do
          @actions.each do |action|
            a(href: action[:href], data: action[:frame] ? { turbo_frame: action[:frame], turbo_action: "advance", action: "click->email-mobile#showDetail" } : { turbo_frame: "_top" },
              class: "inline-flex h-[26px] items-center rounded-lg border border-border px-2.5 text-[12px] font-medium text-foreground no-underline hover:bg-secondary") do
              plain(action[:label])
            end
          end
        end
      end
    end
  end
end
