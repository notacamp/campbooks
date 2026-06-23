# frozen_string_literal: true

module Campbooks
  class ChatTyping < Campbooks::Base
    # @param id [String] DOM ID for the typing indicator container
    # @param status [String] live label shown next to "Scout" (e.g. what it's
    #   doing right now — "Searching your inbox…")
    def initialize(id: "typing_indicator", status: "Thinking…")
      @id = id
      @status = status
    end

    def view_template
      div(id: @id, class: "flex items-start gap-2.5 px-4 py-2.5 animate-fade-in") do
        render Campbooks::ScoutAvatar.new(size: :sm, pulse: true)
        div(class: "flex-1 min-w-0") do
          div(class: "flex items-center gap-2 px-0.5") do
            span(class: "text-[12px] font-semibold text-foreground") { "Scout" }
            span(class: "text-[10px] text-muted-foreground") { @status }
          end
          div(class: "mt-2 flex items-center gap-1.5 px-0.5") do
            span(class: "w-1.5 h-1.5 rounded-full bg-accent-500", style: "animation: typingBounce 1.4s ease-in-out 0s infinite both")
            span(class: "w-1.5 h-1.5 rounded-full bg-accent-500", style: "animation: typingBounce 1.4s ease-in-out 0.2s infinite both")
            span(class: "w-1.5 h-1.5 rounded-full bg-accent-500", style: "animation: typingBounce 1.4s ease-in-out 0.4s infinite both")
          end
        end
      end
    end
  end
end
