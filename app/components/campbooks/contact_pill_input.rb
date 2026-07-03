# frozen_string_literal: true

module Campbooks
  class ContactPillInput < Campbooks::Base
    # bare: renders without the boxed border — for the hairline envelope rows of
    # the new compose surfaces, where the row (not the control) carries focus.
    def initialize(name:, value: "", placeholder: "Search contacts...", label: nil, bare: false, **attrs)
      @name = name
      @value = value
      @placeholder = placeholder
      @label = label
      @bare = bare
      @attrs = attrs
    end

    def view_template
      div(
        data: { controller: "contact-pill-input" },
        class: "relative flex-1 min-w-0"
      ) do
        # Hidden input that stores comma-separated addresses for form submission
        input(
          type: "hidden",
          name: @name,
          value: @value,
          data: { contact_pill_input_target: "hidden" }
        )

        # Pill container + search input, styled like the original form inputs
        div(
          data: { contact_pill_input_target: "pills" },
          class: class_names(
            "flex flex-wrap items-center gap-1 min-h-[28px]",
            @bare ? "bg-transparent" : "bg-card border border-gray-200 rounded-md px-2 py-0.5 focus-within:border-accent-500 focus-within:ring-1 focus-within:ring-accent-500",
            @attrs[:class]
          )
        ) do
          input(
            type: "text",
            data: {
              contact_pill_input_target: "search",
              action: "input->contact-pill-input#search keydown->contact-pill-input#handleKeydown paste->contact-pill-input#handlePaste"
            },
            placeholder: @placeholder,
            class: "flex-1 min-w-[120px] text-sm border-none outline-none bg-transparent px-1 py-0.5"
          )
        end

        # Dropdown
        div(
          data: { contact_pill_input_target: "dropdown" },
          class: "hidden absolute z-50 mt-1 left-0 right-0 bg-card border border-gray-200 rounded-lg shadow-lg overflow-hidden max-h-48 overflow-y-auto"
        )
      end
    end
  end
end
