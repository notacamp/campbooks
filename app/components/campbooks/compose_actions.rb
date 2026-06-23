# frozen_string_literal: true

module Campbooks
  class ComposeActions < Campbooks::Base
    def initialize(actions:)
      @actions = actions || []
    end

    def view_template
      return if @actions.empty?

      body_actions = @actions.select { |a| a["tool"] == "set_body" }
      other_actions = @actions.reject { |a| a["tool"] == "set_body" }

      div(class: "mt-3 space-y-2") do
        # All non-body actions as a single row of buttons
        unless other_actions.empty?
          div(class: "flex items-center gap-1.5 flex-wrap") do
            other_actions.each do |action|
              tool = action["tool"]
              style = section_styles[tool]
              render_button(action, style[:color]) if style
            end
          end
        end

        # Body preview cards below
        render_body_actions(body_actions) if body_actions.any?
      end
    end

    private

    def render_button(action, color)
      args = action["args"] || {}
      tool = action["tool"]

      button_attrs = {
        type: "button",
        class: class_names(
          "inline-flex items-center gap-1 text-[11px] font-medium rounded-full px-2.5 py-0.5 transition-colors border",
          BUTTON_COLORS[color]
        ),
        data: data_attrs_for(tool, args)
      }

      button(**button_attrs) do
        raw(safe('<svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/></svg>'))
        plain(action["label"].to_s)
      end
    end

    def section_styles
      labels = t(".section_labels")
      {
        "select_account" => { label: labels[:select_account], color: "blue" },
        "set_recipients"  => { label: labels[:set_recipients],  color: "green" },
        "set_subject"     => { label: labels[:set_subject],     color: "purple" },
        "set_signature"   => { label: labels[:set_signature],   color: "yellow" },
        "set_body"        => { label: labels[:set_body],        color: "accent" },
        "send_email"      => { label: labels[:send_email],      color: "red" }
      }
    end

    def render_body_actions(group)
      div(class: "mt-1 space-y-2") do
        group.each do |action|
          args = action["args"] || {}
          body_text = args["body"].to_s

          div(class: "bg-accent-50 border border-accent-200 rounded-lg px-3 py-2") do
            div(class: "text-[12px] leading-relaxed text-gray-800 max-h-32 overflow-y-auto mb-2", style: "font-family: system-ui, sans-serif;") do
              raw(safe(body_text))
            end
            button(
              type: "button",
              class: "inline-flex items-center gap-1 text-[11px] font-medium text-accent-700 bg-accent-100 hover:bg-accent-200 border border-accent-300 rounded-full px-2.5 py-0.5 transition-colors",
              data: { action: "click->compose-chat#setBody", compose_chat_body_param: body_text }
            ) do
              raw(safe('<svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/></svg>'))
              plain(t(".use_this_draft"))
            end
          end
        end
      end
    end

    def data_attrs_for(tool, args)
      case tool
      when "select_account"
        { action: "click->compose-chat#setFromAccount", compose_chat_account_id_param: args["account_id"] }
      when "set_recipients"
        { action: "click->compose-chat#setRecipients", compose_chat_to_param: args["to"], compose_chat_cc_param: args["cc"] }
      when "set_subject"
        { action: "click->compose-chat#setSubject", compose_chat_subject_param: args["subject"] }
      when "set_signature"
        { action: "click->compose-chat#setSignature", compose_chat_signature_id_param: args["signature_id"] }
      when "send_email"
        { action: "click->compose-chat#sendEmail" }
      else
        {}
      end
    end

    BUTTON_COLORS = {
      "blue"    => "text-blue-700 bg-blue-50 hover:bg-blue-100 border-blue-200 dark:text-blue-300 dark:bg-blue-500/10 dark:border-blue-500/25",
      "green"   => "text-green-700 bg-green-50 hover:bg-green-100 border-green-200 dark:text-green-300 dark:bg-green-500/10 dark:border-green-500/25",
      "purple"  => "text-purple-700 bg-purple-50 hover:bg-purple-100 border-purple-200 dark:text-purple-300 dark:bg-purple-500/10 dark:border-purple-500/25",
      "yellow"  => "text-yellow-700 bg-yellow-50 hover:bg-yellow-100 border-yellow-200 dark:text-yellow-300 dark:bg-yellow-500/10 dark:border-yellow-500/25",
      "accent"  => "text-accent-700 bg-accent-50 hover:bg-accent-100 border-accent-200",
      "red"     => "text-red-700 bg-red-50 hover:bg-red-100 border-red-200 dark:text-red-300 dark:bg-red-500/10 dark:border-red-500/25"
    }.freeze
  end
end
