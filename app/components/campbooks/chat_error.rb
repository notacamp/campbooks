# frozen_string_literal: true

module Campbooks
  # Shown in place of the typing indicator when Scout's reply fails or comes back
  # empty. Calm, not alarming (transient failure, not the user's fault), with a
  # one-tap retry that re-sends the last message through the normal flow.
  class ChatError < Campbooks::Base
    # @param retry_content [String, nil] the user's last message, re-sent on retry
    def initialize(retry_content: nil)
      @retry_content = retry_content
    end

    RETRY_ICON = '<svg class="w-3.5 h-3.5 text-accent-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>'

    def view_template
      div(id: "agent_error", class: "flex items-start gap-3 px-1 py-3 animate-fade-in", data: { followups: true }) do
        div(class: "flex-shrink-0 mt-0.5") { render Campbooks::ScoutAvatar.new(size: :sm) }
        div(class: "flex flex-col items-start flex-1 min-w-0") do
          div(class: "flex items-center gap-2 px-0.5") do
            span(class: "text-[12px] font-semibold text-foreground") { "Scout" }
          end
          div(class: "mt-1 text-[14px] leading-relaxed text-muted-foreground") do
            plain t(".error_message")
          end
          if @retry_content.present?
            div(class: "mt-2.5") do
              button(
                type: "button",
                data: { action: "chat-input#prompt", chat_input_text_param: @retry_content },
                class: "group/chip inline-flex items-center gap-1.5 rounded-full border border-border bg-card " \
                       "px-3 py-1.5 text-[12px] font-medium text-foreground/80 cursor-pointer transition-all " \
                       "duration-150 ease-out hover:-translate-y-0.5 hover:text-accent-700 hover:border-accent-300 " \
                       "hover:bg-accent-50 hover:shadow-sm active:translate-y-0"
              ) do
                raw(safe(RETRY_ICON))
                span { t(".retry") }
              end
            end
          end
        end
      end
    end
  end
end
