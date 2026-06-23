# frozen_string_literal: true

module Campbooks
  # One ring in the /documents Skim tray: a document CATEGORY shown as an
  # Instagram-stories style avatar ring (category icon inside a same-hue gradient
  # border, count badge, label below). Tapping it opens the document Skim viewer at
  # that category. category: nil renders the leading "Review all" ring. Icon and
  # hue come from Campbooks::DocSkimTheme so the ring matches the viewer header.
  # The document-world analogue of Campbooks::SkimRing.
  class DocSkimRing < Campbooks::Base
    # @param category [String, Symbol, nil] a DocumentType category, or nil for "Review all"
    # @param label [String] text under the ring
    # @param count [Integer, nil] document count badge
    def initialize(category: nil, label:, count: nil, done: false, **attrs)
      @category = category
      @label = label
      @count = count
      @done = done
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      button(
        type: "button",
        class: class_names("group flex w-[4.25rem] flex-shrink-0 snap-start flex-col items-center gap-1.5 outline-none", custom),
        title: t(".ring_title", label: @label),
        **@attrs
      ) do
        div(class: "relative") do
          div(
            class: class_names(
              "rounded-full p-[2.5px] transition-transform duration-150 group-hover:scale-105 group-active:scale-95 group-focus-visible:ring-2 group-focus-visible:ring-offset-2 group-focus-visible:ring-ring",
              @done ? "bg-border" : "bg-ember-gradient shadow-ember"
            )
          ) do
            div(class: "flex h-14 w-14 items-center justify-center rounded-full bg-card") do
              svg(class: class_names("h-6 w-6", @done ? "text-muted-foreground" : "text-foreground"), fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "1.8", aria_hidden: "true") do
                raw(safe(Campbooks::DocSkimTheme.icon(@category)))
              end
            end
          end
          count_badge if @count&.positive?
        end
        span(class: "max-w-full truncate text-[11px] font-medium text-gray-600 dark:text-gray-300") { @label }
      end
    end

    private

    def count_badge
      span(
        class: class_names(
          "absolute -bottom-0.5 -right-0.5 inline-flex min-w-[18px] items-center justify-center rounded-full border-2 border-card px-1 text-[10px] font-semibold leading-none tabular-nums",
          @done ? "bg-muted text-muted-foreground" : "bg-primary text-primary-foreground"
        )
      ) { abbreviated_count }
    end

    def abbreviated_count
      @count < 1000 ? @count.to_s : "#{(@count / 1000.0).round(1)}k"
    end
  end
end
