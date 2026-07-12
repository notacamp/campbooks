class Template < ApplicationRecord
  validates :name, presence: true, uniqueness: true

  def document_types
    data["document_types"] || []
  end

  def tags
    data["tags"] || []
  end

  def ai_adapters
    data["ai_adapters"] || []
  end

  def ai_configurations
    data["ai_configurations"] || []
  end

  def apply_document_types(workspace, selected_names = nil)
    items = document_types
    items = items.select { |dt| selected_names.include?(dt["name"]) } if selected_names
    items.each do |attrs|
      workspace.document_types.find_or_create_by!(name: attrs["name"]) do |dt|
        dt.category = attrs["category"]
        dt.color = attrs["color"]
        dt.prompt = attrs["prompt"]
        # Built-in types always get the canonical enriched schema — stored template
        # data may predate the schema-driven field format (label_key/position/typing).
        dt.extraction_schema =
          DocumentTypes::BuiltinSchemas.for(attrs["name"]) || attrs["extraction_schema"]
      end
    end
  end

  def apply_tags(workspace, selected_names = nil)
    items = tags
    items = items.select { |t| selected_names.include?(t["name"]) } if selected_names
    items.each do |attrs|
      workspace.tags.find_or_create_by!(name: attrs["name"]) do |tag|
        tag.color = attrs["color"]
        tag.prompt = attrs["prompt"]
        tag.source = :local
      end
    end
  end

  def apply_ai_configurations(workspace, selected_purposes = nil)
    # First, ensure adapters exist
    adapters_data = ai_adapters
    adapters_data.each do |attrs|
      workspace.ai_adapters.find_or_create_by!(name: attrs["name"]) do |adapter|
        adapter.provider = attrs["provider"]
        adapter.enabled = true
      end
    end

    # Then create purpose → adapter assignments
    items = ai_configurations
    items = items.select { |c| selected_purposes.include?(c["purpose"]) } if selected_purposes
    items.each do |attrs|
      adapter = workspace.ai_adapters.find_by!(name: attrs["ai_adapter_name"])
      workspace.ai_configurations.find_or_create_by!(purpose: attrs["purpose"]) do |config|
        config.ai_adapter = adapter
        config.enabled = true
        config.model = attrs["model"].presence || AiConfiguration::DEFAULT_MODEL[adapter.provider] || "gpt-4o-mini"
        config.max_tokens = attrs["max_tokens"] || 1000
        config.temperature = attrs["temperature"] || 0.0
        config.system_prompt = attrs["system_prompt"] if attrs["system_prompt"].present?
      end
    end
  end
end
