require "test_helper"

class Emails::SmartGroupsTest < ActiveSupport::TestCase
  setup do
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
    Emails::SmartGroups.new(@user, [ @account.id ])
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

  test "bundles a thread whose messages are all in noise buckets" do
    thread = create_thread("promotions", "promotions")

    assert_includes service.bundled_scope.pluck(:id), thread.id
  end

  test "keeps a thread inline when any message is personal or important" do
    mixed = create_thread("promotions", "personal")
    alert = create_thread("notifications", "important")

    bundled = service.bundled_scope.pluck(:id)
    assert_not_includes bundled, mixed.id
    assert_not_includes bundled, alert.id
  end

  test "keeps a thread inline when any message has no category (fail-open)" do
    thread = create_thread("promotions", nil)

    assert_not_includes service.bundled_scope.pluck(:id), thread.id
  end

  test "never bundles a thread the user replied to" do
    thread = create_thread("promotions")
    thread.update!(last_outbound_at: Time.current)

    assert_not_includes service.bundled_scope.pluck(:id), thread.id
  end

  test "never bundles a pinned thread" do
    thread = create_thread("promotions")
    thread.email_messages.first.update!(pinned_at: Time.current)

    assert_not_includes service.bundled_scope.pluck(:id), thread.id
  end

  test "never bundles mail from a starred contact" do
    contact = @workspace.contacts.create!(email: "vip@bulk.test", starred_at: Time.current)
    thread = create_thread("promotions", contact: contact)

    assert_not_includes service.bundled_scope.pluck(:id), thread.id
  end

  test "bundled_scope is nil when the master toggle is off" do
    create_thread("promotions")
    @user.update!(inbox_smart_groups: { "enabled" => false })

    assert_nil service.bundled_scope
  end

  test "a disabled bucket's threads stay inline" do
    thread = create_thread("promotions")
    @user.update!(inbox_smart_groups: { "promotions" => false })

    bundled = service.bundled_scope
    assert_not_includes bundled.pluck(:id), thread.id
  end

  test "bundled_scope_for returns nil for unknown or disabled buckets" do
    assert_nil service.bundled_scope_for("nonsense")

    @user.update!(inbox_smart_groups: { "social" => false })
    assert_nil service.bundled_scope_for("social")
  end

  test "bundled_scope_for scopes to a single bucket" do
    promo = create_thread("promotions")
    social = create_thread("social")

    ids = service.bundled_scope_for("promotions").pluck(:id)
    assert_includes ids, promo.id
    assert_not_includes ids, social.id
  end

  test "a thread spanning two enabled noise buckets bundles, but belongs to neither single bucket" do
    thread = create_thread("promotions", "notifications")

    assert_includes service.bundled_scope.pluck(:id), thread.id
    assert_not_includes service.bundled_scope_for("promotions").pluck(:id), thread.id
    assert_not_includes service.bundled_scope_for("notifications").pluck(:id), thread.id
  end

  test "build_groups returns counts and up to three distinct senders, skipping empty buckets" do
    create_thread("promotions", from: "a@shop.test")
    create_thread("promotions", from: "b@shop.test")
    create_thread("promotions", from: "c@shop.test")
    create_thread("promotions", from: "d@shop.test")
    create_thread("notifications", from: "ci@builds.test")

    groups = service.build_groups([])
    buckets = groups.map { |g| g[:bucket] }

    assert_includes buckets, "promotions"
    assert_includes buckets, "notifications"
    assert_not_includes buckets, "social"
    assert_not_includes buckets, "updates"

    promo = groups.find { |g| g[:bucket] == "promotions" }
    assert_equal 4, promo[:count]
    assert_equal 3, promo[:senders].size
    assert_equal :smart, promo[:type]
  end

  test "build_groups is empty when the master toggle is off" do
    create_thread("promotions")
    @user.update!(inbox_smart_groups: { "enabled" => false })

    assert_equal [], service.build_groups([])
  end
end
