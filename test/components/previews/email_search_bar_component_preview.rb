# frozen_string_literal: true

class EmailSearchBarComponentPreview < ViewComponent::Preview
  FOLDERS = [ { id: "1", name: "Inbox" }, { id: "2", name: "Sent" }, { id: "3", name: "Archive" } ].freeze
  Account = Struct.new(:id, :email_address)
  Tag = Struct.new(:id, :name, :color)
  ACCOUNTS = [ Account.new(1, "me@work.com"), Account.new(2, "me@personal.com") ].freeze
  TAGS = [ Tag.new(1, "Invoice", "#3b82f6"), Tag.new(2, "Receipt", "#10b981"), Tag.new(3, "Urgent", "#ef4444") ].freeze

  def default
    render(Campbooks::EmailSearchBar.new(folders: FOLDERS, accounts: ACCOUNTS, tags: TAGS))
  end

  def with_query
    render(Campbooks::EmailSearchBar.new(search_params: { q: "invoice march" }, folders: FOLDERS, accounts: ACCOUNTS, tags: TAGS))
  end

  # Filters applied → the Filters button shows an active-count badge.
  def with_active_filters
    render(Campbooks::EmailSearchBar.new(
      search_params: { folder: "Archive", unread: "1", priority: "high", tag_ids: [ "1" ] },
      folders: FOLDERS, accounts: ACCOUNTS, tags: TAGS
    ))
  end
end
