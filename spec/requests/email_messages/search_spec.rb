require "rails_helper"

RSpec.describe "Email inbox search", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    # Avoid the mail-client folder listing during the full-page render.
    allow_any_instance_of(EmailMessagesController)
      .to receive(:folder_mappings)
      .and_return({ name_to_ids: {}, id_to_name: {}, id_to_account: {} })
  end

  it "requires authentication" do
    get search_email_messages_path, params: { q: "anything" }
    expect(response).to have_http_status(:redirect)
  end

  context "when signed in" do
    before { sign_in(user) }

    it "renders emails matching the keyword query" do
      create(:email_message, email_account: account, subject: "Invoice March")
      create(:email_message, email_account: account, subject: "Weekly Newsletter")

      get search_email_messages_path, params: { q: "invoice" }

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Invoice March")
      expect(response.body).not_to include("Weekly Newsletter")
    end

    it "never leaks emails from accounts the user cannot read" do
      other = create(:email_account, workspace: workspace)
      create(:email_message, email_account: other, subject: "TopSecretLeak")

      get search_email_messages_path, params: { q: "topsecretleak" }

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include("TopSecretLeak")
    end

    it "applies structured filters (unread)" do
      create(:email_message, email_account: account, subject: "FreshUnread", read: false)
      create(:email_message, email_account: account, subject: "AlreadyRead", read: true)

      get search_email_messages_path, params: { unread: "1" }

      expect(response.body).to include("FreshUnread")
      expect(response.body).not_to include("AlreadyRead")
    end

    it "serves the infinite-scroll page as a turbo stream" do
      create(:email_message, email_account: account, subject: "Bulk Match")

      get search_email_messages_path(q: "bulk"), headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(response.body).to include("turbo-stream")
    end

    it "restores the inbox when the search is empty" do
      create(:email_message, email_account: account, subject: "PlainInboxMessage")

      get search_email_messages_path

      expect(response).to have_http_status(:ok)
    end

    it "renders the waiting band for threads matching the query and excludes them from the flat list" do
      thread = create(:email_thread, email_account: account,
        last_outbound_at: 8.hours.ago,
        follow_up_dismissed_at: nil,
        follow_up_last_analyzed_at: nil)
      create(:email_message, email_account: account,
        email_thread: thread, subject: "AwaitingReplyMatch", received_at: 8.hours.ago)

      get search_email_messages_path, params: { q: "AwaitingReplyMatch" }

      expect(response).to have_http_status(:ok)

      doc = Nokogiri::HTML(response.body)
      band = doc.at_css("#waiting_replies_section")
      expect(band).to be_present
      # The matching thread renders inside the band…
      expect(band.text).to include("AwaitingReplyMatch")
      # …and its message is NOT repeated in the flat result list below.
      flat = doc.at_css("#email_search_list")
      expect(flat&.text.to_s).not_to include("AwaitingReplyMatch")
    end

    it "keeps a non-matching waiting thread out of the band during search" do
      thread = create(:email_thread, email_account: account,
        last_outbound_at: 8.hours.ago,
        follow_up_dismissed_at: nil,
        follow_up_last_analyzed_at: nil)
      create(:email_message, email_account: account,
        email_thread: thread, subject: "UnrelatedWaiting", received_at: 8.hours.ago)
      create(:email_message, email_account: account, subject: "InvoiceHit")

      get search_email_messages_path, params: { q: "InvoiceHit" }

      band = Nokogiri::HTML(response.body).at_css("#waiting_replies_section")
      expect(band.text).not_to include("UnrelatedWaiting") if band
    end

    it "keeps unthreaded messages in filter-mode results while a band is present" do
      thread = create(:email_thread, email_account: account,
        last_outbound_at: 8.hours.ago,
        follow_up_dismissed_at: nil,
        follow_up_last_analyzed_at: nil)
      create(:email_message, email_account: account,
        email_thread: thread, subject: "WaitingWithAttachment",
        has_attachment: true, received_at: 8.hours.ago)
      create(:email_message, email_account: account,
        email_thread: nil, subject: "UnthreadedAttachment",
        has_attachment: true)

      get search_email_messages_path, params: { q: "has:attachment" }

      # A plain NOT IN on band thread ids would drop NULL-thread rows.
      expect(response.body).to include("UnthreadedAttachment")
    end

    it "applies modifier-only query (has:attachment) via the filter path" do
      with_att    = create(:email_message, email_account: account, subject: "AttachMsg", has_attachment: true)
      without_att = create(:email_message, email_account: account, subject: "PlainMsg",  has_attachment: false)

      get search_email_messages_path, params: { q: "has:attachment" }

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("AttachMsg")
      expect(response.body).not_to include("PlainMsg")
    end
  end
end
