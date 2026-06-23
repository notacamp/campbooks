require "rails_helper"

RSpec.describe Emails::SubjectNormalizer do
  describe ".key (thread match key)" do
    # Each pair MUST collapse to the same key — these are the real splits found in
    # dev data (12,928 messages across 7,348 threads; 61 split conversations).
    same = {
      "single Re/Fwd"        => [ "FW: Pedido de orçamento", "Pedido de orçamento" ],
      "stacked RE: FW:"      => [ "RE: FW: Pedido de simulação de seguro", "Pedido de simulação de seguro" ],
      "stacked FW: FW:"      => [ "FW: FW: Contrato cessão quota", "Contrato cessão quota" ],
      "mixed Fwd/Fw"         => [ "Fwd: CE-56-IQ (MG138A)", "Fw: CE-56-IQ (MG138A)", "CE-56-IQ (MG138A)" ],
      "case only"            => [ "Test", "test", "TEST" ],
      "address case"         => [ "info@ReformaAgraria.pt", "info@reformaagraria.pt" ],
      "bracket then reply"   => [ "[Team] Re: Project update", "Project update" ],
      "outlook numbered"     => [ "Re[2]: Project update", "Project update" ],
      "localized de"         => [ "AW: Projekt", "Projekt" ],
      "localized fr forward" => [ "TR: Rapport", "Rapport" ],
      "localized pt forward" => [ "Enc: Fatura", "Fatura" ],
      "whitespace + colon sp"=> [ "Re :  Spaced  out", "Spaced out" ]
    }

    same.each do |label, subjects|
      it "groups #{label} into one key" do
        keys = subjects.map { |s| described_class.key(s) }
        expect(keys.uniq.size).to eq(1), "expected one key, got #{keys.uniq.inspect}"
      end
    end

    it "does NOT over-strip a real word that happens to end in a colon" do
      expect(described_class.key("Invoice: March")).to eq("invoice: march")
      expect(described_class.key("Note: read this")).to eq("note: read this")
    end

    it "keeps genuinely different conversations apart" do
      expect(described_class.key("Re: Lunch")).not_to eq(described_class.key("Re: Dinner"))
    end

    it "is blank for a subject that is only markers" do
      expect(described_class.key("Re: Fwd:")).to eq("")
    end
  end

  describe ".display (human thread subject)" do
    it "strips the marker run but preserves the original case" do
      expect(described_class.display("RE: FW: Pedido de Simulação")).to eq("Pedido de Simulação")
    end

    it "leaves a clean subject untouched" do
      expect(described_class.display("Marcação de escritura")).to eq("Marcação de escritura")
    end
  end
end
