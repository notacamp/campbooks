# frozen_string_literal: true

class EmailAccountFilterPreview < Lookbook::Preview
  # Minimal stand-in for EmailAccount that mirrors the display helpers the
  # component relies on (display_name / select_label / avatar_initial).
  FakeAccount = Struct.new(:id, :color, :name, :email_address, keyword_init: true) do
    def display_name = name.presence || email_address
    def select_label = name.present? ? "#{name} (#{email_address})" : email_address
    def avatar_initial = display_name.strip.first.to_s.upcase
  end

  def default
    render(Campbooks::EmailAccountFilter.new(accounts: [
      FakeAccount.new(id: 1, color: "#3b82f6", email_address: "work@example.com"),
      FakeAccount.new(id: 2, color: "#ef4444", email_address: "personal@example.com"),
      FakeAccount.new(id: 3, color: "#10b981", email_address: "team@example.com")
    ]))
  end

  # Accounts with custom names — avatar initials and tooltips use the name.
  def named_accounts
    render(Campbooks::EmailAccountFilter.new(accounts: [
      FakeAccount.new(id: 1, color: "#3b82f6", name: "Marketing", email_address: "mkt@acme.com"),
      FakeAccount.new(id: 2, color: "#8b5cf6", name: "Support", email_address: "help@acme.com"),
      FakeAccount.new(id: 3, color: "#f59e0b", email_address: "noname@acme.com")
    ]))
  end

  def two_accounts
    render(Campbooks::EmailAccountFilter.new(accounts: [
      FakeAccount.new(id: 1, color: "#8b5cf6", email_address: "primary@acme.com"),
      FakeAccount.new(id: 2, color: "#f59e0b", email_address: "support@acme.com")
    ]))
  end

  def single_account
    render(Campbooks::EmailAccountFilter.new(accounts: [
      FakeAccount.new(id: 1, color: "#3b82f6", email_address: "only@example.com")
    ]))
  end

  def medium_size
    render(Campbooks::EmailAccountFilter.new(accounts: [
      FakeAccount.new(id: 1, color: "#ec4899", email_address: "alice@example.com"),
      FakeAccount.new(id: 2, color: "#06b6d4", email_address: "bob@example.com"),
      FakeAccount.new(id: 3, color: "#f97316", email_address: "carol@example.com")
    ], size: :md))
  end

  def many_accounts
    colors = %w[#3b82f6 #ef4444 #f59e0b #10b981 #8b5cf6 #ec4899 #06b6d4 #f97316]
    accounts = colors.map.with_index do |color, i|
      FakeAccount.new(id: i + 1, color: color, email_address: "account#{i + 1}@example.com")
    end
    render(Campbooks::EmailAccountFilter.new(accounts: accounts))
  end
end
