require "rails_helper"

RSpec.describe Contacts::Block do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace, list_status: :neutral, starred_at: Time.current) }

  it "blocks the contact, clears its star, and enqueues the archive job for the acting user" do
    expect {
      Contacts::Block.call(contact, user: user)
    }.to have_enqueued_job(SenderBlockArchiveJob).with(contact.id, user.id)

    expect(contact.reload).to be_blocked
    expect(contact.starred_at).to be_nil
  end

  it "no-ops for a nil contact" do
    expect { Contacts::Block.call(nil, user: user) }.not_to have_enqueued_job(SenderBlockArchiveJob)
  end
end

RSpec.describe Contacts::Unblock do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace, list_status: :blocked) }

  it "unblocks the contact and enqueues the unarchive (restore) job" do
    expect {
      Contacts::Unblock.call(contact, user: user)
    }.to have_enqueued_job(SenderUnblockUnarchiveJob).with(contact.id, user.id)

    expect(contact.reload).to be_neutral
  end
end
