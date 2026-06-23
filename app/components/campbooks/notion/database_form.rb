# frozen_string_literal: true

module Campbooks
  module Notion
    # Renders one input per writable property of a Notion database, built live from
    # the database schema (the get_database response). Field names are namespaced so
    # the controller can rebuild a properties payload:
    #   scalars      → notion[properties][<Prop>]
    #   multi_select → notion[properties][<Prop>][]
    #   files        → notion[file_props][<Prop>]  (attach the context file)
    #
    # The controller re-reads the schema to learn each property's type, so the client
    # never has to be trusted for types. Read-only / unsupported types are skipped.
    class DatabaseForm < Campbooks::Base
      SCALAR_TYPES = %w[title rich_text number select status multi_select date checkbox url email phone_number].freeze

      # @param schema [Hash] the Notion get_database response
      # @param file_label [String, nil] label of the file that can be attached to a files property
      # @param values [Hash] optional prefill, keyed by property name
      def initialize(schema:, file_label: nil, values: {})
        @schema = schema || {}
        @file_label = file_label
        @values = (values || {}).with_indifferent_access
      end

      def view_template
        if writable_properties.empty?
          div(class: "text-sm text-gray-500") { t(".no_fields") }
          return
        end

        div(class: "space-y-4") do
          writable_properties.each { |name, prop| field(name, prop) }
        end
      end

      private

      # Title first, then source order.
      def writable_properties
        (@schema["properties"] || {}).select { |_n, p| supported?(p["type"]) }
          .sort_by { |_n, p| p["type"] == "title" ? 0 : 1 }
      end

      def supported?(type)
        SCALAR_TYPES.include?(type) || type == "files"
      end

      def field(name, prop)
        type = prop["type"]
        case type
        when "files"
          files_field(name)
        when "select", "status"
          select_field(name, option_names(prop, type), required: false)
        when "multi_select"
          multi_select_field(name, option_names(prop, type))
        when "checkbox"
          checkbox_field(name)
        else
          input_field(name, type)
        end
      end

      def input_field(name, type)
        render Campbooks::Input.new(
          field_name(name),
          type: input_type(type),
          label: name,
          required: type == "title",
          value: @values[name]
        )
      end

      def select_field(name, options, required:)
        render Campbooks::Select.new(
          field_name(name),
          label: name,
          options: options,
          selected: @values[name],
          include_blank: t(".select_blank")
        )
      end

      def multi_select_field(name, options)
        selected = Array(@values[name])
        div(class: "space-y-1") do
          label(class: "block text-sm font-medium text-gray-700") { name }
          div(class: "flex flex-wrap gap-3 pt-1") do
            options.each do |opt|
              render Campbooks::Checkbox.new(
                "#{field_name(name)}[]",
                label: opt,
                value: opt,
                checked: selected.include?(opt)
              )
            end
          end
        end
      end

      def checkbox_field(name)
        div(class: "space-y-1") do
          # Hidden false so an unchecked box still posts a value (Rails idiom).
          input(type: "hidden", name: field_name(name), value: "false")
          render Campbooks::Checkbox.new(
            field_name(name),
            label: name,
            value: "true",
            checked: truthy?(@values[name])
          )
        end
      end

      def files_field(name)
        div(class: "space-y-1") do
          label(class: "block text-sm font-medium text-gray-700") { name }
          if @file_label.present?
            render Campbooks::Checkbox.new(
              "notion[file_props][#{name}]",
              label: t(".attach_file", file: @file_label),
              value: "1",
              checked: true
            )
          else
            p(class: "text-sm text-gray-500") { t(".no_file") }
          end
        end
      end

      def field_name(name)
        "notion[properties][#{name}]"
      end

      def option_names(prop, type)
        ((prop.dig(type, "options")) || []).map { |o| o["name"] }
      end

      def input_type(type)
        case type
        when "number" then :number
        when "date" then :date
        when "url" then :url
        when "email" then :email
        else :text
        end
      end

      def truthy?(value)
        value.to_s.match?(/\A(true|1|yes|on)\z/i)
      end
    end
  end
end
