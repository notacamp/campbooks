# frozen_string_literal: true

module Campbooks
  # A row of tappable prompt chips. Each chip drops its text into the Scout
  # composer and sends it (via the `chat-input#prompt` Stimulus action), so the
  # chat always offers an obvious next move instead of dead-ending.
  #
  # Used in two places:
  #   - the empty-state briefing (starter prompts)
  #   - under Scout's latest reply (follow-up prompts the model proposes)
  class ChatSuggestions < Campbooks::Base
    # @param prompts [Array<String>] chip labels (also the text that gets sent)
    # @param heading [String, nil] small caption above the chips
    # @param dismissable [Boolean] mark the row so it clears when the user sends
    #   the next message (used for per-reply follow-ups, not the briefing)
    # @param align [Symbol] :start or :center
    def initialize(prompts: [], heading: nil, dismissable: false, align: :start)
      @prompts = Array(prompts).map(&:to_s).reject(&:blank?).first(4)
      @heading = heading
      @dismissable = dismissable
      @align = align
    end

    ARROW = '<svg class="w-3 h-3 flex-shrink-0 text-accent-500 transition-transform group-hover/chip:translate-x-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M13 7l5 5m0 0l-5 5m5-5H6"/></svg>'

    def view_template
      return if @prompts.empty?

      div(
        class: class_names("flex flex-col gap-1.5", ("items-center" if @align == :center)),
        data: (@dismissable ? { followups: true } : {})
      ) do
        if @heading
          span(class: "text-[11px] font-semibold uppercase tracking-wide text-muted-foreground/80") { @heading }
        end
        div(class: class_names("flex flex-wrap gap-1.5", ("justify-center" if @align == :center))) do
          @prompts.each { |text| chip(text) }
        end
      end
    end

    private

    def chip(text)
      button(
        type: "button",
        data: { action: "chat-input#prompt", chat_input_text_param: text },
        class: "group/chip inline-flex items-center gap-1.5 rounded-full border border-border bg-card " \
               "px-3 py-1.5 text-[12px] font-medium text-foreground/80 cursor-pointer " \
               "transition-all duration-150 ease-out hover:-translate-y-0.5 hover:text-accent-700 " \
               "hover:border-accent-300 hover:bg-accent-50 hover:shadow-sm active:translate-y-0"
      ) do
        raw(safe(ARROW))
        span { text }
      end
    end
  end
end
