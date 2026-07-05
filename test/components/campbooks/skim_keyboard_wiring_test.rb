# frozen_string_literal: true

require "test_helper"

# Regression guard for the Skim follow-ups keyboard/click wiring.
#
# Phlex rewrites underscores in *symbol* data-attribute values to dashes
# (:follow_ups -> "follow-ups", :dismiss_follow_up -> "dismiss-follow-up"). The
# skim-mode Stimulus controller reads these verbatim ("follow_ups",
# "dismiss_follow_up"), so a symbol value silently broke the follow-up card's D
# shortcut AND its Dismiss button — the ONLY theme/action here with an underscore,
# which is why only follow-ups was affected. These pin the underscore form.
class Campbooks::SkimKeyboardWiringTest < ActiveSupport::TestCase
  def render(component)
    ApplicationController.render(component, layout: false)
  end

  test "follow-up card's Dismiss button keeps the underscore action id" do
    html = render(Campbooks::SkimCard.new(
      theme: :follow_ups, category: :personal, title: "Q3 invoice approval",
      count: 1, follow_up_reason: "You asked Maria to approve the Q3 invoice."
    ))

    assert_includes html, 'data-skim-action="dismiss_follow_up"',
      "skim-mode#onClick switches on 'dismiss_follow_up'; a dasherized id makes the button inert"
    refute_includes html, "dismiss-follow-up",
      "the symbol value was dasherized — pass a String so Phlex keeps the underscore"
  end

  test "follow-up frame carries the underscore theme the keyboard handler checks" do
    rings = [ {
      theme: :follow_ups,
      label: "Follow-ups",
      clusters: [ {
        category: :personal, title: "Q3 invoice approval", count: 1,
        follow_up_reason: "Waiting on Maria", email_ids: [ "e1" ],
        emails: [], samples: [], position: 1, total: 1
      } ]
    } ]

    html = render(Campbooks::SkimStack.new(rings: rings))

    assert_includes html, 'data-skim-theme="follow_ups"',
      "skim-mode#onKeydown gates the D shortcut on currentTheme === 'follow_ups'"
    refute_includes html, 'data-skim-theme="follow-ups"',
      "the :follow_ups symbol was dasherized — pass a String so the JS check matches"
  end
end
