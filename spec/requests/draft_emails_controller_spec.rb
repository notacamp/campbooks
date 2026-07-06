require "rails_helper"

RSpec.describe "DraftEmails", type: :request do
  before do
    @workspace = Workspace.create!(name: "Draft Ctrl WS")
    @user = create_user("drafts")
    sign_in(@user)
  end

  # ── Create ────────────────────────────────────────────────────────────────

  it "requires authentication" do
    delete session_path
    post draft_emails_path, params: { draft_email: { subject: "x" } }, as: :json

    expect(response).to have_http_status(:found)
  end

  it "create persists a draft scoped to the current user and workspace" do
    expect {
      post draft_emails_path, params: {
        draft_email: {
          mode: "reply", subject: "Re: Numbers", to_address: "ana@acme.test",
          body: "<p>Confirmed.</p>",
          attachments_json: [ { signed_id: "sid", filename: "a.pdf", byte_size: 12 } ]
        }
      }, as: :json
    }.to change(DraftEmail, :count).by(1)

    expect(response).to have_http_status(:created)
    draft = created_draft
    expect(draft.user).to eq(@user)
    expect(draft.workspace).to eq(@workspace)
    expect(draft.mode).to eq("reply")
    expect(draft.attachment_entries).to eq([ { "signed_id" => "sid", "filename" => "a.pdf", "byte_size" => 12 } ])
  end

  it "create links in_reply_to only when the user can read the message" do
    readable = create_message(create_account(@user))
    foreign  = create_message(create_account(create_user("other")))

    post draft_emails_path, params: { draft_email: { mode: "reply", in_reply_to_id: readable.id, body: "x" } }, as: :json
    expect(created_draft.in_reply_to).to eq(readable)

    post draft_emails_path, params: { draft_email: { mode: "reply", in_reply_to_id: foreign.id, body: "x" } }, as: :json
    expect(created_draft.in_reply_to).to be_nil, "a message the user can't read must not be linked"
  end

  it "create drops a signature belonging to another user" do
    other       = create_user("sigowner")
    foreign_sig = other.signatures.create!(name: "Theirs", content: "<p>bye</p>")

    post draft_emails_path, params: { draft_email: { subject: "x", signature_id: foreign_sig.id } }, as: :json

    expect(response).to have_http_status(:created)
    expect(DraftEmail.find(JSON.parse(response.body).fetch("id")).signature_id).to be_nil
  end

  # ── Update ────────────────────────────────────────────────────────────────

  it "update saves changes to own draft only" do
    draft = DraftEmail.create!(workspace: @workspace, user: @user, subject: "before")

    patch draft_email_path(draft), params: { draft_email: { subject: "after" } }, as: :json
    expect(response).to have_http_status(:ok)
    expect(draft.reload.subject).to eq("after")

    other   = create_user("stranger")
    foreign = DraftEmail.create!(workspace: @workspace, user: other, subject: "theirs")
    patch draft_email_path(foreign), params: { draft_email: { subject: "hijack" } }, as: :json
    expect(response).to have_http_status(:not_found)
    expect(foreign.reload.subject).to eq("theirs")
  end

  # ── Destroy ───────────────────────────────────────────────────────────────

  it "destroy removes the draft" do
    draft = DraftEmail.create!(workspace: @workspace, user: @user, subject: "gone")

    expect {
      delete draft_email_path(draft), as: :json
    }.to change(DraftEmail, :count).by(-1)

    expect(response).to have_http_status(:no_content)
  end

  # ── Send ──────────────────────────────────────────────────────────────────

  it "a successful send consumes the draft" do
    account = create_account(@user)
    message = create_message(account)
    draft   = DraftEmail.create!(workspace: @workspace, user: @user, mode: :reply, in_reply_to: message, body: "<p>hi</p>")

    with_sender_success do
      expect {
        post send_message_email_message_path(message), params: {
          to_address: "ana@acme.test", subject: "Re: Numbers", body: "<p>hi</p>",
          draft_email_id: draft.id
        }, headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change(DraftEmail, :count).by(-1)
    end
  end

  it "send never consumes another user's draft" do
    account = create_account(@user)
    message = create_message(account)
    other   = create_user("victim")
    foreign = DraftEmail.create!(workspace: @workspace, user: other, subject: "keep me")

    with_sender_success do
      expect {
        post send_message_email_message_path(message), params: {
          to_address: "ana@acme.test", subject: "s", body: "b",
          draft_email_id: foreign.id
        }, headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.not_to change(DraftEmail, :count)
    end
  end

  private

  def create_user(prefix)
    @workspace.users.create!(
      name: prefix.capitalize,
      email_address: "#{prefix}-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  def create_account(user, can_read: true, can_send: true)
    account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    account.email_account_users.create!(user: user, owner: true, can_read: can_read, can_send: can_send)
    account
  end

  def create_message(account)
    thread = account.email_threads.create!(subject: "Numbers")
    account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}", provider_folder_id: "INBOX",
      from_address: "ana@acme.test", to_address: account.email_address,
      subject: "Numbers", received_at: Time.current, read: false, has_attachment: false
    )
  end

  # UUID primary keys make Model.last ordering random — always resolve the
  # created draft from the response.
  def created_draft
    DraftEmail.find(JSON.parse(response.body).fetch("id"))
  end

  # Repo idiom for stubbing a class method (see inbox_broadcaster_test):
  # swap Emails::Sender.call for a success result, restore afterwards.
  def with_sender_success
    result = Struct.new(:error_code) { def ok? = true }.new(nil)
    sc       = Emails::Sender.singleton_class
    original = sc.instance_method(:call)
    sc.send(:define_method, :call) { |*, **| result }
    yield
  ensure
    sc.send(:define_method, :call, original)
  end
end
