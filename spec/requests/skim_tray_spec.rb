require "rails_helper"

# Regression: the skim tray is lazily loaded into a turbo-frame on the inbox and
# home feed, so a 500 here renders as Turbo's "Content missing" on both pages.
# v0.10.0 broke it for any mail that walks the categorizer ladder to the end:
# SkimScope loads partial records and the provider-hint rung read a column the
# SELECT didn't include (provider_labels -> ActiveModel::MissingAttributeError).
RSpec.describe "Skim tray", type: :request do
  before do
    @workspace = Workspace.create!(name: "Skim Tray WS")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    sign_in(@user)
  end

  it "renders for mail that falls through to the provider-hint rung" do
    # A residual-personal email: human sender, plain subject, no bulk/security
    # signal — the categorizer walks the whole ladder and consults the provider
    # hint, which reads provider_labels on the partially-selected record.
    create_message(subject: "Hello there", from_address: "anna@quietsender.example")

    get skim_tray_path

    expect(response).to have_http_status(:ok)
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("turbo-frame#skim_tray")).not_to be_empty
  end

  it "routes provider-hinted residual mail into its noise ring" do
    create_message(
      subject: "Our spring lookbook", from_address: "anna@brandstudio.example",
      provider_labels: [ "CATEGORY_PROMOTIONS" ]
    )

    get skim_tray_path

    expect(response).to have_http_status(:ok)
    doc   = Nokogiri::HTML(response.body)
    frame = doc.at_css("turbo-frame#skim_tray")
    expect(frame).not_to be_nil
    expect(frame.css("[data-skim-overlay-theme-param='promotions']")).not_to be_empty
  end

  private

  def create_message(subject:, from_address:, provider_labels: [], received_at: 1.hour.ago)
    thread = @account.email_threads.create!(subject: subject)
    @account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX", from_address: from_address,
      to_address: @account.email_address, subject: subject, received_at: received_at,
      read: false, has_attachment: false, provider_labels: provider_labels
    )
  end
end
