# frozen_string_literal: true

module Campbooks
  # A facepile: the overlapping participant avatars of an email thread. Composes
  # Campbooks::ContactAvatar so each face keeps its contact-popover. Avatars cut
  # out from one another with a canvas-colored ring (ring-background), leftmost on
  # top; participants past `max` collapse into a "+N" chip. Stays neutral gray —
  # never Ember, which is reserved for Scout/live/win.
  #
  #   render Campbooks::ContactAvatarGroup.new(
  #     participants: [{ email: "ann@x.com", contact_id: 1 }, { email: "bob@y.com" }],
  #     size: :xl, max: 3
  #   )
  class ContactAvatarGroup < Campbooks::Base
    RING = "ring-2 ring-background"

    # Tighter overlap on smaller avatars so the pile reads as one cluster.
    OVERLAP = { sm: "-space-x-2", md: "-space-x-2", lg: "-space-x-2.5", xl: "-space-x-2.5" }.freeze

    CHIP = {
      sm: "w-6 h-6 text-[10px]",
      md: "w-7 h-7 text-[10px]",
      lg: "w-8 h-8 text-[11px]",
      xl: "w-[38px] h-[38px] text-xs"
    }.freeze

    # @param participants [Array<Hash>] [{ email:, contact_id: }] newest first
    # @param size [Symbol] :sm | :md | :lg | :xl (matches ContactAvatar)
    # @param max [Integer] how many faces before the rest fold into "+N"
    # @param variant [Symbol] :neutral | :accent (passed to each ContactAvatar)
    # @param account_color [String, nil] optional hex color for the email-account ring
    def initialize(participants:, size: :md, max: 3, variant: :neutral, account_color: nil)
      @participants = dedupe(participants)
      @size = size
      @max = [ max, 1 ].max
      @variant = variant
      @account_color = account_color
    end

    def view_template
      return if @participants.empty?

      shown = @participants.first(@max)
      overflow = @participants.size - shown.size

      div(class: class_names("flex items-center", OVERLAP[@size]), data: { testid: "facepile" }) do
        shown.each_with_index do |p, i|
          # Leftmost (latest sender) sits on top; the pile cascades under it.
          span(class: class_names("relative rounded-full", RING), style: "z-index: #{shown.size - i}") do
            render Campbooks::ContactAvatar.new(
              email: p[:email], contact_id: p[:contact_id], size: @size, variant: @variant,
              account_color: @account_color
            )
          end
        end

        overflow_chip(overflow) if overflow.positive?
      end
    end

    private

    def overflow_chip(count)
      # Match ContactAvatar's neutral fill EXACTLY (no dark: override): the app's
      # gray ramp auto-inverts under .dark, so an explicit dark:bg-gray-700 would
      # double-invert and render the chip light. Reads as a muted sibling avatar.
      span(
        class: class_names(
          CHIP[@size], RING,
          "relative rounded-full flex items-center justify-center font-semibold",
          "bg-gray-200 text-gray-600"
        ),
        style: "z-index: 0",
        aria_label: "#{count} more"
      ) { plain("+#{count}") }
    end

    # One face per address, case-insensitive, first occurrence (newest) wins.
    def dedupe(participants)
      Array(participants)
        .map { |p| p.respond_to?(:to_h) ? p.to_h.symbolize_keys : p }
        .reject { |p| p[:email].to_s.strip.empty? }
        .uniq { |p| p[:email].to_s.strip.downcase }
    end
  end
end
