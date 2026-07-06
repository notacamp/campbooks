# frozen_string_literal: true

require "rails_helper"

RSpec.describe "TagGroups", type: :request do
  before do
    @workspace = Workspace.create!(name: "TagGroups Ctrl WS #{SecureRandom.hex(4)}")
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
    @promo_group = @promo_tag.group_name
    sign_in_as(@user)
  end

  # ── validate_group! ─────────────────────────────────────────────────────────

  it "archive_all returns 400 when group param is blank" do
    post tag_group_archive_all_path, params: { group: "" }
    expect(response).to have_http_status(:bad_request)
  end

  it "archive_all returns 400 when group matches no workspace tag" do
    post tag_group_archive_all_path, params: { group: "Nonexistent Group" }
    expect(response).to have_http_status(:bad_request)
  end

  it "mark_all_read returns 400 when group param is blank" do
    post tag_group_mark_all_read_path, params: { group: "" }
    expect(response).to have_http_status(:bad_request)
  end

  it "mark_all_read returns 400 when group matches no workspace tag" do
    post tag_group_mark_all_read_path, params: { group: "Unknown Group" }
    expect(response).to have_http_status(:bad_request)
  end

  # ── authentication ──────────────────────────────────────────────────────────

  it "archive_all redirects unauthenticated requests" do
    delete "/session"
    post tag_group_archive_all_path, params: { group: @promo_group }
    expect(response).to be_redirect
  end

  it "mark_all_read redirects unauthenticated requests" do
    delete "/session"
    post tag_group_mark_all_read_path, params: { group: @promo_group }
    expect(response).to be_redirect
  end

  # ── archive_all ─────────────────────────────────────────────────────────────

  it "archive_all with a valid group redirects to email_messages_path" do
    create_message(tags: [ @promo_tag ])

    post tag_group_archive_all_path, params: { group: @promo_group }

    expect(response).to redirect_to(email_messages_path)
  end

  it "archive_all with a valid group but no messages still redirects cleanly" do
    post tag_group_archive_all_path, params: { group: @promo_group }
    expect(response).to redirect_to(email_messages_path)
  end

  # ── mark_all_read ────────────────────────────────────────────────────────────

  it "mark_all_read with a valid group redirects to email_messages_path with group param" do
    create_message(tags: [ @promo_tag ], read: false)

    post tag_group_mark_all_read_path, params: { group: @promo_group }

    expect(response).to redirect_to(email_messages_path(group: @promo_group))
  end

  it "mark_all_read marks the group threads unread messages as read" do
    msg = create_message(tags: [ @promo_tag ], read: false)

    post tag_group_mark_all_read_path, params: { group: @promo_group }

    expect(msg.reload.read).to be(true), "Message must be marked read after mark_all_read"
  end

  private

  def create_message(tags: [], read: false)
    thread = @account.email_threads.create!(subject: "T #{SecureRandom.hex(4)}")
    msg = @account.email_messages.create!(
      email_thread: thread,
      provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: "sender@example.com",
      to_address: @account.email_address,
      subject: "Test",
      received_at: 1.hour.ago,
      read: read,
      has_attachment: false
    )
    Array(tags).each { |t| msg.email_message_tags.create!(tag: t) }
    msg
  end
end
