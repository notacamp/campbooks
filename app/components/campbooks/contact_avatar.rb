# frozen_string_literal: true

module Campbooks
  class ContactAvatar < Campbooks::Base
    # @param email [String] the email or address to derive the initial from
    # @param sent [Boolean] true = outgoing (blue), false = incoming (gray)
    # @param size [Symbol] :sm (24px), :md (28px), :lg (32px)
    # @param contact_id [Integer, nil] for contact-popover lookup
    # @param variant [Symbol] :neutral (lists/threads) or :accent (headers)
    # @param show_direction [Boolean] whether to overlay an ↑/↓ indicator
    # @param account_color [String, nil] optional hex color for ring
    def initialize(
      email:,
      sent: false,
      size: :md,
      contact_id: nil,
      variant: :neutral,
      show_direction: false,
      account_color: nil,
      **attrs
    )
      @email = email
      @sent = sent
      @size = size
      @contact_id = contact_id
      @variant = variant
      @show_direction = show_direction
      @account_color = account_color
      @attrs = attrs
    end

    def view_template
      div(class: "relative flex-shrink-0") do
        avatar_circle
        direction_indicator if @show_direction
      end
    end

    private

    def avatar_circle
      popover_data = build_popover_data

      div(
        class: class_names(
          SIZE_CLASSES[@size],
          "rounded-full flex items-center justify-center flex-shrink-0",
          @sent ? COLOR_SENT : COLOR_RECEIVED[@variant],
          popover_data.any? ? "cursor-default" : nil
        ),
        style: build_style,
        data: popover_data,
        **@attrs
      ) do
        plain((@email.presence || "?")[0].upcase)
      end
    end

    def build_popover_data
      return {} if @sent || (@contact_id.nil? && @email.blank?)

      data = {
        controller: "contact-popover",
        action: "mouseenter->contact-popover#mouseEnter mouseleave->contact-popover#mouseLeave click->contact-popover#click"
      }
      data[:contact_popover_contact_id_value] = @contact_id if @contact_id.present?
      data[:contact_popover_email_value] = @email if @email.present?
      # Profile target for touch devices: tapping the avatar navigates straight to
      # the contact's page (no hover popover). By id when known, else resolved from
      # the address via the lookup redirect.
      data[:contact_popover_profile_url_value] = profile_url
      data
    end

    def profile_url
      if @contact_id.present?
        helpers.contact_path(@contact_id)
      else
        helpers.lookup_contacts_path(email: @email)
      end
    end

    def build_style
      parts = []
      parts << "box-shadow: 0 0 0 2px #{@account_color}" if @account_color
      parts.join("; ").presence
    end

    def direction_indicator
      span(
        class: class_names(
          "absolute -bottom-0.5 -right-0.5 rounded-full flex items-center justify-center font-bold text-white border border-white",
          DIRECTION_SIZE[@size],
          @sent ? "bg-blue-500" : "bg-gray-400"
        )
      ) { @sent ? t(".sent_indicator") : t(".received_indicator") }
    end

    FONT_SIZES = { sm: "text-[10px]", md: "text-xs", lg: "text-xs" }.freeze

    SIZE_CLASSES = {
      sm: "w-6 h-6 text-[10px] font-semibold",
      md: "w-7 h-7 text-xs font-semibold",
      lg: "w-8 h-8 text-xs font-semibold",
      xl: "w-[38px] h-[38px] text-[13px] font-semibold"
    }.freeze

    DIRECTION_SIZE = {
      sm: "w-3 h-3 text-[7px] leading-none",
      md: "w-3.5 h-3.5 text-[8px] leading-none",
      lg: "w-3.5 h-3.5 text-[8px] leading-none",
      xl: "w-4 h-4 text-[8px] leading-none"
    }.freeze

    COLOR_SENT = "bg-blue-50 text-blue-600 dark:bg-blue-500/10 dark:text-blue-400"

    COLOR_RECEIVED = {
      neutral: "bg-gray-200 text-gray-600",
      accent:  "bg-accent-100 text-accent-700"
    }.freeze
  end
end
