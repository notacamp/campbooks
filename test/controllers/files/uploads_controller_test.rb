require "test_helper"

module Files
  # The Files upload supports a per-upload "Analyze with AI" toggle: on → the file
  # enters the AI pipeline immediately (pending + DocumentProcessJob); off → it is
  # stored as-is (skipped, no job).
  class UploadsControllerTest < ActionDispatch::IntegrationTest
    include ActiveJob::TestHelper

    setup do
      @workspace = Workspace.create!(name: "Upload Test", slug: "upl-#{SecureRandom.hex(4)}")
      @user = @workspace.users.create!(
        name: "Upload Tester",
        email_address: "upl-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
      post session_path, params: { email_address: @user.email_address, password: "password123" }
    end

    test "analyze=1 stores a pending document and enqueues analysis" do
      assert_enqueued_jobs 1, only: DocumentProcessJob do
        post files_uploads_path, params: { files: [ pdf_upload ], analyze: "1" }
      end

      doc = @workspace.documents.order(:id).last
      assert_equal "pending", doc.ai_status
      assert_equal "pending", doc.review_status
    end

    test "without the toggle the file is stored as-is and nothing is enqueued" do
      assert_no_enqueued_jobs only: DocumentProcessJob do
        post files_uploads_path, params: { files: [ pdf_upload ] }
      end

      doc = @workspace.documents.order(:id).last
      assert_equal "skipped", doc.ai_status
      assert_equal "approved", doc.review_status
    end

    private

    def pdf_upload
      Rack::Test::UploadedFile.new(StringIO.new("%PDF-1.4 test"), "application/pdf", original_filename: "test.pdf")
    end
  end
end
