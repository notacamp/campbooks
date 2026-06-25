require "rails_helper"

RSpec.describe "Email discussion comments", type: :request do
  let(:workspace) { create(:workspace) }
  let(:author) { create(:user, workspace: workspace, name: "Aria Author") }
  let!(:teammate) { create(:user, workspace: workspace, name: "Tom Teammate") }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:email_message) { create(:email_message, email_account: account) }

  before do
    create(:email_account_user, :collaborator, user: author, email_account: account)
    sign_in(author)
    # Scout replies only when text AI is available; post-#24 that needs a configured
    # provider, which this bare workspace has none of. The behaviour under test is
    # the @scout trigger, so make text AI available.
    allow(Ai::ProviderSetup).to receive(:available?).and_return(true)
  end

  def post_comment(content)
    post email_message_comments_path(email_message), params: { content: content }, as: :turbo_stream
  end

  it "records a plain comment without invoking Scout" do
    expect { post_comment("Looks fine to me") }.not_to have_enqueued_job(EmailChatReplyJob)
    expect(AgentMessage.where(author_type: :user).last.content).to eq("Looks fine to me")
  end

  it "invokes Scout only when tagged with @scout" do
    expect { post_comment("@scout summarize this") }.to have_enqueued_job(EmailChatReplyJob)
  end

  it "auto-follows the author" do
    post_comment("starting a discussion")
    agent_thread = email_message.reload.email_thread.agent_thread
    expect(agent_thread.followers).to include(author)
  end

  it "pulls in and notifies a mentioned teammate" do
    expect { post_comment("@Tom Teammate take a look") }
      .to change { teammate.notifications.where(category: :mention).count }.by(1)

    agent_thread = email_message.reload.email_thread.agent_thread
    expect(agent_thread.followers).to include(teammate)
  end

  it "emails a mentioned teammate" do
    expect { post_comment("@Tom Teammate ping") }
      .to have_enqueued_mail(NotificationMailer, :mention)
  end
end
