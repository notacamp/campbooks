require "rails_helper"

RSpec.describe Contacts::ApplyBlock do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace, list_status: :blocked) }
  let(:client)    { double("MailClient", archive_folder_id: "ARCHIVE", move_to_folder: true) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow_any_instance_of(EmailAccount).to receive(:mail_client).and_return(client)
    Current.acting_user = user # ApplyBlock/BulkArchive scope on Current.user
  end

  after { Current.reset }

  def mail_from_contact(**attrs)
    create(:email_message, { email_account: account, contact: contact, provider_folder_id: "INBOX" }.merge(attrs))
  end

  it "archives all of the blocked contact's mail (moves it to the Archive folder)" do
    m1 = mail_from_contact
    m2 = mail_from_contact

    Contacts::ApplyBlock.call(contact)

    expect(client).to have_received(:move_to_folder)
      .with(contain_exactly(m1.provider_message_id, m2.provider_message_id), "ARCHIVE")
    expect(m1.reload.provider_folder_id).to eq("ARCHIVE")
    expect(m2.reload.provider_folder_id).to eq("ARCHIVE")
  end

  it "removes the blocked sender's mail from Skim and the feed (the user-visible payoff)" do
    allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ])
    mail = mail_from_contact(ai_action_prompt: "Reply", ai_todo_dismissed: false, received_at: 1.day.ago)

    # While in the inbox it's a Skim candidate.
    expect(Emails::SkimScope.for(user).map(&:id)).to include(mail.id)

    Contacts::ApplyBlock.call(contact)

    # Archived → out of the inbox folder → gone from Skim and the feed source.
    expect(Emails::SkimScope.for(user).map(&:id)).not_to include(mail.id)
    feed_ids = Feed::Sources::EmailAction.new(user).candidates.map { |c| c[:subject].id }
    expect(feed_ids).not_to include(mail.id)
  end

  it "only archives mail within the acting user's readable accounts" do
    mine = mail_from_contact
    other_account = create(:email_account, workspace: workspace) # not shared with user
    theirs = create(:email_message, email_account: other_account, contact: contact, provider_folder_id: "INBOX")

    Contacts::ApplyBlock.call(contact)

    expect(mine.reload.provider_folder_id).to eq("ARCHIVE")
    expect(theirs.reload.provider_folder_id).to eq("INBOX")
  end

  it "is a no-op when the contact has no mail" do
    expect(Contacts::ApplyBlock.call(contact)).to eq(archived_count: 0)
    expect(client).not_to have_received(:move_to_folder)
  end
end
