# frozen_string_literal: true

module Campbooks
  module Money
    # Locale-aware ordinals for the loan copy.
    #   ordinal(18)   → "18th" / "18.ª" (prestação, cuota) / "18e" (échéance)
    #   day_label(5)  → "5th" in English; the bare day elsewhere ("no dia 5",
    #                   "el día 5", "le 5"), where the locale string carries the noun.
    module Ordinals
      def ordinal(number)
        case I18n.locale.to_s[0, 2]
        when "en" then number.to_i.ordinalize
        when "fr" then number.to_i == 1 ? "1re" : "#{number}e"
        else "#{number}.ª"
        end
      end

      def day_label(day)
        I18n.locale.to_s.start_with?("en") ? day.to_i.ordinalize : day.to_s
      end
    end
  end
end
