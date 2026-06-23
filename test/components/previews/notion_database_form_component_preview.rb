# frozen_string_literal: true

class NotionDatabaseFormComponentPreview < ViewComponent::Preview
  # A representative Notion database schema covering every supported property type
  # plus a read-only type (created_time) that should be skipped.
  SCHEMA = {
    "properties" => {
      "Name" => { "type" => "title", "title" => {} },
      "Notes" => { "type" => "rich_text", "rich_text" => {} },
      "Amount" => { "type" => "number", "number" => {} },
      "Status" => { "type" => "status", "status" => { "options" => [ { "name" => "Todo" }, { "name" => "Doing" }, { "name" => "Done" } ] } },
      "Category" => { "type" => "select", "select" => { "options" => [ { "name" => "Invoice" }, { "name" => "Receipt" } ] } },
      "Tags" => { "type" => "multi_select", "multi_select" => { "options" => [ { "name" => "Urgent" }, { "name" => "Personal" } ] } },
      "Due date" => { "type" => "date", "date" => {} },
      "Paid" => { "type" => "checkbox", "checkbox" => {} },
      "Website" => { "type" => "url", "url" => {} },
      "Contact" => { "type" => "email", "email" => {} },
      "Attachment" => { "type" => "files", "files" => {} },
      "Created" => { "type" => "created_time", "created_time" => {} }
    }
  }.freeze

  # Documents context — a single file is available to attach to a files property.
  def with_file
    render(Campbooks::Notion::DatabaseForm.new(schema: SCHEMA, file_label: "invoice-2026.pdf"))
  end

  # No file in context — the files property shows a muted note.
  def without_file
    render(Campbooks::Notion::DatabaseForm.new(schema: SCHEMA))
  end

  def empty_database
    render(Campbooks::Notion::DatabaseForm.new(schema: { "properties" => {} }))
  end
end
