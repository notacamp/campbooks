# frozen_string_literal: true

module Campbooks
  class CollapsedFolderItem < Campbooks::Base
    # Renders a collapsed folder icon with optional count badge for the email sidebar.
    #
    # @param label [String] folder display name
    # @param href [String] link URL
    # @param count [Integer, nil] unread/message count (hidden when nil or zero)
    # @param active [Boolean] whether the folder is currently selected
    #
    # @example
    #   render(Campbooks::CollapsedFolderItem.new(
    #     label: "Inbox", href: email_messages_path(folder_id: 1), count: 5, active: true,
    #     data: { turbo_frame: "_top" }
    #   )) do
    #     folder_icon_small("Inbox")
    #   end
    def initialize(label:, href:, count: nil, active: false, **attrs)
      @label = label
      @href = href
      @count = count
      @active = active
      @attrs = attrs
    end

    def view_template(&content)
      custom_class = @attrs.delete(:class)

      a(
        href: @href,
        class: class_names(
          "w-8 h-8 flex items-center justify-center rounded-md transition-colors relative",
          @active ? "bg-accent-50 text-accent-600" : "text-gray-400 hover:text-gray-600 hover:bg-gray-50",
          custom_class
        ),
        title: @count ? "#{@label} (#{@count})" : @label,
        **@attrs
      ) do
        __yield_content__(&content)
        render_count_badge if @count && @count > 0
      end
    end

    private

    def render_count_badge
      span(
        class: class_names(
          "absolute -top-0.5 -right-0.5 min-w-[11px] h-[11px] rounded-full text-[6px] font-bold tabular-nums flex items-center justify-center leading-none",
          @active ? "bg-accent-500 text-white" : "bg-gray-400 text-white"
        ),
        style: "padding:0 2px"
      ) { plain(@count > 99 ? "99+" : @count.to_s) }
    end
  end
end
