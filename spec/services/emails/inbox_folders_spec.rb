require "rails_helper"

RSpec.describe Emails::InboxFolders do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let!(:inbox)    { create(:email_message, email_account: account, provider_folder_id: "INBOX") }
  let!(:archived) { create(:email_message, email_account: account, provider_folder_id: "ARCHIVE") }

  describe ".constrain" do
    let(:scope) { EmailMessage.where(email_account: account) }

    it "filters the scope to the resolved inbox folder ids (drops archived mail)" do
      allow(described_class).to receive(:ids_for).with([ account ]).and_return([ "INBOX" ])

      expect(described_class.constrain(scope, [ account ])).to contain_exactly(inbox)
    end

    it "fails open (applies no filter) when no inbox ids resolve" do
      allow(described_class).to receive(:ids_for).and_return([])

      expect(described_class.constrain(scope, [ account ])).to contain_exactly(inbox, archived)
    end
  end
end
