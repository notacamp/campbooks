require "rails_helper"

RSpec.describe EmailScanLog, type: :model do
  describe "associations" do
    it { is_expected.to belong_to(:email_account) }
  end

  describe "validations" do
    it { is_expected.to validate_presence_of(:status) }
  end

  describe "enums" do
    it {
      is_expected.to define_enum_for(:status)
        .with_values(running: 0, completed: 1, failed: 2)
    }
  end
end
