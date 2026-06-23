require "rails_helper"

RSpec.describe ThreadFollow, type: :model do
  it "is valid for a user and a thread" do
    expect(build(:thread_follow)).to be_valid
  end

  it "is unique per (user, agent_thread)" do
    follow = create(:thread_follow)
    dup = build(:thread_follow, user: follow.user, agent_thread: follow.agent_thread)
    expect(dup).not_to be_valid
  end

  it "is reachable from the thread's followers" do
    thread = create(:agent_thread)
    member = create(:user, workspace: thread.workspace)
    create(:thread_follow, user: member, agent_thread: thread)
    expect(thread.followers).to include(member)
  end
end
