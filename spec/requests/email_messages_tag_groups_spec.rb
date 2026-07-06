# frozen_string_literal: true

require "rails_helper"

# Integration tests verifying that the tag-group exclusion is wired correctly
# through the EmailMessagesController. The HTML index action redirects to the
# latest qualifying message; by controlling which threads carry bucket tags we
# can verify that the exclusion scope is (or is not) applied.
RSpec.describe "Email messages tag groups", type: :request do
  before do
    @workspace = Workspace.create!(name: "Tag Groups Ctrl WS #{SecureRandom.hex(4)}")
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
    sign_in_as(@user)
  end

  # ── inbox root -- grouped thread excluded ────────────────────────────────────

  # The newest message belongs to a grouped (promotions) thread; the older one is
  # plain. The redirect must land on the plain thread's message because the grouped
  # thread is collapsed out of the main list.
  it "grouped thread is excluded from the inbox root redirect target" do
    grouped_msg = create_message(subject: "Grouped", received_at: 1.hour.ago, tags: [ @promo_tag ])
    plain_msg   = create_message(subject: "Plain",   received_at: 2.hours.ago)

    get email_messages_path

    # Without exclusion the redirect would land on grouped_msg (it's newer).
    # With exclusion it must land on plain_msg.
    expect(response).to be_redirect
    expect(response.location).to include(plain_msg.id.to_s),
                                 "Expected redirect to the plain (non-grouped) message"
    expect(response.location).not_to include(grouped_msg.id.to_s)
  end

  # When ALL inbox threads are grouped, the list is empty and the controller
  # renders the empty state (not a redirect).
  it "inbox root renders empty state when every thread is in a group" do
    create_message(subject: "Grouped", tags: [ @promo_tag ])

    get email_messages_path

    # HTML: no latest message after exclusion -> renders :empty, not a redirect.
    expect(response).to have_http_status(:ok)
    expect(response).not_to be_redirect
  end

  # ── inbox root -- group drill-in ─────────────────────────────────────────────

  it "drilling into a group redirects to the grouped thread's message" do
    grouped_msg = create_message(subject: "Grouped", received_at: 1.hour.ago, tags: [ @promo_tag ])
    _plain_msg  = create_message(subject: "Plain",   received_at: 2.hours.ago)

    get email_messages_path(group: @promo_tag.group_name)

    expect(response).to be_redirect
    expect(response.location).to include(grouped_msg.id.to_s),
                                 "Drill-in must redirect to the grouped message"
  end

  # ── guard: replied thread stays in the main list ────────────────────────────

  # A replied thread is guarded -- it must NOT be excluded from the main list
  # even if it carries a bucket tag.
  it "a replied tagged thread is not excluded from the inbox root" do
    replied_thread = @account.email_threads.create!(
      subject: "Replied", last_outbound_at: Time.current
    )
    replied_msg = @account.email_messages.create!(
      email_thread: replied_thread,
      provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: "sender@example.com",
      to_address: @account.email_address,
      subject: "Replied",
      received_at: 1.hour.ago,
      read: false,
      has_attachment: false
    )
    replied_msg.email_message_tags.create!(tag: @promo_tag)

    plain_msg = create_message(subject: "Plain", received_at: 2.hours.ago)

    get email_messages_path

    expect(response).to be_redirect
    # replied_msg is newer AND is guarded -> it wins the redirect
    expect(response.location).to include(replied_msg.id.to_s),
                                 "Replied+tagged thread must remain in the main list (guarded)"
    expect(response.location).not_to include(plain_msg.id.to_s)
  end

  # ── folder view -- grouped thread shows inline ───────────────────────────────

  # When a folder_id param is present the inbox_root? guard is false, so no
  # exclusion is applied and grouped threads appear alongside regular ones.
  it "grouped thread appears inline in a specific folder view" do
    grouped_msg = create_message(subject: "Grouped", received_at: 1.hour.ago, tags: [ @promo_tag ])
    _plain_msg  = create_message(subject: "Plain",   received_at: 2.hours.ago)

    # folder_id: "INBOX" makes inbox_root? false -> no exclusion.
    get email_messages_path(folder_id: "INBOX")

    expect(response).to be_redirect
    # With exclusion disabled the newest message (grouped) is the redirect target.
    expect(response.location).to include(grouped_msg.id.to_s),
                                 "Grouped thread must appear inline in the folder view"
  end

  private

  def create_message(subject: "Test", received_at: 1.hour.ago, tags: [])
    thread = @account.email_threads.create!(subject: subject)
    msg = @account.email_messages.create!(
      email_thread: thread,
      provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: "sender@example.com",
      to_address: @account.email_address,
      subject: subject,
      received_at: received_at,
      read: false,
      has_attachment: false
    )
    Array(tags).each { |t| msg.email_message_tags.create!(tag: t) }
    msg
  end
end
