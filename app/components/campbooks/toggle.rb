# frozen_string_literal: true

module Campbooks
  class Toggle < Campbooks::Base
    # @param name [String] input name attribute (required, for form submission)
    # @param checked [Boolean] initial checked state (default false)
    # @param disabled [Boolean] disabled state (default false)
    # @param label [String, nil] optional label text rendered to the right of the toggle
    def initialize(name:, checked: false, disabled: false, label: nil, **attrs)
      @name = name
      @checked = checked
      @disabled = disabled
      @label = label
      @attrs = attrs
    end

    def view_template
      label(class: label_classes) do
        div(class: "relative") do
          input(
            type: :checkbox,
            name: @name,
            checked: @checked,
            disabled: @disabled,
            class: "sr-only peer",
            **@attrs
          )

          div(class: track_classes)
          div(class: knob_classes)
        end

        if @label
          span(class: "text-sm text-gray-900 select-none") { @label }
        end
      end
    end

    private

    def label_classes
      class_names(
        "flex items-center gap-2",
        (@disabled ? "cursor-not-allowed" : "cursor-pointer")
      )
    end

    def track_classes
      "w-9 h-5 bg-gray-200 peer-checked:bg-accent-600 peer-disabled:opacity-50 " \
      "peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-accent-300 " \
      "rounded-full transition-colors"
    end

    def knob_classes
      "absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full shadow " \
      "peer-checked:translate-x-4 transition-transform"
    end
  end
end
