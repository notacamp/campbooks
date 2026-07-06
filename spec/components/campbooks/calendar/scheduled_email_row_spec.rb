require "rails_helper"

RSpec.describe Campbooks::Calendar::ScheduledEmailRow, type: :component do
  def render_for(scheduled_email)
    ApplicationController.render(described_class.new(scheduled_email: scheduled_email), layout: false)
  end

  it "shows the recipient without double-escaping angle brackets" do
    se = ScheduledEmail.new(id: SecureRandom.uuid, subject: "Re: Kickoff",
                            to_address: "Jordan Lee <jordan@acme.com>",
                            scheduled_at: Time.current.change(hour: 16))
    html = render_for(se)

    expect(html).to include("jordan@acme.com")
    expect(html).not_to include("&amp;lt;") # the double-escape regression
  end

  it "renders the recurring glyph only for recurring schedules" do
    attrs = { id: SecureRandom.uuid, subject: "Weekly", to_address: "x@example.com", scheduled_at: Time.current.change(hour: 9) }

    expect(render_for(ScheduledEmail.new(**attrs, rrule: "FREQ=WEEKLY"))).to include("M17 2l4 4")
    expect(render_for(ScheduledEmail.new(**attrs))).not_to include("M17 2l4 4")
  end
end
