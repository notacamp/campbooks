# frozen_string_literal: true

module Campbooks
  # A modern, keyboard-accessible multi-select rendered as a wrap of toggleable
  # "pills". Each pill is a hidden checkbox (the real form control — so it needs no
  # JavaScript and tabs/toggles like any checkbox) whose visible label fills in via
  # Tailwind's `peer-checked:` when selected, matching the app's filled-pill accent.
  #
  # Backs an array param (a `name` ending in `[]`): a leading empty hidden field is
  # emitted so clearing every pill still submits an (empty) set, letting the server
  # replace the whole collection. This mirrors the `hidden_field_tag name, ""`
  # pattern the task forms already relied on.
  #
  # Each option is a Hash:
  #   { value:, label:, checked: false, color: "#hex" (optional dot), avatar: "Name" (optional) }
  #
  # @example
  #   Campbooks::PillMultiSelect.new(name: "task[tag_ids][]", options: [
  #     { value: 1, label: "Invoices", color: "#f97316", checked: true },
  #     { value: 2, label: "Receipts", color: "#22c55e" }
  #   ])
  class PillMultiSelect < Campbooks::Base
    def initialize(name:, options:, include_hidden: true)
      @name = name
      @options = options
      @include_hidden = include_hidden
    end

    def view_template
      div(class: "flex flex-wrap gap-1.5") do
        input(type: "hidden", name: @name, value: "") if @include_hidden
        @options.each { |option| pill(option) }
      end
    end

    private

    def pill(option)
      id = field_id(option[:value])
      avatar = option[:avatar].present?

      label(for: id, class: "cursor-pointer select-none") do
        input(
          type: "checkbox",
          id: id,
          name: @name,
          value: option[:value].to_s,
          checked: option[:checked] ? true : false,
          class: "peer sr-only"
        )
        span(class: pill_classes(avatar:)) do
          leading(option)
          span(class: "min-w-0 truncate") { option[:label] }
        end
      end
    end

    def leading(option)
      if option[:avatar].present?
        render Campbooks::Avatar.new(name: option[:avatar], size: :sm)
      elsif option[:color].present?
        span(class: "inline-block h-2 w-2 shrink-0 rounded-full", style: "background-color: #{option[:color]}")
      end
    end

    def pill_classes(avatar:)
      class_names(
        "inline-flex max-w-[16rem] items-center gap-1.5 rounded-full border py-1 pr-3 text-sm font-medium transition",
        avatar ? "pl-1" : "pl-3",
        "border-border bg-card text-foreground hover:border-foreground/30 hover:bg-muted",
        "peer-checked:border-foreground peer-checked:bg-foreground peer-checked:text-background",
        "peer-focus-visible:ring-2 peer-focus-visible:ring-accent-400 peer-focus-visible:ring-offset-1 peer-focus-visible:ring-offset-background"
      )
    end

    def field_id(value)
      base = @name.to_s.tr("[]", "_").squeeze("_").chomp("_")
      "#{base}_#{value}"
    end
  end
end
