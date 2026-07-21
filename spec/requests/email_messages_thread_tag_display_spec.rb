# frozen_string_literal: true

require "rails_helper"

# Tags stored on an older message in a thread must appear on the thread row chip
# strip (via _thread_tags.html.erb) and in the open-email tag picker
# (via _tags.html.erb / EmailDetail), even when the currently opened/latest
# message carries no tags of its own.
RSpec.describe "Email messages thread tag display", type: :request do
  before do
    @workspace = Workspace.create!(name: "Thread Tag Display WS #{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    @tag = @workspace.tags.create!(name: "suppliers", color: "#3b82f6", source: :local, kind: :user, hidden: false)

    @thread = @account.email_threads.create!(subject: "Invoice")

    # Older inbound message — tagged with "suppliers"
    @older_msg = @account.email_messages.create!(
      email_thread: @thread,
      provider_message_id: "m-old-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: "vendor@example.com",
      to_address: @account.email_address,
      subject: "Invoice",
      received_at: 2.hours.ago,
      read: true,
      has_attachment: false
    )
    @older_msg.email_message_tags.create!(tag: @tag)

    # Newer untagged message (the "latest") — this would be the reply the user sent
    @newer_msg = @account.email_messages.create!(
      email_thread: @thread,
      provider_message_id: "m-new-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: @account.email_address,
      to_address: "vendor@example.com",
      subject: "Re: Invoice",
      received_at: 1.hour.ago,
      read: true,
      has_attachment: false
    )

    sign_in_as(@user)
  end

  it "inbox thread row chip strip includes the tag from the older sibling message" do
    # The inbox redirects to the latest qualifying message; follow it to get the
    # full page where the sidebar renders thread_tags for the thread.
    get email_messages_path
    expect(response).to be_redirect
    get response.location
    expect(response).to have_http_status(:success)
    expect(response.body).to include("suppliers")
  end

  it "show page of the untagged latest message includes the tag from the sibling" do
    get email_message_path(@newer_msg)
    # May redirect for folder-context resolution; follow if so.
    get response.location if response.redirect?
    expect(response).to have_http_status(:success)
    expect(response.body).to include("suppliers")
  end
end
