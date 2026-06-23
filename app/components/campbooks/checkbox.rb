module Campbooks
  class Checkbox < Campbooks::Base
    # @param name [String, Symbol] maps to checkbox name/id
    # @param label [String] required for accessibility, rendered to the right
    # @param checked [Boolean]
    # @param value [String] defaults to "1"
    # @param disabled [Boolean]
    def initialize(name, label:, checked: false, value: "1", disabled: false, **attrs)
      @name = name
      @label = label
      @checked = checked
      @value = value
      @disabled = disabled
      @attrs = attrs
    end

    def view_template
      label(for: checkbox_id, class: "flex items-center gap-2 cursor-pointer") do
        input(
          type: "checkbox",
          name: @name,
          id: checkbox_id,
          value: @value,
          checked: @checked,
          disabled: @disabled,
          class: "w-3.5 h-3.5 rounded border-gray-300 text-accent-600 focus:ring-accent-500",
          **@attrs
        )
        span(class: "text-sm text-gray-700") { @label }
      end
    end

    private

    def checkbox_id
      @attrs[:id] || @name.to_s.tr("[]", "_").delete("^a-zA-Z0-9_").squeeze("_").chomp("_")
    end
  end
end
