# frozen_string_literal: true

module Campbooks
  # Scout's identity mark: a sparkle on the warm Ember gradient.
  # Used everywhere Scout speaks (chat messages, typing indicator, header,
  # briefing) so the assistant reads as one consistent, branded presence.
  class ScoutAvatar < Campbooks::Base
    # @param size [Symbol] :xs, :sm, :md, :lg, :xl
    # @param pulse [Boolean] add a soft breathing glow (used while thinking)
    def initialize(size: :md, pulse: false, **attrs)
      @size = size
      @pulse = pulse
      @attrs = attrs
    end

    SIZE_CLASSES = {
      xs: "w-6 h-6",
      sm: "w-7 h-7",
      md: "w-8 h-8",
      lg: "w-10 h-10",
      xl: "w-12 h-12"
    }.freeze

    ICON_CLASSES = {
      xs: "w-3.5 h-3.5",
      sm: "w-4 h-4",
      md: "w-[18px] h-[18px]",
      lg: "w-5 h-5",
      xl: "w-6 h-6"
    }.freeze

    def view_template
      custom_class = @attrs.delete(:class)
      div(
        class: class_names(
          "relative flex items-center justify-center flex-shrink-0 rounded-full text-white",
          "bg-ember-gradient shadow-ember",
          "ring-1 ring-inset ring-white/25",
          SIZE_CLASSES[@size],
          custom_class
        ),
        **@attrs
      ) do
        # Soft breathing halo while Scout is working
        if @pulse
          span(class: "absolute inset-0 rounded-full animate-ping", style: "background-color: var(--ember-solid); opacity: 0.35")
        end
        raw(safe(sparkle_svg))
      end
    end

    private

    def sparkle_svg
      %(<svg viewBox="0 0 24 24" fill="currentColor" class="relative #{ICON_CLASSES[@size]}"><path fill-rule="evenodd" d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5z" clip-rule="evenodd"/></svg>)
    end
  end
end
