module Notion
  # Turns a flat map of user-supplied/templated values into a Notion `properties`
  # payload for create/update. Reuses Notion::FieldMapper.build_notion_property for
  # the common scalar types and layers on files (via uploaded file ids), people and
  # relation. Read-only Notion property types are silently skipped.
  #
  #   inputs       => { "Name" => { type: "title", value: "Invoice 42" }, ... }
  #   file_uploads => { "Attachment" => [ { id: "abc", name: "invoice.pdf" } ] }
  class PropertyBuilder
    READ_ONLY_TYPES = %w[
      formula rollup created_time created_by last_edited_time last_edited_by unique_id
    ].freeze

    def self.build(inputs, file_uploads: {})
      props = {}

      (inputs || {}).each do |name, spec|
        spec = spec.with_indifferent_access if spec.respond_to?(:with_indifferent_access)
        type = (spec[:type] || "rich_text").to_s
        value = spec[:value]

        next if READ_ONLY_TYPES.include?(type)
        # Checkbox legitimately accepts "false"/blank; everything else skips blanks.
        next if value.blank? && type != "checkbox"

        built = build_property(value, type)
        props[name] = built unless built.nil?
      end

      (file_uploads || {}).each do |name, files|
        files = Array(files).compact
        next if files.empty?
        props[name] = Notion::FileUploader.files_property(
          files.map { |f| f[:id] || f["id"] },
          names: files.map { |f| f[:name] || f["name"] }
        )
      end

      props
    end

    def self.build_property(value, type)
      case type
      when "people"
        ids = value.is_a?(Array) ? value : value.to_s.split(",").map(&:strip)
        { "people" => ids.reject(&:blank?).map { |id| { "object" => "user", "id" => id } } }
      when "relation"
        ids = value.is_a?(Array) ? value : value.to_s.split(",").map(&:strip)
        { "relation" => ids.reject(&:blank?).map { |id| { "id" => id } } }
      when "files"
        # External URL fallback when no upload id is available.
        urls = value.is_a?(Array) ? value : [ value ]
        { "files" => urls.reject(&:blank?).map { |u| { "type" => "external", "name" => "file", "external" => { "url" => u.to_s } } } }
      else
        Notion::FieldMapper.build_notion_property(value, type)
      end
    end
  end
end
