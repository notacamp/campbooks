module NotionHelper
  # Notion property types a workflow step can fill in (read-only and files excluded;
  # files are handled separately via the "Files property" field).
  WORKFLOW_WRITABLE_TYPES = %w[
    title rich_text number select status multi_select date checkbox url email phone_number
  ].freeze

  # [[name, prop], ...] of writable properties, title first, for the per-field form.
  def notion_writable_properties(schema)
    (schema["properties"] || {})
      .select { |_name, prop| WORKFLOW_WRITABLE_TYPES.include?(prop["type"]) }
      .sort_by { |_name, prop| prop["type"] == "title" ? 0 : 1 }
  end

  # Extracts a readable title from a Notion database object (search/list result).
  def notion_database_title(db)
    (db["title"] || []).map { |t| t["plain_text"] }.join.presence ||
      t("documents.notion_exports.shared.untitled")
  end

  # Extracts a readable title from a Notion page object by finding its title property.
  def notion_page_title(page)
    prop = (page["properties"] || {}).values.find { |v| v["type"] == "title" }
    ((prop && prop["title"]) || []).map { |t| t["plain_text"] }.join.presence ||
      t("documents.notion_exports.shared.untitled")
  end
end
