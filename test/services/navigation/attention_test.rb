require "test_helper"

module Navigation
  # The "needs review" attention dot moved from the (now-merged) Documents nav item to
  # the Files item — the queue logic is unchanged, only which section it lights.
  class AttentionTest < ActiveSupport::TestCase
    setup do
      @workspace = Workspace.create!(name: "Attn WS", slug: "attn-#{SecureRandom.hex(4)}")
      @user = @workspace.users.create!(
        name: "Attn Tester",
        email_address: "attn-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
    end

    def build_needs_review_doc(viewed_at:)
      doc = @workspace.documents.new(document_type: "other", source: :manual_upload,
                                     ai_status: :completed, review_status: :pending, viewed_at: viewed_at)
      doc.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
      doc.save!
      doc
    end

    test "the files dot lights when an unviewed needs_review document exists" do
      build_needs_review_doc(viewed_at: nil)
      assert Attention.new(@user).dot?(:files)
    end

    test "the files dot clears once the document has been viewed" do
      build_needs_review_doc(viewed_at: Time.current)
      assert_not Attention.new(@user).dot?(:files)
    end

    test "the legacy :documents section no longer lights a dot" do
      build_needs_review_doc(viewed_at: nil)
      assert_not Attention.new(@user).dot?(:documents)
    end
  end
end
