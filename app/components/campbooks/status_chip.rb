# frozen_string_literal: true

module Campbooks
  # A small, tone-coded status chip — Paper's STATUS column (and anywhere a meaning-bearing
  # state wants a quiet pill). Tones map to the shared tone-* utilities; the `ember` tone is
  # Scout's glass surface and pairs with a leading spark (needs-review). Feed it a
  # Documents::Status::Result via `.for`, or a tone/label directly.
  #
  #   render Campbooks::StatusChip.for(Documents::Status.for(document))
  #   render Campbooks::StatusChip.new(tone: :warning, label: "Unpaid")
  #
  # @param tone [Symbol] :warning | :destructive | :success | :ember | :muted
  # @param label [String] the chip text
  # @param spark [Boolean] show the Ember spark (needs review)
  class StatusChip < Campbooks::Base
    SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-3 w-3 flex-shrink-0" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'

    BASE = "inline-flex items-center gap-1 h-[22px] px-2 rounded-md text-[11.5px] font-medium whitespace-nowrap"

    TONE_CLASSES = {
      warning:     "tone-amber",
      destructive: "tone-red",
      success:     "tone-green",
      muted:       "tone-neutral",
      ember:       "scout-glass text-foreground"
    }.freeze

    def self.for(result, **attrs)
      new(tone: result.tone, label: result.chip_text, spark: result.spark?, **attrs)
    end

    def initialize(tone:, label:, spark: false, **attrs)
      @tone = tone
      @label = label
      @spark = spark
      @attrs = attrs
    end

    def view_template
      span(class: class_names(BASE, TONE_CLASSES.fetch(@tone, TONE_CLASSES[:muted]), @attrs.delete(:class)), **@attrs) do
        span(class: "flex-shrink-0 text-[color:var(--ember-solid)]") { raw(safe(SPARK)) } if @spark
        span { @label }
      end
    end
  end
end
