ENV["RAILS_ENV"] ||= "test"
require_relative "../config/environment"
require "rails/test_help"
require "webmock/minitest"

# Requiring webmock/minitest blocks all HTTP; re-allow by default so existing
# tests keep their behavior. Tests that must be hermetic (e.g. anything near a
# mail-provider client) call WebMock.disable_net_connect! in their own setup.
WebMock.allow_net_connect!

module ActiveSupport
  class TestCase
    # Run tests in parallel with specified workers
    parallelize(workers: :number_of_processors)

    # Setup all fixtures in test/fixtures/*.yml for all tests in alphabetical order.
    fixtures :all

    # FactoryBot definitions from spec/factories — shared with the RSpec suite.
    include FactoryBot::Syntax::Methods

    # Add more helper methods to be used by all tests here...

    # Run a block with specific ENV vars set (nil deletes), restoring afterwards.
    # Used to exercise platform-key / provider-key presence without touching .env.
    def with_env(vars)
      previous = {}
      vars.each do |key, value|
        previous[key] = ENV[key]
        value.nil? ? ENV.delete(key) : ENV[key] = value
      end
      yield
    ensure
      previous.each { |key, value| value.nil? ? ENV.delete(key) : ENV[key] = value }
    end

    # Run a block as if SELF_HOSTED were set, restoring the flag afterwards.
    def with_self_hosted
      previous = Rails.application.config.self_hosted
      Rails.application.config.self_hosted = true
      yield
    ensure
      Rails.application.config.self_hosted = previous
    end
  end
end

module ActionDispatch
  class IntegrationTest
    # Sign in through the real cookie-session endpoint, as a browser would.
    # An active session is signed out first — SessionsController silently
    # ignores a login attempt while one is live (redirect_if_authenticated).
    def sign_in_as(user, password: "password123")
      delete session_path if cookies[:session_id].present?
      post session_path, params: { email_address: user.email_address, password: password }
    end
  end
end
