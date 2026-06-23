module Campbooks
  class Input < Campbooks::Base
    # @param name [String, Symbol] maps to input name/id
    # @param type [Symbol] :text (default), :email, :password, :number, :date, :url
    # @param label [String, nil] renders a <label> above the input
    # @param placeholder [String, nil]
    # @param value [String, nil]
    # @param error [String, nil] shows red border + error message
    # @param required [Boolean] adds required marker on label
    # @param hint [String, nil] small text below input
    # @param disabled [Boolean]
    # @param rounded [Symbol] :lg (default, full-width) or :md (inline/filter)
    def initialize(name, type: :text, label: nil, placeholder: nil, value: nil, error: nil,
                   required: false, hint: nil, disabled: false, rounded: :lg, **attrs)
      @name = name
      @type = type
      @label = label
      @placeholder = placeholder
      @value = value
      @error = error
      @required = required
      @hint = hint
      @disabled = disabled
      @rounded = rounded
      @attrs = attrs
    end

    def view_template
      div(class: "space-y-1") do
        if @label
          label(for: input_id, class: "block text-sm font-medium text-gray-700 mb-1.5") do
            span { @label }
            if @required
              span(class: "text-red-500 dark:text-red-400 ml-0.5") { "*" }
            end
          end
        end

        input(
          type: @type,
          name: @name,
          id: input_id,
          placeholder: @placeholder,
          value: display_value,
          disabled: @disabled,
          required: @required,
          class: input_classes,
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

    # Phlex only accepts primitive attribute values (String/Symbol/Integer/Float/
    # bool/nil). Model values reach us as BigDecimal (tax_rate), Date (document_date),
    # Money, etc., which would raise Phlex::ArgumentError. Coerce to a string here so
    # every caller can pass a raw model attribute. BigDecimal needs to_s("F") or it
    # renders in scientific notation (0.23e2 instead of 23.0).
    def display_value
      return nil if @value.nil?
      return @value.to_s("F") if @value.is_a?(BigDecimal)

      @value.to_s
    end

    def input_id
      @attrs[:id] || @name.to_s.tr("[]", "_").delete("^a-zA-Z0-9_").squeeze("_").chomp("_")
    end

    def error_id
      "#{input_id}-error"
    end

    def hint_id
      "#{input_id}-hint"
    end

    def describedby_id
      if @error
        error_id
      elsif @hint
        hint_id
      end
    end

    def input_classes
      class_names(
        "block w-full shadow-sm text-sm border-gray-300 focus:border-accent-500 focus:ring-accent-500",
        @rounded == :lg ? "rounded-lg" : "rounded-md",
        @error && "border-red-500 focus:border-red-500 focus:ring-red-500 dark:border-red-500/40",
        @disabled && "bg-gray-100 text-gray-500 cursor-not-allowed"
      )
    end
  end
end
