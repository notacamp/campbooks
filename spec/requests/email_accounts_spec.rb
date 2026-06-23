require "rails_helper"

RSpec.describe "EmailAccounts permissions", type: :request do
  let(:workspace) { create(:workspace) }
  let(:owner) { create(:user, workspace: workspace) }
  let(:sharee) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, :owner, user: owner, email_account: account)
    create(:email_account_user, :collaborator, user: sharee, email_account: account)
  end

  describe "DELETE /email_accounts/:id (disconnect)" do
    it "lets the owner disconnect the account" do
      sign_in(owner)
      delete email_account_path(account)

      expect(account.reload.active).to be(false)
    end

    it "blocks a non-owner sharee from disconnecting" do
      sign_in(sharee)
      delete email_account_path(account)

      expect(account.reload.active).to be(true)
    end
  end

  describe "POST send_message (compose)" do
    let!(:message) { create(:email_message, email_account: account) }

    it "refuses to send a reply from an account the user can only read" do
      # sharee here is a viewer: read but not send.
      account.email_account_users.find_by(user: sharee).update!(can_send: false)
      sign_in(sharee)

      expect_any_instance_of(Zoho::MailClient).not_to receive(:send_message)

      post send_message_email_message_path(message),
           params: { mode: "reply", to_address: "x@example.com", subject: "Hi", body: "Yo" }

      expect(response).to have_http_status(:redirect)
    end
  end
end
