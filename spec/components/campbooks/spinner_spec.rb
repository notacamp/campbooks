# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Spinner, type: :component do
  def render_spinner(**opts)
    ApplicationController.render(described_class.new(**opts), layout: false)
  end

  it "renders a positioned status wrapper so the sr-only label stays inside it" do
    html = render_spinner
    # sr-only is position:absolute; without a positioned wrapper the label would
    # anchor to the page at the spinner's in-flow position and, inside a scroll
    # pane, stretch the document (the People list's infinite-scroll sentinel).
    expect(html).to match(/<div[^>]*role="status"[^>]*class="[^"]*\brelative\b[^"]*"/)
    expect(html).to include('class="sr-only"')
    expect(html).to include("Loading")
  end

  it "applies the size classes to the spinning element and custom classes alongside" do
    html = render_spinner(size: :lg, class: "mx-auto")
    expect(html).to include("w-8 h-8")
    expect(html).to include("mx-auto")
    expect(html).to include("animate-spin")
  end
end
