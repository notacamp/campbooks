# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmailRuleRunJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "owner@acme.com") }
  let(:mail_client) { instance_double(Zoho::MailClient) }

  before do
    allow(Zoho::MailClient).to receive(:new).with(account).and_return(mail_client)
    allow(mail_client).to receive(:list_folders).and_return([])
    allow(mail_client).to receive(:respond_to?).with(:archive_folder_id).and_return(true)
    allow(mail_client).to receive(:archive_folder_id).and_return("Archive")
    allow(mail_client).to receive(:move_to_folder)
  end

  def make_email(attrs = {})
    create(:email_message, email_account: account, read: false, **attrs)
  end

  def make_thread
    create(:email_thread, email_account: account)
  end

  # -------------------------------------------------------------------------
  # Full run lifecycle
  # -------------------------------------------------------------------------
  describe "full run" do
    let(:tag)  { create(:tag, workspace: workspace) }
    # Use mark_read: true so action_present validation passes at creation time;
    # the test is primarily about the tag-apply behaviour.
    let(:rule) do
      r = create(:email_rule, workspace: workspace,
                 criteria: { "subject" => [ "invoice" ] },
                 archive: false, mark_read: true)
      r.tags << tag
      r
    end
    let(:run) do
      create(:email_rule_run, email_rule: rule, workspace: workspace, status: :queued)
    end

    before { allow(MarkReadJob).to receive(:perform_later) }

    let!(:matching) { [
      make_email(subject: "Invoice #1"),
      make_email(subject: "Invoice #2")
    ] }
    let!(:non_matching) { make_email(subject: "Hello world") }

    it "transitions status from queued -> running -> completed" do
      described_class.perform_now(run.id)
      expect(run.reload.status).to eq("completed")
    end

    it "sets matched_count to the number of matching emails" do
      described_class.perform_now(run.id)
      expect(run.reload.matched_count).to eq(2)
    end

    it "sets processed_count to the number of processed emails" do
      described_class.perform_now(run.id)
      expect(run.reload.processed_count).to eq(2)
    end

    it "applies the rule to each matching email" do
      described_class.perform_now(run.id)
      matching.each do |email|
        expect(email.reload.tags).to include(tag)
      end
    end

    it "does not apply the rule to non-matching emails" do
      described_class.perform_now(run.id)
      expect(non_matching.reload.tags).not_to include(tag)
    end

    it "sets finished_at on the run" do
      described_class.perform_now(run.id)
      expect(run.reload.finished_at).not_to be_nil
    end

    it "updates last_run_at on the rule" do
      described_class.perform_now(run.id)
      expect(rule.reload.last_run_at).not_to be_nil
    end
  end

  # -------------------------------------------------------------------------
  # undoable threshold
  # -------------------------------------------------------------------------
  describe "undoable threshold" do
    let(:rule) do
      create(:email_rule, workspace: workspace,
             criteria: { "subject" => [ "invoice" ] }, archive: true)
    end
    let(:run) do
      create(:email_rule_run, email_rule: rule, workspace: workspace, status: :queued)
    end

    context "when matched_count <= 25_000" do
      before do
        allow_any_instance_of(EmailRules::Matcher).to receive(:scope).and_return(
          EmailMessage.where(id: [])  # empty scope for simplicity
        )
        allow_any_instance_of(EmailRules::Matcher).to receive(:count).and_return(5)
        allow_any_instance_of(EmailRules::Matcher).to receive(:scope).and_return(
          EmailMessage.none
        )
      end

      it "sets undoable to true" do
        described_class.perform_now(run.id)
        expect(run.reload.undoable).to be true
      end
    end

    context "when matched_count > 25_000" do
      before do
        stub_const("EmailRuleRunJob::UNDOABLE_THRESHOLD", 2)
        thread = make_thread
        3.times { make_email(subject: "invoice", email_thread: thread) }
      end

      it "sets undoable to false" do
        described_class.perform_now(run.id)
        expect(run.reload.undoable).to be false
      end
    end
  end

  # -------------------------------------------------------------------------
  # Undo restores state
  # -------------------------------------------------------------------------
  describe "undo via EmailRules::UndoRun" do
    let(:tag)    { create(:tag, workspace: workspace) }
    let!(:thread) { make_thread }
    # Use let! so email is created before the job runs; otherwise the lazy let
    # would not create the email until the first expect, which is after perform.
    let!(:email) do
      make_email(
        subject: "invoice",
        read: false,
        email_thread: thread,
        provider_message_id: "msg_undo_1"
      )
    end
    let(:rule) do
      r = create(:email_rule, workspace: workspace,
                 criteria: { "subject" => [ "invoice" ] },
                 archive: true, mark_read: true)
      r.tags << tag
      r
    end
    let(:run) do
      create(:email_rule_run, email_rule: rule, workspace: workspace, status: :queued)
    end

    before do
      allow(MarkReadJob).to receive(:perform_later)
      allow(MarkUnreadJob).to receive(:perform_later)
      allow(mail_client).to receive(:respond_to?).with(:inbox_folder_id).and_return(true)
      allow(mail_client).to receive(:respond_to?).with(:move_to_folder).and_return(true)
      allow(mail_client).to receive(:inbox_folder_id).and_return("Inbox")
    end

    it "undo removes provenance-tracked tags" do
      described_class.perform_now(run.id)
      expect(email.reload.tags).to include(tag)

      EmailRules::UndoRun.call(run.reload)
      expect(email.reload.tags).not_to include(tag)
    end

    it "undo marks emails unread when they were marked read by the run" do
      described_class.perform_now(run.id)
      expect(email.reload.read).to be true

      EmailRules::UndoRun.call(run.reload)
      expect(email.reload.read).to be false
    end

    it "undo unarchives emails when they were archived by the run" do
      described_class.perform_now(run.id)
      expect(run.reload.archived_email_ids).to include(email.id)

      EmailRules::UndoRun.call(run.reload)
      expect(mail_client).to have_received(:move_to_folder).with(
        [ "msg_undo_1" ], "Inbox"
      )
    end

    it "sets status to undone" do
      described_class.perform_now(run.id)
      EmailRules::UndoRun.call(run.reload)
      expect(run.reload.status).to eq("undone")
    end

    it "raises when run is not completed" do
      expect { EmailRules::UndoRun.call(run) }.to raise_error(ArgumentError)
    end

    it "raises when undoable is false" do
      completed_run = create(:email_rule_run, email_rule: rule, workspace: workspace,
                             status: :completed, undoable: false)
      expect { EmailRules::UndoRun.call(completed_run) }.to raise_error(ArgumentError)
    end

    it "undo leaves the rule's ingest-time tag applications untouched" do
      # Tagged by the same rule at ingest time — provenance set, but NOT part
      # of this run (email arrived later / matched at ingest).
      ingest_email = make_email(subject: "unrelated", provider_message_id: "msg_ingest")
      EmailMessageTag.create!(email_message: ingest_email, tag: tag, applied_by_rule_id: rule.id)

      described_class.perform_now(run.id)
      EmailRules::UndoRun.call(run.reload)

      expect(email.reload.tags).not_to include(tag)
      expect(ingest_email.reload.tags).to include(tag)
    end

    it "undo restores the whole archived thread, not just the matched message" do
      sibling = make_email(
        subject: "totally different",
        email_thread: thread,
        provider_message_id: "msg_undo_sibling"
      )

      described_class.perform_now(run.id)
      EmailRules::UndoRun.call(run.reload)

      expect(mail_client).to have_received(:move_to_folder).with(
        array_including("msg_undo_1", "msg_undo_sibling"), "Inbox"
      )
      expect(sibling.reload.provider_folder_id).to eq("Inbox")
    end
  end

  # -------------------------------------------------------------------------
  # Idempotency on retry
  # -------------------------------------------------------------------------
  describe "idempotency" do
    let(:rule) do
      create(:email_rule, workspace: workspace,
             criteria: { "subject" => [ "invoice" ] }, mark_read: true)
    end
    let(:run) do
      create(:email_rule_run, email_rule: rule, workspace: workspace, status: :queued)
    end
    let!(:email) { make_email(subject: "invoice", read: false) }

    before { allow(MarkReadJob).to receive(:perform_later) }

    it "is a no-op when run is already completed" do
      run.update!(status: :completed)
      expect { described_class.perform_now(run.id) }.not_to change(EmailMessageTag, :count)
    end

    it "resumes and completes a run that a previous attempt left as failed" do
      run.update!(status: :failed, processed_count: 1)

      described_class.perform_now(run.id)

      expect(run.reload.status).to eq("completed")
      expect(run.processed_count).to eq(1)
      expect(email.reload.read).to be true
    end

    it "resumes a run stranded in running by a crashed worker" do
      run.update!(status: :running)

      described_class.perform_now(run.id)

      expect(run.reload.status).to eq("completed")
    end

    it "skips an email whose actions raise and still completes the run" do
      bad = make_email(subject: "invoice", read: false, provider_message_id: "msg_bad")
      allow(MarkReadJob).to receive(:perform_later).with(account.id, [ "msg_bad" ])
        .and_raise(RuntimeError, "provider hiccup")

      described_class.perform_now(run.id)

      expect(run.reload.status).to eq("completed")
      expect(run.processed_count).to eq(2)
      expect(email.reload.read).to be true
      expect(bad.reload.read).to be true # DB update happened before the provider push raised
    end
  end
end
