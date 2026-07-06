# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::TagGroups do
  before do
    @workspace = Workspace.create!(name: "Tag Groups Svc WS #{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    Tags::DefaultGroups.provision!(@workspace)
    @promo_tag = Tags::DefaultGroups.bucket_tag_for(@workspace, "promotions")
    @notif_tag = Tags::DefaultGroups.bucket_tag_for(@workspace, "notifications")
    # A user-defined ("custom") tag group — grouped, but with no default_bucket,
    # so it keeps the full engagement guards (replied / important) that the four
    # built-in noise buckets deliberately drop.
    @custom_tag = @workspace.tags.create!(
      name: "Work", color: "#3b82f6", group_name: "Work",
      source: :local, kind: :user, hidden: false
    )
  end

  def service
    described_class.new(@workspace, [ @account.id ])
  end

  def create_thread(subject: "T", from: "s@bulk.test", contact: nil, tags: [], categories: [ nil ])
    thread = @account.email_threads.create!(subject: subject)
    categories.each_with_index do |cat, i|
      msg = @account.email_messages.create!(
        email_thread: thread,
        provider_message_id: "m-#{SecureRandom.hex(4)}",
        provider_folder_id: "INBOX",
        from_address: from,
        to_address: @account.email_address,
        subject: subject,
        received_at: (categories.size - i).hours.ago,
        read: false,
        has_attachment: false,
        category: cat,
        contact: contact
      )
      Array(tags).each { |t| msg.email_message_tags.create!(tag: t) }
    end
    thread
  end

  # ── excluded_scope ──────────────────────────────────────────────────────────

  it "excluded_scope includes a thread tagged with a grouped bucket tag" do
    thread = create_thread(tags: [ @promo_tag ])
    scope = service.excluded_scope
    expect(scope).not_to be_nil
    expect(scope.where(id: thread.id).exists?).to be(true), "Expected grouped thread in excluded_scope"
  end

  it "excluded_scope does not include an untagged thread" do
    grouped_thread = create_thread(tags: [ @promo_tag ])
    plain_thread   = create_thread

    scope = service.excluded_scope
    expect(scope.where(id: grouped_thread.id).exists?).to be(true)
    expect(scope.where(id: plain_thread.id).exists?).to be(false)
  end

  it "excluded_scope returns nil when the workspace has no grouped tags" do
    bare_ws = Workspace.create!(name: "Bare WS #{SecureRandom.hex(4)}")
    bare_account = EmailAccount.create!(
      workspace: bare_ws, email_address: "bare-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    # No Tags::DefaultGroups.provision! -- no grouped tags exist.
    svc = described_class.new(bare_ws, [ bare_account.id ])
    expect(svc.excluded_scope).to be_nil
  end

  # ── guards: replied ─────────────────────────────────────────────────────────

  it "guard: a replied thread in a CUSTOM group is NOT in excluded_scope" do
    thread = create_thread(tags: [ @custom_tag ])
    thread.update!(last_outbound_at: Time.current)
    expect(service.excluded_scope.where(id: thread.id).exists?)
      .to be(false), "Replied thread in a custom group must stay in the main list"
  end

  it "default bucket: a replied thread IS still collapsed (buckets drop the replied guard)" do
    thread = create_thread(tags: [ @promo_tag ])
    thread.update!(last_outbound_at: Time.current)
    expect(service.excluded_scope.where(id: thread.id).exists?)
      .to be(true), "A default-bucket thread collapses even when the owner replied"
  end

  # ── guards: pinned ──────────────────────────────────────────────────────────

  it "guard: a pinned thread is NOT in excluded_scope" do
    thread = create_thread(tags: [ @promo_tag ])
    thread.email_messages.first.update!(pinned_at: Time.current)
    expect(service.excluded_scope.where(id: thread.id).exists?)
      .to be(false), "Pinned thread must stay in the main list"
  end

  # ── guards: starred sender ──────────────────────────────────────────────────

  it "guard: a thread from a starred contact is NOT in excluded_scope" do
    contact = @workspace.contacts.create!(
      email: "vip-#{SecureRandom.hex(4)}@bulk.test",
      starred_at: Time.current
    )
    thread = create_thread(from: contact.email, contact: contact, tags: [ @promo_tag ])
    expect(service.excluded_scope.where(id: thread.id).exists?)
      .to be(false), "Thread from starred contact must stay in the main list"
  end

  # ── guards: important message ───────────────────────────────────────────────

  it "guard: a CUSTOM-group thread with an important message is NOT in excluded_scope" do
    thread = create_thread(tags: [ @custom_tag ], categories: [ "important" ])
    expect(service.excluded_scope.where(id: thread.id).exists?)
      .to be(false), "Custom-group thread with an important message must stay in the main list"
  end

  it "default bucket: a thread with an important message IS still collapsed" do
    thread = create_thread(tags: [ @promo_tag ], categories: [ "important" ])
    expect(service.excluded_scope.where(id: thread.id).exists?)
      .to be(true), "A default-bucket thread collapses even with an important sibling message"
  end

  # ── group_scope ─────────────────────────────────────────────────────────────

  it "group_scope returns only threads for the requested group" do
    promo_thread = create_thread(tags: [ @promo_tag ])
    notif_thread = create_thread(tags: [ @notif_tag ])

    promo_scope = service.group_scope(@promo_tag.group_name)
    expect(promo_scope).not_to be_nil
    expect(promo_scope.where(id: promo_thread.id).exists?).to be(true), "promo thread must be in promo scope"
    expect(promo_scope.where(id: notif_thread.id).exists?).to be(false), "notif thread must NOT be in promo scope"
  end

  it "group_scope returns nil for an unknown group name" do
    expect(service.group_scope("Nonexistent Group")).to be_nil
  end

  it "group_scope respects the engagement guards for a CUSTOM group -- replied thread excluded" do
    thread = create_thread(tags: [ @custom_tag ])
    thread.update!(last_outbound_at: Time.current)
    scope = service.group_scope(@custom_tag.group_name)
    expect(scope.where(id: thread.id).exists?)
      .to be(false), "Replied thread must be excluded from a custom group_scope too"
  end

  it "group_scope for a default bucket INCLUDES a replied thread" do
    thread = create_thread(tags: [ @promo_tag ])
    thread.update!(last_outbound_at: Time.current)
    scope = service.group_scope(@promo_tag.group_name)
    expect(scope.where(id: thread.id).exists?)
      .to be(true), "A default-bucket drill-in still contains the replied thread"
  end

  # ── multi-membership ────────────────────────────────────────────────────────

  it "a thread tagged into two groups appears in both group_scopes" do
    multi_thread = create_thread(tags: [ @promo_tag ])
    multi_thread.email_messages.first.email_message_tags.create!(tag: @notif_tag)

    promo_scope = service.group_scope(@promo_tag.group_name)
    notif_scope = service.group_scope(@notif_tag.group_name)

    expect(promo_scope.where(id: multi_thread.id).exists?)
      .to be(true), "Multi-tagged thread must appear in promo group_scope"
    expect(notif_scope.where(id: multi_thread.id).exists?)
      .to be(true), "Multi-tagged thread must appear in notif group_scope"
  end

  it "multi-membership thread is counted in both build_groups rows" do
    multi_thread = create_thread(tags: [ @promo_tag ])
    multi_thread.email_messages.first.email_message_tags.create!(tag: @notif_tag)

    groups = service.build_groups([ "INBOX" ])
    promo_row = groups.find { |g| g[:label] == @promo_tag.group_name }
    notif_row = groups.find { |g| g[:label] == @notif_tag.group_name }

    expect(promo_row).not_to be_nil, "Promo group row must be present"
    expect(notif_row).not_to be_nil, "Notif group row must be present"
    expect(promo_row[:count]).to eq(1), "Multi-tagged thread counted in promo group"
    expect(notif_row[:count]).to eq(1), "Multi-tagged thread counted in notif group"
  end

  # A replied thread that belongs to BOTH a default bucket and a custom group:
  # the bucket's light guards win, so it collapses out of the main list and shows
  # in the bucket drill-in — but the custom group still honors the replied guard,
  # so it does NOT appear in the custom drill-in. Every collapsed thread thus
  # still surfaces in at least one drill-in.
  it "multi-membership: a replied bucket+custom thread collapses (bucket wins) but leaves the custom drill-in" do
    thread = create_thread(tags: [ @promo_tag ])
    thread.email_messages.first.email_message_tags.create!(tag: @custom_tag)
    thread.update!(last_outbound_at: Time.current)

    expect(service.excluded_scope.where(id: thread.id).exists?)
      .to be(true), "Bucket membership must collapse a replied multi-tagged thread"
    expect(service.group_scope(@promo_tag.group_name).where(id: thread.id).exists?)
      .to be(true), "Replied multi-tagged thread appears in the bucket drill-in"
    expect(service.group_scope(@custom_tag.group_name).where(id: thread.id).exists?)
      .to be(false), "Replied thread stays out of the custom drill-in (full guards)"
  end

  # ── build_groups ────────────────────────────────────────────────────────────

  it "build_groups returns label, count, senders, and color for each non-empty group" do
    create_thread(tags: [ @promo_tag ])
    groups = service.build_groups([ "INBOX" ])
    promo = groups.find { |g| g[:label] == @promo_tag.group_name }
    expect(promo).not_to be_nil
    expect(promo[:count]).to eq(1)
    expect(promo).to have_key(:senders)
    expect(promo).to have_key(:color)
  end

  it "build_groups skips groups that have zero qualifying threads" do
    # Create only a promotions thread; notifications has nothing.
    create_thread(tags: [ @promo_tag ])
    groups = service.build_groups([ "INBOX" ])
    labels = groups.map { |g| g[:label] }
    expect(labels).to include(@promo_tag.group_name)
    expect(labels).not_to include(@notif_tag.group_name)
  end

  it "build_groups excludes engagement-guarded threads from CUSTOM group counts" do
    replied = create_thread(tags: [ @custom_tag ])
    replied.update!(last_outbound_at: Time.current)

    groups = service.build_groups([ "INBOX" ])
    custom = groups.find { |g| g[:label] == @custom_tag.group_name }
    expect(custom).to be_nil, "Replied thread must not contribute to a custom group count"
  end

  it "build_groups counts a replied thread for a default bucket" do
    replied = create_thread(tags: [ @promo_tag ])
    replied.update!(last_outbound_at: Time.current)

    groups = service.build_groups([ "INBOX" ])
    promo = groups.find { |g| g[:label] == @promo_tag.group_name }
    expect(promo).not_to be_nil, "Default bucket must count the replied thread"
    expect(promo[:count]).to eq(1)
  end

  it "build_groups senders list contains at most 3 entries" do
    3.times { |i| create_thread(from: "sender#{i}@example.com", tags: [ @promo_tag ]) }
    create_thread(from: "sender3@example.com", tags: [ @promo_tag ]) # 4th distinct sender

    groups = service.build_groups([ "INBOX" ])
    promo = groups.find { |g| g[:label] == @promo_tag.group_name }
    expect(promo).not_to be_nil
    expect(promo[:senders].size).to be <= 3, "Expected at most 3 senders in the row"
  end
end
