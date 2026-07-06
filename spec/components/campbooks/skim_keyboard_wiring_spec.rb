# frozen_string_literal: true

require "rails_helper"

# Regression guard for the Skim follow-ups keyboard/click wiring.
#
# Phlex rewrites underscores in *symbol* data-attribute values to dashes
# (:follow_ups -> "follow-ups", :dismiss_follow_up -> "dismiss-follow-up"). The
# skim-mode Stimulus controller reads these verbatim ("follow_ups",
# "dismiss_follow_up"), so a symbol value silently broke the follow-up card's D
# shortcut AND its Dismiss button — the ONLY theme/action here with an underscore,
# which is why only follow-ups was affected. These pin the underscore form.
RSpec.describe "Campbooks::Skim keyboard wiring", type: :component do
  def render_component(component)
    ApplicationController.render(component, layout: false)
  end

  it "follow-up card's Dismiss button keeps the underscore action id" do
    html = render_component(Campbooks::SkimCard.new(
      theme: :follow_ups, category: :personal, title: "Q3 invoice approval",
      count: 1, follow_up_reason: "You asked Maria to approve the Q3 invoice."
    ))

    expect(html).to include('data-skim-action="dismiss_follow_up"'),
      "skim-mode#onClick switches on 'dismiss_follow_up'; a dasherized id makes the button inert"
    expect(html).not_to include("dismiss-follow-up"),
      "the symbol value was dasherized — pass a String so Phlex keeps the underscore"
  end

  it "follow-up frame carries the underscore theme the keyboard handler checks" do
    rings = [ {
      theme: :follow_ups,
      label: "Follow-ups",
      clusters: [ {
        category: :personal, title: "Q3 invoice approval", count: 1,
        follow_up_reason: "Waiting on Maria", email_ids: [ "e1" ],
        emails: [], samples: [], position: 1, total: 1
      } ]
    } ]

    html = render_component(Campbooks::SkimStack.new(rings: rings))

    expect(html).to include('data-skim-theme="follow_ups"'),
      "skim-mode#onKeydown gates the D shortcut on currentTheme === 'follow_ups'"
    expect(html).not_to include('data-skim-theme="follow-ups"'),
      "the :follow_ups symbol was dasherized — pass a String so the JS check matches"
  end
end
