# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmailMessage, type: :model do
  describe "channel" do
    it "defaults to email" do
      expect(create(:email_message).channel).to eq("email")
    end
  end
end
