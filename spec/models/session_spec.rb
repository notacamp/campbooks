require "rails_helper"

RSpec.describe Session, type: :model do
  let(:user) { create(:user) }

  describe "expiry" do
    it "is expired once idle past the inactivity limit" do
      session = create(:session, user: user)
      session.update_column(:updated_at, (described_class::INACTIVITY_LIMIT + 1.day).ago)
      expect(session).to be_expired
    end

    it "is not expired when recently active" do
      expect(create(:session, user: user)).not_to be_expired
    end

    it ".expired returns only sessions idle past the limit" do
      fresh = create(:session, user: user)
      stale = create(:session, user: user)
      stale.update_column(:updated_at, (described_class::INACTIVITY_LIMIT + 1.day).ago)

      expect(described_class.expired).to contain_exactly(stale)
    end
  end

  describe "#touch_if_stale" do
    it "slides the window for a session over a day idle" do
      session = create(:session, user: user)
      session.update_column(:updated_at, 2.days.ago)
      expect { session.touch_if_stale }.to change { session.reload.updated_at }
    end

    it "leaves a recently-active session untouched (no per-request write)" do
      session = create(:session, user: user)
      expect { session.touch_if_stale }.not_to change { session.reload.updated_at }
    end
  end
end
