# frozen_string_literal: true

module Campbooks
  # A passive feed nudge — Scout surfacing a stale thread ("X sent this N days
  # ago, reply?"). Feed-only, lighter than a FeedCard (no Scout-note block).
  #
  # @param name [String] who/what the nudge is about
  # @param body [String] the rest of the sentence (plain text)
  class NudgeCard < Campbooks::Base
    def initialize(name:, body:, **attrs)
      @name = name
      @body = body
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(class: class_names("rounded-[22px] border border-border bg-card px-5 py-[18px]", custom), **@attrs) do
        div(class: "flex items-start gap-3") do
          span(class: "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground") do
            raw safe(clock_icon)
          end
          div(class: "flex-1 pt-0.5 text-sm leading-relaxed text-muted-foreground") do
            span(class: "font-semibold text-foreground") { @name }
            whitespace
            plain @body
          end
        end
        div(class: "mt-3 flex justify-end gap-2") do
          render Campbooks::Button.new(variant: :ghost, size: :sm) { "Dismiss" }
          render Campbooks::Button.new(variant: :primary, size: :sm) { "Reply" }
        end
      end
    end

    private

    def clock_icon
      %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-[17px] w-[17px]"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>)
    end
  end
end
