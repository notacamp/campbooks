require "rails_helper"

# Campbooks::Swipeable is driven entirely by the data the swipe-actions Stimulus
# controller reads off the wrapper, so the contract worth pinning is: the right
# id/controller/targets are emitted and each side's ordered stage config is
# serialized as the JSON the JS parses. Visual behaviour is covered by the
# Lookbook preview + Playwright.
RSpec.describe Campbooks::Swipeable, type: :component do
  def render_component(**opts, &block)
    ApplicationController.render(described_class.new(**opts), layout: false, &block)
  end

  def left_stages(html)
    raw = html[/data-swipe-actions-left-value="([^"]*)"/, 1]
    JSON.parse(CGI.unescapeHTML(raw))
  end

  it "wires the controller, content target, and forwards id/data to the wrapper" do
    html = render_component(
      id: "row_1",
      data: { email_account_id: 42 },
      left: [ { key: "archive", label: "Archive", icon: :archive, color: "blue", endpoint: "/t", params: { tool: "archive" } } ]
    )

    expect(html).to include('id="row_1"')
    expect(html).to include('data-controller="swipe-actions"')
    expect(html).to include('data-email-account-id="42"')
    expect(html).to include('data-swipe-actions-target="content"')
  end

  it "serializes each side's ordered stages (label, color, endpoint, params, picker)" do
    html = render_component(
      left: [
        { key: "archive", label: "Archive", icon: :archive, color: "blue", endpoint: "/t", params: { tool: "archive" } },
        { key: "snooze", label: "Snooze", icon: :snooze, color: "amber", endpoint: "/t", params: { tool: "snooze" }, picker: "snooze" }
      ],
      right: [ { key: "trash", label: "Trash", icon: :trash, color: "orange", endpoint: "/t", params: { tool: "trash" } } ]
    )

    stages = left_stages(html)
    expect(stages.map { |s| s["key"] }).to eq(%w[archive snooze])
    expect(stages.first).to include("color" => "blue", "endpoint" => "/t", "removes" => true)
    expect(stages.first["params"]).to eq("tool" => "archive")
    expect(stages.last["picker"]).to eq("snooze")
  end

  it "renders the action panel only for sides that have stages" do
    # swipe-left only (Dismiss): the right-anchored panel is revealed, no left one
    html = render_component(left: [ { key: "dismiss", label: "Dismiss", icon: :dismiss, color: "neutral", endpoint: "/d", params: {} } ])

    expect(html).to include('data-swipe-actions-target="rightPanel"')
    expect(html).not_to include('data-swipe-actions-target="leftPanel"')
  end

  it "marks replace-in-place actions with removes:false" do
    html = render_component(right: [ { key: "approve", label: "Approve", icon: :approve, color: "green", endpoint: "/a", params: {}, removes: false } ])
    right = JSON.parse(CGI.unescapeHTML(html[/data-swipe-actions-right-value="([^"]*)"/, 1]))
    expect(right.first["removes"]).to be(false)
  end
end
