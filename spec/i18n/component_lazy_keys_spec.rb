require "rails_helper"

# i18n-tasks can't extract Phlex components' lazy keys (app/components is
# excluded in config/i18n-tasks.yml because Campbooks::Base#t scopes ".key" to
# components.<class path> — a scheme i18n-tasks doesn't know). That let the
# EmailTemplatePicker ship with no translations at all (#291): the composer
# showed raw "translation missing" text in production.
#
# This spec closes the gap: scan every component for static lazy keys and
# assert each resolves in every available locale. Dynamic keys (interpolated
# like t(".tone_#{tone}")) don't match the regex and stay uncovered — prefer
# static keys where possible.
RSpec.describe "Phlex component lazy i18n keys" do
  LAZY_KEY = /\bt\(\s*["']\.([a-z0-9_.]+)["']/

  def component_scope(path)
    rel = path.sub(%r{.*app/components/campbooks/}, "").sub(/\.rb\z/, "")
    "components.#{rel.tr('/', '.')}"
  end

  def keys_in(source)
    # Comments can mention t(".key") in prose (base.rb does) — skip them.
    source.lines.reject { |l| l.lstrip.start_with?("#") }.join
          .scan(LAZY_KEY).flatten.uniq
  end

  it "resolves every static lazy key in every locale" do
    missing = []

    Dir[Rails.root.join("app/components/campbooks/**/*.rb")].each do |path|
      scope = component_scope(path)
      keys_in(File.read(path)).each do |key|
        full = "#{scope}.#{key}"
        I18n.available_locales.each do |locale|
          next if I18n.exists?(full, locale)
          # Pluralized keys resolve through their one/other children.
          next if I18n.exists?("#{full}.other", locale)

          missing << "#{locale}: #{full} (#{path.sub("#{Rails.root}/", '')})"
        end
      end
    end

    expect(missing).to be_empty, <<~MSG
      Missing component translations (add them to config/locales/<locale>/…):
      #{missing.sort.join("\n")}
    MSG
  end
end
