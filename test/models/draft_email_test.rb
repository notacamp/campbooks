require "test_helper"

class DraftEmailTest < ActiveSupport::TestCase
  setup do
    @workspace = Workspace.create!(name: "Draft WS")
    @user = @workspace.users.create!(
      name: "Drafter", email_address: "drafter-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
  end

  def build_draft(**attrs)
    DraftEmail.create!(workspace: @workspace, user: @user, mode: :new_message, **attrs)
  end

  test "resumable_for returns the most recently edited draft" do
    older = build_draft(subject: "Older")
    newer = build_draft(subject: "Newer")
    older.update!(updated_at: 1.hour.ago)

    assert_equal newer, DraftEmail.resumable_for(@user)
  end

  test "prune_for keeps only the newest MAX_PER_USER drafts" do
    (DraftEmail::MAX_PER_USER + 3).times { |i| build_draft(subject: "d#{i}") }

    DraftEmail.prune_for(@user)

    assert_equal DraftEmail::MAX_PER_USER, @user.draft_emails.count
    assert @user.draft_emails.exists?(subject: "d#{DraftEmail::MAX_PER_USER + 2}"), "newest draft must survive the prune"
    assert_not @user.draft_emails.exists?(subject: "d0"), "oldest draft must be pruned"
  end

  test "display_title prefers subject, then first recipient" do
    assert_equal "Hello", build_draft(subject: "Hello", to_address: "a@b.com").display_title
    assert_equal "a@b.com", build_draft(to_address: "a@b.com, c@d.com").display_title
    assert_nil build_draft.display_title
  end

  test "attachment_entries keeps only well-formed entries" do
    draft = build_draft(attachments_json: [
      { "signed_id" => "sid1", "filename" => "a.pdf", "byte_size" => 10, "junk" => "x" },
      { "filename" => "no-id.pdf" },
      "not-a-hash"
    ])

    assert_equal [ { "signed_id" => "sid1", "filename" => "a.pdf", "byte_size" => 10 } ], draft.attachment_entries
  end

  test "drafts are destroyed with their user" do
    build_draft(subject: "orphan?")

    assert_difference -> { DraftEmail.count }, -1 do
      @user.destroy!
    end
  end
end
