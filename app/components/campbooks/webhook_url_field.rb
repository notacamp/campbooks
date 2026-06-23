module Campbooks
  # Read-only display of a workflow's inbound webhook URL with a one-click copy
  # button. When the URL isn't ready yet (the workflow hasn't been saved as a
  # webhook), it falls back to a hint instead.
  class WebhookUrlField < Campbooks::Base
    # @param url [String, nil] the full webhook URL, or nil if not generated yet
    # @param label [String] field label
    # @param hint [String, nil] helper text under the field
    def initialize(url:, label: nil, hint: nil, **attrs)
      @url = url
      @label = label
      @hint = hint
      @attrs = attrs
    end

    def view_template
      div(**@attrs) do
        label(class: "block text-sm font-medium text-gray-700 mb-1.5") { @label || t(".default_label") }

        if @url.present?
          render_field
        else
          render_pending
        end

        if @hint
          p(class: "mt-1.5 text-xs text-gray-500") { @hint }
        end
      end
    end

    private

    def render_field
      div(
        class: "flex flex-col gap-2 sm:flex-row sm:items-stretch",
        data: { controller: "clipboard" }
      ) do
        input(
          type: "text",
          value: @url,
          readonly: true,
          data: { clipboard_target: "source" },
          class: "block w-full min-w-0 rounded-lg border-gray-300 bg-gray-50 text-sm font-mono text-gray-700 shadow-sm focus:border-accent-500 focus:ring-accent-500 dark:bg-gray-800/50"
        )
        button(
          type: "button",
          data: { clipboard_target: "button", action: "clipboard#copy" },
          class: "inline-flex items-center justify-center gap-1.5 px-3 py-2 bg-gray-100 hover:bg-gray-200 text-gray-700 text-sm font-medium rounded-lg transition-colors cursor-pointer whitespace-nowrap dark:bg-gray-700 dark:hover:bg-gray-600 dark:text-gray-200"
        ) do
          raw safe('<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/></svg>')
          plain t(".copy")
        end
      end
    end

    def render_pending
      div(class: "rounded-lg border border-dashed border-gray-300 bg-gray-50 px-3 py-2.5 text-sm text-gray-500 dark:bg-gray-800/50") do
        plain t(".pending_hint")
      end
    end
  end
end
