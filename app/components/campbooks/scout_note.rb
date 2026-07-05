# frozen_string_literal: true

module Campbooks
  # Scout's contribution on a feed card. Two shapes:
  #   * default — a light Ember-glass block (avatar, name, "AI" tag, its read): Scout
  #     as an entity weighing in, for surfaces with room to give it presence.
  #   * compact — one Ember spark + a bold "Scout" + the read, clamped to a few
  #     lines (`lines:`), for the dense home feed where a full block on every card is
  #     too much. Still unmistakably Scout (the Meaning Rule), at a fraction of the
  #     height. The home feed passes `lines: 3` to keep cards short; the email drawer
  #     keeps the roomier default.
  # Yields an optional trailing slot.
  #
  # @param message [String] Scout's read / recommendation text
  # @param time [String, nil] e.g. "read it just now" (default shape only)
  # @param compact [Boolean] render the compact shape
  # @param lines [Integer] compact clamp height before "Read more" (compact only)
  class ScoutNote < Campbooks::Base
    SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[13px] w-[13px]" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'

    def initialize(message:, time: nil, compact: false, lines: 10, **attrs)
      @message = message
      @time = time
      @compact = compact
      @lines = lines
      @attrs = attrs
    end

    def view_template(&block)
      @compact ? compact_template(&block) : full_template(&block)
    end

    private

    def compact_template
      div(class: class_names("flex items-start gap-1.5 text-[13px] leading-relaxed", @attrs.delete(:class)), **@attrs) do
        span(class: "mt-[3px] flex-shrink-0", style: "color: var(--ember-solid)") { raw safe(SPARK) }
        # Scout's read can run long; clamp it to `lines` (feed: 3) with a "Read more"
        # toggle rather than cutting it dead. The Scout label rides inside the
        # clamped text so it stays unmistakably Scout (the Meaning Rule).
        render Campbooks::ClampText.new(lines: @lines, class: "min-w-0 flex-1 text-foreground/80") do
          span(class: "font-semibold text-foreground") { "Scout" }
          whitespace
          plain @message
        end
        yield if block_given?
      end
    end

    def full_template
      div(class: class_names("scout-glass rounded-2xl p-4", @attrs.delete(:class)), **@attrs) do
        div(class: "flex items-center gap-2") do
          render Campbooks::ScoutAvatar.new(size: :xs)
          span(class: "text-[13px] font-bold text-foreground") { "Scout" }
          span(class: "rounded bg-ember-gradient px-1.5 py-0.5 text-[10px] font-bold tracking-wide text-white") { "AI" }
          span(class: "ml-auto text-[11px] text-muted-foreground") { @time } if @time
        end
        p(class: "mt-2.5 text-[13px] leading-relaxed text-foreground/85") { @message }
        yield if block_given?
      end
    end
  end
end
