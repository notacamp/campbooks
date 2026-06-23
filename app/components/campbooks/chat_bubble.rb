module Campbooks
  class ChatBubble < Campbooks::Base
    # @param author [String, nil] display name shown above the bubble
    # @param role [Symbol] :ai (left-aligned, gray bg) or :user (right-aligned, accent bg)
    # @param timestamp [String, nil] time string (e.g., "2 min ago")
    def initialize(author: nil, role: :ai, timestamp: nil)
      @author = author
      @role = role
      @timestamp = timestamp
    end

    def view_template(&block)
      div(class: wrapper_classes) do
        if @author
          p(class: "text-xs text-gray-500 mb-1") { @author }
        end

        div(class: bubble_classes) do
          __yield_content__(&block)
        end

        if @timestamp
          p(class: "text-xs text-gray-400 mt-1") { @timestamp }
        end
      end
    end

    private

    def wrapper_classes
      base = "flex flex-col"
      @role == :user ? "#{base} items-end" : "#{base} items-start"
    end

    def bubble_classes
      if @role == :ai
        "bg-gray-100 rounded-lg px-4 py-3 text-sm text-gray-900"
      else
        "bg-accent-600 text-white rounded-lg px-4 py-3 text-sm"
      end
    end

    # Sub-component for a typing indicator (three bouncing dots).
    class TypingIndicator < Campbooks::Base
      def view_template
        div(class: "flex items-center gap-1 px-4 py-3 bg-gray-100 rounded-lg") do
          span(
            class: "w-2 h-2 rounded-full bg-gray-400 animate-bounce",
            style: "animation-delay: 0s"
          )
          span(
            class: "w-2 h-2 rounded-full bg-gray-400 animate-bounce",
            style: "animation-delay: 0.2s"
          )
          span(
            class: "w-2 h-2 rounded-full bg-gray-400 animate-bounce",
            style: "animation-delay: 0.4s"
          )
        end
      end
    end
  end
end
