require "rails_helper"

RSpec.describe Tools::Unarchive do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:thread) { EmailThread.create!(subject: "Quote", email_account: account) }
  let!(:message) { create(:email_message, email_account: account, email_thread: thread, provider_message_id: "p1") }

  it "moves the thread's messages back to the inbox folder" do
    client = double("MailClient", inbox_folder_id: "INBOX", move_to_folder: true)
    allow(message.email_account).to receive(:mail_client).and_return(client)

    Tools::Unarchive.call(message)

    expect(client).to have_received(:move_to_folder).with([ "p1" ], "INBOX")
    expect(message.reload.provider_folder_id).to eq("INBOX")
  end

  it "no-ops gracefully when the client can't move folders" do
    client = double("MailClient")
    allow(message.email_account).to receive(:mail_client).and_return(client)

    expect { Tools::Unarchive.call(message) }.not_to raise_error
  end
end
