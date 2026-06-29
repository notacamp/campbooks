require "rails_helper"

RSpec.describe Files::ScoutThreadLinker do
  let(:workspace)     { create(:workspace, scout_thread_posts: true) }
  let(:user)          { create(:user, workspace: workspace) }
  let(:email_account) { create(:email_account, workspace: workspace) }

  around do |example|
    Current.workspace = workspace
    example.run
    Current.workspace = nil
  end

  # A document extracted + classified from an email, with a mailbox owner so the
  # discussion thread can be created.
  def linked_document
    create(:email_account_user, :owner, email_account: email_account, user: user)
    thread = create(:email_thread, email_account: email_account, subject: "Invoice 42")
    email = create(:email_message, email_account: email_account, email_thread: thread)
    doc = create(:document, :other, workspace: workspace, ai_status: :completed)
    doc.document_email_messages.create!(email_message: email)
    doc
  end

  it "posts a Scout message, stamps posted_to_thread_at, and emits an event" do
    doc = linked_document

    expect { described_class.call(doc) }.to change(AgentMessage, :count).by(1)
    expect(AgentMessage.last).to have_attributes(author_type: "ai")
    expect(doc.reload.posted_to_thread_at).to be_present
    expect(workspace.events.where(name: "document.linked_to_thread").count).to eq(1)
  end

  it "is idempotent — never posts twice for the same document" do
    doc = linked_document
    described_class.call(doc)
    expect { described_class.call(doc) }.not_to change(AgentMessage, :count)
  end

  it "does nothing when the workspace has not opted in" do
    workspace.update!(scout_thread_posts: false)
    doc = linked_document

    expect { described_class.call(doc) }.not_to change(AgentMessage, :count)
    expect(doc.reload.posted_to_thread_at).to be_nil
  end

  it "does nothing for a document with no source email" do
    doc = create(:document, :other, workspace: workspace, ai_status: :completed)
    expect { described_class.call(doc) }.not_to change(AgentMessage, :count)
  end
end
