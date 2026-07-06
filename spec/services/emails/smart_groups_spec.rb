require "rails_helper"

RSpec.describe Emails::SmartGroups do
  before do
    @workspace = Workspace.create!(name: "Smart Groups WS")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
  end

  def service
    described_class.new(@user, [ @account.id ])
  end

  def create_thread(*categories, subject: "Thread", from: "sender@bulk.test", contact: nil)
    thread = @account.email_threads.create!(subject: subject)
    categories.each_with_index do |category, i|
      @account.email_messages.create!(
        email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
        provider_folder_id: "INBOX", from_address: from, to_address: @account.email_address,
        subject: subject, received_at: (categories.size - i).hours.ago, read: false,
        has_attachment: false, category: category, contact: contact
      )
    end
    thread
  end

  it "bundles a thread whose messages are all in noise buckets" do
    thread = create_thread("promotions", "promotions")

    expect(service.bundled_scope.pluck(:id)).to include(thread.id)
  end

  it "keeps a thread inline when any message is personal or important" do
    mixed = create_thread("promotions", "personal")
    alert = create_thread("notifications", "important")

    bundled = service.bundled_scope.pluck(:id)
    expect(bundled).not_to include(mixed.id)
    expect(bundled).not_to include(alert.id)
  end

  it "keeps a thread inline when any message has no category (fail-open)" do
    thread = create_thread("promotions", nil)

    expect(service.bundled_scope.pluck(:id)).not_to include(thread.id)
  end

  it "never bundles a thread the user replied to" do
    thread = create_thread("promotions")
    thread.update!(last_outbound_at: Time.current)

    expect(service.bundled_scope.pluck(:id)).not_to include(thread.id)
  end

  it "never bundles a pinned thread" do
    thread = create_thread("promotions")
    thread.email_messages.first.update!(pinned_at: Time.current)

    expect(service.bundled_scope.pluck(:id)).not_to include(thread.id)
  end

  it "never bundles mail from a starred contact" do
    contact = @workspace.contacts.create!(email: "vip@bulk.test", starred_at: Time.current)
    thread = create_thread("promotions", contact: contact)

    expect(service.bundled_scope.pluck(:id)).not_to include(thread.id)
  end

  it "bundled_scope is nil when the master toggle is off" do
    create_thread("promotions")
    @user.update!(inbox_smart_groups: { "enabled" => false })

    expect(service.bundled_scope).to be_nil
  end

  it "a disabled bucket's threads stay inline" do
    thread = create_thread("promotions")
    @user.update!(inbox_smart_groups: { "promotions" => false })

    bundled = service.bundled_scope
    expect(bundled.pluck(:id)).not_to include(thread.id)
  end

  it "bundled_scope_for returns nil for unknown or disabled buckets" do
    expect(service.bundled_scope_for("nonsense")).to be_nil

    @user.update!(inbox_smart_groups: { "social" => false })
    expect(service.bundled_scope_for("social")).to be_nil
  end

  it "bundled_scope_for scopes to a single bucket" do
    promo = create_thread("promotions")
    social = create_thread("social")

    ids = service.bundled_scope_for("promotions").pluck(:id)
    expect(ids).to include(promo.id)
    expect(ids).not_to include(social.id)
  end

  it "a thread spanning two enabled noise buckets bundles, but belongs to neither single bucket" do
    thread = create_thread("promotions", "notifications")

    expect(service.bundled_scope.pluck(:id)).to include(thread.id)
    expect(service.bundled_scope_for("promotions").pluck(:id)).not_to include(thread.id)
    expect(service.bundled_scope_for("notifications").pluck(:id)).not_to include(thread.id)
  end

  it "build_groups returns counts and up to three distinct senders, skipping empty buckets" do
    create_thread("promotions", from: "a@shop.test")
    create_thread("promotions", from: "b@shop.test")
    create_thread("promotions", from: "c@shop.test")
    create_thread("promotions", from: "d@shop.test")
    create_thread("notifications", from: "ci@builds.test")

    groups = service.build_groups([])
    buckets = groups.map { |g| g[:bucket] }

    expect(buckets).to include("promotions")
    expect(buckets).to include("notifications")
    expect(buckets).not_to include("social")
    expect(buckets).not_to include("updates")

    promo = groups.find { |g| g[:bucket] == "promotions" }
    expect(promo[:count]).to eq(4)
    expect(promo[:senders].size).to eq(3)
    expect(promo[:type]).to eq(:smart)
  end

  it "build_groups is empty when the master toggle is off" do
    create_thread("promotions")
    @user.update!(inbox_smart_groups: { "enabled" => false })

    expect(service.build_groups([])).to eq([])
  end
end
