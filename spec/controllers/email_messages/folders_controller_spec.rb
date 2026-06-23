require "rails_helper"

RSpec.describe EmailMessages::FoldersController, type: :controller do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before do
    session_record = create(:session, user: user)
    cookies.signed[:session_id] = session_record.id
    Current.workspace = workspace
    allow_any_instance_of(SetupStatus).to receive(:complete?).and_return(true)
  end

  def readable_account
    account = create(:email_account, workspace: workspace)
    EmailAccountUser.create!(user: user, email_account: account, can_read: true)
    account
  end

  describe "GET index" do
    it "returns the message account's folders as JSON" do
      message = create(:email_message, email_account: readable_account)
      allow_any_instance_of(EmailAccount).to receive(:folders)
        .and_return([ { id: "f1", name: "Inbox" }, { id: "f2", name: "Receipts" } ])

      get :index, params: { id: message.id }, format: :json

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["folders"]).to eq([
        { "id" => "f1", "name" => "Inbox" },
        { "id" => "f2", "name" => "Receipts" }
      ])
    end

    it "does not expose folders for accounts the user cannot read" do
      hidden = create(:email_account, workspace: workspace) # never linked to the user
      message = create(:email_message, email_account: hidden)

      get :index, params: { id: message.id }, format: :json

      expect(response).to have_http_status(:not_found)
      expect(response.parsed_body["folders"]).to eq([])
    end
  end
end
