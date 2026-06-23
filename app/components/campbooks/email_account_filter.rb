# frozen_string_literal: true

module Campbooks
  class EmailAccountFilter < Campbooks::Base
    SIZE_CLASSES = {
      xs: "w-5 h-5 text-[9px]",
      sm: "w-6 h-6 text-[10px]",
      md: "w-7 h-7 text-xs"
    }.freeze

    # @param accounts [Array<EmailAccount>] the accounts to show as filter toggles
    # @param size [Symbol] :xs (20px), :sm (24px), or :md (28px)
    def initialize(accounts:, size: :xs, **attrs)
      @accounts = accounts
      @size = size
      @attrs = attrs
    end

    def view_template
      return if @accounts.size < 2

      extra_class = @attrs.delete(:class)
      div(
        class: class_names("flex items-center gap-1", extra_class),
        data: { controller: "email-account-filter" },
        **@attrs
      ) do
        @accounts.each do |account|
          size_class = SIZE_CLASSES[@size]

          button(
            type: "button",
            class: class_names(
              size_class,
              "rounded-full flex items-center justify-center flex-shrink-0 font-semibold text-white transition-opacity duration-150"
            ),
            style: "background-color: #{account.color}",
            title: account.select_label,
            data: {
              controller: "contact-popover",
              email_account_filter_target: "toggle",
              action: "click->email-account-filter#toggle mouseenter->contact-popover#mouseEnter mouseleave->contact-popover#mouseLeave",
              email_account_id: account.id,
              contact_popover_url_value: helpers.popover_email_account_path(account)
            }
          ) do
            plain account.avatar_initial
          end
        end
      end
    end
  end
end
