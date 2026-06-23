# frozen_string_literal: true

module Campbooks
  # One ring in the inbox Skim tray: a THEME shown as an Instagram-stories style
  # avatar ring (theme icon inside a same-hue gradient border, count badge, label
  # below). Tapping it opens the Skim viewer at that theme, which then walks the
  # theme's mail by time. theme: nil renders the leading "Skim all" ring. Icon and
  # hue come from Campbooks::SkimTheme so the ring matches the viewer header.
  class SkimRing < Campbooks::Base
    # @param theme [Symbol, nil] one of Emails::SkimBuilder::THEME_ORDER, or nil for "Skim all"
    # @param label [String] text under the ring
    # @param count [Integer, nil] email count badge
    def initialize(theme: nil, label:, count: nil, done: false, **attrs)
      @theme = theme
      @label = label
      @count = count
      @done = done
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      button(
        type: "button",
        class: class_names("skim-ring group flex w-[4.25rem] flex-shrink-0 snap-start flex-col items-center gap-1.5 outline-none", custom),
        title: "Skim #{@label}",
        **@attrs
      ) do
        div(class: "relative") do
          div(
            class: class_names(
              "skim-ring__circle rounded-full p-[2.5px] transition-transform duration-150 group-hover:scale-105 group-active:scale-95 group-focus-visible:ring-2 group-focus-visible:ring-offset-2 group-focus-visible:ring-ring",
              @done ? "bg-border" : "bg-ember-gradient shadow-ember"
            )
          ) do
            div(class: "skim-ring__inner flex h-14 w-14 items-center justify-center rounded-full bg-card") do
              svg(class: class_names("skim-ring__icon h-6 w-6", @done ? "text-muted-foreground" : "text-foreground"), fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "1.8", aria_hidden: "true") do
                raw(safe(Campbooks::SkimTheme.icon(@theme)))
              end
            end
          end
          count_badge if @count&.positive?
        end
        span(class: "skim-ring__label max-w-full truncate text-[11px] font-medium text-gray-600 dark:text-gray-300") { @label }
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
