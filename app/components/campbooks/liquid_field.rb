module Campbooks
  class LiquidField < Campbooks::Base
    def initialize(name:, label:, value: nil, hint: nil, variables: {}, **attrs)
      @name = name
      @label = label
      @value = value
      @hint = hint
      @variables = variables
      @attrs = attrs
    end

    def view_template
      div(**@attrs) do
        div(class: "flex items-center justify-between mb-1.5") do
          label(for: field_id, class: "block text-sm font-medium text-gray-700") { @label }
          render_variables_popover
        end

        textarea(
          name: @name,
          id: field_id,
          rows: 3,
          class: "block w-full rounded-lg border-gray-300 shadow-sm text-sm font-mono focus:border-accent-500 focus:ring-accent-500",
          placeholder: "{{ email.from }}"
        ) { @value.to_s }

        if @hint
          p(class: "mt-1 text-xs text-gray-500") { @hint }
        end
      end
    end

    private

    def field_id
      @field_id ||= @name.to_s.gsub(/[\[\]]/, "_").gsub("__", "_").gsub(/_+$/, "")
    end

    def render_variables_popover
      return unless @variables.any?

      div(class: "relative", data: { controller: "dropdown" }) do
        button(
          type: "button",
          class: "text-xs text-accent-600 hover:text-accent-700 font-medium cursor-pointer",
          data: { action: "click->dropdown#toggle" }
        ) do
          span { t(".variables_toggle") }
        end

        div(
          class: "hidden absolute right-0 top-6 z-20 w-64 bg-card rounded-lg shadow-lg border border-gray-200 p-3",
          data: { dropdown_target: "panel" }
        ) do
          div(class: "text-xs font-medium text-gray-500 mb-2") { t(".available_variables") }
          div(class: "space-y-1.5 max-h-48 overflow-y-auto") do
            @variables.each do |var, desc|
              div(class: "text-xs") do
                code(class: "text-accent-600 font-mono bg-accent-50 px-1 py-0.5 rounded") { "{{ #{var} }}" }
                span(class: "text-gray-500 ml-1.5") { desc }
              end
            end
          end
        end
      end
    end
  end
end
