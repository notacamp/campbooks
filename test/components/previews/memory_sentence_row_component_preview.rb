# frozen_string_literal: true

# Previews for Campbooks::Memory::SentenceRow — one line of Scout's memory. Shows
# the three origins (taught / learned / default) and their actions, plus a small
# stacked list (as the settings page renders them, divide-y).
class MemorySentenceRowComponentPreview < ViewComponent::Preview
  def taught
    render(Campbooks::Memory::SentenceRow.new(entry: taught_entry))
  end

  def learned
    render(Campbooks::Memory::SentenceRow.new(entry: learned_entry))
  end

  def default_origin
    render(Campbooks::Memory::SentenceRow.new(entry: default_entry))
  end

  def list
    render_with_template(locals: { entries: [ taught_entry, learned_entry, default_entry ] })
  end

  private

  def sentence(markers)
    Scout::Memory::Sentence.parse(markers)
  end

  def taught_entry
    Scout::Memory::Entry.new(
      id: "rule:preview", facet: :filing, source_key: :email_rules,
      sentence: sentence("File anything from **@edp.pt** under **Utilities** and archive it."),
      origin: :taught, origin_detail: "Taught by you · Jul 12",
      form_path: "/settings/inbox/rules", actions: %i[edit remove]
    )
  end

  def learned_entry
    Scout::Memory::Entry.new(
      id: "skim:domain:archive:preview", facet: :stack, source_key: :skim_habits,
      sentence: sentence("You usually archive mail from **@newsletter.com** (9 of your last 12)."),
      origin: :learned, origin_detail: "Learned from how you skim", actions: %i[confirm remove]
    )
  end

  def default_entry
    Scout::Memory::Entry.new(
      id: "default:review", facet: :filing, source_key: :defaults,
      sentence: sentence("Every document I read waits for your review before it's filed for good."),
      origin: :default, origin_detail: "Default", actions: []
    )
  end
end
