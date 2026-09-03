# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::Notice do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  subject(:source) { described_class.new(user) }

  def action_required_notification(**attrs)
    create(:notification, user: user, category: :system, priority: :action_required, **attrs)
  end

  describe "#candidates" do
    it "returns action_required notifications as candidates" do
      n = action_required_notification(title: "Reconnect inbox")

      candidates = source.candidates

      expect(candidates.size).to eq(1)
      expect(candidates.first[:subject]).to eq(n)
      expect(candidates.first[:dedupe_key]).to eq("notice:#{n.id}")
      expect(candidates.first[:attention]).to be(true)
      expect(candidates.first[:score]).to be > 50 # above the attention floor
    end

    it "excludes archived notifications" do
      n = action_required_notification(title: "Old notice")
      n.archive!

      expect(source.candidates).to be_empty
    end

    it "excludes non-action_required notifications" do
      create(:notification, user: user, category: :activity, priority: :activity, title: "FYI notice")

      expect(source.candidates).to be_empty
    end

    it "excludes notifications belonging to another user" do
      other = create(:user, workspace: workspace)
      create(:notification, user: other, category: :system, priority: :action_required, title: "Other's notice")

      expect(source.candidates).to be_empty
    end
  end

  describe "#still_valid?" do
    it "is true when the notification still needs action" do
      n = action_required_notification(title: "Reconnect")
      item = instance_double(FeedItem)

      expect(source.still_valid?(item, n)).to be(true)
    end

    it "is false when the notification has been archived (user pressed Done)" do
      n = action_required_notification(title: "Reconnect")
      n.archive!

      expect(source.still_valid?(nil, n)).to be(false)
    end

    it "is false when the notification is resolved" do
      n = action_required_notification(title: "Reconnect")
      n.update!(resolved_at: Time.current)

      expect(source.still_valid?(nil, n)).to be(false)
    end

    it "is false when the subject is nil (record deleted)" do
      expect(source.still_valid?(nil, nil)).to be(false)
    end
  end
end
