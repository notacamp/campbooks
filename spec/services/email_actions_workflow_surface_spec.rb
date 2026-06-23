require "rails_helper"

# The :workflow surface is what the workflow engine's email_action step exposes.
# Only safe, single-email actions belong there — never bulk or timing-dependent ones.
RSpec.describe EmailActions, ".tools_for(:workflow)" do
  it "exposes safe single-email actions" do
    ids = described_class.tools_for(:workflow).map(&:id)
    expect(ids).to include("add_tag", "remove_tag", "archive", "trash", "forward_email")
  end

  it "excludes bulk and timing-dependent actions" do
    ids = described_class.tools_for(:workflow).map(&:id)
    expect(ids).not_to include("reclassify", "bulk_archive", "bulk_tag", "snooze", "unsnooze")
  end
end
