require "rails_helper"

RSpec.describe "Workflows send-account permission", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:sendable_account) { create(:email_account, workspace: workspace) }
  let(:unsendable_account) { create(:email_account, workspace: workspace) }
  let(:workflow) { create(:workflow, workspace: workspace) }
  let!(:step) do
    workflow.steps.create!(position: 0, step_type: "action", action_type: "send_email", config: {})
  end

  before do
    create(:email_account_user, :collaborator, user: user, email_account: sendable_account)
    sign_in(user)
  end

  def save_step_with_account(account_id)
    patch workflow_path(workflow), params: {
      workflow: {
        name: workflow.name,
        steps_attributes: { "0" => { id: step.id, config: { email_account_id: account_id } } }
      }
    }
  end

  it "saves a step that sends from an account the editor can send from" do
    save_step_with_account(sendable_account.id)

    expect(step.reload.config["email_account_id"].to_s).to eq(sendable_account.id.to_s)
  end

  it "rejects a step that sends from an account the editor cannot send from" do
    save_step_with_account(unsendable_account.id)

    expect(response).to have_http_status(:unprocessable_entity)
    expect(step.reload.config["email_account_id"]).to be_blank
  end
end
