require "test_helper"

class DraftEmailsControllerTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "Draft Ctrl WS")
    @user = create_user("drafts")
    sign_in(@user)
  end

  def create_user(prefix)
    @workspace.users.create!(
      name: prefix.capitalize,
      email_address: "#{prefix}-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
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

  test "requires authentication" do
    delete session_path
    post draft_emails_path, params: { draft_email: { subject: "x" } }, as: :json

    assert_response :redirect
  end

  test "create persists a draft scoped to the current user and workspace" do
    assert_difference -> { DraftEmail.count }, 1 do
      post draft_emails_path, params: {
        draft_email: {
          mode: "reply", subject: "Re: Numbers", to_address: "ana@acme.test",
          body: "<p>Confirmed.</p>",
          attachments_json: [ { signed_id: "sid", filename: "a.pdf", byte_size: 12 } ]
        }
      }, as: :json
    end

    assert_response :created
    draft = DraftEmail.last
    assert_equal @user, draft.user
    assert_equal @workspace, draft.workspace
    assert_equal "reply", draft.mode
    assert_equal [ { "signed_id" => "sid", "filename" => "a.pdf", "byte_size" => 12 } ], draft.attachment_entries
    assert_equal draft.id, JSON.parse(response.body)["id"]
  end

  test "create links in_reply_to only when the user can read the message" do
    readable = create_message(create_account(@user))
    foreign = create_message(create_account(create_user("other")))

    post draft_emails_path, params: { draft_email: { mode: "reply", in_reply_to_id: readable.id, body: "x" } }, as: :json
    assert_equal readable, DraftEmail.last.in_reply_to

    post draft_emails_path, params: { draft_email: { mode: "reply", in_reply_to_id: foreign.id, body: "x" } }, as: :json
    assert_nil DraftEmail.last.in_reply_to, "a message the user can't read must not be linked"
  end

  test "create drops a signature belonging to another user" do
    other = create_user("sigowner")
    foreign_sig = other.signatures.create!(name: "Theirs", content: "<p>bye</p>")

    post draft_emails_path, params: { draft_email: { subject: "x", signature_id: foreign_sig.id } }, as: :json

    assert_response :created
    assert_nil DraftEmail.last.signature_id
  end

  test "update saves changes to own draft only" do
    draft = DraftEmail.create!(workspace: @workspace, user: @user, subject: "before")

    patch draft_email_path(draft), params: { draft_email: { subject: "after" } }, as: :json
    assert_response :success
    assert_equal "after", draft.reload.subject

    other = create_user("stranger")
    foreign = DraftEmail.create!(workspace: @workspace, user: other, subject: "theirs")
    patch draft_email_path(foreign), params: { draft_email: { subject: "hijack" } }, as: :json
    assert_response :not_found
    assert_equal "theirs", foreign.reload.subject
  end

  test "destroy removes the draft" do
    draft = DraftEmail.create!(workspace: @workspace, user: @user, subject: "gone")

    assert_difference -> { DraftEmail.count }, -1 do
      delete draft_email_path(draft), as: :json
    end
    assert_response :no_content
  end

  test "a successful send consumes the draft" do
    account = create_account(@user)
    message = create_message(account)
    draft = DraftEmail.create!(workspace: @workspace, user: @user, mode: :reply, in_reply_to: message, body: "<p>hi</p>")

    with_sender_success do
      assert_difference -> { DraftEmail.count }, -1 do
        post send_message_email_message_path(message), params: {
          to_address: "ana@acme.test", subject: "Re: Numbers", body: "<p>hi</p>",
          draft_email_id: draft.id
        }, headers: { "Accept" => "text/vnd.turbo-stream.html" }
      end
    end
  end

  test "send never consumes another user's draft" do
    account = create_account(@user)
    message = create_message(account)
    other = create_user("victim")
    foreign = DraftEmail.create!(workspace: @workspace, user: other, subject: "keep me")

    with_sender_success do
      assert_no_difference -> { DraftEmail.count } do
        post send_message_email_message_path(message), params: {
          to_address: "ana@acme.test", subject: "s", body: "b",
          draft_email_id: foreign.id
        }, headers: { "Accept" => "text/vnd.turbo-stream.html" }
      end
    end
  end

  private

  # Repo idiom for stubbing a class method (see inbox_broadcaster_test):
  # swap Emails::Sender.call for a success result, restore afterwards.
  def with_sender_success
    result = Struct.new(:error_code) { def ok? = true }.new(nil)
    sc = Emails::Sender.singleton_class
    original = sc.instance_method(:call)
    sc.send(:define_method, :call) { |*, **| result }
    yield
  ensure
    sc.send(:define_method, :call, original)
  end
end
