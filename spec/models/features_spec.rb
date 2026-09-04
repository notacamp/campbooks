require "rails_helper"

# Features reads ENV at call time, so flip the vars per-example and restore them
# afterwards (the suite defaults them ON in config/environments/test.rb).
RSpec.describe Features do
  around do |example|
    keys = %w[ENABLE_WORKFLOWS ENABLE_EMAIL_BOARD ENABLE_MICROSOFT ENABLE_MICROSOFT_MAILBOX]
    saved = keys.index_with { |k| ENV[k] }
    begin
      example.run
    ensure
      saved.each { |k, v| v.nil? ? ENV.delete(k) : (ENV[k] = v) }
    end
  end

  describe ".workflows?" do
    it "is true only when ENABLE_WORKFLOWS == '1'" do
      ENV["ENABLE_WORKFLOWS"] = "1"
      expect(Features.workflows?).to be(true)

      ENV["ENABLE_WORKFLOWS"] = "0"
      expect(Features.workflows?).to be(false)

      ENV.delete("ENABLE_WORKFLOWS")
      expect(Features.workflows?).to be(false)
    end
  end

  describe ".email_board?" do
    it "is true only when ENABLE_EMAIL_BOARD == '1'" do
      ENV["ENABLE_EMAIL_BOARD"] = "1"
      expect(Features.email_board?).to be(true)

      ENV.delete("ENABLE_EMAIL_BOARD")
      expect(Features.email_board?).to be(false)
    end
  end

  describe ".microsoft?" do
    before do
      ENV.delete("ENABLE_MICROSOFT")
      ENV.delete("ENABLE_MICROSOFT_MAILBOX")
    end

    it "is off by default" do
      expect(Features.microsoft?).to be(false)
    end

    it "is on when ENABLE_MICROSOFT == '1'" do
      ENV["ENABLE_MICROSOFT"] = "1"
      expect(Features.microsoft?).to be(true)
    end

    it "honors the legacy ENABLE_MICROSOFT_MAILBOX flag" do
      ENV["ENABLE_MICROSOFT_MAILBOX"] = "1"
      expect(Features.microsoft?).to be(true)
    end
  end
end
