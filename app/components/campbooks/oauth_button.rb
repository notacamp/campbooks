# frozen_string_literal: true

module Campbooks
  class OauthButton < Campbooks::Base
    PROVIDER_CONFIG = {
      zoho: {
        label: "Zoho Mail",
        classes: "bg-card hover:bg-gray-50 text-gray-700 border border-gray-300",
        icon: '<svg class="w-4 h-4" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="2" width="20" height="20" rx="4" fill="#E42527"/><path d="M7 8h10l-6 4 6 4H7" fill="none" stroke="white" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>'
      },
      google: {
        label: "Google",
        classes: "bg-card hover:bg-gray-50 text-gray-700 border border-gray-300",
        icon: '<svg class="w-4 h-4" viewBox="0 0 24 24" aria-hidden="true"><path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4"/><path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/><path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05"/><path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/></svg>'
      },
      microsoft: {
        label: "Microsoft 365",
        classes: "bg-card hover:bg-gray-50 text-gray-700 border border-gray-300",
        icon: '<svg class="w-4 h-4" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="2" width="9" height="9" fill="#F25022"/><rect x="13" y="2" width="9" height="9" fill="#7FBA00"/><rect x="2" y="13" width="9" height="9" fill="#00A4EF"/><rect x="13" y="13" width="9" height="9" fill="#FFB900"/></svg>'
      }
    }.freeze

    SIZE_CLASSES = {
      md: "px-4 py-2 text-sm",
      lg: "w-full py-2.5 px-4 text-sm shadow-sm"
    }.freeze

    BASE_CLASSES = "inline-flex items-center justify-center gap-2 font-medium rounded-lg transition-colors cursor-pointer no-underline"

    # @param provider [Symbol] :zoho, :google, :microsoft
    # @param href [String] OAuth authorization URL
    # @param size [Symbol] :md (default) or :lg (full-width auth page)
    def initialize(provider:, href:, size: :md, **attrs)
      @provider = provider
      @href = href
      @size = size
      @attrs = attrs
    end

    def view_template
      config = PROVIDER_CONFIG[@provider]
      custom_class = @attrs.delete(:class)
      merged = class_names(BASE_CLASSES, config[:classes], SIZE_CLASSES[@size], custom_class)

      a(href: @href, class: merged, **@attrs) do
        raw(safe(config[:icon]))
        span { config[:label] }
      end
    end
  end
end
