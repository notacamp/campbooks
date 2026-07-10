# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::EmailSearchBar, type: :component do
  subject(:html) { ApplicationController.render(described_class.new, layout: false) }

  it "wires the email-search Stimulus controller on the form" do
    expect(html).to include('data-controller="email-search"')
    expect(html).to include("email-search#submitNow")
  end

  # The busy-state plumbing the email-search controller toggles while a search
  # request is in flight. If any of these hooks disappear, a slow semantic
  # search reads as a frozen pane again.
  it "renders the in-flight progress bar plumbing" do
    expect(html).to include('data-email-search-target="progress"')
    expect(html).to include("animate-search-progress")
    expect(html).to include('data-email-search-target="spinner"')
    expect(html).to include('data-email-search-target="searchIcon"')
  end

  it "carries the inbox URL so clearing the query can restore the real inbox" do
    expect(html).to include("data-email-search-inbox-url-value")
  end
end
