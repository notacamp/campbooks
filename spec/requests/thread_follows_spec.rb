require "rails_helper"

RSpec.describe "Thread follows", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:email_message) { create(:email_message, email_account: account) }

  before do
    create(:email_account_user, :viewer, user: user, email_account: account)
    sign_in(user)
  end

  it "follows a thread, creating the discussion lazily" do
    expect { post follow_email_message_path(email_message), as: :turbo_stream }
      .to change { ThreadFollow.count }.by(1)
    expect(response.body).to include("Following")
  end

  it "unfollows a thread" do
    post follow_email_message_path(email_message), as: :turbo_stream
    expect { delete follow_email_message_path(email_message), as: :turbo_stream }
      .to change { ThreadFollow.count }.by(-1)
    expect(response.body).to include("Follow")
  end
end
