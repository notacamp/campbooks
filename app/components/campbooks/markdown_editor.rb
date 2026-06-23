module Campbooks
  class MarkdownEditor < Campbooks::Base
    # @param name [String, Symbol] maps to textarea name/id
    # @param value [String] initial markdown content
    # @param label [String, nil]
    # @param rows [Integer] number of visible rows, default 3
    # @param placeholder [String, nil]
    def initialize(name, value: nil, label: nil, rows: 3, placeholder: nil, **attrs)
      @name = name
      @value = value
      @label = label
      @rows = rows
      @placeholder = placeholder
      @attrs = attrs
    end

    def view_template
      div(
        class: "markdown-editor",
        data: {
          controller: "markdown-editor",
          markdown_editor_rows_value: @rows
        }
      ) do
        if @label
          label(for: textarea_id, class: "block text-sm font-medium text-gray-700 mb-1") { @label }
        end

        div(class: "flex border-b border-gray-200", role: "tablist") do
          button(
            type: "button",
            data: { action: "click->markdown-editor#showWrite", md_tab: "write" },
            class: "flex-1 px-3 py-1.5 text-xs font-medium border-b-2 border-accent-600 text-accent-600 bg-accent-50/50 rounded-tl-md transition-colors",
            role: "tab",
            aria_selected: "true"
          ) { t(".write_tab") }

          button(
            type: "button",
            data: { action: "click->markdown-editor#showPreview", md_tab: "preview" },
            class: "flex-1 px-3 py-1.5 text-xs font-medium border-b-2 border-transparent text-gray-500 hover:text-gray-700 rounded-tr-md transition-colors",
            role: "tab",
            aria_selected: "false"
          ) { t(".preview_tab") }
        end

        textarea(
          name: @name,
          id: textarea_id,
          placeholder: @placeholder,
          rows: @rows,
          class: "block w-full rounded-b-lg rounded-t-none border border-t-0 border-gray-300 shadow-sm text-sm focus:border-accent-500 focus:ring-accent-500",
          data: { markdown_editor_target: "textarea" },
          **@attrs
        ) { @value.to_s }

        div(
          data: { markdown_editor_target: "preview" },
          class: "hidden prose prose-sm max-w-none rounded-b-lg border border-t-0 border-gray-300 bg-card p-3 min-h-[60px] text-sm text-gray-700"
        )
      end
    end

    private

    def textarea_id
      @attrs[:id] || @name.to_s.tr("[]", "_").delete("^a-zA-Z0-9_").squeeze("_").chomp("_")
    end
  end
end
