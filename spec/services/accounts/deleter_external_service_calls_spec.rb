# frozen_string_literal: true

require "rails_helper"

# Focused test: verifies that Accounts::Deleter purges the deleted workspace's
# ExternalServiceCall rows without touching other workspaces' rows.
RSpec.describe "Accounts::Deleter ExternalServiceCalls" do
  let(:ws_a) { create(:workspace) }
  let(:ws_b) { create(:workspace) }
  let(:user_a) { create(:user, workspace: ws_a) }
  let(:ws_a_call) { create(:external_service_call, workspace: ws_a) }
  let(:ws_b_call) { create(:external_service_call, workspace: ws_b) }
  let(:nil_call) { ExternalServiceCall.create!(service: "smtp", status: :success, workspace_id: nil) }

  before do
    # Materialize all records before running the deletion
    ws_a_call
    ws_b_call
    nil_call
  end

  it "deleting the sole member of a workspace purges that workspace's ExternalServiceCall rows" do
    expect { Accounts::Deleter.new(user_a).delete! }
      .to change(ExternalServiceCall, :count).by(-1)

    # The ws_a call is gone.
    expect(ExternalServiceCall.exists?(ws_a_call.id)).to be(false)
  end

  it "deleting ws_a leaves ws_b and nil-workspace rows intact" do
    Accounts::Deleter.new(user_a).delete!

    expect(ExternalServiceCall.exists?(ws_b_call.id)).to be(true), "ws_b call must survive"
    expect(ExternalServiceCall.exists?(nil_call.id)).to be(true),  "nil-workspace call must survive"
  end
end
