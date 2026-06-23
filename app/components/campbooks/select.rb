module Campbooks
  class Select < Campbooks::Base
    # @param name [String, Symbol] maps to select name/id
    # @param label [String, nil] renders a <label> above the select
    # @param options [Array] array of [label, value] pairs, or array of strings (label = value)
    # @param selected [String, nil] preselected value
    # @param include_blank [String, nil] adds blank option with this label
    def initialize(name, label: nil, options: [], selected: nil, include_blank: nil, **attrs)
      @name = name
      @label = label
      @options = options
      @selected = selected
      @include_blank = include_blank
      @attrs = attrs
    end

    def view_template
      div(class: "space-y-1") do
        if @label
          label(for: select_id, class: "block text-sm font-medium text-gray-700") { @label }
        end

        select(
          name: @name,
          id: select_id,
          class: "mt-1 block w-full rounded-md border-gray-300 shadow-sm text-sm focus:border-accent-500 focus:ring-accent-500",
          **@attrs
        ) do
          if @include_blank
            option(value: "", selected: @selected.nil? || @selected == "") { @include_blank }
          end

          @options.each do |option|
            if option.is_a?(Array)
              label, value = option
            else
              label = option
              value = option
            end

            option(
              value: value,
              selected: value.to_s == @selected.to_s
            ) { label }
          end
        end
      end
    end

    private

    def select_id
      @attrs[:id] || @name.to_s.tr("[]", "_").delete("^a-zA-Z0-9_").squeeze("_").chomp("_")
    end
  end
end
