require "rails_helper"

# Smart-group behavior of the inbox list: bundled threads leave the main list
# and surface as group rows; the drill-in shows exactly one bucket; folder
# views are untouched.
RSpec.describe "Email messages smart groups", type: :request do
  before do
    @workspace = Workspace.create!(name: "SG Inbox WS")
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

  it "bundled threads leave the main list and surface as a group row" do
    personal = create_message(subject: "Coffee tomorrow", category: "personal")
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_message_path(personal)

    expect(response).to have_http_status(:ok)
    expect(response.body).not_to include("MEGA SALE WEEKEND")
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{email_messages_path(smart_group: "promotions")}']")).not_to be_empty
  end

  it "the drill-in lists only the bucket's threads with bulk actions" do
    create_message(subject: "Coffee tomorrow", category: "personal")
    promo = create_message(subject: "MEGA SALE WEEKEND", category: "promotions")
    create_message(subject: "CI build failed", category: "notifications")

    get email_message_path(promo, smart_group: "promotions")

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("MEGA SALE WEEKEND")
    expect(response.body).not_to include("CI build failed")
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("form[action='#{smart_group_archive_all_path("promotions")}']")).not_to be_empty
    expect(doc.css("form[action='#{smart_group_mark_all_read_path("promotions")}']")).not_to be_empty
  end

  it "folder views show bundled threads inline and no group rows" do
    promo = create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_message_path(promo, folder_id: "INBOX")

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("MEGA SALE WEEKEND")
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{email_messages_path(smart_group: "promotions")}']")).to be_empty
  end

  it "the inbox redirect lands on an unbundled message even when bundled mail is newer" do
    personal = create_message(subject: "Coffee tomorrow", category: "personal", received_at: 2.hours.ago)
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions", received_at: 5.minutes.ago)

    get email_messages_path

    expect(response).to redirect_to(email_message_path(personal))
  end

  it "an all-bundled inbox renders the sorted empty state with group rows" do
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_messages_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include(I18n.t("email_messages.empty.all_sorted_title"))
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{email_messages_path(smart_group: "promotions")}']")).not_to be_empty
  end

  it "disabling the feature restores inline noise" do
    @user.update!(inbox_smart_groups: { "enabled" => false })
    personal = create_message(subject: "Coffee tomorrow", category: "personal")
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_message_path(personal)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("MEGA SALE WEEKEND")
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{email_messages_path(smart_group: "promotions")}']")).to be_empty
  end

  private

  def create_message(subject:, category:, received_at: 1.hour.ago)
    thread = @account.email_threads.create!(subject: subject)
    @account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX", from_address: "sender@bulk.test",
      to_address: @account.email_address, subject: subject, received_at: received_at,
      read: true, has_attachment: false, category: category
    )
  end
end
