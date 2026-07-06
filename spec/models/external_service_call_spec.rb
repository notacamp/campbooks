# frozen_string_literal: true

require "rails_helper"

RSpec.describe ExternalServiceCall, type: :model do
  # ── Validations ───────────────────────────────────────────────────────────────

  it "valid with required attributes" do
    call = described_class.new(service: "google_mail", status: :success)
    expect(call).to be_valid
  end

  it "requires service" do
    call = described_class.new(service: "", status: :success)
    expect(call).not_to be_valid
    expect(call.errors[:service]).to include("can't be blank")
  end

  # ── Enum ──────────────────────────────────────────────────────────────────────

  it "status_success? predicate works" do
    call = described_class.new(service: "smtp", status: :success)
    expect(call).to be_status_success
    expect(call).not_to be_status_error
  end

  it "status_error? predicate works" do
    call = described_class.new(service: "smtp", status: :error)
    expect(call).to be_status_error
    expect(call).not_to be_status_success
  end

  it "status_success scope returns only successful rows" do
    ok  = create(:external_service_call)
    err = create(:external_service_call, :error)

    expect(described_class.status_success).to include(ok)
    expect(described_class.status_success).not_to include(err)
  end

  it "status_error scope returns only error rows" do
    ok  = create(:external_service_call)
    err = create(:external_service_call, :error)

    expect(described_class.status_error).to include(err)
    expect(described_class.status_error).not_to include(ok)
  end

  # ── Scopes ────────────────────────────────────────────────────────────────────

  it "recent orders by created_at desc" do
    first  = create(:external_service_call, created_at: 2.hours.ago)
    second = create(:external_service_call, created_at: 1.hour.ago)

    ordered = described_class.recent.to_a
    expect(ordered.first).to eq(second)
    expect(ordered.last).to eq(first)
  end

  it "since returns only rows at or after the given time" do
    old   = create(:external_service_call, created_at: 2.days.ago)
    fresh = create(:external_service_call, created_at: 1.hour.ago)

    result = described_class.since(6.hours.ago)
    expect(result).to include(fresh)
    expect(result).not_to include(old)
  end

  it "for_service filters by service name" do
    gmail = create(:external_service_call, service: "google_mail")
    zoho  = create(:external_service_call, service: "zoho_mail")

    result = described_class.for_service("google_mail")
    expect(result).to include(gmail)
    expect(result).not_to include(zoho)
  end
end
