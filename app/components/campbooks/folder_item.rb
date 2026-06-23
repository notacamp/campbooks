module Campbooks
  class FolderItem < Campbooks::Base
    # @param label [String] display name (e.g., "Inbox")
    # @param href [String] link URL
    # @param count [Integer, nil] unread count badge (hidden when nil or zero)
    # @param active [Boolean] highlights the item when true
    # @param attrs [Hash] additional HTML attributes on the <a> tag
    def initialize(label:, href:, count: nil, active: false, **attrs)
      @label = label
      @href = href
      @count = count
      @active = active
      @attrs = attrs
    end

    # Slot for an SVG icon or any leading content.
    def with_icon(&block)
      @icon = block
    end

    def view_template(&content)
      if content
        capture(&content)
      end

      a(href: @href, class: folder_classes, **@attrs) do
        if @icon
          span(class: "w-5 h-5 flex items-center justify-center flex-shrink-0", &@icon)
        end

        span(class: "flex-1 truncate text-[13px]") { @label }

        if @count && @count > 0
          span(
            class: "min-w-[18px] h-4 px-1 text-[10px] font-bold text-primary-foreground bg-primary rounded-full inline-flex items-center justify-center ml-auto"
          ) { @count.to_s }
        end
      end
    end

    private

    def folder_classes
      tokens = [
        "flex items-center gap-2.5 px-2.5 py-2 rounded-lg transition-colors"
      ]
      if @active
        tokens << "bg-accent-50 text-accent-700 font-medium"
      else
        tokens << "text-gray-600 hover:bg-gray-50 hover:text-gray-900"
      end
      class_names(tokens)
    end
  end
end
