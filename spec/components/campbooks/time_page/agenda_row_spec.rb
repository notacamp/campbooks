# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::TimePage::AgendaRow, type: :component do
  let(:zone) { ActiveSupport::TimeZone["UTC"] }

  def item_for(kind: :event, emphasis: :normal, why: nil, **attrs)
    defaults = {
      kind: kind, at: 2.hours.from_now, day: Date.current,
      all_day: false, overdue: false, duration_minutes: 30,
      title: "Test meeting", source_label: "Google Calendar", source_path: nil,
      color: "#4a90e2", record: double("record", join_url: nil), actions: [],
      emphasis: emphasis, why: why
    }
    ::Time::AgendaItem.new(**defaults.merge(attrs))
  end

  def render_row(item)
    ApplicationController.render(described_class.new(item: item, zone: zone), layout: false)
  end

  describe "normal event" do
    it "renders the title" do
      html = render_row(item_for(title: "Project sync"))
      expect(html).to include("Project sync")
    end
  end

  describe ":prep emphasis" do
    it "renders the Prep chip" do
      html = render_row(item_for(emphasis: :prep, why: nil))
      expect(html).to include("Prep")
    end

    it "renders the why line when present" do
      html = render_row(item_for(emphasis: :prep, why: "with Sofia, you reply fast"))
      expect(html).to include("with Sofia, you reply fast")
    end
  end

  describe ":quiet emphasis" do
    it "renders the Declined badge" do
      html = render_row(item_for(emphasis: :quiet))
      expect(html).to include("Declined")
    end

    it "renders the title with muted styling" do
      html = render_row(item_for(emphasis: :quiet, title: "Quiet meeting"))
      expect(html).to include("Quiet meeting")
      expect(html).to include("text-muted-foreground")
    end
  end
end
