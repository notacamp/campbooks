require "rails_helper"

# A thread the user can't access must be indistinguishable from one that doesn't
# exist — both 404 — so we never leak that an out-of-reach thread exists. The
# controller's find is intentionally global, so access is enforced here, not by scope.
RSpec.describe "EmailThreads access control", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:thread) { EmailThread.create!(subject: "Secret thread", email_account: account) }

  before { sign_in(user) }

  it "404s a thread the user cannot access (no read grant, no follow)" do
    get email_thread_path(thread)

    expect(response).to have_http_status(:not_found)
  end

  it "does not 404 a thread the user can read (redirects to its latest message)" do
    create(:email_account_user, user: user, email_account: account) # can_read: true by default
    create(:email_message, email_account: account, email_thread: thread)

    get email_thread_path(thread)

    expect(response).not_to have_http_status(:not_found)
    expect(response).to have_http_status(:redirect)
  end
end
