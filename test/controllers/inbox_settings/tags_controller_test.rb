require "test_helper"

class InboxSettings::TagsControllerTest < ActionDispatch::IntegrationTest
  setup do
    WebMock.disable_net_connect!
    # Zoho/Google OAuth clients read these from ENV in their initializers.
    # Set dummy values so constructing a mail client doesn't raise KeyError.
    ENV["ZOHO_CLIENT_ID"]     ||= "test-zoho-id"
    ENV["ZOHO_CLIENT_SECRET"] ||= "test-zoho-secret"
    @workspace = Workspace.create!(name: "Tags Ctrl WS", slug: "tags-ctrl-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Tagger",
      email_address: "tagger-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in_as(@user)
  end

  teardown do
    WebMock.allow_net_connect!
    Rails.cache.clear
  end

  # Stub the Zoho token refresh so the OauthClient never makes a live call.
  # The test env uses :null_store so Rails.cache can't be pre-seeded; stubbing
  # the refresh endpoint is the reliable alternative.
  def stub_zoho_token_refresh(refresh_token: "zoho-refresh-tok", token: "fake-access-token")
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
      email_address: "inbox-#{SecureRandom.hex(4)}@example.com",
      provider: :zoho,
      provider_account_id: "999",
      refresh_token: "zoho-refresh-tok",
      active: true
    )
    account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    account
  end

  def create_external_tag(account, name: "Newsletter")
    @workspace.tags.create!(
      name: name,
      color: "#ff0000",
      source: :external,
      email_account: account,
      external_label_id: "label-123"
    )
  end

  def create_local_tag(name: "Finance")
    @workspace.tags.create!(name: name, color: "#00ff00")
  end

  # 1. External tag rename pushes to provider FIRST, then saves locally.
  test "updating an external tag pushes the rename to the provider and saves locally" do
    account = create_zoho_account
    stub_zoho_token_refresh(refresh_token: account.refresh_token)
    tag = create_external_tag(account)

    zoho_base = "https://mail.zoho.eu/api"
    stub_request(:put, "#{zoho_base}/accounts/#{account.provider_account_id}/labels/#{tag.external_label_id}")
      .to_return(
        status: 200,
        body: { status: { code: 200, description: "success" } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    patch inbox_settings_tag_path(tag), params: { tag: { name: "Newsletters", color: "#ff0000" } },
          as: :turbo_stream

    assert response.status < 500, "Expected non-5xx response, got #{response.status}"
    assert_equal "Newsletters", tag.reload.name
  end

  # 2. Provider failure leaves the tag unchanged and renders an error (no 500).
  test "when the provider rejects the update the tag name is unchanged and no 500 is raised" do
    account = create_zoho_account
    stub_zoho_token_refresh(refresh_token: account.refresh_token)
    tag = create_external_tag(account)
    original_name = tag.name

    zoho_base = "https://mail.zoho.eu/api"
    stub_request(:put, "#{zoho_base}/accounts/#{account.provider_account_id}/labels/#{tag.external_label_id}")
      .to_return(
        status: 500,
        body: { status: { code: 500, description: "server error" } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    patch inbox_settings_tag_path(tag), params: { tag: { name: "Changed", color: "#ff0000" } },
          as: :turbo_stream

    assert response.status < 500, "Expected non-5xx response, got #{response.status}"
    assert_equal original_name, tag.reload.name
  end

  # 3. Destroying an external tag calls provider delete_label, then destroys locally.
  test "destroying an external tag removes it from the provider and destroys the record" do
    account = create_zoho_account
    stub_zoho_token_refresh(refresh_token: account.refresh_token)
    tag = create_external_tag(account)

    zoho_base = "https://mail.zoho.eu/api"
    stub_request(:delete, "#{zoho_base}/accounts/#{account.provider_account_id}/labels/#{tag.external_label_id}")
      .to_return(
        status: 200,
        body: { status: { code: 200 } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    assert_difference -> { Tag.count }, -1 do
      delete inbox_settings_tag_path(tag), as: :turbo_stream
    end

    assert response.status < 500, "Expected non-5xx response, got #{response.status}"
    assert_raises(ActiveRecord::RecordNotFound) { tag.reload }
  end

  # 4. Updating a local tag never issues any HTTP.
  #    WebMock.disable_net_connect! is in setup — any stray call raises automatically.
  test "updating a local tag does not make any HTTP calls and updates the record" do
    tag = create_local_tag

    patch inbox_settings_tag_path(tag), params: { tag: { name: "Accounting", color: "#00ff00" } },
          as: :turbo_stream

    assert response.status < 500, "Expected non-5xx response, got #{response.status}"
    assert_equal "Accounting", tag.reload.name
  end

  # 4b. Destroying a local tag never issues any HTTP.
  test "destroying a local tag does not make any HTTP calls and removes the record" do
    tag = create_local_tag

    assert_difference -> { Tag.count }, -1 do
      delete inbox_settings_tag_path(tag), as: :turbo_stream
    end

    assert response.status < 500, "Expected non-5xx response, got #{response.status}"
  end
end
