# frozen_string_literal: true

require "test_helper"

class ExternalServiceCallTest < ActiveSupport::TestCase
  # ── Validations ───────────────────────────────────────────────────────────────

  test "valid with required attributes" do
    call = ExternalServiceCall.new(service: "google_mail", status: :success)
    assert call.valid?
  end

  test "requires service" do
    call = ExternalServiceCall.new(service: "", status: :success)
    assert_not call.valid?
    assert_includes call.errors[:service], "can't be blank"
  end

  # ── Enum ──────────────────────────────────────────────────────────────────────

  test "status_success? predicate works" do
    call = ExternalServiceCall.new(service: "smtp", status: :success)
    assert call.status_success?
    assert_not call.status_error?
  end

  test "status_error? predicate works" do
    call = ExternalServiceCall.new(service: "smtp", status: :error)
    assert call.status_error?
    assert_not call.status_success?
  end

  test "status_success scope returns only successful rows" do
    ok  = create(:external_service_call)
    err = create(:external_service_call, :error)

    assert_includes ExternalServiceCall.status_success, ok
    assert_not_includes ExternalServiceCall.status_success, err
  end

  test "status_error scope returns only error rows" do
    ok  = create(:external_service_call)
    err = create(:external_service_call, :error)

    assert_includes ExternalServiceCall.status_error, err
    assert_not_includes ExternalServiceCall.status_error, ok
  end

  # ── Scopes ────────────────────────────────────────────────────────────────────

  test "recent orders by created_at desc" do
    first  = create(:external_service_call, created_at: 2.hours.ago)
    second = create(:external_service_call, created_at: 1.hour.ago)

    ordered = ExternalServiceCall.recent.to_a
    assert_equal second, ordered.first
    assert_equal first, ordered.last
  end

  test "since returns only rows at or after the given time" do
    old   = create(:external_service_call, created_at: 2.days.ago)
    fresh = create(:external_service_call, created_at: 1.hour.ago)

    result = ExternalServiceCall.since(6.hours.ago)
    assert_includes result, fresh
    assert_not_includes result, old
  end

  test "for_service filters by service name" do
    gmail = create(:external_service_call, service: "google_mail")
    zoho  = create(:external_service_call, service: "zoho_mail")

    result = ExternalServiceCall.for_service("google_mail")
    assert_includes result, gmail
    assert_not_includes result, zoho
  end
end
