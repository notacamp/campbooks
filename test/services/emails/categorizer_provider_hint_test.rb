require "test_helper"

# The provider noise hint is rescue-only: it may pull the personal residual
# into a noise bucket, but never overrides a rule (machine sender, security
# subject, …) and never surfaces a non-noise hint.
class Emails::CategorizerProviderHintTest < ActiveSupport::TestCase
  HintedEmail = Struct.new(:subject, :from_address, :provider_category_hint, keyword_init: true)
  PlainEmail = Struct.new(:subject, :from_address, keyword_init: true)

  test "rescues the personal residual when the provider says promotions" do
    email = HintedEmail.new(
      subject: "Our spring lookbook", from_address: "anna@brandstudio.example",
      provider_category_hint: :promotions
    )

    result = Emails::Categorizer.new(email).call

    assert_equal :promotions, result.category
    assert_equal [ "provider category label" ], result.reasons
    assert result.confidence >= 0.9
  end

  test "rescues into social and updates the same way" do
    %i[social updates].each do |bucket|
      email = HintedEmail.new(
        subject: "Hello there", from_address: "someone@quietsender.example",
        provider_category_hint: bucket
      )

      assert_equal bucket, Emails::Categorizer.new(email).call.category
    end
  end

  test "never overrides a machine-sender rule" do
    email = HintedEmail.new(
      subject: "Nightly build finished", from_address: "no-reply@ci.example",
      provider_category_hint: :updates
    )

    result = Emails::Categorizer.new(email).call

    assert_equal :notifications, result.category
  end

  test "never overrides the security pre-screen path" do
    email = HintedEmail.new(
      subject: "Your verification code is 123456", from_address: "auto@login.example",
      provider_category_hint: :promotions
    )

    result = Emails::Categorizer.new(email).call

    assert_equal :important, result.category
  end

  test "ignores non-noise hints" do
    email = HintedEmail.new(
      subject: "Quick question", from_address: "sam@humanmail.example",
      provider_category_hint: :personal
    )

    result = Emails::Categorizer.new(email).call

    assert_equal :personal, result.category
    assert_equal 0.4, result.confidence
  end

  test "unchanged residual behavior when the email exposes no hint" do
    email = PlainEmail.new(subject: "Quick question", from_address: "sam@humanmail.example")

    result = Emails::Categorizer.new(email).call

    assert_equal :personal, result.category
    assert_equal 0.4, result.confidence
  end
end
