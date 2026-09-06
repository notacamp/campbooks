# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::AutoStartJob do
  include ActiveJob::TestHelper

  let(:workspace) { create(:workspace, plan: "pro") }
  let!(:user)     { create(:user, workspace: workspace) }

  around { |ex| with_env("ENABLE_ACCOUNTING" => "1") { ex.run } }

  it "reconciles the filed statement as the given user and hands it to the parser" do
    doc = create(:document, :bank_statement, :approved, workspace: workspace)

    expect { described_class.perform_now(doc.id, created_by_id: user.id) }
      .to change(Reconciliation, :count).by(1)
      .and have_enqueued_job(Reconciliations::ParseJob)

    expect(Reconciliation.last).to have_attributes(statement_document: doc, created_by: user)
    expect(Current.workspace).to be_nil
  end

  it "leaves a statement alone when the service says no" do
    doc = create(:document, :bank_statement, :rejected, workspace: workspace)
    expect { described_class.perform_now(doc.id) }.not_to change(Reconciliation, :count)
  end

  it "discards a job for a document that is gone" do
    expect { described_class.perform_now(SecureRandom.uuid) }.not_to raise_error
  end
end
