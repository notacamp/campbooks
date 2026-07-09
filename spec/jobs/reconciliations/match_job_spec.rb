# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::MatchJob, type: :job do
  let(:workspace) { Workspace.create!(name: "MatchJob WS") }
  let(:user)      { workspace.users.create!(name: "Eve", email_address: "eve-mj@example.com", password: "password123") }

  def create_reconciliation
    doc = workspace.documents.build(
      document_type: :bank_statement,
      ai_status:     :skipped,
      review_status: :pending,
      source:        :manual_upload
    )
    doc.original_file.attach(
      io:           StringIO.new("Date;Desc;Amount\n01-01-2024;Test;100.00\n"),
      filename:     "stmt.csv",
      content_type: "text/csv"
    )
    doc.save!
    Reconciliation.create!(
      workspace:          workspace,
      created_by:         user,
      statement_document: doc,
      currency:           "EUR",
      status:             :matching
    )
  end

  let(:reconciliation) { create_reconciliation }

  before do
    # Silence broadcasts in all paths
    allow_any_instance_of(described_class).to receive(:broadcast_update!).and_return(nil)
  end

  describe "happy path" do
    before do
      allow_any_instance_of(Reconciliations::Matcher).to receive(:call).and_return(nil)
    end

    it "sets status to :ready" do
      described_class.perform_now(reconciliation.id)
      expect(reconciliation.reload.status).to eq("ready")
    end

    it "broadcasts after completion" do
      expect_any_instance_of(described_class).to receive(:broadcast_update!)
      described_class.perform_now(reconciliation.id)
    end

    it "resets Current.workspace afterward" do
      described_class.perform_now(reconciliation.id)
      expect(Current.workspace).to be_nil
    end
  end

  describe "transient errors (rate-limit / network)" do
    # Matcher propagates TRANSIENT_ERRORS after fix #8.
    # MatchJob must NOT touch status or broadcast — leave :matching so the
    # retry_on reschedule can succeed on a later attempt.
    let(:transient_error) { Faraday::TooManyRequestsError.new("429 Too Many Requests") }

    before do
      allow_any_instance_of(Reconciliations::Matcher).to receive(:call).and_raise(transient_error)
    end

    it "re-raises the error so retry_on can fire" do
      # Call #perform directly: perform_now would let retry_on intercept the
      # exception and schedule a retry instead of raising.
      expect {
        described_class.new.perform(reconciliation.id)
      }.to raise_error(Faraday::TooManyRequestsError)
    end

    it "leaves status as :matching (no false :failed flash to users)" do
      begin
        described_class.perform_now(reconciliation.id)
      rescue Faraday::TooManyRequestsError
        nil
      end
      expect(reconciliation.reload.status).to eq("matching")
    end

    it "does not broadcast a failed state" do
      expect_any_instance_of(described_class).not_to receive(:broadcast_update!)
      begin
        described_class.perform_now(reconciliation.id)
      rescue Faraday::TooManyRequestsError
        nil
      end
    end

    it "does not write parse_error" do
      begin
        described_class.perform_now(reconciliation.id)
      rescue Faraday::TooManyRequestsError
        nil
      end
      expect(reconciliation.reload.parse_error).to be_nil
    end
  end

  describe "non-transient StandardError (bug / data error)" do
    let(:bug_error) { RuntimeError.new("unexpected nil") }

    before do
      allow_any_instance_of(Reconciliations::Matcher).to receive(:call).and_raise(bug_error)
    end

    it "re-raises the error" do
      expect {
        described_class.new.perform(reconciliation.id)
      }.to raise_error(RuntimeError, "unexpected nil")
    end

    it "sets status to :failed" do
      begin
        described_class.perform_now(reconciliation.id)
      rescue RuntimeError
        nil
      end
      expect(reconciliation.reload.status).to eq("failed")
    end

    it "stores parse_error" do
      begin
        described_class.perform_now(reconciliation.id)
      rescue RuntimeError
        nil
      end
      expect(reconciliation.reload.parse_error).to include("RuntimeError")
    end

    it "broadcasts the failed state" do
      expect_any_instance_of(described_class).to receive(:broadcast_update!)
      begin
        described_class.perform_now(reconciliation.id)
      rescue RuntimeError
        nil
      end
    end
  end
end
