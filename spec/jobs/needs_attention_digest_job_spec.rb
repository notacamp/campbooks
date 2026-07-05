# frozen_string_literal: true

require "rails_helper"

RSpec.describe NeedsAttentionDigestJob, type: :job do
  before { ActiveJob::Base.queue_adapter.enqueued_jobs.clear }

  it "fans out a per-user job only for opted-in users with a workspace" do
    ws        = Workspace.create!(name: "Attention Sweep WS")
    opted_in  = ws.users.create!(name: "In",  email_address: "attention-in@example.com",  password: "changeme123")
    opted_out = ws.users.create!(name: "Out", email_address: "attention-out@example.com", password: "changeme123",
                                 email_on_waiting_on_replies_digest: false)

    described_class.perform_now

    fanned_out = ActiveJob::Base.queue_adapter.enqueued_jobs
                               .select { |job| job[:job] == NeedsAttentionDigestMailJob }
                               .map { |job| job[:args].first }

    expect(fanned_out).to include(opted_in.id)
    expect(fanned_out).not_to include(opted_out.id)
  end
end
