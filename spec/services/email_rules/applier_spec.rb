# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmailRules::Applier, type: :service do
  let(:workspace)   { create(:workspace) }
  let(:account)     { create(:email_account, workspace: workspace, email_address: "owner@acme.com") }
  let(:mail_client) { instance_double(Zoho::MailClient) }

  before do
    allow(Zoho::MailClient).to receive(:new).with(account).and_return(mail_client)
    allow(mail_client).to receive(:list_folders).and_return([])
  end

  def make_email(attrs = {})
    create(:email_message, email_account: account, read: false, **attrs)
  end

  # -------------------------------------------------------------------------
  # #call — ingest-time path
  # -------------------------------------------------------------------------
  describe "#call" do
    it "applies matching enabled rules and skips disabled ones" do
      tag = create(:tag, workspace: workspace)
      # Rule uses mark_read: true so action_present validation passes at
      # creation time (before the tag association is persisted).
      enabled  = create(:email_rule, workspace: workspace,
                        criteria: { "from" => [ "@stripe.com" ] },
                        archive: false, mark_read: true, enabled: true)
      enabled.tags << tag
      _disabled = create(:email_rule, workspace: workspace,
                         criteria: { "from" => [ "@stripe.com" ] },
                         archive: true, enabled: false)
      allow(MarkReadJob).to receive(:perform_later)

      email = make_email(from_address: "billing@stripe.com")
      described_class.new(email).call

      expect(email.tags).to include(tag)
    end

    it "skips rules that do not match" do
      tag = create(:tag, workspace: workspace)
      rule = create(:email_rule, workspace: workspace,
                    criteria: { "from" => [ "@stripe.com" ] },
                    archive: false, mark_read: true)  # mark_read satisfies action_present
      rule.tags << tag

      email = make_email(from_address: "other@acme.com")
      described_class.new(email).call

      expect(email.tags).not_to include(tag)
    end

    it "does not raise when a rule application fails (failure-tolerant)" do
      _rule = create(:email_rule, workspace: workspace,
                     criteria: { "from" => [ "@stripe.com" ] }, archive: true)

      allow_any_instance_of(EmailRules::Matcher).to receive(:matches?).and_return(true)
      allow_any_instance_of(described_class).to receive(:apply).and_raise(RuntimeError, "boom")

      email = make_email(from_address: "a@stripe.com")
      expect { described_class.new(email).call }.not_to raise_error
    end
  end

  # -------------------------------------------------------------------------
  # #apply — individual actions
  # -------------------------------------------------------------------------
  describe "#apply" do
    # -----------------------------------------------------------------------
    # Tags
    # -----------------------------------------------------------------------
    describe "tag action" do
      let(:tag)  { create(:tag, workspace: workspace) }
      # Rule uses mark_read: true so it passes action_present validation at creation
      # time before the tag is added.  The test verifies tag application logic.
      let(:rule) do
        r = create(:email_rule, workspace: workspace,
                   criteria: { "from" => [ "@a.com" ] },
                   archive: false, mark_read: true)
        r.tags << tag
        r
      end
      let(:email) { make_email }

      before { allow(MarkReadJob).to receive(:perform_later) }

      it "applies the tag to the email" do
        described_class.new(email).apply(rule)
        expect(email.reload.tags).to include(tag)
      end

      it "sets applied_by_rule_id on the created EmailMessageTag" do
        described_class.new(email).apply(rule)
        tag_row = EmailMessageTag.find_by(email_message: email, tag: tag)
        expect(tag_row.applied_by_rule_id).to eq(rule.id)
      end

      it "is idempotent — does not duplicate the tag" do
        described_class.new(email).apply(rule)
        described_class.new(email).apply(rule)
        count = EmailMessageTag.where(email_message: email, tag: tag).count
        expect(count).to eq(1)
      end

      it "does not overwrite applied_by_rule_id when tag already exists" do
        # Pre-existing tag row with no provenance
        existing = EmailMessageTag.create!(email_message: email, tag: tag, applied_by_rule_id: nil)
        described_class.new(email).apply(rule)
        expect(existing.reload.applied_by_rule_id).to be_nil
      end
    end

    # -----------------------------------------------------------------------
    # Archive
    # -----------------------------------------------------------------------
    describe "archive action" do
      let(:thread) { create(:email_thread, email_account: account) }
      let(:email)  { make_email(email_thread: thread, provider_message_id: "msg_1") }
      let(:rule)   { create(:email_rule, workspace: workspace, criteria: { "from" => [ "@a.com" ] }, archive: true) }

      before do
        allow(mail_client).to receive(:respond_to?).with(:archive_folder_id).and_return(true)
        allow(mail_client).to receive(:archive_folder_id).and_return("Archive")
        allow(mail_client).to receive(:move_to_folder)
      end

      it "archives the email thread" do
        described_class.new(email).apply(rule)
        expect(mail_client).to have_received(:move_to_folder)
      end

      it "is a no-op when the email is already in the archive folder" do
        # Simulate email already in archive folder via the local mirror
        account.email_folders.create!(
          provider_folder_id: "Archive",
          name: "Archive",
          position: 1
        )
        email.update_columns(provider_folder_id: "Archive")

        described_class.new(email).apply(rule)
        expect(mail_client).not_to have_received(:move_to_folder)
      end
    end

    # -----------------------------------------------------------------------
    # Mark read
    # -----------------------------------------------------------------------
    describe "mark_read action" do
      let(:rule) { create(:email_rule, workspace: workspace, criteria: { "from" => [ "@a.com" ] }, mark_read: true) }
      let(:email) { make_email(read: false, provider_message_id: "msg_42") }

      before do
        allow(MarkReadJob).to receive(:perform_later)
      end

      it "marks the email read" do
        described_class.new(email).apply(rule)
        expect(email.reload.read).to be true
      end

      it "enqueues MarkReadJob for provider sync" do
        described_class.new(email).apply(rule)
        expect(MarkReadJob).to have_received(:perform_later).with(account.id, [ "msg_42" ])
      end

      it "is a no-op when email is already read" do
        email.update_columns(read: true)
        described_class.new(email).apply(rule)
        expect(MarkReadJob).not_to have_received(:perform_later)
      end
    end

    # -----------------------------------------------------------------------
    # Folder membership
    # -----------------------------------------------------------------------
    describe "folder action" do
      let(:folder) { create(:mail_folder, workspace: workspace) }
      let(:rule) do
        create(:email_rule, workspace: workspace,
               criteria: { "from" => [ "@a.com" ] },
               archive: false, mark_read: false,
               mail_folder: folder)
      end
      let(:email) { make_email }

      it "creates a FolderMembership for the email" do
        expect {
          described_class.new(email).apply(rule)
        }.to change(FolderMembership, :count).by(1)

        membership = FolderMembership.last
        expect(membership.mail_folder).to eq(folder)
        expect(membership.folderable).to eq(email)
      end

      it "is idempotent — does not create duplicate memberships" do
        described_class.new(email).apply(rule)
        expect {
          described_class.new(email).apply(rule)
        }.not_to change(FolderMembership, :count)
      end
    end

    # -----------------------------------------------------------------------
    # matched_count increment
    # -----------------------------------------------------------------------
    describe "matched_count" do
      let(:rule) { create(:email_rule, workspace: workspace, criteria: { "from" => [ "@a.com" ] }, archive: true) }
      let(:email) { make_email }

      before do
        allow(mail_client).to receive(:respond_to?).with(:archive_folder_id).and_return(true)
        allow(mail_client).to receive(:archive_folder_id).and_return("Archive")
        allow(mail_client).to receive(:move_to_folder)
        create(:email_thread, email_account: account).tap { |t| email.update_columns(email_thread_id: t.id) }
      end

      it "increments rule.matched_count by 1 per apply call" do
        expect {
          described_class.new(email).apply(rule)
        }.to change { rule.reload.matched_count }.by(1)
      end
    end

    # -----------------------------------------------------------------------
    # Run bookkeeping (undo arrays)
    # -----------------------------------------------------------------------
    describe "run bookkeeping" do
      let(:thread) { create(:email_thread, email_account: account) }
      let(:email)  { make_email(email_thread: thread, provider_message_id: "msg_book") }
      let(:rule) do
        create(:email_rule, workspace: workspace,
               criteria: { "from" => [ "@a.com" ] },
               archive: true, mark_read: true)
      end
      let(:run) do
        create(:email_rule_run, email_rule: rule, workspace: workspace, undoable: true)
      end

      before do
        allow(mail_client).to receive(:respond_to?).with(:archive_folder_id).and_return(true)
        allow(mail_client).to receive(:archive_folder_id).and_return("Archive")
        allow(mail_client).to receive(:move_to_folder)
        allow(MarkReadJob).to receive(:perform_later)
      end

      it "appends to archived_email_ids when email is archived" do
        described_class.new(email).apply(rule, run: run)
        expect(run.archived_email_ids).to include(email.id)
      end

      it "appends to marked_read_email_ids when email is marked read" do
        described_class.new(email).apply(rule, run: run)
        expect(run.marked_read_email_ids).to include(email.id)
      end

      it "does not append to undo arrays when undoable is false" do
        run.update_columns(undoable: false)
        described_class.new(email).apply(rule, run: run)
        expect(run.archived_email_ids).to be_empty
        expect(run.marked_read_email_ids).to be_empty
      end

      it "appends to moved_email_ids when email is moved to folder" do
        folder = create(:mail_folder, workspace: workspace)
        rule2 = create(:email_rule, workspace: workspace,
                       criteria: { "from" => [ "@a.com" ] },
                       archive: false, mark_read: false,
                       mail_folder: folder)
        run2 = create(:email_rule_run, email_rule: rule2, workspace: workspace, undoable: true)
        email2 = make_email

        described_class.new(email2).apply(rule2, run: run2)
        expect(run2.moved_email_ids).to include(email2.id)
      end
    end
  end
end
