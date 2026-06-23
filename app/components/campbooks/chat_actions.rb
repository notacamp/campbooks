# frozen_string_literal: true

module Campbooks
  class ChatActions < Campbooks::Base
    # @param auto_actions [Array<Hash>] executed actions with "success", "message", "tool" keys
    # @param suggested_actions [Array<Hash>] pending actions with "tool", "args", "label" keys
    # @param tool_url_builder [Proc, nil] callable(tool, args) -> { url:, method:, data: {} } or nil
    def initialize(auto_actions: [], suggested_actions: [], tool_url_builder: nil, message_id: nil, wrapper_class: "mt-3 flex items-center gap-2 flex-wrap")
      @auto_actions = auto_actions || []
      @suggested_actions = suggested_actions || []
      @tool_url_builder = tool_url_builder
      @message_id = message_id
      @wrapper_class = wrapper_class
    end

    def view_template
      return if @auto_actions.empty? && @suggested_actions.empty?

      div(id: @message_id ? "actions_agent_message_#{@message_id}" : nil,
          class: @wrapper_class) do
        @auto_actions.each { |a| render_auto_action(a) }
        @suggested_actions.each { |a| render_suggested_action(a) }
      end
    end

    private

    CHECK_SVG = '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7"/></svg>'.freeze
    CROSS_SVG = '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M6 18L18 6M6 6l12 12"/></svg>'.freeze

    def render_auto_action(action)
      if action["success"]
        span(class: "inline-flex items-center gap-1 text-[12px] font-medium text-green-700 bg-green-50 border border-green-200 dark:text-green-300 dark:bg-green-500/10 dark:border-green-500/25 rounded-full px-3 py-1") do
          raw(safe(CHECK_SVG))
          plain action["message"].to_s
        end
      else
        span(class: "inline-flex items-center gap-1 text-[12px] font-medium text-red-700 bg-red-50 border border-red-200 dark:text-red-300 dark:bg-red-500/10 dark:border-red-500/25 rounded-full px-3 py-1") do
          raw(safe(CROSS_SVG))
          plain action["message"].to_s
        end
      end
    end

    def render_suggested_action(action)
      tool = action["tool"]
      args = action["args"] || {}
      label = action["label"].presence || suggested_label(tool, args)
      style = button_style(tool)

      if @tool_url_builder
        url_info = @tool_url_builder.call(tool, args)
        if url_info
          form_data = { turbo_stream: true }.merge(url_info[:data] || {})
          render_form(url_info[:url], url_info[:method] || :post, form_data, style, tool, label)
          return
        end
      end

      button(
        type: "button",
        class: "inline-flex items-center gap-1 text-[12px] font-medium transition-colors rounded-full px-3 py-1 #{style}"
      ) { raw(safe("#{action_icon(tool)}#{label}")) }
    end

    def render_form(url, method, data, style, tool, label)
      form(action: url, method: :post, class: "inline", data: data) do
        if method != :post
          input(type: "hidden", name: "_method", value: method.to_s)
        end
        input(type: "hidden", name: "agent_message_id", value: @message_id.to_s) if @message_id
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        button(type: "submit",
               class: "inline-flex items-center gap-1 text-[12px] font-medium transition-colors rounded-full px-3 py-1 #{style}") do
          raw(safe("#{action_icon(tool)}#{label}"))
        end
      end
    end

    def suggested_label(tool, args)
      case tool
      when "bulk_archive", "archive" then t(".archive")
      when "bulk_tag"         then args["action"] == "remove" ? t(".remove_with_name", name: args["tag_name"]) : t(".tag_with_name", name: args["tag_name"])
      when "add_tag"           then t(".tag_prefix", name: args["tag_name"])
      when "remove_tag"        then t(".remove_tag_prefix", name: args["tag_name"])
      when "reclassify"        then t(".reclassify")
      when "draft_reply"       then t(".draft_reply")
      when "forward_email"     then t(".forward_to", address: args["to_address"])
      when "create_calendar_event" then t(".create_event")
      when "snooze"            then t(".snooze")
      when "unsnooze"          then t(".unsnooze")
      when "star_sender"       then t(".star_sender")
      when "unstar_sender"     then t(".unstar_sender")
      when "block_sender"      then t(".block_sender")
      when "unblock_sender"    then t(".unblock_sender")
      when "allow_sender"      then t(".allow_sender")
      else tool.to_s.humanize
      end
    end

    def button_style(tool)
      case tool
      when "bulk_tag", "add_tag", "remove_tag"
        args = @suggested_actions.find { |a| a["tool"] == tool }&.dig("args") || {}
        args["action"] == "remove" || tool == "remove_tag" ?
          "text-gray-600 bg-gray-100 hover:bg-gray-200 border border-gray-300" :
          "text-blue-700 bg-blue-50 hover:bg-blue-100 border border-blue-200 dark:text-blue-300 dark:bg-blue-500/10 dark:border-blue-500/25"
      when "bulk_archive", "archive"
        "text-gray-600 bg-gray-100 hover:bg-gray-200 border border-gray-300"
      when "reclassify"
        "text-accent-700 bg-accent-50 hover:bg-accent-100 border border-accent-200"
      when "draft_reply", "forward_email", "create_calendar_event"
        "text-accent-700 bg-accent-50 hover:bg-accent-100 border border-accent-200"
      when "snooze"
        "text-yellow-700 bg-yellow-50 hover:bg-yellow-100 border border-yellow-200 dark:text-yellow-300 dark:bg-yellow-500/10 dark:border-yellow-500/25"
      when "unsnooze"
        "text-gray-600 bg-gray-100 hover:bg-gray-200 border border-gray-300"
      when "star_sender", "allow_sender"
        "text-accent-700 bg-accent-50 hover:bg-accent-100 border border-accent-200 dark:text-accent-300 dark:bg-accent-500/10 dark:border-accent-500/25"
      when "block_sender"
        "text-red-700 bg-red-50 hover:bg-red-100 border border-red-200 dark:text-red-300 dark:bg-red-500/10 dark:border-red-500/25"
      when "unstar_sender", "unblock_sender"
        "text-gray-600 bg-gray-100 hover:bg-gray-200 border border-gray-300"
      else
        "text-gray-600 bg-gray-100 hover:bg-gray-200 border border-gray-300"
      end
    end

    def action_icon(tool)
      case tool
      when "bulk_archive", "archive"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/></svg>'
      when "bulk_tag", "add_tag"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4"/></svg>'
      when "remove_tag"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M6 18L18 6M6 6l12 12"/></svg>'
      when "reclassify"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>'
      when "draft_reply"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/></svg>'
      when "forward_email"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M13 7l5 5m0 0l-5 5m5-5H6"/></svg>'
      when "create_calendar_event"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"/></svg>'
      when "snooze"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'
      when "unsnooze"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0"/></svg>'
      when "star_sender", "unstar_sender"
        '<svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path d="M11.48 3.5a.56.56 0 011.04 0l2.12 4.92 5.34.46c.49.04.69.66.31.98l-4.05 3.5 1.21 5.22c.11.48-.41.86-.83.6L12 17.27l-4.63 2.91c-.42.26-.94-.12-.83-.6l1.21-5.22-4.05-3.5c-.38-.32-.18-.94.31-.98l5.34-.46 2.12-4.92z"/></svg>'
      when "block_sender"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18.364 5.636l-12.728 12.728M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'
      when "allow_sender", "unblock_sender"
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'
      else
        '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7"/></svg>'
      end
    end
  end
end
