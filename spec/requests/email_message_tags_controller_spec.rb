require "rails_helper"

RSpec.describe "EmailMessageTags", type: :request do
  before do
    WebMock.disable_net_connect!
    # Zoho/Google OAuth clients read these from ENV in their initializers.
    # Set dummy values so constructing a mail client doesn't raise KeyError.
    ENV["ZOHO_CLIENT_ID"]     ||= "test-zoho-id"
    ENV["ZOHO_CLIENT_SECRET"] ||= "test-zoho-secret"
    @workspace = Workspace.create!(name: "EMT Ctrl WS", slug: "emt-ctrl-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Sender",
      email_address: "sender-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in_as(@user)
  end

  after do
    WebMock.allow_net_connect!
    Rails.cache.clear
  end

  # Stub the Zoho token refresh so the OauthClient never makes a live call.
  # The test env uses :null_store, so Rails.cache seeding is a no-op; stubbing
  # the refresh endpoint is the reliable alternative.
  def stub_zoho_token_refresh(refresh_token: "emt-zoho-refresh", token: "fake-access-token")
    stub_request(:post, "https://accounts.zoho.eu/oauth/v2/token")
      .with(body: hash_including("refresh_token" => refresh_token))
      .to_return(
        status: 200,
        body: { access_token: token }.to_json,
        headers: { "Content-Type" => "application/json" }
      )
  end

  def create_zoho_account
    account = EmailAccount.create!(
      workspace: @workspace,
      email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :zoho,
      provider_account_id: "888",
      refresh_token: "emt-zoho-refresh",
      active: true
    )
    account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    account
  end

  def create_google_account
    account = EmailAccount.create!(
      workspace: @workspace,
      email_address: "gbox-#{SecureRandom.hex(4)}@example.com",
      provider: :google,
      provider_account_id: "999",
      refresh_token: "emt-google-refresh",
      active: true
    )
    account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    account
  end

  def create_message(account)
    thread = account.email_threads.create!(subject: "Test")
    account.email_messages.create!(
      email_thread: thread,
      provider_message_id: "msg-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: "other@example.com",
      to_address: account.email_address,
      subject: "Test",
      received_at: Time.current,
      read: false,
      has_attachment: false
    )
  end

  def create_external_tag(account, name: "Promo")
    @workspace.tags.create!(
      name: name,
      color: "#0000ff",
      source: :external,
      email_account: account,
      external_label_id: "label-ext-1"
    )
  end

  def create_local_tag(name: "Finance")
    @workspace.tags.create!(name: name, color: "#00ff00")
  end

  # 5. Adding an external tag hits the provider modify endpoint + creates the join row.
  it "adding an external tag calls the provider and creates the join" do
    account = create_zoho_account
    stub_zoho_token_refresh(refresh_token: account.refresh_token)
    message = create_message(account)
    tag     = create_external_tag(account)

    zoho_base = "https://mail.zoho.eu/api"
    stub_request(:put, "#{zoho_base}/accounts/#{account.provider_account_id}/updatemessage")
      .to_return(
        status: 200,
        body: { status: { code: 200 } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    expect {
      post email_message_tags_path(message), params: { tag_id: tag.id }, as: :turbo_stream
    }.to change { message.reload.tags.count }.by(1)

    expect(response.status).to be < 500
    expect(WebMock).to have_requested(:put, "#{zoho_base}/accounts/#{account.provider_account_id}/updatemessage")
  end

  # 6. Adding a local tag creates the join row with no HTTP.
  it "adding a local tag creates the join with no HTTP" do
    account = create_zoho_account
    message = create_message(account)
    tag     = create_local_tag

    expect {
      post email_message_tags_path(message), params: { tag_id: tag.id }, as: :turbo_stream
    }.to change { message.reload.tags.count }.by(1)

    expect(response.status).to be < 500
  end

  # 7. Removing an external tag hits the provider and removes the join row.
  it "removing an external tag calls the provider and removes the join" do
    account = create_zoho_account
    stub_zoho_token_refresh(refresh_token: account.refresh_token)
    message = create_message(account)
    tag     = create_external_tag(account)

    zoho_base = "https://mail.zoho.eu/api"
    # First add with a stub so the join row exists
    stub_request(:put, "#{zoho_base}/accounts/#{account.provider_account_id}/updatemessage")
      .to_return(
        status: 200,
        body: { status: { code: 200 } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    post email_message_tags_path(message), params: { tag_id: tag.id }, as: :turbo_stream
    expect(message.reload.tags.count).to eq(1)

    expect {
      delete email_message_tag_path(message, tag), as: :turbo_stream
    }.to change { message.reload.tags.count }.by(-1)

    expect(response.status).to be < 500
    # Provider is called for both add and remove
    expect(WebMock).to have_requested(:put, "#{zoho_base}/accounts/#{account.provider_account_id}/updatemessage").twice
  end
end
