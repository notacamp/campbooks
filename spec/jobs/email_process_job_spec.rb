require "rails_helper"

RSpec.describe EmailProcessJob, type: :job do
  describe "#perform" do
    let(:account) { create(:email_account) }
    let(:scan_log) { create(:email_scan_log, email_account: account, documents_created: 0) }
    let(:mail_client) { instance_double(Zoho::MailClient) }

    before do
      allow(Zoho::MailClient).to receive(:new).with(account).and_return(mail_client)
      allow(mail_client).to receive(:get_message_content).and_return("<html>Email body</html>")
      # Categorization now resolves inbox folder ids via the client; these fixtures
      # have no inbox folder, so return an empty list (matches pre-folder behavior).
      allow(mail_client).to receive(:list_folders).and_return([])
    end

    context "with attachments" do
      let(:email_message) do
        create(:email_message,
          email_account: account,
          email_scan_log: scan_log,
          status: :fetched,
          has_attachment: true)
      end

      before do
        allow(mail_client).to receive(:list_message_attachments)
                                .with(email_message.provider_message_id, email_message.provider_folder_id)
                                .and_return([
                                  { "attachmentId" => "att_1", "attachmentName" => "invoice.pdf" }
                                ])
        allow(mail_client).to receive(:download_attachment)
                                .with(email_message.provider_message_id, email_message.provider_folder_id, "att_1")
                                .and_return("fake-pdf-binary")
      end

      it "downloads attachments and creates documents" do
        expect {
          described_class.perform_now(email_message.id)
        }.to change(Document, :count).by(1)
          .and change { scan_log.reload.documents_created }.by(1)

        doc = Document.last
        expect(doc.source).to eq("email")
        expect(doc.email_message_id).to eq(email_message.provider_message_id)
        expect(doc.email_account).to eq(account)
        expect(doc.ai_status).to eq("pending")
        expect(doc.review_status).to eq("pending")
        expect(doc.original_file).to be_attached
        expect(doc.original_file.filename.to_s).to eq("invoice.pdf")
      end

      it "attaches file to email message" do
        described_class.perform_now(email_message.id)
        expect(email_message.reload.files).to be_attached
        expect(email_message.files.first.filename.to_s).to eq("invoice.pdf")
      end

      it "enqueues DocumentProcessJob" do
        expect {
          described_class.perform_now(email_message.id)
        }.to have_enqueued_job(DocumentProcessJob)
      end

      it "is idempotent on retry" do
        # First run creates documents
        expect {
          described_class.perform_now(email_message.id)
        }.to change(Document, :count).by(1)

        # Second run is a no-op (email already processed)
        expect {
          described_class.perform_now(email_message.id)
        }.to_not change(Document, :count)
      end

      it "handles individual attachment failure gracefully" do
        allow(mail_client).to receive(:download_attachment)
                                .with(email_message.provider_message_id, email_message.provider_folder_id, "att_1")
                                .and_raise(StandardError.new("timeout"))

        described_class.perform_now(email_message.id)
        expect(email_message.reload.status).to eq("processed")
        expect(Document.count).to eq(0)
      end
    end

    context "with a tracking-pixel image attachment" do
      # A 1x1 PNG — the kind of invisible tracking pixel / signature spacer that
      # arrives as a plain attachment. It is not a document, and a 1x1 image makes
      # the vision model 400, so it must never become a Document.
      let(:one_by_one_png) do
        "\x89PNG\r\n\x1A\n".b + [ 13 ].pack("N") + "IHDR".b + [ 1, 1 ].pack("N2") + ("\x00".b * 5)
      end

      let(:email_message) do
        create(:email_message,
          email_account: account,
          email_scan_log: scan_log,
          status: :fetched,
          has_attachment: true)
      end

      before do
        allow(mail_client).to receive(:list_message_attachments)
                                .with(email_message.provider_message_id, email_message.provider_folder_id)
                                .and_return([
                                  { "attachmentId" => "att_px", "attachmentName" => "logo.png", "mimeType" => "image/png" }
                                ])
        allow(mail_client).to receive(:download_attachment)
                                .with(email_message.provider_message_id, email_message.provider_folder_id, "att_px")
                                .and_return(one_by_one_png)
      end

      it "does not create a Document for it" do
        expect {
          described_class.perform_now(email_message.id)
        }.to_not change(Document, :count)
      end

      it "does not enqueue DocumentProcessJob" do
        expect {
          described_class.perform_now(email_message.id)
        }.to_not have_enqueued_job(DocumentProcessJob)
      end

      it "still marks the email as processed" do
        described_class.perform_now(email_message.id)
        expect(email_message.reload.status).to eq("processed")
      end
    end

    context "without attachments" do
      let(:email_message) do
        create(:email_message,
          email_account: account,
          email_scan_log: scan_log,
          status: :fetched,
          has_attachment: false)
      end

      it "does not create documents" do
        expect {
          described_class.perform_now(email_message.id)
        }.to_not change(Document, :count)
      end
    end

    it "marks email as processed and fetches body" do
      email_message = create(:email_message,
        email_account: account,
        email_scan_log: scan_log,
        status: :fetched,
        has_attachment: false)

      described_class.perform_now(email_message.id)
      expect(email_message.reload.status).to eq("processed")
      expect(email_message.reload.body).to eq("<html>Email body</html>")
    end

    it "skips already processed messages" do
      email_message = create(:email_message,
        email_account: account,
        email_scan_log: scan_log,
        status: :fetched,
        has_attachment: false)
      email_message.processed!

      expect {
        described_class.perform_now(email_message.id)
      }.not_to change { email_message.reload.status }
    end

    context "when the workspace has no AI provider configured" do
      let(:email_message) do
        create(:email_message,
          email_account: account,
          email_scan_log: scan_log,
          status: :fetched,
          has_attachment: false)
      end

      before do
        # Strict gate: no provider set up for this workspace (platform env keys
        # don't count for automatic background processing).
        allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)
      end

      it "ingests the email but runs no triage, classification or reminder extraction" do
        expect(Emails::Triage).not_to receive(:new)
        expect(Ai::EmailClassifier).not_to receive(:new)

        expect {
          described_class.perform_now(email_message.id)
        }.not_to have_enqueued_job(Reminders::EmailExtractionJob)

        # Ingestion still completes — the message is recorded, just not analysed.
        expect(email_message.reload.status).to eq("processed")
        expect(email_message.reload.body).to eq("<html>Email body</html>")
        expect(email_message.tags).to be_empty
      end
    end

    # Regression: AI model resolution (Ai::Configuration.for) reads Current.workspace.
    # The job used not to set it, so triage/classifier resolved no adapter, fell back
    # to a keyless Anthropic client, 401'd, and was swallowed — categories were set but
    # NO tags. Every other AI job sets Current.workspace; this guards that this one does.
    context "when a text AI provider is configured" do
      let(:email_message) do
        create(:email_message, email_account: account, email_scan_log: scan_log,
                               status: :fetched, has_attachment: false)
      end

      before { allow(Ai::ProviderSetup).to receive(:configured?).and_return(true) }

      it "sets Current.workspace to the email's workspace while triaging, then resets it" do
        captured = :unset
        decision = Emails::Triage::Decision.new(
          category: "personal", confidence: 0.4, tag: nil, source: :embedding
        )
        allow(Emails::Triage).to receive(:new) do
          captured = Current.workspace
          instance_double(Emails::Triage, call: decision)
        end

        described_class.perform_now(email_message.id)

        expect(captured).to eq(account.workspace)
        expect(Current.workspace).to be_nil # reset in `ensure`
      end
    end

    # Future mail from a blocked sender is auto-archived at ingest, so it never
    # reaches the inbox folder (and thus never Skim or the feed). The block-time
    # archive of *existing* mail is covered in spec/services/contacts/*.
    context "when the sender is already blocked" do
      let!(:blocked) do
        create(:contact, workspace: account.workspace, email_account: account,
                         email: "spammer@junk.test", list_status: :blocked)
      end
      let(:email_message) do
        create(:email_message,
          email_account: account,
          email_scan_log: scan_log,
          status: :fetched,
          has_attachment: false,
          from_address: "spammer@junk.test")
      end

      before do
        allow(mail_client).to receive(:archive_folder_id).and_return("ARCHIVE")
        allow(mail_client).to receive(:move_to_folder).and_return(true)
      end

      it "auto-archives the incoming mail on ingest" do
        described_class.perform_now(email_message.id)

        expect(email_message.reload.contact).to eq(blocked)
        expect(mail_client).to have_received(:move_to_folder)
        expect(email_message.reload.provider_folder_id).to eq("ARCHIVE")
      end
    end

    # Our own digests are delivered to the user's mailbox and re-ingested here. They
    # must stay readable but skip the whole AI pipeline — even with a provider set up,
    # so this proves the self_generated flag (not the AI gate) does the skipping.
    context "when the email is self-generated (our own digest)" do
      let(:email_message) do
        create(:email_message,
          email_account: account,
          email_scan_log: scan_log,
          status: :fetched,
          has_attachment: false,
          self_generated_kind: "digest")
      end

      before { allow(Ai::ProviderSetup).to receive(:configured?).and_return(true) }

      it "stays readable and visible but runs none of the AI pipeline" do
        expect(Emails::Triage).not_to receive(:new)
        expect(Contacts::Identifier).not_to receive(:new)

        expect { described_class.perform_now(email_message.id) }
          .not_to have_enqueued_job(Reminders::EmailExtractionJob)

        email_message.reload
        # Readable: body fetched, marked processed, threaded into the inbox.
        expect(email_message.status).to eq("processed")
        expect(email_message.body).to eq("<html>Email body</html>")
        expect(email_message.email_thread).to be_present
        # Not mined: no triage tags applied.
        expect(email_message.tags).to be_empty
      end
    end
  end
end
