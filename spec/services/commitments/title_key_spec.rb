require "rails_helper"

RSpec.describe Commitments::TitleKey do
  describe ".of" do
    it "strips a trailing ISO date appended after 'on'" do
      expect(described_class.of("Flight to Paris on 2026-12-20"))
        .to eq(described_class.of("Flight to Paris"))
    end

    it "strips a trailing ISO date appended after 'de' (Portuguese/Spanish)" do
      expect(described_class.of("Voo para Paris de 20/12/2026"))
        .to eq(described_class.of("Voo para Paris"))
    end

    it "strips a trailing time appended after 'at'" do
      expect(described_class.of("Dentist appointment at 16:10"))
        .to eq(described_class.of("Dentist appointment"))
    end

    it "strips a bare ISO date at the end of a title" do
      expect(described_class.of("Submit tax form 2026-12-20"))
        .to eq(described_class.of("Submit tax form"))
    end

    it "keeps two genuinely different titles distinct" do
      expect(described_class.of("Pay invoice #123"))
        .not_to eq(described_class.of("Pay invoice #456"))
    end

    it "folds Re: and Fwd: prefixes via SubjectNormalizer" do
      expect(described_class.of("Re: Pay invoice #123"))
        .to eq(described_class.of("Pay invoice #123"))
      expect(described_class.of("Fwd: Submit tax form"))
        .to eq(described_class.of("Submit tax form"))
    end

    it "is case- and whitespace-insensitive" do
      expect(described_class.of("  PAY INVOICE #123  "))
        .to eq(described_class.of("pay invoice #123"))
    end
  end
end
