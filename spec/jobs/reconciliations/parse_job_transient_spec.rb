# frozen_string_literal: true

require "rails_helper"

# When the AI provider rate-limits us, a PDF statement must not fail for good:
# the job retries with backoff while the statement stays "parsing", and only
# when the retries are spent does it fail, blaming the provider, not the file.
RSpec.describe Reconciliations::ParseJob, "when the AI provider is unavailable", type: :job do
  include ActiveJob::TestHelper

  let(:workspace) { Workspace.create!(name: "ParseJob WS") }
  let!(:user)     { workspace.users.create!(name: "Dave", email_address: "dave-pj2@example.com", password: "password123") }

  let(:reconciliation) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("%PDF-1.4 fake"), filename: "stmt.pdf", content_type: "application/pdf")
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user, statement_document: doc, currency: "EUR")
  end

  before do
    allow_any_instance_of(described_class).to receive(:broadcast_update!).and_return(nil)
    allow(Ai::ProviderSetup).to receive(:configured?).with(anything, :documents).and_return(true)
    allow_any_instance_of(Ai::BankStatementParser).to receive(:call)
      .and_raise(Faraday::TooManyRequestsError.new("the server responded with status 429"))
  end

  it "parses one statement at a time" do
    expect(described_class.concurrency_limit).to eq(1)
  end

  it "retries with backoff and leaves the statement parsing, with no error on it" do
    expect { described_class.perform_now(reconciliation.id) }
      .to have_enqueued_job(described_class).with(reconciliation.id)

    reconciliation.reload
    expect(reconciliation.status).to eq("parsing")
    expect(reconciliation.parse_error).to be_nil
  end

  # ActiveJob counts retries per retry_on declaration (exception_executions),
  # keyed by the exception list's to_s; that is what decides "attempts spent".
  def job_after(transient_attempts:, generic_attempts: 3)
    job = described_class.new(reconciliation.id)
    job.executions = transient_attempts + generic_attempts
    job.exception_executions = {
      Ai::Adapters::Base::TRANSIENT_ERRORS.to_s => transient_attempts,
      [ StandardError ].to_s                     => generic_attempts
    }
    job
  end

  it "keeps retrying past the generic three attempts" do
    job = job_after(transient_attempts: 4)
    expect { job.perform_now }.to have_enqueued_job(described_class).with(reconciliation.id)
    expect(reconciliation.reload.status).to eq("parsing")
  end

  it "gives up after the last attempt, blaming the provider, and tells the user" do
    job = job_after(transient_attempts: described_class::TRANSIENT_ATTEMPTS - 1)

    expect { job.perform_now }.to change(Notification, :count).by(1)
    expect(enqueued_jobs.count { |j| j["job_class"] == described_class.name }).to eq(0)

    reconciliation.reload
    expect(reconciliation.status).to eq("failed")
    expect(reconciliation.parse_error).to include("AI provider")
    expect(reconciliation.parse_error).not_to include("AI parsing failed")
    expect(Current.workspace).to be_nil
  end

  it "still fails a bad file at once, without retrying" do
    allow_any_instance_of(Ai::BankStatementParser).to receive(:call)
      .and_raise(Reconciliations::ParseError.new("AI parsing failed: garbage"))

    expect { described_class.perform_now(reconciliation.id) }.not_to have_enqueued_job(described_class)
    expect(reconciliation.reload.status).to eq("failed")
  end
end
