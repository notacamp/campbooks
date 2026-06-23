# frozen_string_literal: true

module Campbooks
  # One turn in the conversational setup flow: a Scout question (with an optional
  # example hint) or the user's answer. Deliberately simpler than the Scout chat
  # message — no tool actions or follow-up chips, just the back-and-forth.
  class AiSetupMessage < Campbooks::Base
    # @param message [AgentMessage]
    # @param hint [String, nil] a short example shown under a Scout question
    #   (rendered live on the streamed turn; not persisted)
    def initialize(message:, hint: nil)
      @message = message
      @hint = hint
    end

    def view_template
      @message.from_ai? ? ai_bubble : user_bubble
    end

    private

    def ai_bubble
      div(class: "chat-message flex items-start gap-2.5 px-4 py-2.5 animate-fade-in") do
        render Campbooks::ScoutAvatar.new(size: :sm)
        div(class: "flex-1 min-w-0") do
          span(class: "block text-[12px] font-semibold text-foreground mb-1 px-0.5") { "Scout" }
          div(class: "text-sm text-foreground whitespace-pre-wrap px-0.5") { @message.content }
          if @hint.present?
            p(class: "mt-1 text-xs text-muted-foreground px-0.5") { t(".example_hint", hint: @hint) }
          end
        end
      end
    end

    def user_bubble
      div(class: "chat-message flex justify-end px-4 py-2.5 animate-fade-in") do
        div(class: "max-w-[85%] rounded-2xl rounded-br-sm bg-accent-600 text-white px-3.5 py-2 text-sm whitespace-pre-wrap") do
          @message.content
        end
      end
    end
  end
end
