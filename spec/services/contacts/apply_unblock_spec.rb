require "rails_helper"

RSpec.describe Contacts::ApplyUnblock do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace, list_status: :neutral) }
  let(:client)    { double("MailClient", inbox_folder_id: "INBOX", move_to_folder: true) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow_any_instance_of(EmailAccount).to receive(:mail_client).and_return(client)
    Current.acting_user = user
  end

  after { Current.reset }

  it "moves the sender's archived mail back to the inbox" do
    m = create(:email_message, email_account: account, contact: contact, provider_folder_id: "ARCHIVE")

    Contacts::ApplyUnblock.call(contact)

    expect(client).to have_received(:move_to_folder).with([ m.provider_message_id ], "INBOX")
    expect(m.reload.provider_folder_id).to eq("INBOX")
  end

  it "is a no-op when the contact has no mail" do
    expect(Contacts::ApplyUnblock.call(contact)).to eq(unarchived_count: 0)
    expect(client).not_to have_received(:move_to_folder)
  end
end
