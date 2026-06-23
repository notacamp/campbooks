require "rails_helper"

RSpec.describe SessionsPruneJob, type: :job do
  it "deletes sessions idle past the limit and keeps fresh ones" do
    user = create(:user)
    fresh = create(:session, user: user)
    stale = create(:session, user: user)
    stale.update_column(:updated_at, (Session::INACTIVITY_LIMIT + 1.day).ago)

    described_class.new.perform

    expect(Session.exists?(fresh.id)).to be(true)
    expect(Session.exists?(stale.id)).to be(false)
  end
end
