# frozen_string_literal: true

class PageHeaderComponentPreview < ViewComponent::Preview
  # Page header with title only.
  def default
    render(Campbooks::PageHeader.new(title: "Dashboard"))
  end

  # Page header with title and subtitle.
  def with_subtitle
    render(Campbooks::PageHeader.new(title: "Documents", subtitle: "Manage and review all documents in your workspace"))
  end

  # Page header with title and action buttons.
  def with_actions
    render(Campbooks::PageHeader.new(title: "Contacts")) do |header|
      header.with_actions do
        button(class: "px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 transition-colors cursor-pointer", type: :button) { "Import" }
        button(class: "px-4 py-2 text-sm font-medium rounded-lg bg-accent-600 text-white hover:bg-accent-700 transition-colors cursor-pointer", type: :button) { "Add Contact" }
      end
    end
  end

  # Page header with subtitle, actions, and lg spacing.
  def all_together
    render(Campbooks::PageHeader.new(title: "Email Accounts", subtitle: "Manage your connected email accounts and scan settings", spacing: :lg)) do |header|
      header.with_actions do
        button(class: "px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 transition-colors cursor-pointer", type: :button) { "Refresh" }
        button(class: "px-4 py-2 text-sm font-medium rounded-lg bg-accent-600 text-white hover:bg-accent-700 transition-colors cursor-pointer", type: :button) { "Connect Account" }
      end
    end
  end
end
