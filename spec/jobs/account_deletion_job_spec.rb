require "rails_helper"

RSpec.describe AccountDeletionJob do
  describe "#perform" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }

    it "calls the deleter for the given user" do
      deleter = instance_double(Accounts::Deleter, delete!: true)
      allow(Accounts::Deleter).to receive(:new).with(user).and_return(deleter)

      described_class.new.perform(user.id)

      expect(deleter).to have_received(:delete!)
    end

    it "is a no-op when the user no longer exists" do
      allow(Accounts::Deleter).to receive(:new)

      described_class.new.perform(0)

      expect(Accounts::Deleter).not_to have_received(:new)
    end
  end
end
