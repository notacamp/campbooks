# Ports the Minitest suite's helpers so migrated specs can toggle ENV vars and
# the self-hosted flag without touching real config. Available in every spec.
module EnvHelper
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

RSpec.configure do |config|
  config.include EnvHelper
end
