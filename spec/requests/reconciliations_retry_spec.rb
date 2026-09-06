# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Reconciliations retry and duplicate guard", type: :request do
  include ActiveJob::TestHelper

  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user)      { create(:user, workspace:) }

  around { |ex| with_env("ENABLE_ACCOUNTING" => "1") { ex.run } }
  before { sign_in(user) }

  describe "POST /reconciliations/:id/retry" do
    let!(:failed) { create(:reconciliation, :failed, workspace:, created_by: user) }

    it "reads a failed statement again and goes back to it" do
      expect { post retry_parse_reconciliation_path(failed) }
        .to have_enqueued_job(Reconciliations::ParseJob).with(failed.id)

      expect(response).to redirect_to(reconciliation_path(failed))
      failed.reload
      expect(failed.status).to eq("pending")
      expect(failed.parse_error).to be_nil
    end

    it "goes back to Money when the retry came from there" do
      post retry_parse_reconciliation_path(failed), params: { surface: "money" }
      expect(response).to redirect_to(money_path)
    end

    it "leaves a statement that isn't failed alone" do
      ready = create(:reconciliation, :ready, :with_period, workspace:, created_by: user)
      expect { post retry_parse_reconciliation_path(ready) }.not_to have_enqueued_job(Reconciliations::ParseJob)
      expect(ready.reload.status).to eq("ready")
      expect(response).to redirect_to(reconciliation_path(ready))
    end

    it "404s for another workspace's statement" do
      other = create(:reconciliation, :failed)
      post retry_parse_reconciliation_path(other)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /reconciliations with a statement that is already being reconciled" do
    let(:doc) { create(:document, :bank_statement, :approved, workspace:) }

    it "opens the existing reconciliation instead of starting a second one" do
      existing = create(:reconciliation, workspace:, created_by: user, statement_document: doc, status: :parsing)

      expect { post reconciliations_path, params: { statement_document_id: doc.id } }
        .not_to change(Reconciliation, :count)

      expect(enqueued_jobs.count { |j| j["job_class"] == "Reconciliations::ParseJob" }).to eq(0)
      expect(response).to redirect_to(reconciliation_path(existing))
    end

    it "still starts a fresh one when the earlier attempt failed" do
      create(:reconciliation, :failed, workspace:, created_by: user, statement_document: doc)

      expect { post reconciliations_path, params: { statement_document_id: doc.id } }
        .to change(Reconciliation, :count).by(1)
        .and have_enqueued_job(Reconciliations::ParseJob)
    end
  end

  describe "the failed statement on the pages" do
    let!(:failed) do
      create(:reconciliation, :failed, workspace:, created_by: user,
             statement_document: create(:document, :bank_statement, :approved, workspace:, bank_name: "Millennium BCP"))
    end

    it "offers Try again on the statement itself" do
      get reconciliation_path(failed)
      expect(response.body).to include(retry_parse_reconciliation_path(failed))
      expect(response.body).to include("Try again")
      expect(response.body).to include("re-exporting from your bank")
    end

    it "drops the re-export hint when the provider, not the file, was the problem" do
      failed.update!(parse_error: I18n.t("reconciliations.parse_job.provider_busy"))
      expect(failed).to be_provider_failure

      get reconciliation_path(failed)
      expect(response.body).to include("AI provider")
      expect(response.body).not_to include("re-exporting from your bank")
    end

    it "offers Try again on the statements list" do
      get money_statements_path
      expect(response.body).to include(retry_parse_reconciliation_path(failed))
    end

    it "leads Money's Needs-you with it" do
      get money_path
      body = response.body
      expect(body).to include(retry_parse_reconciliation_path(failed))
      expect(body).to match(/couldn(&#39;|')t be read/)
      expect(body).to include('name="surface" value="money"')
    end
  end
end
