require "rails_helper"

RSpec.describe DraftEmail do
  before do
    @workspace = Workspace.create!(name: "Draft WS")
    @user = @workspace.users.create!(
      name: "Drafter", email_address: "drafter-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
  end

  def build_draft(**attrs)
    DraftEmail.create!(workspace: @workspace, user: @user, mode: :new_message, **attrs)
  end

  it "resumable_for returns the most recently edited draft" do
    older = build_draft(subject: "Older")
    newer = build_draft(subject: "Newer")
    older.update!(updated_at: 1.hour.ago)

    expect(DraftEmail.resumable_for(@user)).to eq(newer)
  end

  it "resumable_for skips dismissed drafts" do
    newest = build_draft(subject: "Newest")
    older  = build_draft(subject: "Older")
    older.update!(updated_at: 1.hour.ago)
    newest.update!(dismissed_at: Time.current)

    expect(DraftEmail.resumable_for(@user)).to eq(older), "a dismissed draft must not grow a pill"

    older.update!(dismissed_at: Time.current)
    expect(DraftEmail.resumable_for(@user)).to be_nil
  end

  it "prune_for keeps only the newest MAX_PER_USER drafts" do
    (DraftEmail::MAX_PER_USER + 3).times { |i| build_draft(subject: "d#{i}") }

    DraftEmail.prune_for(@user)

    expect(@user.draft_emails.count).to eq(DraftEmail::MAX_PER_USER)
    expect(@user.draft_emails.exists?(subject: "d#{DraftEmail::MAX_PER_USER + 2}")).to be_truthy, "newest draft must survive the prune"
    expect(@user.draft_emails.exists?(subject: "d0")).to be_falsey, "oldest draft must be pruned"
  end

  it "display_title prefers subject, then first recipient" do
    expect(build_draft(subject: "Hello", to_address: "a@b.com").display_title).to eq("Hello")
    expect(build_draft(to_address: "a@b.com, c@d.com").display_title).to eq("a@b.com")
    expect(build_draft.display_title).to be_nil
  end

  it "attachment_entries keeps only well-formed entries" do
    draft = build_draft(attachments_json: [
      { "signed_id" => "sid1", "filename" => "a.pdf", "byte_size" => 10, "junk" => "x" },
      { "filename" => "no-id.pdf" },
      "not-a-hash"
    ])

    expect(draft.attachment_entries).to eq([ { "signed_id" => "sid1", "filename" => "a.pdf", "byte_size" => 10 } ])
  end

  it "drafts are destroyed with their user" do
    build_draft(subject: "orphan?")

    expect { @user.destroy! }.to change { DraftEmail.count }.by(-1)
  end
end
