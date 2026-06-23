require "rails_helper"

RSpec.describe SenderBlockArchiveJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace, list_status: :blocked) }

  after { Current.reset }

  it "sets the acting user so the background archive is scoped to that user's accounts" do
    seen_user = nil
    allow(Contacts::ApplyBlock).to receive(:call) do |arg|
      seen_user = Current.user # ApplyBlock/BulkArchive gate on Current.user
      expect(arg).to eq(contact)
      { archived_count: 0 }
    end

    described_class.perform_now(contact.id, user.id)

    expect(seen_user).to eq(user)
  end

  it "no-ops for an unknown contact" do
    expect(Contacts::ApplyBlock).not_to receive(:call)
    expect { described_class.perform_now(-1, user.id) }.not_to raise_error
  end
end
