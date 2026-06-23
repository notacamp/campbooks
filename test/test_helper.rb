ENV["RAILS_ENV"] ||= "test"
require_relative "../config/environment"
require "rails/test_help"

module ActiveSupport
  class TestCase
    # Run tests in parallel with specified workers
    parallelize(workers: :number_of_processors)

    # Setup all fixtures in test/fixtures/*.yml for all tests in alphabetical order.
    fixtures :all

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
