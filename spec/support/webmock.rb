require "webmock/rspec"

# Match the Minitest suite's default (test_helper.rb): outbound HTTP is allowed
# unless a spec opts into hermetic behaviour with WebMock.disable_net_connect! in
# its own setup (as the mail/calendar provider-client specs do).
#
# Reset the flag before EVERY example: webmock/rspec clears registered stubs
# between examples but NOT the net-connect setting, so a hermetic spec that calls
# disable_net_connect! would otherwise leak that state into every spec that runs
# after it (which then blows up on an unstubbed provider call). This global
# before-hook runs before any example-group `before`, so a spec that re-disables
# net still gets its hermetic behaviour during its own run.
RSpec.configure do |config|
  config.before(:each) { WebMock.allow_net_connect! }
end
