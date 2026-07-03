# frozen_string_literal: true

module Campbooks
  # One inbox provider as a tappable connect card: brand mark, name, a one-line
  # description, and a trailing arrow. Submits the mailbox-connect POST
  # (/email_accounts?provider=…) as a native form — Turbo must not intercept the
  # OAuth redirect. The single connect affordance for onboarding, the setup hub,
  # and empty states, replacing the old solid red/blue provider buttons (brand
  # color stays inside the glyph; the card itself is quiet).
  #
  #   render(Campbooks::ConnectProviderCard.new(provider: :google))
  #   render(Campbooks::ConnectProviderCard.new(provider: :zoho, compact: true))
  class ConnectProviderCard < Campbooks::Base
    GLYPHS = {
      google: '<svg class="h-5 w-5" viewBox="0 0 24 24" aria-hidden="true"><path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4"/><path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/><path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05"/><path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/></svg>',
      zoho: '<svg class="h-5 w-5" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="2" width="20" height="20" rx="4" fill="#E42527"/><path d="M7 8h10l-6 4 6 4H7" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
      microsoft: '<svg class="h-5 w-5" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="2" width="9" height="9" fill="#F25022"/><rect x="13" y="2" width="9" height="9" fill="#7FBA00"/><rect x="2" y="13" width="9" height="9" fill="#00A4EF"/><rect x="13" y="13" width="9" height="9" fill="#FFB900"/></svg>'
    }.freeze

    # @param provider [Symbol] :google, :zoho, :microsoft
    # @param compact [Boolean] drop the description line (dense contexts)
    def initialize(provider:, compact: false, **attrs)
      @provider = provider.to_sym
      @compact = compact
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      form(
        action: helpers.email_accounts_path(provider: @provider),
        method: "post", data: { turbo: false },
        class: class_names("w-full", custom_class),
        **@attrs
      ) do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        button(
          type: "submit",
          class: "group flex w-full cursor-pointer items-center gap-3.5 rounded-2xl border border-border bg-card px-4 text-left transition-all duration-150 hover:border-accent-300 hover:shadow-sm active:scale-[0.99] " +
                 (@compact ? "py-2.5" : "py-3.5")
        ) do
          span(class: "flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-border bg-background") do
            raw(safe(GLYPHS.fetch(@provider)))
          end
          span(class: "min-w-0 flex-1") do
            span(class: "block truncate text-[15px] font-semibold text-foreground") { t(".#{@provider}.label") }
            unless @compact
              span(class: "block truncate text-[13px] text-muted-foreground") { t(".#{@provider}.description") }
            end
          end
          span(class: "shrink-0 text-muted-foreground transition-transform duration-150 group-hover:translate-x-0.5 group-hover:text-foreground") do
            raw(safe(arrow_svg))
          end
        end
      end
    end

    private

    def arrow_svg
      %(<svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4" aria-hidden="true"><path fill-rule="evenodd" d="M3 10a.75.75 0 01.75-.75h8.69L9.22 6.03a.75.75 0 111.06-1.06l4.5 4.5a.75.75 0 010 1.06l-4.5 4.5a.75.75 0 11-1.06-1.06l3.22-3.22H3.75A.75.75 0 013 10z" clip-rule="evenodd"/></svg>)
    end
  end
end
