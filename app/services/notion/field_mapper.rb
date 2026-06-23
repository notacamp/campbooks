module Notion
  class FieldMapper
    # Maps Campbooks document fields to Notion page properties.
    # Uses the mapping definition stored in NotionDatabaseMapping#field_mappings.
    #
    # field_mappings structure:
    #   { "campbooks_field" => { "notion_property" => "Prop Name", "type" => "rich_text"|"title"|... } }

    def self.to_notion_properties(document, mapping)
      return {} unless mapping&.field_mappings&.any?

      properties = {}

      mapping.field_mappings.each do |campbooks_field, config|
        value = resolve_field(document, campbooks_field)
        next if value.blank?

        notion_prop = config["notion_property"] || campbooks_field.humanize
        prop_type = config["type"] || "rich_text"

        properties[notion_prop] = build_notion_property(value, prop_type)
      end

      # Always add a title property — required by Notion
      unless properties.values.any? { |v| v.key?("title") }
        title = document.display_title.presence || "Untitled Document"
        properties["Name"] = { "title" => [ { "text" => { "content" => title } } ] }
      end

      properties
    end

    def self.from_notion_page(page_data, mapping)
      return {} unless mapping&.field_mappings&.any?

      metadata = {}
      properties = page_data["properties"] || {}

      mapping.field_mappings.each do |campbooks_field, config|
        notion_prop_name = config["notion_property"]
        next unless notion_prop_name

        notion_prop = properties[notion_prop_name]
        next unless notion_prop

        value = extract_notion_value(notion_prop)
        metadata[campbooks_field] = value if value.present?
      end

      metadata
    end

    def self.resolve_field(document, field_name)
      # Check metadata first
      if document.metadata&.key?(field_name)
        return document.metadata[field_name]
      end

      # Check direct columns
      if document.respond_to?(field_name)
        return document.public_send(field_name)
      end

      nil
    end

    def self.build_notion_property(value, type)
      case type
      when "title"
        { "title" => [ { "text" => { "content" => value.to_s } } ] }
      when "rich_text"
        { "rich_text" => [ { "text" => { "content" => value.to_s } } ] }
      when "number"
        num = value.is_a?(Numeric) ? value : value.to_f
        { "number" => num }
      when "date"
        date_str = value.is_a?(Date) ? value.iso8601 : value.to_s
        { "date" => { "start" => date_str } }
      when "select"
        { "select" => { "name" => value.to_s } }
      when "multi_select"
        items = value.is_a?(Array) ? value : [ value ]
        { "multi_select" => items.map { |v| { "name" => v.to_s } } }
      when "checkbox"
        { "checkbox" => value.to_s.match?(/\A(true|yes|1)\z/i) }
      when "url"
        { "url" => value.to_s }
      when "email"
        { "email" => value.to_s }
      when "phone_number"
        { "phone_number" => value.to_s }
      when "status"
        { "status" => { "name" => value.to_s } }
      else
        { "rich_text" => [ { "text" => { "content" => value.to_s } } ] }
      end
    end

    def self.extract_notion_value(property)
      return nil unless property.is_a?(Hash) && property["type"]

      type = property["type"]
      case type
      when "title"
        property["title"]&.first&.dig("text", "content")
      when "rich_text"
        property["rich_text"]&.first&.dig("text", "content")
      when "number"
        property["number"]
      when "date"
        date_data = property["date"]
        date_data ? Date.parse(date_data["start"]) : nil
      when "select"
        property.dig("select", "name")
      when "multi_select"
        property["multi_select"]&.map { |s| s["name"] }
      when "checkbox"
        property["checkbox"]
      when "url"
        property["url"]
      when "email"
        property["email"]
      when "phone_number"
        property["phone_number"]
      when "status"
        property.dig("status", "name")
      when "formula"
        # Formula can return various types
        formula_type = property.dig("formula", "type")
        extract_formula_value(property["formula"], formula_type)
      else
        nil
      end
    end

    def self.extract_formula_value(formula, type)
      case type
      when "string" then formula["string"]
      when "number" then formula["number"]
      when "date" then formula.dig("date", "start")
      when "boolean" then formula["boolean"]
      else nil
      end
    end
  end
end
