# frozen_string_literal: true

module Campbooks
  class ChatForm < Campbooks::Base
    def initialize(
      url:, method: :post, form_id: "chat_form", input_id: "chat_input",
      placeholder: "Ask Scout anything…", stimulus_controller: "chat-input",
      hidden_fields: {}, mention_candidates: nil
    )
      @url = url
      @method = method
      @form_id = form_id
      @input_id = input_id
      @placeholder = placeholder
      @stimulus_controller = stimulus_controller
      @hidden_fields = hidden_fields
      # When present (array of { name:, kind: }), enables @mention autocomplete.
      @mention_candidates = mention_candidates
    end

    SEND_ICON = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M12 19V5M5 12l7-7 7 7"/></svg>'

    def view_template
      form(
        id: @form_id,
        action: @url,
        method: @method == :get ? "get" : "post",
        data: form_data
      ) do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)

        @hidden_fields.each do |key, value|
          next if value.blank?
          input(type: "hidden", name: key, value: value.to_s)
        end

        div(class: "relative flex items-end rounded-2xl border border-input bg-background shadow-sm " \
                   "transition-[border-color,box-shadow] duration-150 " \
                   "focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/25") do
          textarea(
            name: "content",
            id: @input_id,
            placeholder: @placeholder,
            autocomplete: "off",
            rows: "1",
            aria_label: t(".textarea_aria_label"),
            data: textarea_data,
            class: "flex-1 min-h-[44px] max-h-40 resize-none border-0 border-transparent bg-transparent " \
                   "py-3 pl-4 pr-14 text-[14px] leading-relaxed text-foreground placeholder:text-muted-foreground " \
                   "focus:outline-none focus:ring-0 focus:border-0 focus:shadow-none"
          )
          button(
            type: "submit",
            aria_label: t(".send_aria_label"),
            disabled: true,
            data: { chat_input_target: "submit" },
            class: "absolute right-2 bottom-2 flex h-9 w-9 items-center justify-center rounded-full " \
                   "bg-gradient-to-br from-accent-500 to-accent-600 text-white shadow-sm " \
                   "transition-all duration-150 ease-out hover:from-accent-400 hover:to-accent-600 " \
                   "hover:shadow-md active:scale-95 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
          ) { raw(safe(SEND_ICON)) }

          mention_menu
        end
        div(class: "mt-1.5 px-1 text-[11px] text-muted-foreground") do
          plain t(".keyboard_hint")
        end
      end
    end

    private

    def mentions?
      @mention_candidates.present?
    end

    def form_data
      controllers = [ @stimulus_controller ]
      data = {}
      if mentions?
        controllers << "mention-autocomplete"
        data[:mention_autocomplete_candidates_value] = @mention_candidates.to_json
      end
      data[:controller] = controllers.compact.join(" ")
      data
    end

    # mention-autocomplete actions are listed first so its keydown handler can
    # swallow Enter (and stop chat-input from sending) while the menu is open.
    def textarea_data
      data = { chat_input_target: "input" }
      actions = []
      if mentions?
        data[:mention_autocomplete_target] = "input"
        actions << "keydown->mention-autocomplete#keydown" << "input->mention-autocomplete#onInput"
      end
      actions << "keydown->chat-input#keydown" << "input->chat-input#grow"
      data[:action] = actions.join(" ")
      data
    end

    def mention_menu
      return unless mentions?

      div(
        role: "listbox",
        data: { mention_autocomplete_target: "menu" },
        class: "hidden absolute bottom-full left-0 mb-1 w-64 max-w-[calc(100%-0.5rem)] max-h-56 overflow-y-auto " \
               "rounded-lg border border-border bg-card shadow-md z-50 py-1"
      )
    end
  end
end
