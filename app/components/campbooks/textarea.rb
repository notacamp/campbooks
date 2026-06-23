module Campbooks
  class Textarea < Campbooks::Base
    # @param name [String, Symbol] maps to textarea name/id
    # @param label [String, nil] renders a <label> above the textarea
    # @param placeholder [String, nil]
    # @param rows [Integer] default 4
    # @param error [String, nil] shows red border + error message
    # @param hint [String, nil] small text below textarea
    # @param rounded [Symbol] :lg (default, full-width) or :md (inline/filter)
    def initialize(name, label: nil, placeholder: nil, rows: 4, error: nil, hint: nil,
                   rounded: :lg, **attrs)
      @name = name
      @label = label
      @placeholder = placeholder
      @rows = rows
      @error = error
      @hint = hint
      @rounded = rounded
      @attrs = attrs
    end

    def view_template
      div(class: "space-y-1") do
        if @label
          label(for: textarea_id, class: "block text-sm font-medium text-gray-700 mb-1.5") do
            span { @label }
          end
        end

        textarea(
          name: @name,
          id: textarea_id,
          placeholder: @placeholder,
          rows: @rows,
          class: textarea_classes,
          aria_invalid: @error ? true : nil,
          aria_describedby: describedby_id,
          **@attrs
        )

        if @error
          p(class: "text-sm text-red-600 dark:text-red-400", id: error_id) { @error }
        end

        if @hint && !@error
          p(class: "text-sm text-gray-500", id: hint_id) { @hint }
        end
      end
    end

    private

    def textarea_id
      @attrs[:id] || @name.to_s.tr("[]", "_").delete("^a-zA-Z0-9_").squeeze("_").chomp("_")
    end

    def error_id
      "#{textarea_id}-error"
    end

    def hint_id
      "#{textarea_id}-hint"
    end

    def describedby_id
      if @error
        error_id
      elsif @hint
        hint_id
      end
    end

    def textarea_classes
      class_names(
        "block w-full shadow-sm text-sm border-gray-300 focus:border-accent-500 focus:ring-accent-500",
        @rounded == :lg ? "rounded-lg" : "rounded-md",
        @error && "border-red-500 focus:border-red-500 focus:ring-red-500 dark:border-red-500/40"
      )
    end
  end
end
