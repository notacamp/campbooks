# frozen_string_literal: true

require "ostruct"

class TableComponentPreview < ViewComponent::Preview
  def default
    columns = [
      { key: :name, label: "Name" },
      { key: :email, label: "Email" },
      { key: :role, label: "Role", align: :center }
    ]
    rows = [
      OpenStruct.new(name: "Alice Smith", email: "alice@example.com", role: "Admin"),
      OpenStruct.new(name: "Bob Jones", email: "bob@example.com", role: "Editor"),
      OpenStruct.new(name: "Carol White", email: "carol@example.com", role: "Viewer")
    ]
    render(Campbooks::Table.new(columns: columns, rows: rows))
  end

  def with_custom_cells
    columns = [
      { key: :name, label: "Name" },
      { key: :role, label: "Role" },
      { key: :status, label: "Status" }
    ]
    rows = [
      OpenStruct.new(name: "Alice", role: "admin", status: "active"),
      OpenStruct.new(name: "Bob", role: "editor", status: "inactive"),
      OpenStruct.new(name: "Carol", role: "viewer", status: "active")
    ]
    render(Campbooks::Table.new(columns: columns, rows: rows)) do |row, col|
      case col[:key]
      when :role
        "<span class=\"font-medium text-accent-600\">#{row.public_send(col[:key]).to_s.humanize}</span>".html_safe
      when :status
        color = row.public_send(:status) == "active" ? "text-green-600" : "text-gray-400"
        "<span class=\"#{color}\">#{row.public_send(col[:key]).to_s.humanize}</span>".html_safe
      else
        "<span class=\"font-medium text-gray-900\">#{row.public_send(col[:key])}</span>".html_safe
      end
    end
  end

  def with_clickable_rows
    columns = [
      { key: :name, label: "Name", width: "w-1/2" },
      { key: :email, label: "Email", width: "w-1/2" }
    ]
    rows = [
      OpenStruct.new(name: "Alice Smith", email: "alice@example.com", id: 1),
      OpenStruct.new(name: "Bob Jones", email: "bob@example.com", id: 2),
      OpenStruct.new(name: "Carol White", email: "carol@example.com", id: 3)
    ]
    render(Campbooks::Table.new(columns: columns, rows: rows, row_url: ->(row) { "/users/#{row.id}" }))
  end

  def empty
    columns = [
      { key: :name, label: "Name" },
      { key: :email, label: "Email" }
    ]
    render(Campbooks::Table.new(columns: columns, rows: [], empty_state: "No users found matching your criteria."))
  end
end
