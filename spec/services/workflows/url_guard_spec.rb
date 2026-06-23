require "rails_helper"

RSpec.describe Workflows::UrlGuard, type: :service do
  describe ".validate!" do
    # Keep the suite hermetic: by default a hostname resolves to a public IP.
    # Individual examples override this to exercise the DNS-rebinding guard.
    before do
      allow(Resolv).to receive(:getaddresses).and_return([ "93.184.216.34" ])
    end

    it "returns the parsed URI for a public https URL" do
      uri = described_class.validate!("https://api.example.com/hook?x=1")
      expect(uri).to be_a(URI::HTTPS)
      expect(uri.host).to eq("api.example.com")
    end

    it "accepts plain http" do
      expect { described_class.validate!("http://api.example.com") }.not_to raise_error
    end

    it "rejects a blank URL" do
      expect { described_class.validate!("") }.to raise_error(described_class::BlockedError, /required/)
    end

    it "rejects non-http(s) schemes" do
      expect { described_class.validate!("ftp://example.com/file") }
        .to raise_error(described_class::BlockedError, /http and https/)
    end

    it "blocks the cloud metadata endpoint" do
      expect { described_class.validate!("http://169.254.169.254/latest/meta-data/") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks localhost" do
      expect { described_class.validate!("http://localhost:3000/hook") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks private network IPs" do
      expect { described_class.validate!("http://10.1.2.3/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
      expect { described_class.validate!("http://192.168.0.5/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks loopback IPs" do
      expect { described_class.validate!("http://127.0.0.1/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks .internal / .local hostnames" do
      expect { described_class.validate!("https://db.internal/health") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    # --- SSRF bypass regressions (2026-06-22 audit) ---

    it "blocks bracketed IPv6 loopback (::1)" do
      expect { described_class.validate!("http://[::1]/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks IPv4-mapped IPv6 loopback (::ffff:127.0.0.1)" do
      expect { described_class.validate!("http://[::ffff:127.0.0.1]/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks IPv4-mapped IPv6 metadata endpoint (::ffff:169.254.169.254)" do
      expect { described_class.validate!("http://[::ffff:169.254.169.254]/latest/") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks the unspecified address 0.0.0.0" do
      expect { described_class.validate!("http://0.0.0.0:8080/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks the IPv6 unspecified address (::)" do
      expect { described_class.validate!("http://[::]/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    # --- SSRF bypass regressions (2026-06-22 pentest) ---

    it "blocks a loopback IP with a trailing-dot FQDN terminator" do
      expect { described_class.validate!("http://127.0.0.1./x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks the metadata IP with a trailing dot" do
      expect { described_class.validate!("http://169.254.169.254./latest/") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks localhost with a trailing dot" do
      expect { described_class.validate!("http://localhost./x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks metadata.google.internal with a trailing dot" do
      expect { described_class.validate!("http://metadata.google.internal./computeMetadata/v1/") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks the RFC 6598 carrier-grade NAT range (100.64.0.0/10)" do
      expect { described_class.validate!("http://100.64.0.1/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks a public hostname that resolves to a private IP (DNS rebinding)" do
      allow(Resolv).to receive(:getaddresses).with("rebind.evil.example").and_return([ "10.0.0.5" ])
      expect { described_class.validate!("https://rebind.evil.example/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "blocks a public hostname that resolves to the metadata IP" do
      allow(Resolv).to receive(:getaddresses).with("meta.evil.example").and_return([ "169.254.169.254" ])
      expect { described_class.validate!("https://meta.evil.example/x") }
        .to raise_error(described_class::BlockedError, /internal host/)
    end

    it "allows a public hostname that resolves only to public IPs" do
      allow(Resolv).to receive(:getaddresses).with("good.example").and_return([ "93.184.216.34" ])
      expect { described_class.validate!("https://good.example/x") }.not_to raise_error
    end

    it "does not block when DNS resolution fails (lets the connection fail closed)" do
      allow(Resolv).to receive(:getaddresses).with("nxdomain.example").and_raise(Resolv::ResolvError)
      expect { described_class.validate!("https://nxdomain.example/x") }.not_to raise_error
    end
  end
end
