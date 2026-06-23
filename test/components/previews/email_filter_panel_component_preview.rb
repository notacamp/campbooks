# frozen_string_literal: true

class EmailFilterPanelComponentPreview < ViewComponent::Preview
  FOLDERS = [ { id: "1", name: "Inbox" }, { id: "2", name: "Sent" }, { id: "3", name: "Archive" } ].freeze
  Account = Struct.new(:id, :email_address)
  Tag = Struct.new(:id, :name, :color)
  ACCOUNTS = [ Account.new(1, "me@work.com"), Account.new(2, "me@personal.com") ].freeze
  TAGS = [ Tag.new(1, "Invoice", "#3b82f6"), Tag.new(2, "Receipt", "#10b981"), Tag.new(3, "Urgent", "#ef4444") ].freeze

  def default
    render(Campbooks::EmailFilterPanel.new(folders: FOLDERS, accounts: ACCOUNTS, tags: TAGS))
  end

  def with_selections
    render(Campbooks::EmailFilterPanel.new(
      folders: FOLDERS, accounts: ACCOUNTS, tags: TAGS,
      active: { folder: "Archive", account_ids: [ "1" ], tag_ids: [ "1", "3" ], tag_match: "all",
                unread: "1", has_attachment: "1", priority: "high", category: "important",
                sender: "billing@", domain: "stripe.com", date_from: "2026-01-01" }
    ))
  end
end
