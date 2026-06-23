# frozen_string_literal: true

module Campbooks
  # A skimmable inbox row: a leading triage CategoryChip, sender + subject, and a
  # one-line muted preview (the AI summary / snippet). Noise rows read quiet;
  # personal / important rows surface via the chip's colour. A list row, not a card.
  class SkimRow < Campbooks::Base
    # @param category [Symbol] triage category (see CategoryChip::CATEGORIES)
    # @param sender [String]
    # @param subject [String]
    # @param preview [String, nil] one-line AI summary or snippet
    # @param time [String, nil]
    # @param unread [Boolean]
    def initialize(category:, sender:, subject:, preview: nil, time: nil, unread: false, **attrs)
      @category = category
      @sender = sender
      @subject = subject
      @preview = preview
      @time = time
      @unread = unread
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(
        class: class_names(
          "group flex items-start gap-2.5 px-4 py-3 border-b border-border last:border-b-0 transition-colors hover:bg-muted/40",
          custom
        ),
        **@attrs
      ) do
        # Unread is signalled by a dot (and weight), not colour alone.
        div(class: "flex-shrink-0 w-1.5 pt-2") do
          div(class: "w-1.5 h-1.5 rounded-full bg-accent-500", aria_label: "Unread") if @unread
        end

        div(class: "flex-shrink-0 pt-0.5") do
          render Campbooks::CategoryChip.new(category: @category, size: :sm, label: false)
        end

        div(class: "min-w-0 flex-1") do
          div(class: "flex items-baseline gap-2") do
            span(class: class_names("flex-shrink-0 truncate max-w-[40%] text-sm text-foreground", @unread ? "font-semibold" : "font-medium")) { @sender }
            span(class: "min-w-0 flex-1 truncate text-sm text-muted-foreground") { @subject }
            span(class: "flex-shrink-0 text-[11px] text-muted-foreground tabular-nums") { @time } if @time
          end

          div(class: "truncate text-xs text-muted-foreground mt-0.5") { @preview } if @preview
        end
      end
    end
  end
end
