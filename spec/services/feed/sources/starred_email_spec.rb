require "rails_helper"

RSpec.describe Feed::Sources::StarredEmail do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:starred)   { create(:contact, workspace: workspace, starred_at: Time.current) }
  subject(:source) { described_class.new(user) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ])
  end

  def starred_email(**attrs)
    create(:email_message, { email_account: account, contact: starred, received_at: 2.days.ago }.merge(attrs))
  end

  it "excludes archived mail from a starred sender" do
    inbox = starred_email(provider_folder_id: "INBOX")
    archived = starred_email(provider_folder_id: "ARCHIVE")

    ids = source.candidates.map { |c| c[:subject].id }
    expect(ids).to include(inbox.id)
    expect(ids).not_to include(archived.id)
  end

  it "#still_valid? becomes false once a starred sender's mail is archived" do
    inbox = starred_email(provider_folder_id: "INBOX")
    archived = starred_email(provider_folder_id: "ARCHIVE")

    expect(source.still_valid?(nil, inbox)).to be(true)
    expect(source.still_valid?(nil, archived)).to be(false)
  end

  it "nominates attention only while unread — or when Scout flags it urgent" do
    unread = starred_email(provider_folder_id: "INBOX", read: false)
    read = starred_email(provider_folder_id: "INBOX", read: true)
    urgent_read = starred_email(provider_folder_id: "INBOX", read: true, ai_priority: :high)

    by_id = source.candidates.index_by { |c| c[:subject].id }
    expect(by_id[unread.id][:attention]).to be(true)
    expect(by_id[read.id][:attention]).to be(false)
    expect(by_id[urgent_read.id][:attention]).to be(true)
  end
end
